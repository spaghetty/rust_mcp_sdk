//! Defines the high-level MCP server.
//!
//! This module provides an ergonomic API for building an MCP server using a builder pattern.
//! It handles listening for connections, dispatching requests to registered handlers,
//! and managing the connection lifecycle.

use crate::adapter::NetworkAdapter;
use crate::protocol::ProtocolConnection;
use crate::types::{
    CallToolParams, CallToolResult, ErrorData, ErrorResponse, Implementation,
    InitializeRequestParams, InitializeResult, Request, RequestId, Response, ServerCapabilities,
    Tool, LATEST_PROTOCOL_VERSION, METHOD_NOT_FOUND,
};
use crate::TcpAdapter;
use anyhow::Result;
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

/// A high-level server for handling MCP requests.
#[derive(Default)]
pub struct Server {
    name: String,
    list_tools_handler: Option<ListToolsHandler>,
    call_tool_handler: Option<CallToolHandler>,
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

    /// Starts the server and listens for incoming TCP connections.
    pub async fn listen(self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("{} listening on {}", self.name, addr);
        let server = Arc::new(self);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            println!("Accepted connection from: {}", client_addr);
            let server_clone = Arc::clone(&server);
            tokio::spawn(async move {
                let adapter = TcpAdapter::new(stream);
                let mut conn = ProtocolConnection::new(adapter);
                if let Err(e) = server_clone.handle_connection(&mut conn).await {
                    eprintln!("Error handling connection from {}: {}", client_addr, e);
                }
            });
        }
    }

    /// Handles a single client connection, starting with the initialize handshake.
    /// This method is generic over any type that implements `NetworkAdapter`.
    pub async fn handle_connection<A: NetworkAdapter>(
        &self,
        conn: &mut ProtocolConnection<A>,
    ) -> Result<()> {
        // First, robustly handle the initialize handshake.
        if let Some(first_req_val) = conn.recv_message::<Value>().await? {
            if let Some(method) = first_req_val.get("method").and_then(Value::as_str) {
                if method == "initialize" {
                    let init_req: Request<InitializeRequestParams> =
                        serde_json::from_value(first_req_val)?;
                    println!(
                        "Received initialize from: {:?}",
                        init_req.params.client_info
                    );
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
                    conn.send_message(init_response).await?;
                } else {
                    return Err(anyhow::anyhow!(
                        "First message was not initialize, but {}",
                        method
                    ));
                }
            } else {
                return Err(anyhow::anyhow!("First message was not a valid request."));
            }
        } else {
            return Ok(()); // Connection closed immediately, which is fine.
        }

        // After successful handshake, enter the main request dispatch loop.
        while let Some(req) = conn.recv_message::<Request<Value>>().await? {
            match req.method.as_str() {
                "tools/list" => {
                    if let Some(handler) = &self.list_tools_handler {
                        let result = (handler)().await?;
                        let response = Response {
                            id: req.id,
                            jsonrpc: "2.0".to_string(),
                            result,
                        };
                        conn.send_message(response).await?;
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
                        conn.send_message(response).await?;
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
                unhandled_method => {
                    self.send_error(
                        conn,
                        req.id,
                        METHOD_NOT_FOUND,
                        &format!("Method '{}' not found", unhandled_method),
                    )
                    .await?;
                }
            };
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
        conn.send_message(error_response).await
    }
}

// --- Unit Tests ---
// These tests verify the server's internal logic in isolation.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::NetworkAdapter;
    use crate::types::JSONRPCResponse;
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    // --- Mock Infrastructure for Testing ---

    /// A mock adapter that uses an in-memory queue.
    #[derive(Default, Clone)]
    struct MockAdapter {
        incoming: Arc<Mutex<VecDeque<String>>>,
        outgoing: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockAdapter {
        /// Helper method for tests to queue up a message as if it were from a client.
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

    // Helper to create a mock connection and a way to interact with it.
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

    // --- Test Cases ---

    #[tokio::test]
    async fn test_handler_registration() {
        // This test ensures that the builder pattern correctly registers handlers.
        let server = Server::new("test")
            .on_list_tools(|| async { Ok(vec![]) })
            .on_call_tool(|_, _| async {
                Ok(CallToolResult {
                    content: vec![],
                    is_error: false,
                })
            });

        assert!(server.list_tools_handler.is_some());
        assert!(server.call_tool_handler.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_to_list_tools_handler() {
        // This test verifies that a "tools/list" request is correctly dispatched.
        let server = Arc::new(Server::new("test").on_list_tools(|| async {
            Ok(vec![Tool {
                name: "test-tool-from-handler".to_string(),
                description: None,
                input_schema: json!({}),
                annotations: None,
            }])
        }));

        let (mut conn, adapter_handle, outgoing) = setup_mock_connection();

        // Simulate client sending an initialize request.
        let init_req = json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "protocolVersion": "test", "clientInfo": {"name": "test", "version": "0"}, "capabilities": {} }
        });
        adapter_handle.push_incoming(serde_json::to_string(&init_req).unwrap());

        // Simulate client sending a tools/list request.
        let list_req = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
        adapter_handle.push_incoming(serde_json::to_string(&list_req).unwrap());

        // Run the handler for the connection.
        server.handle_connection(&mut conn).await.unwrap();

        // Check the server's response.
        let responses = outgoing.lock().unwrap();
        assert_eq!(responses.len(), 2); // Init response + list response

        let list_response_str = responses.get(1).unwrap();
        let list_response: JSONRPCResponse<Vec<Tool>> =
            serde_json::from_str(list_response_str).unwrap();

        if let JSONRPCResponse::Success(res) = list_response {
            assert_eq!(res.result.len(), 1);
            assert_eq!(res.result[0].name, "test-tool-from-handler");
        } else {
            panic!("Expected a successful response for tools/list");
        }
    }
}
