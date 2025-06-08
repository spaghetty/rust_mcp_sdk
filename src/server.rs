//! Defines the high-level MCP server.
//!
//! This module provides an ergonomic API for building an MCP server using a builder pattern.
//! It handles listening for connections, dispatching requests to registered handlers,
//! and managing the connection lifecycle.

use crate::adapter::NetworkAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, CallToolResult, ErrorData, ErrorResponse, Implementation,
    InitializeRequestParams, InitializeResult, ReadResourceParams, ReadResourceResult, Request,
    RequestId, Resource, Response, ServerCapabilities, Tool, LATEST_PROTOCOL_VERSION,
    METHOD_NOT_FOUND,
};
use crate::TcpAdapter;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;

// --- Handler Type Definitions ---
type ListToolsHandler =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Vec<Tool>>> + Send>> + Send + Sync>;
type CallToolHandler = Arc<
    dyn Fn(String, Value) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
>;
type ListResourcesHandler =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Vec<Resource>>> + Send>> + Send + Sync>;
type ReadResourceHandler = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<ReadResourceResult>> + Send>>
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
    /// Creates a new `Server`.
    pub fn new(name: &str) -> Self {
        Server {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Registers the handler for `tools/list` requests.
    pub fn on_list_tools<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Tool>>> + Send + 'static,
    {
        self.list_tools_handler = Some(Arc::new(move || Box::pin(handler())));
        self
    }

    /// Registers the handler for `tools/call` requests.
    pub fn on_call_tool<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(String, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CallToolResult>> + Send + 'static,
    {
        self.call_tool_handler = Some(Arc::new(move |name, args| Box::pin(handler(name, args))));
        self
    }

    /// Registers the handler for `resources/list` requests.
    pub fn on_list_resources<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Resource>>> + Send + 'static,
    {
        self.list_resources_handler = Some(Arc::new(move || Box::pin(handler())));
        self
    }

    /// Registers the handler for `resources/read` requests.
    pub fn on_read_resource<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult>> + Send + 'static,
    {
        self.read_resource_handler = Some(Arc::new(move |uri| Box::pin(handler(uri))));
        self
    }

    /// Starts the server and listens for incoming TCP connections.
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
                }
            });
        }
    }

    /// REFACTORED: Handles a single client connection using a stateful loop.
    pub async fn handle_connection<A: NetworkAdapter>(
        &self,
        conn: &mut ProtocolConnection<A>,
    ) -> Result<()> {
        let mut is_initialized = false;

        while let Some(raw_req) = conn.recv_message::<Value>().await? {
            // The first message MUST be an initialize request.
            if !is_initialized {
                if let Some("initialize") = raw_req.get("method").and_then(Value::as_str) {
                    let init_req: Request<InitializeRequestParams> =
                        serde_json::from_value(raw_req)?;

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
                    continue; // Continue to the next loop iteration for the next request.
                } else {
                    return Err(anyhow!(
                        "First message from client was not an 'initialize' request."
                    ));
                }
            }

            // After initialization, handle all other requests.
            let req: Request<Value> = serde_json::from_value(raw_req)?;

            match req.method.as_str() {
                "tools/list" => {
                    if let Some(handler) = &self.list_tools_handler {
                        let result = (handler)().await?;
                        let response = Response {
                            id: req.id,
                            jsonrpc: "2.0".to_string(),
                            result,
                        };
                        conn.send_serializable(response).await?;
                    } else {
                        self.send_error(
                            conn,
                            req.id,
                            METHOD_NOT_FOUND,
                            "tools/list handler not registered",
                        )
                        .await?;
                    }
                }
                "tools/call" => {
                    if let Some(handler) = &self.call_tool_handler {
                        let params: CallToolParams = serde_json::from_value(req.params)?;
                        let result = (handler)(params.name, params.arguments).await?;
                        let response = Response {
                            id: req.id,
                            jsonrpc: "2.0".to_string(),
                            result,
                        };
                        conn.send_serializable(response).await?;
                    } else {
                        self.send_error(
                            conn,
                            req.id,
                            METHOD_NOT_FOUND,
                            "tools/call handler not registered",
                        )
                        .await?;
                    }
                }
                "resources/list" => {
                    if let Some(handler) = &self.list_resources_handler {
                        let result = (handler)().await?;
                        let response = Response {
                            id: req.id,
                            jsonrpc: "2.0".to_string(),
                            result,
                        };
                        conn.send_serializable(response).await?;
                    } else {
                        self.send_error(
                            conn,
                            req.id,
                            METHOD_NOT_FOUND,
                            "resources/list handler not registered",
                        )
                        .await?;
                    }
                }
                "resources/read" => {
                    if let Some(handler) = &self.read_resource_handler {
                        let params: ReadResourceParams = serde_json::from_value(req.params)?;
                        let result = (handler)(params.uri).await?;
                        let response = Response {
                            id: req.id,
                            jsonrpc: "2.0".to_string(),
                            result,
                        };
                        conn.send_serializable(response).await?;
                    } else {
                        self.send_error(
                            conn,
                            req.id,
                            METHOD_NOT_FOUND,
                            "resources/read handler not registered",
                        )
                        .await?;
                    }
                }
                "initialize" => {
                    return Err(anyhow!("Client sent 'initialize' request twice."));
                }
                unhandled_method => {
                    self.send_error(
                        conn,
                        req.id,
                        METHOD_NOT_FOUND,
                        &format!("Method '{}' not found", unhandled_method),
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// Helper to send a JSON-RPC error response.
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
