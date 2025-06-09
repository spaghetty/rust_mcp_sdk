//! Defines the internal `ClientSession` that manages the connection's background task.

use crate::{
    adapter::NetworkAdapter,
    protocol::ProtocolConnection,
    types::{JSONRPCResponse, Request, RequestId},
};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, oneshot, Mutex};

// --- Type Aliases ---
pub(crate) type ResponseResult = Result<Value, anyhow::Error>;
pub(crate) type ResponseSender = oneshot::Sender<ResponseResult>;
pub(crate) type PendingRequestMap = Arc<Mutex<HashMap<RequestId, ResponseSender>>>;
pub(crate) type NotificationHandler = Arc<dyn Fn(Value) + Send + Sync>;
pub(crate) type NotificationHandlerMap = Arc<DashMap<String, NotificationHandler>>;

pub(crate) struct ClientSession<A: NetworkAdapter> {
    connection: ProtocolConnection<A>,
    pending_requests: PendingRequestMap,
    notification_handlers: NotificationHandlerMap,
    request_receiver: mpsc::Receiver<(Request<Value>, ResponseSender)>,
}

impl<A: NetworkAdapter + Send + 'static> ClientSession<A> {
    pub(crate) fn new(
        connection: ProtocolConnection<A>,
        pending_requests: PendingRequestMap,
        notification_handlers: NotificationHandlerMap,
        request_receiver: mpsc::Receiver<(Request<Value>, ResponseSender)>,
    ) -> Self {
        Self {
            connection,
            pending_requests,
            notification_handlers,
            request_receiver,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                biased;

                Some((request, responder)) = self.request_receiver.recv() => {
                    self.pending_requests.lock().await.insert(request.id.clone(), responder);
                    if let Err(e) = self.connection.send_serializable(request).await {
                        eprintln!("[Client] Error writing message to server: {}", e);
                        break;
                    }
                },
                read_result = self.connection.recv_message::<Value>() => {
                    match read_result {
                        Ok(Some(raw_message)) => {
                            if raw_message.get("id").is_some() {
                                Self::handle_response(raw_message, &self.pending_requests).await;
                            } else if raw_message.get("method").is_some() {
                                Self::handle_notification(raw_message, self.notification_handlers.clone());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::NetworkAdapter, protocol::ProtocolConnection, types::ListToolsChangedParams,
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, Ordering}; // CORRECTED: Added missing import
    use std::time::Duration;
    use tokio::sync::{mpsc as async_mpsc, Mutex as TokioMutex};
    use tokio::task::JoinHandle;

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
        pending_requests: PendingRequestMap,
        notification_handlers: NotificationHandlerMap,
        request_sender: mpsc::Sender<(Request<Value>, ResponseSender)>,
        _connection_handle: JoinHandle<()>,
    }

    fn setup_session_test() -> TestHarness {
        let adapter = MockAdapter::new();
        let connection = ProtocolConnection::new(adapter.clone());
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let notification_handlers = Arc::new(DashMap::new());
        let (request_sender, request_receiver) = mpsc::channel(32);

        let session = ClientSession {
            connection,
            pending_requests: Arc::clone(&pending_requests),
            notification_handlers: Arc::clone(&notification_handlers),
            request_receiver,
        };

        let connection_handle = tokio::spawn(session.run());

        TestHarness {
            adapter,
            pending_requests,
            notification_handlers,
            request_sender,
            _connection_handle: connection_handle,
        }
    }

    #[tokio::test]
    async fn test_session_handles_response() {
        let harness = setup_session_test();
        let (tx, rx) = oneshot::channel::<ResponseResult>();

        let request_id = RequestId::Num(1);
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
    async fn test_session_handles_notification() {
        let harness = setup_session_test();
        let handler_was_called = Arc::new(AtomicBool::new(false));
        let handler_was_called_clone = Arc::clone(&handler_was_called);

        let handler: NotificationHandler = Arc::new(move |params: Value| {
            let _params: ListToolsChangedParams = serde_json::from_value(params).unwrap();
            handler_was_called_clone.store(true, Ordering::SeqCst);
        });
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
    async fn test_session_sends_requests() {
        let harness = setup_session_test();

        let request_payload = Request {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Num(1),
            method: "test".to_string(),
            params: Value::Null,
        };
        let (tx, _rx) = oneshot::channel();

        harness
            .request_sender
            .send((request_payload, tx))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        let sent_message = harness.adapter.pop_outgoing().await.unwrap();
        assert!(sent_message.contains("\"method\":\"test\""));
    }
}
