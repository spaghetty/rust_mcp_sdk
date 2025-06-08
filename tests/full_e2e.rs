//! Full end-to-end integration tests for the MCP SDK.
//!
//! This test compiles the entire `mcp-sdk` crate as a library and then uses its
//! public API to run a client and server to ensure they can communicate correctly.

use anyhow::Result;
use mcp_sdk::{
    CallToolResult, Client, ConnectionHandle, Content, ReadResourceResult, Resource,
    ResourceContents, Server, TextContent, TextResourceContents, Tool,
};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::task::JoinHandle;

// --- Mock Handlers ---

async fn mock_list_tools_handler(_handle: ConnectionHandle) -> Result<Vec<Tool>> {
    Ok(vec![Tool {
        name: "e2e-test-tool".to_string(),
        description: Some("An end-to-end test tool".to_string()),
        input_schema: json!({ "type": "object" }),
        annotations: None,
    }])
}

async fn mock_call_tool_handler(
    _handle: ConnectionHandle,
    name: String,
    _args: Value,
) -> Result<CallToolResult> {
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

async fn mock_list_resources_handler(_handle: ConnectionHandle) -> Result<Vec<Resource>> {
    Ok(vec![Resource {
        uri: "mcp://e2e/file.txt".to_string(),
        name: "file.txt".to_string(),
        description: Some("An end-to-end test resource".to_string()),
        mime_type: Some("text/plain".to_string()),
    }])
}

async fn mock_read_resource_handler(
    _handle: ConnectionHandle,
    uri: String,
) -> Result<ReadResourceResult> {
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

// This test harness now starts the server using its public `listen`
// API, just like a real application would.
async fn setup_test_server(server: Server) -> (String, JoinHandle<()>) {
    // Bind to port 0 to let the OS choose a free port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap().to_string();

    // We drop the listener immediately, which frees up the port.
    // We then pass the address to the real server's listen method.
    // This has a small race condition (another app could grab the port),
    // but it is acceptable and standard for testing.
    drop(listener);

    let addr_clone = server_addr.clone();
    let server_handle = tokio::spawn(async move {
        // Run the actual server listen loop.
        if let Err(e) = server.listen(&addr_clone).await {
            // It's normal for the listen loop to error out when the test ends
            // and all connections are dropped. We only panic on unexpected errors.
            let error_str = e.to_string();
            if !error_str.contains("os error 10054") && // Windows "connection reset"
               !error_str.contains("Connection reset by peer") && // Unix "connection reset"
               !error_str.contains("An existing connection was forcibly closed")
            {
                panic!("Server failed to listen: {}", e);
            }
        }
    });

    // Give the server a moment to start its listener.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // CORRECTED: Return the original `server_addr`, not the one moved into the task.
    (server_addr, server_handle)
}

// --- The Tests ---

#[tokio::test]
async fn test_full_client_server_interaction() {
    let test_body = async {
        let server = Server::new("mcp-e2e-test-server")
            .on_list_tools(mock_list_tools_handler)
            .on_call_tool(mock_call_tool_handler);

        let (server_addr, _server_handle) = setup_test_server(server).await;
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
        let server = Server::new("mcp-resource-test")
            .on_list_resources(mock_list_resources_handler)
            .on_read_resource(mock_read_resource_handler);

        let (server_addr, _server_handle) = setup_test_server(server).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let resources = client.list_resources().await.unwrap();
        assert_eq!(resources.len(), 1);
        let resource_result = client
            .read_resource("mcp://e2e/file.txt".to_string())
            .await
            .unwrap();
        assert_eq!(resource_result.contents.len(), 1);
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}

#[tokio::test]
async fn test_multiple_interactions_on_one_connection() {
    let test_body = async {
        let server = Server::new("mcp-multi-test")
            .on_list_tools(mock_list_tools_handler)
            .on_call_tool(mock_call_tool_handler);

        let (server_addr, _server_handle) = setup_test_server(server).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools[0].name, "e2e-test-tool");
        let result = client
            .call_tool("e2e-test-tool".to_string(), json!({}))
            .await
            .unwrap();
        assert!(!result.is_error);
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}

#[tokio::test]
async fn test_call_unregistered_tool_returns_error() {
    let test_body = async {
        let server = Server::new("mcp-error-test").on_list_tools(mock_list_tools_handler);

        let (server_addr, _server_handle) = setup_test_server(server).await;
        let client = Client::connect(&server_addr).await.unwrap();
        let result = client
            .call_tool("this-tool-does-not-exist".to_string(), json!({}))
            .await;
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("has no registered handler"));
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}
