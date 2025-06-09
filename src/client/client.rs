//! Defines the public-facing `Client` struct and its API methods.

use super::session::{ClientSession, NotificationHandler, NotificationHandlerMap, ResponseResult};
use crate::{
    adapter::TcpAdapter,
    protocol::ProtocolConnection,
    types::{
        CallToolParams, CallToolResult, ClientCapabilities, GetPromptParams, GetPromptResult,
        Implementation, InitializeRequestParams, InitializeResult, ListPromptsParams,
        ListPromptsResult, ListResourcesParams, ListToolsParams, ReadResourceParams,
        ReadResourceResult, Request, RequestId, Resource, Tool, LATEST_PROTOCOL_VERSION,
    },
};
use anyhow::Result;
use dashmap::DashMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

/// A high-level client for interacting with an MCP server.
pub struct Client {
    next_request_id: AtomicI64,
    request_sender: mpsc::Sender<(Request<Value>, oneshot::Sender<ResponseResult>)>,
    notification_handlers: NotificationHandlerMap,
    session_handle: JoinHandle<()>,
}

impl Client {
    /// Connects to an MCP server, performs the initialization handshake,
    /// and spawns the background task to manage the connection.
    pub async fn connect(addr: &str) -> Result<Self> {
        let adapter = TcpAdapter::connect(addr).await?;
        let connection = ProtocolConnection::new(adapter);

        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let notification_handlers = Arc::new(DashMap::new());
        let (request_sender, request_receiver) = mpsc::channel(32);

        let session = ClientSession::new(
            connection,
            pending_requests,
            Arc::clone(&notification_handlers),
            request_receiver,
        );

        let session_handle = tokio::spawn(session.run());

        let client = Self {
            next_request_id: AtomicI64::new(1), // Start subsequent requests from 1
            request_sender,
            notification_handlers,
            session_handle,
        };

        // Perform the MCP initialize handshake.
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

        // The first request must have ID 0.
        let init_response: InitializeResult = client
            .send_request_with_id(RequestId::Num(0), "initialize", init_params)
            .await?;

        println!(
            "[Client] Handshake successful. Server: {:?}",
            init_response.server_info
        );

        Ok(client)
    }

    /// Registers a handler for the `tools/listChanged` notification.
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
        let request_id = self.new_request_id();
        self.send_request_with_id(request_id, method, params).await
    }

    async fn send_request_with_id<P, R>(&self, id: RequestId, method: &str, params: P) -> Result<R>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        let request_payload = Request {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params: serde_json::to_value(params)?,
        };

        let (tx, rx) = oneshot::channel();
        self.request_sender.send((request_payload, tx)).await?;
        let response_val = rx.await??;
        Ok(serde_json::from_value(response_val)?)
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

    // NEW: Add methods for prompts
    pub async fn list_prompts(&self) -> Result<ListPromptsResult> {
        self.send_request("prompts/list", ListPromptsParams {})
            .await
    }

    pub async fn get_prompt(
        &self,
        name: String,
        arguments: Option<Value>,
    ) -> Result<GetPromptResult> {
        self.send_request("prompts/get", GetPromptParams { name, arguments })
            .await
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.session_handle.abort();
    }
}
