//! Defines the high-level MCP client.
//!
//! This module provides a simple, ergonomic API for connecting to and interacting with
//! an MCP server. It handles connection, initialization, request/response lifecycle,
//! and error handling, abstracting away the underlying protocol and transport details.

use crate::adapter::{NetworkAdapter, TcpAdapter};
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, CallToolResult, ClientCapabilities, Implementation, InitializeRequestParams,
    InitializeResult, JSONRPCResponse, ListResourcesParams, ListToolsParams, ReadResourceParams,
    ReadResourceResult, Request, RequestId, Resource, Tool, LATEST_PROTOCOL_VERSION,
};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

// --- Type Aliases ---
type ResponseResult = Result<Value, anyhow::Error>;
type ResponseSender = oneshot::Sender<ResponseResult>;
type PendingRequestMap = Arc<Mutex<HashMap<RequestId, ResponseSender>>>;
type NotificationHandler = Arc<dyn Fn(Value) + Send + Sync>;
type NotificationHandlerMap = Arc<DashMap<String, NotificationHandler>>;

pub struct Client {
    next_request_id: AtomicI64,
    pending_requests: PendingRequestMap,
    request_sender: mpsc::Sender<String>,
    connection_handle: JoinHandle<()>,
    notification_handlers: NotificationHandlerMap,
}

impl Client {
    pub async fn connect(addr: &str) -> Result<Self> {
        let adapter = TcpAdapter::connect(addr).await?;
        let connection = ProtocolConnection::new(adapter);

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let notification_handlers = Arc::new(DashMap::new());
        let (request_sender, request_receiver) = mpsc::channel::<String>(32);

        let client = Self {
            next_request_id: AtomicI64::new(1),
            pending_requests: Arc::clone(&pending_requests),
            request_sender,
            connection_handle: tokio::spawn(Self::connection_loop(
                connection,
                pending_requests,
                Arc::clone(&notification_handlers),
                request_receiver,
            )),
            notification_handlers,
        };

        let init_params = InitializeRequestParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            client_info: Implementation {
                name: "mcp-rust-sdk-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ClientCapabilities {
                tools: Some(crate::types::ToolsCapability {
                    list_changed: Some(true),
                }),
                ..Default::default()
            },
        };

        let init_result_val = client
            .send_request_with_id(RequestId::Num(0), "initialize", init_params)
            .await?;
        let init_response: InitializeResult = serde_json::from_value(init_result_val)?;

        println!(
            "[Client] Handshake successful. Server: {:?}",
            init_response.server_info
        );

