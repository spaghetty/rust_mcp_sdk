//! Defines the ServerSession, which manages the state and logic for a single client connection.

use super::server::Server;
use crate::adapter::NetworkAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, ErrorData, ErrorResponse, GetPromptParams, Implementation,
    InitializeRequestParams, InitializeResult, ListPromptsParams, ListResourcesParams,
    ListToolsParams, Notification, ReadResourceParams, Request, RequestId, Response,
    LATEST_PROTOCOL_VERSION, METHOD_NOT_FOUND,
};
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc;

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
pub(crate) struct ServerSession<A: NetworkAdapter> {
    connection: ProtocolConnection<A>,
    server: Arc<Server>,
    is_initialized: bool,
}

impl<A: NetworkAdapter + Send + 'static> ServerSession<A> {
    pub(crate) fn new(connection: ProtocolConnection<A>, server: Arc<Server>) -> Self {
        Self {
            connection,
            server,
            is_initialized: false,
        }
    }

    pub(crate) async fn run(mut self) -> Result<()> {
        let (notification_tx, mut notification_rx) = mpsc::channel::<String>(32);

        loop {
            tokio::select! {
                result = self.connection.recv_message::<Value>() => {
                    let raw_req = match result {
                        Ok(Some(msg)) => msg,
                        Ok(None) | Err(_) => {
                            notification_rx.close();
                            while let Some(notif_json) = notification_rx.recv().await {
                                self.connection.send_raw(&notif_json).await?;
                            }
                            return Ok(());
                        }
                    };
                    let handle = ConnectionHandle { notification_sender: notification_tx.clone() };
                    if let Err(e) = self.dispatch_request(raw_req, handle).await {
                         eprintln!("[Server] Error dispatching request: {}", e);
                    }
                },
                Some(notif_json) = notification_rx.recv() => {
                    self.connection.send_raw(&notif_json).await?;
                }
            }
        }
    }

    async fn dispatch_request(&mut self, raw_req: Value, handle: ConnectionHandle) -> Result<()> {
        if !self.is_initialized {
            return self.handle_initialize(raw_req).await;
        }

        let req: Request<Value> = serde_json::from_value(raw_req)?;

        match req.method.as_str() {
            "tools/list" => {
                let handler = self.server.list_tools_handler.clone();
                self.dispatch(req, &handler, |h, _: ListToolsParams| h(handle.clone()))
                    .await
            }
            "tools/call" => {
                let handler = self.server.call_tool_handler.clone();
                self.dispatch(req, &handler, |h, p: CallToolParams| {
                    h(handle.clone(), p.name, p.arguments)
                })
                .await
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
            // NEW: Add dispatch logic for prompts
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
            "initialize" => Err(anyhow!("Client sent 'initialize' request twice.")),
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
        if let Some("initialize") = raw_req.get("method").and_then(Value::as_str) {
            let init_req: Request<InitializeRequestParams> = serde_json::from_value(raw_req)?;
            let init_response = Response {
                jsonrpc: "2.0".to_string(),
                id: init_req.id,
                result: InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    server_info: Implementation {
                        name: self.server.name.clone(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    capabilities: Default::default(),
                },
            };
            self.connection.send_serializable(init_response).await?;
            self.is_initialized = true;
            Ok(())
        } else {
            Err(anyhow!(
                "First message from client was not an 'initialize' request."
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
            let params: P = serde_json::from_value(req.params)?;
            let result = f(handler, params).await?;
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
        adapter::NetworkAdapter,
        server::server::Server,
        types::{CallToolResult, JSONRPCResponse, ListToolsChangedParams, Tool},
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
        let server = Arc::new(
            Server::new("test").on_list_tools(|_handle| async { Ok(vec![Tool::default()]) }),
        );

        let list_req = serde_json::to_string(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }),
        )
        .unwrap();
        let outgoing = run_session_with_requests(server, vec![make_init_request(), list_req]).await;

        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2);
        let list_response_str = responses.iter().find(|s| s.contains("\"id\":1")).unwrap();
        let list_response: JSONRPCResponse<Vec<Tool>> =
            serde_json::from_str(list_response_str).unwrap();

        if let JSONRPCResponse::Success(res) = list_response {
            assert_eq!(res.result.len(), 1);
        } else {
            panic!("Expected a successful response");
        }
    }

    #[tokio::test]
    async fn test_session_sends_notification() {
        let server = Arc::new(Server::new("test").on_call_tool(|handle, _, _| async move {
            handle
                .send_notification(Notification {
                    jsonrpc: "2.0".to_string(),
                    method: "test/notification".to_string(),
                    params: ListToolsChangedParams {},
                })
                .await
                .unwrap();
            Ok(CallToolResult::default())
        }));

        let call_req = serde_json::to_string(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "foo", "arguments": {}} })).unwrap();
        let outgoing = run_session_with_requests(server, vec![make_init_request(), call_req]).await;

        let responses = outgoing.lock().unwrap();
        // CORRECTED: The server sends a response to init, a response to the call, AND a notification.
        assert_eq!(responses.len(), 3);
        let notif_found = responses.iter().any(|s| s.contains("test/notification"));
        assert!(notif_found, "The test notification was not found");
    }
}
