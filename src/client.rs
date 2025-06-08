//! Defines the high-level MCP client.
//!
//! This module provides a simple, ergonomic API for connecting to and interacting with
//! an MCP server. It handles connection, initialization, request/response lifecycle,
//! and error handling, abstracting away the underlying protocol and transport details.

use crate::adapter::TcpAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, CallToolResult, ClientCapabilities, Implementation, InitializeRequestParams,
    InitializeResult, JSONRPCResponse, ListResourcesParams, ListToolsParams, ReadResourceParams,
    ReadResourceResult, Request, RequestId, Resource, Tool, LATEST_PROTOCOL_VERSION,
};
use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

/// The type used for the value returned in a response channel.
type ResponseResult = Result<Value, anyhow::Error>;
/// A sender for a oneshot channel that will carry a response.
type ResponseSender = oneshot::Sender<ResponseResult>;

/// A map from a request ID to a sender that will resolve the request's Future.
/// This version uses Tokio's async-aware Mutex for guaranteed safety.
type PendingRequestMap = Arc<Mutex<HashMap<RequestId, ResponseSender>>>;

/// A high-level client for interacting with an MCP server.
pub struct Client {
    /// The next ID to use for a request.
    next_request_id: AtomicI64,
    /// A map of request IDs to channels for sending back the response.
    pending_requests: PendingRequestMap,
    /// A channel for sending requests to the connection's write loop.
    request_sender: mpsc::Sender<String>,
    /// A handle to the background task that manages the connection.
    connection_handle: JoinHandle<()>,
}

impl Client {
    /// Connects to an MCP server, performs the initialization handshake,
    /// and spawns the background task to manage the connection.
    pub async fn connect(addr: &str) -> Result<Self> {
        let adapter = TcpAdapter::connect(addr).await?;
        let connection = ProtocolConnection::new(adapter);

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let (request_sender, request_receiver) = mpsc::channel::<String>(32);

        let client = Self {
            next_request_id: AtomicI64::new(1),
            pending_requests: Arc::clone(&pending_requests),
            request_sender,
            connection_handle: tokio::spawn(Self::connection_loop(
                connection,
                pending_requests,
                request_receiver,
            )),
        };

        let init_params = InitializeRequestParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            client_info: Implementation {
                name: "mcp-rust-sdk-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ClientCapabilities::default(),
        };

        // We deserialize the result of the handshake directly.
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

    /// The main loop for a connection.
    async fn connection_loop(
        mut connection: ProtocolConnection<TcpAdapter>,
        pending_requests: PendingRequestMap,
        mut request_receiver: mpsc::Receiver<String>,
    ) {
        loop {
            tokio::select! {
                read_result = connection.recv_message::<Value>() => {
                    match read_result {
                        Ok(Some(raw_message)) => {
                            if raw_message.get("id").is_some() {
                                Self::handle_response(raw_message, &pending_requests).await;
                            } else if raw_message.get("method").is_some() {
                                Self::handle_notification(raw_message);
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
                Some(request_json) = request_receiver.recv() => {
                    // CORRECTED: Use the new `send_raw` method to respect the protocol layer.
                    if let Err(e) = connection.send_raw(&request_json).await {
                        eprintln!("[Client] Error writing message to server: {}", e);
                        break;
                    }
                }
            }
        }
    }

    /// Handles a response message from the server.
    async fn handle_response(raw_message: Value, pending_requests: &PendingRequestMap) {
        if let Ok(id) = serde_json::from_value::<RequestId>(raw_message["id"].clone()) {
            // Lock the map and remove the sender for the given ID.
            if let Some(sender) = pending_requests.lock().await.remove(&id) {
                // Deserialize into a generic success/error response
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

    /// Handles a notification message from the server.
    fn handle_notification(raw_message: Value) {
        if let Ok(method) = serde_json::from_value::<String>(raw_message["method"].clone()) {
            println!(
                "[Client] Received notification with method: '{}'. Params: {}",
                method, raw_message["params"]
            );
        }
    }

    /// Generates a new, unique request ID.
    fn new_request_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Num(id)
    }

    /// A generic method to send any request and await its response.
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

    /// Low-level method to send a request and get back the raw `Value` of the result.
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
            // Lock the map and insert the sender.
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

        // Await the response from the oneshot channel, then propagate any errors.
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