        Ok(client)
    }

    async fn connection_loop<A>(
        mut connection: ProtocolConnection<A>,
        pending_requests: PendingRequestMap,
        notification_handlers: NotificationHandlerMap,
        mut request_receiver: mpsc::Receiver<String>,
    ) where
        A: NetworkAdapter + Send + 'static,
    {
        loop {
            tokio::select! {
                // CORRECTED: Prioritize sending outgoing messages over waiting for incoming ones.
                biased;

                Some(request_json) = request_receiver.recv() => {
                    if let Err(e) = connection.send_raw(&request_json).await {
                        eprintln!("[Client] Error writing message to server: {}", e);
                        break;
                    }
                },
                read_result = connection.recv_message::<Value>() => {
                    match read_result {
                        Ok(Some(raw_message)) => {
                            if raw_message.get("id").is_some() {
                                Self::handle_response(raw_message, &pending_requests).await;
                            } else if raw_message.get("method").is_some() {
                                Self::handle_notification(raw_message, notification_handlers.clone());
                            }
                        },
                        Ok(None) => {
                             println!("[Client] Connection closed by server.");
                             break;
                        }
                        Err(e) => {
                            eprintln!("[Client] Error reading message from server: {}", e);
                            break;
                        }
                    }
                },
            }
        }
    }

    async fn handle_response(raw_message: Value, pending_requests: &PendingRequestMap) {
        if let Ok(id) = serde_json::from_value::<RequestId>(raw_message["id"].clone()) {
            if let Some(sender) = pending_requests.lock().await.remove(&id) {
                let response: Result<JSONRPCResponse<Value>, _> =
                    serde_json::from_value(raw_message);
                match response {
                    Ok(JSONRPCResponse::Success(success)) => {
                        let _ = sender.send(Ok(success.result));
                    }
                    Ok(JSONRPCResponse::Error(err)) => {
                        let _ = sender.send(Err(anyhow!(
                            "Server returned an error: code={}, message='{}'",
                            err.error.code,
                            err.error.message
                        )));
                    }
                    Err(e) => {
                        let _ = sender.send(Err(anyhow!("Failed to deserialize response: {}", e)));
                    }
                }
            }
        }
    }

    fn handle_notification(raw_message: Value, handlers: NotificationHandlerMap) {
        if let Some(method) = raw_message.get("method").and_then(Value::as_str) {
            if let Some(handler) = handlers.get(method) {
                let handler = handler.clone();
                let params = raw_message.get("params").cloned().unwrap_or(Value::Null);

                tokio::spawn(async move {
                    (handler)(params);
                });
            } else {
                println!("[Client] Received unhandled notification: {}", method);
            }
        }
    }

    pub fn on_tools_list_changed<F, P>(&self, handler: F)
    where
        F: Fn(P) + Send + Sync + 'static,
        P: DeserializeOwned,
    {
        let wrapped_handler: NotificationHandler =
            Arc::new(
                move |params: Value| match serde_json::from_value::<P>(params) {
                    Ok(typed_params) => (handler)(typed_params),
                    Err(e) => eprintln!(
                        "[Client] Failed to deserialize params for 'tools/listChanged': {}",
                        e
                    ),
                },
            );

        self.notification_handlers.insert(
            "notifications/tools/list_changed".to_string(),
            wrapped_handler,
        );
    }

    fn new_request_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Num(id)
    }

    async fn send_request<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        let response_val = self
            .send_request_with_id(self.new_request_id(), method, params)
            .await?;
        Ok(serde_json::from_value(response_val)?)
    }

    async fn send_request_with_id<P>(
        &self,
        request_id: RequestId,
        method: &str,
        params: P,
    ) -> Result<Value>
    where
        P: serde::Serialize,
    {
        let (tx, rx) = oneshot::channel::<ResponseResult>();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: request_id,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request)?;
        self.request_sender.send(request_json).await?;

        rx.await?
    }

    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        self.send_request("tools/list", ListToolsParams {}).await
    }

    pub async fn call_tool(&self, name: String, arguments: Value) -> Result<CallToolResult> {
        self.send_request("tools/call", CallToolParams { name, arguments })
            .await
    }

    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        self.send_request("resources/list", ListResourcesParams {})
            .await
    }

    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        self.send_request("resources/read", ReadResourceParams { uri })
            .await
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.connection_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::NetworkAdapter;
    use crate::types::ListToolsChangedParams;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;
    use tokio::sync::{mpsc as async_mpsc, Mutex as TokioMutex};

    // --- Mock Adapter for Client Tests ---
    #[derive(Clone)]
    struct MockAdapter {
        incoming_tx: async_mpsc::Sender<String>,
        incoming_rx: Arc<TokioMutex<async_mpsc::Receiver<String>>>,
        outgoing: Arc<TokioMutex<Vec<String>>>,
    }

    impl MockAdapter {
        fn new() -> Self {
            let (incoming_tx, incoming_rx) = async_mpsc::channel(32);
            Self {
                incoming_tx,
                incoming_rx: Arc::new(TokioMutex::new(incoming_rx)),
                outgoing: Arc::new(TokioMutex::new(Vec::new())),
            }
        }
        async fn push_incoming(&self, msg: String) {
            self.incoming_tx.send(msg).await.unwrap();
        }
        async fn pop_outgoing(&self) -> Option<String> {
            self.outgoing.lock().await.pop()
        }
    }

    #[async_trait]
    impl NetworkAdapter for MockAdapter {
        async fn send(&mut self, msg: &str) -> Result<()> {
            self.outgoing.lock().await.push(msg.to_string());
            Ok(())
        }

        async fn recv(&mut self) -> Result<Option<String>> {
            Ok(self.incoming_rx.lock().await.recv().await)
        }
    }

    // --- Test Harness ---
    struct TestHarness {
        adapter: MockAdapter,
        // CORRECTED: The harness now holds onto the state for tests to use.
        pending_requests: PendingRequestMap,
        notification_handlers: NotificationHandlerMap,
        request_sender: mpsc::Sender<String>,
        _connection_handle: JoinHandle<()>,
    }

    fn setup_loop_test() -> TestHarness {
        let adapter = MockAdapter::new();
        let connection = ProtocolConnection::new(adapter.clone());
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let notification_handlers = Arc::new(DashMap::new());
        let (request_sender, request_receiver) = mpsc::channel::<String>(32);

        // CORRECTED: Clone the Arcs *before* moving them into the task.
        let pending_req_clone = Arc::clone(&pending_requests);
        let notif_handlers_clone = Arc::clone(&notification_handlers);

        let connection_handle = tokio::spawn(Client::connection_loop(
            connection,
            pending_req_clone,
            notif_handlers_clone,
            request_receiver,
        ));

        TestHarness {
            adapter,
            pending_requests,
            notification_handlers,
            request_sender,
            _connection_handle: connection_handle,
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn test_connection_loop_handles_response() {
        let harness = setup_loop_test();
        let (tx, rx) = oneshot::channel::<ResponseResult>();

        let request_id = RequestId::Num(1);
        // CORRECTED: Access the state via the harness struct.
        harness.pending_requests.lock().await.insert(request_id, tx);

        let response_json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "status": "ok" }
        })
        .to_string();
        harness.adapter.push_incoming(response_json).await;

        let result = tokio::time::timeout(Duration::from_secs(1), rx)
            .await
            .expect("Test timed out")
            .expect("Oneshot channel failed");

        assert_eq!(result.unwrap(), json!({ "status": "ok" }));
    }

    #[tokio::test]
    async fn test_connection_loop_handles_notification() {
        let harness = setup_loop_test();
        let handler_was_called = Arc::new(AtomicBool::new(false));
        let handler_was_called_clone = Arc::clone(&handler_was_called);

        let handler: NotificationHandler = Arc::new(move |params: Value| {
            let _params: ListToolsChangedParams = serde_json::from_value(params).unwrap();
            handler_was_called_clone.store(true, Ordering::SeqCst);
        });
        // CORRECTED: Access the state via the harness struct.
        harness
            .notification_handlers
            .insert("notifications/tools/list_changed".to_string(), handler);

        let notification_json = json!({
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
            "params": {}
        })
        .to_string();
        harness.adapter.push_incoming(notification_json).await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            handler_was_called.load(Ordering::SeqCst),
            "Notification handler was not called"
        );
    }

    #[tokio::test]
    async fn test_connection_loop_sends_requests() {
        let harness = setup_loop_test();

        let request_json = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        })
        .to_string();
        harness
            .request_sender
            .send(request_json.clone())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let sent_message = harness.adapter.pop_outgoing().await.unwrap();
        assert_eq!(sent_message, request_json);
    }
}
