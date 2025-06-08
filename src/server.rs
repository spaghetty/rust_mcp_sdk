//! Defines the high-level MCP server.
//!
//! This module provides an ergonomic API for building an MCP server using a builder pattern.
//! It handles listening for connections, dispatching requests to registered handlers,
//! and managing the connection lifecycle.

use crate::adapter::NetworkAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, CallToolResult, ErrorData, ErrorResponse, Implementation,
    InitializeRequestParams, InitializeResult, Notification, ReadResourceParams,
    ReadResourceResult, Request, RequestId, Resource, Response, ServerCapabilities, Tool,
    LATEST_PROTOCOL_VERSION, METHOD_NOT_FOUND,
};
use crate::TcpAdapter;
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

// --- Connection Handle ---
#[derive(Clone)]
pub struct ConnectionHandle {
    notification_sender: mpsc::Sender<String>,
}

impl ConnectionHandle {
    pub async fn send_notification<T: Serialize>(
        &self,
        notification: Notification<T>,
    ) -> Result<()> {
        let json_string = serde_json::to_string(&notification)?;
        self.notification_sender.send(json_string).await?;
        Ok(())
    }
}

// --- Handler Type Definitions ---
type ListToolsHandler = Arc<
    dyn Fn(ConnectionHandle) -> Pin<Box<dyn Future<Output = Result<Vec<Tool>>> + Send>>
        + Send
        + Sync,
>;
type CallToolHandler = Arc<
    dyn Fn(
            ConnectionHandle,
            String,
            Value,
        ) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
>;
type ListResourcesHandler = Arc<
    dyn Fn(ConnectionHandle) -> Pin<Box<dyn Future<Output = Result<Vec<Resource>>> + Send>>
        + Send
        + Sync,
