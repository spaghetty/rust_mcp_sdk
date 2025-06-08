//! Full end-to-end integration tests for the MCP SDK.
//!
//! This test compiles the entire `mcp-sdk` crate as a library and then uses its
//! public API to run a client and server to ensure they can communicate correctly.

use anyhow::Result;
use async_trait::async_trait;
use mcp_sdk::{
    CallToolResult, Client, Content, NetworkAdapter, ProtocolConnection, ReadResourceResult,
    Resource, ResourceContents, Server, TcpAdapter, TextContent, TextResourceContents, Tool,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

// --- New Logging Adapter for Debugging ---

/// A wrapper around a NetworkAdapter that logs all messages sent and received.
struct LoggingTcpAdapter<A: NetworkAdapter> {
    inner: A,
    peer: String,
}

impl<A: NetworkAdapter> LoggingTcpAdapter<A> {
    fn new(inner: A, peer: String) -> Self {
        Self { inner, peer }
    }
}

#[async_trait]
impl<A: NetworkAdapter> NetworkAdapter for LoggingTcpAdapter<A> {
    async fn send(&mut self, msg: &str) -> Result<()> {
        println!("[{}] SENDING: {}", self.peer, msg);
        self.inner.send(msg).await
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        let result = self.inner.recv().await;
        match &result {
            Ok(Some(msg)) => println!("[{}] RECEIVED: {}", self.peer, msg),
            Ok(None) => println!("[{}] RECEIVED: Connection Closed", self.peer),
            Err(e) => println!("[{}] RECEIVE ERROR: {}", self.peer, e),
        }
        result
    }
}

// --- Mock Handlers ---

async fn mock_list_tools_handler() -> Result<Vec<Tool>> {
    Ok(vec![Tool {
        name: "e2e-test-tool".to_string(),
        description: Some("An end-to-end test tool".to_string()),
        input_schema: json!({ "type": "object" }),
        annotations: None,
    }])
}

async fn mock_call_tool_handler(name: String, _args: Value) -> Result<CallToolResult> {
    if name != "e2e-test-tool" {
        return Err(anyhow::anyhow!("Unknown tool in e2e test"));
    }
    Ok(CallToolResult {
        content: vec![Content::Text(TextContent {
            r#type: "text".to_string(),
            text: "e2e test successful".to_string(),
        })],
        is_error: false,
    })
}

async fn mock_list_resources_handler() -> Result<Vec<Resource>> {
    Ok(vec![Resource {
        uri: "mcp://e2e/file.txt".to_string(),
        name: "file.txt".to_string(),
        description: Some("An end-to-end test resource".to_string()),
        mime_type: Some("text/plain".to_string()),
    }])
}

async fn mock_read_resource_handler(uri: String) -> Result<ReadResourceResult> {
    if uri != "mcp://e2e/file.txt" {
        return Err(anyhow::anyhow!("Unknown resource in e2e test"));
    }
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::Text(TextResourceContents {
            uri: uri.clone(),
            mime_type: Some("text/plain".to_string()),
            text: "Hello, Resource!".to_string(),
        })],
    })
}

// --- Test Setup ---

async fn setup_test_server(
    server: Arc<Server>,
    num_connections_to_accept: u32,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move {
        for i in 0..num_connections_to_accept {
            if let Ok((stream, _addr)) = listener.accept().await {
                let server_clone = Arc::clone(&server);

                // MODIFIED: Wrap the standard TcpAdapter with our logging one.
                let peer = format!("Server-Conn-{}", i);
                let adapter = LoggingTcpAdapter::new(TcpAdapter::new(stream), peer);
                let mut conn = ProtocolConnection::new(adapter);

                tokio::spawn(async move {
                    server_clone
                        .handle_connection(&mut conn)
                        .await
                        .unwrap_or_else(|e| {
                            if !e.to_string().contains("Connection reset by peer")
                                && !e.to_string().contains(
                                    "An existing connection was forcibly closed by the remote host",
                                )
                            {
                                // This is the error we were seeing before.
                                eprintln!("Test server handler failed: {}", e)
                            }
                        });
                });
            }
        }
    });

    (server_addr, server_handle)
}

// --- The Tests ---

#[tokio::test]
async fn test_full_client_server_interaction() {
    let test_body = async {
        let server = Arc::new(
            Server::new("mcp-e2e-test-server")
                .on_list_tools(mock_list_tools_handler)
                .on_call_tool(mock_call_tool_handler),
        );
        let (server_addr, _server_handle) = setup_test_server(server, 1).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "e2e-test-tool");
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}

#[tokio::test]
async fn test_full_resource_interaction() {
    let test_body = async {
        let server = Arc::new(
            Server::new("mcp-resource-test")
                .on_list_resources(mock_list_resources_handler)
                .on_read_resource(mock_read_resource_handler),
        );
        let (server_addr, _server_handle) = setup_test_server(server, 1).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let resources = client.list_resources().await.unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].uri, "mcp://e2e/file.txt");
        let resource_result = client
            .read_resource("mcp://e2e/file.txt".to_string())
            .await
            .unwrap();
        assert_eq!(resource_result.contents.len(), 1);
        match &resource_result.contents[0] {
            ResourceContents::Text(text_contents) => {
                assert_eq!(text_contents.text, "Hello, Resource!");
            }
            _ => panic!("Expected TextResourceContents"),
        }
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}

#[tokio::test]
async fn test_multiple_interactions_on_one_connection() {
    let test_body = async {
        let server = Arc::new(
            Server::new("mcp-multi-test")
                .on_list_tools(mock_list_tools_handler)
                .on_call_tool(mock_call_tool_handler),
        );
        let (server_addr, _server_handle) = setup_test_server(server, 1).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools[0].name, "e2e-test-tool");
        let result = client
            .call_tool("e2e-test-tool".to_string(), json!({}))
            .await
            .unwrap();
        assert!(!result.is_error);
        let tools_again = client.list_tools().await.unwrap();
        assert_eq!(tools_again[0].name, "e2e-test-tool");
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}

#[tokio::test]
async fn test_call_unregistered_tool_returns_error() {
    let test_body = async {
        let server = Arc::new(Server::new("mcp-error-test").on_list_tools(mock_list_tools_handler));
        let (server_addr, _server_handle) = setup_test_server(server, 1).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let result = client
            .call_tool("this-tool-does-not-exist".to_string(), json!({}))
            .await;
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("tools/call handler not registered"));
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}
