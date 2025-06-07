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
use anyhow::Result;
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};

/// A high-level client for interacting with an MCP server.
pub struct Client {
    connection: tokio::sync::Mutex<ProtocolConnection<TcpAdapter>>,
    next_request_id: AtomicI64,
    // We can store server capabilities here later.
    // server_capabilities: ServerCapabilities,
}

impl Client {
    /// Connects to an MCP server and performs the initialization handshake.
    pub async fn connect(addr: &str) -> Result<Self> {
        let adapter = TcpAdapter::connect(addr).await?;
        let mut connection = ProtocolConnection::new(adapter);

        // Perform the MCP initialize handshake.
        let init_request_id = RequestId::Num(0); // Standard to use 0 for init
        let init_params = InitializeRequestParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            client_info: Implementation {
                name: "mcp-rust-sdk-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ClientCapabilities::default(),
        };

        let init_request = Request {
            jsonrpc: "2.0".to_string(),
            id: init_request_id.clone(),
            method: "initialize".to_string(),
            params: init_params,
        };

        connection.send_message(init_request).await?;

        let response: JSONRPCResponse<InitializeResult> = connection
            .recv_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed during initialization"))?;

        match response {
            JSONRPCResponse::Success(success) => {
                if success.id != init_request_id {
                    return Err(anyhow::anyhow!("Mismatched initialize response ID"));
                }
                println!(
                    "Handshake successful. Server: {:?}",
                    success.result.server_info
                );
                // Can store success.result.capabilities if needed
            }
            JSONRPCResponse::Error(err) => {
                return Err(anyhow::anyhow!("Initialization failed: {:?}", err.error));
            }
        }

        Ok(Self {
            connection: tokio::sync::Mutex::new(connection),
            next_request_id: AtomicI64::new(1), // Start subsequent requests from 1
        })
    }

    /// Generates a new, unique request ID for a JSON-RPC message.
    fn new_request_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        RequestId::Num(id)
    }

    /// Fetches the list of available tools from the server.
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let request_id = self.new_request_id();
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: request_id.clone(),
            method: "tools/list".to_string(),
            params: ListToolsParams {},
        };

        let mut conn = self.connection.lock().await;
        conn.send_message(request).await?;

        let response: JSONRPCResponse<Vec<Tool>> = conn
            .recv_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed by server"))?;

        match response {
            JSONRPCResponse::Success(success) => Ok(success.result),
            JSONRPCResponse::Error(err) => {
                Err(anyhow::anyhow!("list_tools failed: {:?}", err.error))
            }
        }
    }

    /// Calls a specific tool on the server.
    pub async fn call_tool(&self, name: String, arguments: Value) -> Result<CallToolResult> {
        let request_id = self.new_request_id();
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: request_id.clone(),
            method: "tools/call".to_string(),
            params: CallToolParams { name, arguments },
        };

        let mut conn = self.connection.lock().await;
        conn.send_message(request).await?;

        let response: JSONRPCResponse<CallToolResult> = conn
            .recv_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed by server"))?;

        match response {
            JSONRPCResponse::Success(success) => Ok(success.result),
            JSONRPCResponse::Error(err) => {
                Err(anyhow::anyhow!("call_tool failed: {:?}", err.error))
            }
        }
    }

    /// Fetches the list of available resources from the server.
    pub async fn list_resources(&self) -> Result<Vec<Resource>> {
        let request_id = self.new_request_id();
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: request_id.clone(),
            method: "resources/list".to_string(),
            params: ListResourcesParams {},
        };

        let mut conn = self.connection.lock().await;
        conn.send_message(request).await?;

        let response: JSONRPCResponse<Vec<Resource>> = conn
            .recv_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed by server"))?;

        match response {
            JSONRPCResponse::Success(success) => Ok(success.result),
            JSONRPCResponse::Error(err) => {
                Err(anyhow::anyhow!("list_resources failed: {:?}", err.error))
            }
        }
    }

    /// Reads the content of a specific resource from the server.
    pub async fn read_resource(&self, uri: String) -> Result<ReadResourceResult> {
        let request_id = self.new_request_id();
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: request_id.clone(),
            method: "resources/read".to_string(),
            params: ReadResourceParams { uri },
        };

        let mut conn = self.connection.lock().await;
        conn.send_message(request).await?;

        let response: JSONRPCResponse<ReadResourceResult> = conn
            .recv_message()
            .await?
            .ok_or_else(|| anyhow::anyhow!("Connection closed by server"))?;

        match response {
            JSONRPCResponse::Success(success) => Ok(success.result),
            JSONRPCResponse::Error(err) => {
                Err(anyhow::anyhow!("read_resource failed: {:?}", err.error))
            }
        }
    }
}

// --- Unit Tests ---
// These tests verify the client's internal logic, especially the handshake.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::NetworkAdapter;
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

    // This is a reimplementation of the client's `connect` logic, but on a generic
    // adapter so that we can test it with our `MockAdapter`.
    async fn connect_with_mock_adapter(
        mut connection: ProtocolConnection<MockAdapter>,
    ) -> Result<()> {
        let init_request_id = RequestId::Num(0);
        let init_params = InitializeRequestParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            client_info: Implementation {
                name: "test-client".into(),
                version: "0.1.0".into(),
            },
            capabilities: ClientCapabilities::default(),
        };
        let init_request = Request {
            jsonrpc: "2.0".to_string(),
            id: init_request_id.clone(),
            method: "initialize".to_string(),
            params: init_params,
        };
        connection.send_message(init_request).await?;
        let response: JSONRPCResponse<InitializeResult> = connection.recv_message().await?.unwrap();
        match response {
            JSONRPCResponse::Success(_) => Ok(()),
            JSONRPCResponse::Error(err) => Err(anyhow::anyhow!("Init failed: {:?}", err.error)),
        }
    }

    #[tokio::test]
    async fn test_connect_sends_initialize_and_receives_ok() {
        // 1. Setup mock adapter and connection
        let adapter = MockAdapter::default();
        let outgoing_buffer = Arc::clone(&adapter.outgoing);
        let connection = ProtocolConnection::new(adapter.clone());

        // 2. Queue up the server's successful response to the initialize request
        let init_response = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "result": {
                "protocolVersion": LATEST_PROTOCOL_VERSION,
                "serverInfo": { "name": "mock-server", "version": "0.1.0" },
                "capabilities": {}
            }
        });
        adapter.push_incoming(serde_json::to_string(&init_response).unwrap());

        // 3. Run our mock connection logic
        let result = connect_with_mock_adapter(connection).await;
        assert!(result.is_ok());

        // 4. Assert that the client sent the correct initialize request
        let sent_messages = outgoing_buffer.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        let sent_req_str = sent_messages.front().unwrap();
        let sent_req: Request<InitializeRequestParams> =
            serde_json::from_str(sent_req_str).unwrap();

        assert_eq!(sent_req.method, "initialize");
        assert_eq!(sent_req.id, RequestId::Num(0));
        assert_eq!(sent_req.params.protocol_version, LATEST_PROTOCOL_VERSION);
    }
}