>;
type ReadResourceHandler = Arc<
    dyn Fn(
            ConnectionHandle,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<ReadResourceResult>> + Send>>
        + Send
        + Sync,
>;

/// A high-level server for handling MCP requests.
#[derive(Default)]
pub struct Server {
    name: String,
    list_tools_handler: Option<ListToolsHandler>,
    call_tool_handler: Option<CallToolHandler>,
    list_resources_handler: Option<ListResourcesHandler>,
    read_resource_handler: Option<ReadResourceHandler>,
}

impl Server {
    pub fn new(name: &str) -> Self {
        Server {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn on_list_tools<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Tool>>> + Send + 'static,
    {
        self.list_tools_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    pub fn on_call_tool<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CallToolResult>> + Send + 'static,
    {
        self.call_tool_handler = Some(Arc::new(move |handle, name, args| {
            Box::pin(handler(handle, name, args))
        }));
        self
    }

    pub fn on_list_resources<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Resource>>> + Send + 'static,
    {
        self.list_resources_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    pub fn on_read_resource<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult>> + Send + 'static,
    {
        self.read_resource_handler =
            Some(Arc::new(move |handle, uri| Box::pin(handler(handle, uri))));
        self
    }

    pub async fn listen(self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("[Server] Listening on {}", addr);
        let server = Arc::new(self);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            println!("[Server] Accepted connection from: {}", client_addr);
            let server_clone = Arc::clone(&server);
            tokio::spawn(async move {
                let adapter = TcpAdapter::new(stream);
                let mut conn = ProtocolConnection::new(adapter);
                if let Err(e) = server_clone.handle_connection(&mut conn).await {
                    eprintln!("[Server] Handler failed for {}: {}", client_addr, e);
                } else {
                    println!(
                        "[Server] Connection with {} closed gracefully.",
                        client_addr
                    );
                }
            });
        }
    }

    pub async fn handle_connection<A: NetworkAdapter>(
        &self,
        conn: &mut ProtocolConnection<A>,
    ) -> Result<()> {
        let mut is_initialized = false;
        let (notification_tx, mut notification_rx) = mpsc::channel::<String>(32);

        loop {
            tokio::select! {
                result = conn.recv_message::<Value>() => {
                    let raw_req = match result {
                        Ok(Some(msg)) => msg,
                        // CORRECTED: If client disconnects or there's an error,
                        // drain pending notifications before exiting.
                        Ok(None) | Err(_) => {
                            notification_rx.close();
                            while let Some(notif_json) = notification_rx.recv().await {
                                conn.send_raw(&notif_json).await?;
                            }
                            return Ok(());
                        }
                    };

                    if !is_initialized {
                        if let Some("initialize") = raw_req.get("method").and_then(Value::as_str) {
                            let init_req: Request<InitializeRequestParams> = serde_json::from_value(raw_req)?;
                            let init_response = Response {
                                jsonrpc: "2.0".to_string(),
                                id: init_req.id,
                                result: InitializeResult {
                                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                                    server_info: Implementation {
                                        name: self.name.clone(),
                                        version: env!("CARGO_PKG_VERSION").to_string(),
                                    },
                                    capabilities: ServerCapabilities::default(),
                                },
                            };
                            conn.send_serializable(init_response).await?;
                            is_initialized = true;
                            continue;
                        } else {
                            return Err(anyhow!("First message from client was not an 'initialize' request."));
                        }
                    }

                    let req: Request<Value> = serde_json::from_value(raw_req)?;
                    let handle = ConnectionHandle { notification_sender: notification_tx.clone() };

                    match req.method.as_str() {
                        "tools/list" => {
                            if let Some(handler) = &self.list_tools_handler {
                                let result = (handler)(handle).await?;
                                conn.send_serializable(Response { id: req.id, jsonrpc: "2.0".to_string(), result }).await?;
                            } else {
                                self.send_error(conn, req.id, METHOD_NOT_FOUND, "tools/list handler not registered").await?;
                            }
                        }
                        "tools/call" => {
                            if let Some(handler) = &self.call_tool_handler {
                                let params: CallToolParams = serde_json::from_value(req.params)?;
                                let result = (handler)(handle, params.name, params.arguments).await?;
                                conn.send_serializable(Response { id: req.id, jsonrpc: "2.0".to_string(), result }).await?;
                            } else {
                                self.send_error(conn, req.id, METHOD_NOT_FOUND, "tools/call handler not registered").await?;
                            }
                        }
                        "resources/list" => {
                            if let Some(handler) = &self.list_resources_handler {
                                let result = (handler)(handle).await?;
                                conn.send_serializable(Response { id: req.id, jsonrpc: "2.0".to_string(), result }).await?;
                            } else {
                                self.send_error(conn, req.id, METHOD_NOT_FOUND, "resources/list handler not registered").await?;
                            }
                        }
                        "resources/read" => {
                            if let Some(handler) = &self.read_resource_handler {
                                let params: ReadResourceParams = serde_json::from_value(req.params)?;
                                let result = (handler)(handle, params.uri).await?;
                                conn.send_serializable(Response { id: req.id, jsonrpc: "2.0".to_string(), result }).await?;
                            } else {
                                self.send_error(conn, req.id, METHOD_NOT_FOUND, "resources/read handler not registered").await?;
                            }
                        }
                        "initialize" => return Err(anyhow!("Client sent 'initialize' request twice.")),
                        unhandled_method => self.send_error(conn, req.id, METHOD_NOT_FOUND, &format!("Method '{}' not found", unhandled_method)).await?,
                    }
                },
                Some(notif_json) = notification_rx.recv() => {
                    conn.send_raw(&notif_json).await?;
                }
            }
        }
    }

    async fn send_error<A: NetworkAdapter>(
        &self,
        conn: &mut ProtocolConnection<A>,
        id: RequestId,
        code: i32,
        message: &str,
    ) -> Result<()> {
        let error_response = ErrorResponse {
            jsonrpc: "2.0".to_string(),
            id,
            error: ErrorData {
                code,
                message: message.to_string(),
            },
        };
        conn.send_serializable(error_response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::NetworkAdapter;
    use crate::types::{
        JSONRPCResponse, ListToolsChangedParams, Notification, ResourceContents,
        TextResourceContents,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    // --- Mock Infrastructure for Testing ---

    #[derive(Default, Clone)]
    struct MockAdapter {
        incoming: Arc<Mutex<VecDeque<String>>>,
        outgoing: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockAdapter {
        fn push_incoming(&self, msg: String) {
            self.incoming.lock().unwrap().push_back(msg);
        }
    }

    #[async_trait]
    impl NetworkAdapter for MockAdapter {
        async fn send(&mut self, msg: &str) -> Result<()> {
            self.outgoing.lock().unwrap().push_back(msg.to_string());
            Ok(())
        }
        async fn recv(&mut self) -> Result<Option<String>> {
            Ok(self.incoming.lock().unwrap().pop_front())
        }
    }

    fn setup_mock_connection() -> (
        ProtocolConnection<MockAdapter>,
        MockAdapter,
        Arc<Mutex<VecDeque<String>>>,
    ) {
        let adapter = MockAdapter::default();
        let test_adapter_handle = adapter.clone();
        let outgoing_buffer = Arc::clone(&adapter.outgoing);
        let conn = ProtocolConnection::new(adapter);
        (conn, test_adapter_handle, outgoing_buffer)
    }

    fn make_init_request() -> String {
        let init_req = json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": "test", "clientInfo": {"name": "test", "version": "0"}, "capabilities": {} }
        });
        serde_json::to_string(&init_req).unwrap()
    }

    // --- Test Cases ---

    #[tokio::test]
    async fn test_handler_registration() {
        // UPDATED: Handlers now take a ConnectionHandle, which we ignore in this test.
        let server = Server::new("test")
            .on_list_tools(|_handle| async { Ok(vec![]) })
            .on_call_tool(|_handle, _, _| async {
                Ok(CallToolResult {
                    content: vec![],
                    is_error: false,
                })
            })
            .on_list_resources(|_handle| async { Ok(vec![]) })
            .on_read_resource(|_handle, _| async { Ok(ReadResourceResult { contents: vec![] }) });

        assert!(server.list_tools_handler.is_some());
        assert!(server.call_tool_handler.is_some());
        assert!(server.list_resources_handler.is_some());
        assert!(server.read_resource_handler.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_to_list_tools_handler() {
        // UPDATED: The handler closure now accepts and ignores the ConnectionHandle.
        let server = Arc::new(Server::new("test").on_list_tools(|_handle| async {
            Ok(vec![Tool {
                name: "test-tool-from-handler".to_string(),
                description: None,
                input_schema: json!({}),
                annotations: None,
            }])
        }));

        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();
        adapter_handle.push_incoming(make_init_request());
        let list_req = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
        adapter_handle.push_incoming(serde_json::to_string(&list_req).unwrap());
        server.handle_connection(&mut conn).await.unwrap();
        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let list_response: JSONRPCResponse<Vec<Tool>> =
            serde_json::from_str(responses.get(1).unwrap()).unwrap();
        if let JSONRPCResponse::Success(res) = list_response {
            assert_eq!(res.result[0].name, "test-tool-from-handler");
        } else {
            panic!("Expected a successful response for tools/list");
        }
    }

    // Replace the old version of this test with this one.
    #[tokio::test]
    async fn test_handler_can_send_notification() {
        let server = Arc::new(Server::new("test").on_call_tool(
            |handle, _name, _args| async move {
                handle
                    .send_notification(Notification {
                        jsonrpc: "2.0".to_string(),
                        method: "notifications/tools/list_changed".to_string(),
                        params: ListToolsChangedParams {},
                    })
                    .await
                    .unwrap();
                Ok(CallToolResult {
                    content: vec![],
                    is_error: false,
                })
            },
        ));

        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();

        // CORRECTED: Run the connection handler in the background.
        let server_handle = tokio::spawn(async move {
            server.handle_connection(&mut conn).await.unwrap();
        });

        // Push the requests for the handler to process.
        adapter_handle.push_incoming(make_init_request());
        let call_req = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "foo", "arguments": {}} });
        adapter_handle.push_incoming(serde_json::to_string(&call_req).unwrap());

        // Give the server task a moment to process both the request and the queued notification.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let responses = outgoing.lock().unwrap();
        assert_eq!(
            responses.len(),
            3,
            "Expected init response, call response, and a notification"
        );

        // The third message should be the notification. We can check this by filtering.
        let notif_found = responses
            .iter()
            .any(|s| s.contains("notifications/tools/list_changed"));
        assert!(
            notif_found,
            "The tools/listChanged notification was not found in the outgoing messages"
        );

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_dispatch_to_list_resources_handler() {
        // UPDATED: The handler closure now accepts and ignores the ConnectionHandle.
        let server = Arc::new(Server::new("test").on_list_resources(|_handle| async {
            Ok(vec![Resource {
                name: "test-resource".to_string(),
                uri: "mcp://test".to_string(),
                description: None,
                mime_type: None,
            }])
        }));

        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();
        adapter_handle.push_incoming(make_init_request());
        let list_req =
            json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list", "params": {} });
        adapter_handle.push_incoming(serde_json::to_string(&list_req).unwrap());
        server.handle_connection(&mut conn).await.unwrap();
        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let list_response: JSONRPCResponse<Vec<Resource>> =
            serde_json::from_str(responses.get(1).unwrap()).unwrap();
        if let JSONRPCResponse::Success(res) = list_response {
            assert_eq!(res.result[0].name, "test-resource");
        } else {
            panic!("Expected a successful response for resources/list");
        }
    }

    #[tokio::test]
    async fn test_dispatch_to_read_resource_handler() {
        // UPDATED: The handler closure now accepts and ignores the ConnectionHandle.
        let server = Arc::new(
            Server::new("test").on_read_resource(|_handle, uri| async move {
                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::Text(TextResourceContents {
                        uri,
                        mime_type: None,
                        text: "resource content".to_string(),
                    })],
                })
            }),
        );

        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();
        adapter_handle.push_incoming(make_init_request());
        let read_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/read",
            "params": { "uri": "mcp://test" }
        });
        adapter_handle.push_incoming(serde_json::to_string(&read_req).unwrap());
        server.handle_connection(&mut conn).await.unwrap();
        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let read_response: JSONRPCResponse<ReadResourceResult> =
            serde_json::from_str(responses.get(1).unwrap()).unwrap();
        if let JSONRPCResponse::Success(res) = read_response {
            assert_eq!(res.result.contents.len(), 1);
        } else {
            panic!("Expected a successful response for resources/read");
        }
    }

    #[tokio::test]
    async fn test_unregistered_resource_handler_returns_error() {
        let server = Arc::new(Server::new("test-no-handlers"));
        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();
        adapter_handle.push_incoming(make_init_request());
        let list_req =
            json!({ "jsonrpc": "2.0", "id": 1, "method": "resources/list", "params": {} });
        adapter_handle.push_incoming(serde_json::to_string(&list_req).unwrap());
        server.handle_connection(&mut conn).await.unwrap();
        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let error_response: JSONRPCResponse<Value> =
            serde_json::from_str(responses.get(1).unwrap()).unwrap();
        if let JSONRPCResponse::Error(err) = error_response {
            assert_eq!(err.id, RequestId::Num(1));
            assert_eq!(err.error.code, METHOD_NOT_FOUND);
            assert!(err.error.message.contains("not registered"));
        } else {
            panic!("Expected an error response");
        }
    }
}
