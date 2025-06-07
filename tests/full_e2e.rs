//! Full end-to-end integration tests for the MCP SDK.
//!
//! This test compiles the entire `mcp-sdk` crate as a library and then uses its
//! public API to run a client and server to ensure they can communicate correctly.

use anyhow::Result;
use mcp_sdk::{
    CallToolResult, Client, Content, ProtocolConnection, ReadResourceResult, Resource,
    ResourceContents, Server, TcpAdapter, TextContent, TextResourceContents, Tool,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

// --- Mock Handlers ---
// These are the application-level logic implementations for our test server.

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

/// A helper to set up a test server in the background.
/// It accepts a specified number of connections and then shuts down.
async fn setup_test_server(
    server: Arc<Server>,
    num_connections_to_accept: u32,
) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move {
        for _ in 0..num_connections_to_accept {
            if let Ok((stream, _)) = listener.accept().await {
                let server_clone = Arc::clone(&server);
                let mut conn = ProtocolConnection::new(TcpAdapter::new(stream));
                // We spawn a task for each connection so the listener can immediately
                // accept the next one if the test requires it.
                tokio::spawn(async move {
                    server_clone
                        .handle_connection(&mut conn)
                        .await
                        .unwrap_or_else(|e| {
                            // Ignore "Connection reset" errors which can happen when the client disconnects
                            if !e.to_string().contains("Connection reset by peer") {
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
    let server = Arc::new(
        Server::new("mcp-e2e-test-server")
            .on_list_tools(mock_list_tools_handler)
            .on_call_tool(mock_call_tool_handler),
    );
    let (server_addr, _server_handle) = setup_test_server(server, 1).await;

    // The Client::connect function now performs the handshake automatically.
    let client = Client::connect(&server_addr).await.unwrap();

    // Test `list_tools` (this happens *after* the handshake).
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "e2e-test-tool");
}

#[tokio::test]
async fn test_full_resource_interaction() {
    let server = Arc::new(
        Server::new("mcp-resource-test")
            .on_list_resources(mock_list_resources_handler)
            .on_read_resource(mock_read_resource_handler),
    );
    let (server_addr, _server_handle) = setup_test_server(server, 1).await;
    let client = Client::connect(&server_addr).await.unwrap();

    // 1. List the available resources
    let resources = client.list_resources().await.unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].name, "file.txt");
    assert_eq!(resources[0].uri, "mcp://e2e/file.txt");

    // 2. Read a specific resource
    let resource_result = client
        .read_resource("mcp://e2e/file.txt".to_string())
        .await
        .unwrap();
    assert_eq!(resource_result.contents.len(), 1);

    // 3. Verify the contents of the read resource
    match &resource_result.contents[0] {
        ResourceContents::Text(text_contents) => {
            assert_eq!(text_contents.uri, "mcp://e2e/file.txt");
            assert_eq!(text_contents.text, "Hello, Resource!");
        }
        _ => panic!("Expected TextResourceContents"),
    }
}

#[tokio::test]
async fn test_multiple_interactions_on_one_connection() {
    let server = Arc::new(
        Server::new("mcp-multi-test")
            .on_list_tools(mock_list_tools_handler)
            .on_call_tool(mock_call_tool_handler),
    );
    let (server_addr, _server_handle) = setup_test_server(server, 1).await;
    let client = Client::connect(&server_addr).await.unwrap();

    // First interaction: list_tools
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools[0].name, "e2e-test-tool");

    // Second interaction: call_tool
    let result = client
        .call_tool("e2e-test-tool".to_string(), json!({}))
        .await
        .unwrap();
    assert!(!result.is_error);

    // Third interaction: list_tools again
    let tools_again = client.list_tools().await.unwrap();
    assert_eq!(tools_again[0].name, "e2e-test-tool");
}

#[tokio::test]
async fn test_call_unregistered_tool_returns_error() {
    // This server only has a `list_tools` handler.
    let server = Arc::new(Server::new("mcp-error-test").on_list_tools(mock_list_tools_handler));
    let (server_addr, _server_handle) = setup_test_server(server, 1).await;
    let client = Client::connect(&server_addr).await.unwrap();

    // Call a tool for which no handler is registered on the server.
    let result = client
        .call_tool("this-tool-does-not-exist".to_string(), json!({}))
        .await;

    // We expect the operation to fail. The error message from the client
    // should contain the error sent by the server.
    assert!(result.is_err());
    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("tools/call handler not registered"));
}
