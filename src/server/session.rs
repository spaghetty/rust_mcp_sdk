//! Defines the ServerSession, which manages the state and logic for a single client connection.

use super::server::Server;
use crate::error::{Error, Result};
use crate::network_adapter::NetworkAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, ErrorData, ErrorResponse, GetPromptParams, Implementation,
    InitializeRequestParams, InitializeResult, ListPromptsParams, ListResourcesParams,
    ListToolsResult, Notification, ReadResourceParams, Request, RequestId, Response,
    ServerCapabilities, Tool, ToolsCapability, LATEST_PROTOCOL_VERSION, METHOD_NOT_FOUND,
};
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

/// A handle given to user-code to allow sending notifications back to the client.
#[derive(Clone)]
pub struct ConnectionHandle {
    pub(crate) notification_sender: mpsc::Sender<String>,
}

impl ConnectionHandle {
    /// Sends a notification to the client associated with this connection.
    pub async fn send_notification<T: Serialize>(
        &self,
        notification: Notification<T>,
    ) -> Result<()> {
        let json_string = serde_json::to_string(&notification)?;
        self.notification_sender.send(json_string).await?;
        Ok(())
    }
}

/// Represents a single, active client connection and manages its lifecycle.
pub struct ServerSession<A: NetworkAdapter> {
    // Made public for integration tests
    connection: ProtocolConnection<A>,
    server: Arc<Server>,
    is_initialized: bool,
}

impl<A: NetworkAdapter + Send + 'static> ServerSession<A> {
    pub fn new(connection: ProtocolConnection<A>, server: Arc<Server>) -> Self {
        // Made public for integration tests
        Self {
            connection,
            server,
            is_initialized: false,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        // Made public for integration tests
        info!("[Session] New session task started. Waiting for messages.");
        let (notification_tx, mut notification_rx) = mpsc::channel::<String>(32);

        loop {
            tokio::select! {
                result = self.connection.recv_message::<Value>() => {
                    let raw_req = match result {
                        Ok(Some(msg)) => msg,
                        Ok(None) => {
                            info!("[Session] Connection closed by client. Draining pending notifications.");
                            // DRAIN NOTIFICATIONS BEFORE RETURNING
                            notification_rx.close(); // Close the sender side of the channel
                            while let Some(notif_json) = notification_rx.recv().await {
                                self.connection.send_raw(&notif_json).await?;
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            info!("[Session] something strage here {}",e);
                            notification_rx.close();
                            while let Some(notif_json) = notification_rx.recv().await {
                                self.connection.send_raw(&notif_json).await?;
                            }
                            return Ok(());
                        }
                    };
                    let handle = ConnectionHandle { notification_sender: notification_tx.clone() };
                    if let Err(e) = self.dispatch_request(raw_req, handle).await {
                         error!("[Server] Error dispatching request: {}", e);
                    }
                },
                Some(notif_json) = notification_rx.recv() => {
                    self.connection.send_raw(&notif_json).await?;
                }
            }
        }
    }

    async fn dispatch_request(&mut self, raw_req: Value, handle: ConnectionHandle) -> Result<()> {
        if raw_req.get("id").is_none() && raw_req.get("method").is_some() {
            // Attempt to parse as a generic notification to get the method name.
            if let Ok(notif) = serde_json::from_value::<Notification<Value>>(raw_req.clone()) {
                if notif.method == "notifications/initialized" {
                    // LSP spec uses "initialized", not "notifications/initialized"
                    info!("[Session] Successful setup notification from client received.");
                    // It's a notification, so we do nothing and wait for the next message.
                    return Ok(());
                } else {
                    info!("Received unknown notification: {}", notif.method);
                }
            } else {
                info!("Received unparsable notification");
            }
        }

        if !self.is_initialized {
            return self.handle_initialize(raw_req).await;
        }

        let req: Request<Value> = serde_json::from_value(raw_req)?;

        use super::server::ToolHandler as ServerToolHandlerEnum; // Alias to avoid confusion if needed, and for clarity

        match req.method.as_str() {
            "tools/list" => {
                // Adjusted to use tools_and_handlers
                let tools: Vec<Tool> = self
                    .server
                    .tools_and_handlers
                    .values()
                    .map(|(tool, _handler)| tool.clone())
                    .collect();
                let result = ListToolsResult { tools };
                let response = Response {
                    id: req.id,
                    jsonrpc: "2.0".to_string(),
                    result,
                };
                self.connection.send_serializable(response).await
            }
            "tools/call" => {
                let params: CallToolParams = serde_json::from_value(req.params)?;
                // Adjusted to use tools_and_handlers and new handler signature
                if let Some((_tool_meta, handler_arc)) =
                    self.server.tools_and_handlers.get(&params.name)
                {
                    let arguments_arc = Arc::new(params.arguments); // Wrap arguments in Arc<Value>
                    let result = match **handler_arc {
                        ServerToolHandlerEnum::Untyped(ref h) => h(handle, arguments_arc).await?,
                        ServerToolHandlerEnum::Typed(ref h) => h(handle, arguments_arc).await?,
                    };
                    let response = Response {
                        id: req.id,
                        jsonrpc: "2.0".to_string(),
                        result,
                    };
                    self.connection.send_serializable(response).await
                } else {
                    self.send_error(
                        req.id,
                        METHOD_NOT_FOUND,
                        &format!("Tool '{}' not found", params.name),
                    )
                    .await
                }
            }
            "resources/list" => {
                let handler = self.server.list_resources_handler.clone();
                self.dispatch(req, &handler, |h, _: ListResourcesParams| h(handle.clone()))
                    .await
            }
            "resources/read" => {
                let handler = self.server.read_resource_handler.clone();
                self.dispatch(req, &handler, |h, p: ReadResourceParams| {
                    h(handle.clone(), p.uri)
                })
                .await
            }
            "prompts/list" => {
                let handler = self.server.list_prompts_handler.clone();
                self.dispatch(req, &handler, |h, _: ListPromptsParams| h(handle.clone()))
                    .await
            }
            "prompts/get" => {
                let handler = self.server.get_prompt_handler.clone();
                self.dispatch(req, &handler, |h, p: GetPromptParams| {
                    h(handle.clone(), p.name, p.arguments)
                })
                .await
            }
            "initialize" => Err(Error::Other(
                "Client sent 'initialize' request twice.".into(),
            )),
            unhandled_method => {
                self.send_error(
                    req.id,
                    METHOD_NOT_FOUND,
                    &format!("Method '{}' not found", unhandled_method),
                )
                .await
            }
        }
    }

    async fn handle_initialize(&mut self, raw_req: Value) -> Result<()> {
        info!("[Session] Initialize handshake started. Session is now in pending.");
        if let Some("initialize") = raw_req.get("method").and_then(Value::as_str) {
            let init_req: Request<InitializeRequestParams> = serde_json::from_value(raw_req)?;
            // --- DYNAMIC CAPABILITIES LOGIC ---
            // 1. Start with default, empty capabilities.
            let mut capabilities = ServerCapabilities::default();

            // 2. Check if any tools have been registered.
            // Adjusted to use tools_and_handlers
            if !self.server.tools_and_handlers.is_empty() {
                // If so, add the "tools" capability to our announcement.
                capabilities.tools = Some(ToolsCapability {
                    // For now, we can hardcode this sub-capability to false.
                    // It signals that we don't support the `tools/listChanged` notification.
                    list_changed: Some(false),
                });
            }
            let init_response = Response {
                jsonrpc: "2.0".to_string(),
                id: init_req.id,
                result: InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    server_info: Implementation {
                        name: self.server.name.clone(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    capabilities: capabilities,
                },
            };
            self.connection.send_serializable(init_response).await?;
            self.is_initialized = true;
            info!("[Session] Initialize handshake successful. Session is now initialized.");
            Ok(())
        } else {
            Err(Error::Other(
                "First message from client was not an 'initialize' request.".into(),
            ))
        }
    }

    async fn dispatch<H, P, R, F, Fut>(
        &mut self,
        req: Request<Value>,
        handler_opt: &Option<H>,
        f: F,
    ) -> Result<()>
    where
        H: Clone + Send + Sync + 'static,
        P: serde::de::DeserializeOwned,
        R: Serialize + Send + Sync,
        F: Fn(H, P) -> Fut,
        Fut: Future<Output = Result<R>>,
    {
        if let Some(handler) = handler_opt.clone() {
            match serde_json::from_value(req.params) {
                Ok(params) => {
                    match f(handler, params).await {
                        Ok(result) => {
                            // On success, send the result back.
                            let response = Response {
                                id: req.id,
                                jsonrpc: "2.0".to_string(),
                                result,
                            };
                            self.connection.send_serializable(response).await
                        }
                        Err(err) => {
                            // If the handler returns an error, send a JSON-RPC error response.
                            self.send_error(req.id, crate::types::INTERNAL_ERROR, &err.to_string())
                                .await
                        }
                    }
                }
                Err(e) => {
                    // If params are invalid, send an `Invalid Params` error.
                    self.send_error(req.id, crate::types::INVALID_PARAMS, &e.to_string())
                        .await
                }
            }
        } else {
            self.send_error(
                req.id,
                METHOD_NOT_FOUND,
                &format!("Method '{}' has no registered handler", req.method),
            )
            .await
        }
    }

    async fn send_error(&mut self, id: RequestId, code: i32, message: &str) -> Result<()> {
        let error_response = ErrorResponse {
            jsonrpc: "2.0".to_string(),
            id,
            error: ErrorData {
                code,
                message: message.to_string(),
            },
        };
        self.connection.send_serializable(error_response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        network_adapter::NetworkAdapter,
        server::server::Server,
        types::{CallToolResult, Content, JSONRPCResponse, ListToolsChangedParams, Tool},
        ProtocolConnection,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    // --- Mock Infrastructure ---
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
            Ok(self.outgoing.lock().unwrap().push_back(msg.to_string()))
        }
        async fn recv(&mut self) -> Result<Option<String>> {
            Ok(self.incoming.lock().unwrap().pop_front())
        }
    }

    /// Test helper to run a session and collect its output.
    async fn run_session_with_requests(
        server: Arc<Server>,
        requests: Vec<String>,
    ) -> Arc<Mutex<VecDeque<String>>> {
        let adapter = MockAdapter::default();
        let outgoing_buffer = Arc::clone(&adapter.outgoing);
        for req in requests {
            adapter.push_incoming(req);
        }

        let conn = ProtocolConnection::new(adapter);
        let session = ServerSession::new(conn, server);

        tokio::time::timeout(std::time::Duration::from_secs(1), session.run())
            .await
            .expect("Session run timed out")
            .expect("Session run failed");

        outgoing_buffer
    }

    fn make_init_request() -> String {
        serde_json::to_string(&json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": "test", "clientInfo": {"name": "test", "version": "0"}, "capabilities": {} }
        })).unwrap()
    }

    // --- Tests for Session Logic ---

    #[tokio::test]
    async fn test_session_dispatches_request() {
        // 1. Setup the server with a specific tool and its handler.
        let server = Arc::new(Server::new("test").register_tool(
            Tool {
                name: "test-tool".to_string(),
                ..Default::default()
            },
            |_handle, _args| async {
                Ok(CallToolResult {
                    content: vec![Content::Text {
                        text: "Success!".to_string(),
                    }],
                    is_error: false,
                })
            },
        ));

        let list_req = serde_json::to_string(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }),
        )
        .unwrap();
        let outgoing = run_session_with_requests(server, vec![make_init_request(), list_req]).await;

        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let list_response_str = responses.iter().find(|s| s.contains("\"id\":1")).unwrap();
        // Changed Vec<Tool> to ListToolsResult for deserialization
        let list_response: JSONRPCResponse<ListToolsResult> =
            serde_json::from_str(list_response_str).unwrap();

        if let JSONRPCResponse::Success(res) = list_response {
            // res.result is now ListToolsResult
            assert_eq!(res.result.tools.len(), 1); // Access the .tools field
            assert_eq!(res.result.tools[0].name, "test-tool"); // Further check tool name
        } else {
            panic!(
                "Expected a successful response for tools/list, got: {:?}",
                list_response_str
            );
        }
    }

    #[tokio::test]
    async fn test_session_sends_notification() {
        // 1. Setup a tool whose handler sends a notification.
        let server = Arc::new(Server::new("test").register_tool(
            Tool {
                name: "notification-tool".to_string(),
                ..Default::default()
            },
            |handle, _args| async move {
                handle
                    .send_notification(Notification {
                        jsonrpc: "2.0".to_string(),
                        method: "test/notification".to_string(),
                        params: Some(ListToolsChangedParams {}),
                    })
                    .await
                    .unwrap();
                Ok(CallToolResult::default())
            },
        ));

        let call_req = serde_json::to_string(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "notification-tool", "arguments": {}} })).unwrap();
        let outgoing = run_session_with_requests(server, vec![make_init_request(), call_req]).await;

        let responses = outgoing.lock().unwrap();
        // The server sends a response to init, a response to the call, AND a notification.
        assert_eq!(responses.len(), 3);
        let notif_found = responses.iter().any(|s| s.contains("test/notification"));
        assert!(notif_found, "The test notification was not found");
    }
    #[tokio::test]
    async fn test_call_nonexistent_tool_sends_error() {
        // 1. Setup a server with NO tools registered.
        let server = Arc::new(Server::new("test"));

        // 2. Create a request to call a tool that does not exist.
        let call_req = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 101,
            "method": "tools/call",
            "params": {"name": "nonexistent-tool", "arguments": {}}
        }))
        .unwrap();

        // 3. Run the session.
        let outgoing = run_session_with_requests(server, vec![make_init_request(), call_req]).await;

        // 4. Assert that the server sent back a well-formed "Method not found" error.
        let responses = outgoing.lock().unwrap();
        let error_response_str = responses.iter().find(|s| s.contains("\"id\":101")).unwrap();
        let error_response: JSONRPCResponse<Value> =
            serde_json::from_str(error_response_str).unwrap();

        match error_response {
            JSONRPCResponse::Success(_) => panic!("Expected an error response, but got success"),
            JSONRPCResponse::Error(err) => {
                assert_eq!(err.error.code, crate::types::METHOD_NOT_FOUND);
                assert!(err
                    .error
                    .message
                    .contains("Tool 'nonexistent-tool' not found"));
            }
        }
    }
}
