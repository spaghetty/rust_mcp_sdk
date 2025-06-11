//! Full end-to-end integration tests for the MCP SDK.
//!
//! This test compiles the entire `mcp-sdk` crate as a library and then uses its
//! public API to run a client and server to ensure they can communicate correctly.

// UPDATED: Use our custom Result type and Error enum.
use mcp_sdk::{
    error::Result, CallToolResult, Client, ConnectionHandle, Content, GetPromptResult,
    ListPromptsResult, Prompt, PromptMessage, ReadResourceResult, Resource, ResourceContents,
    Server, TextResourceContents, Tool,
};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::task::JoinHandle;

// --- Mock Handlers (with updated signatures) ---

// UPDATED: All mock handlers now return the SDK's custom Result type.
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
        // In a real application, you might use a more specific error variant.
        return Err(mcp_sdk::Error::Other(format!(
            "Unknown tool in e2e test: {}",
            name
        )));
    }
    Ok(CallToolResult {
        content: vec![Content::Text {
            text: "e2e test successful".to_string(),
        }],
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
        return Err(mcp_sdk::Error::Other(format!(
            "Unknown resource in e2e test: {}",
            uri
        )));
    }
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::Text(TextResourceContents {
            uri: uri.clone(),
            mime_type: Some("text/plain".to_string()),
            text: "Hello, Resource!".to_string(),
        })],
    })
}

async fn mock_list_prompts_handler(_handle: ConnectionHandle) -> Result<ListPromptsResult> {
    Ok(ListPromptsResult {
        prompts: vec![Prompt {
            name: "e2e-prompt".to_string(),
            description: Some("An end-to-end test prompt.".to_string()),
            arguments: None,
        }],
    })
}

async fn mock_get_prompt_handler(
    _handle: ConnectionHandle,
    name: String,
    _args: Option<Value>,
) -> Result<GetPromptResult> {
    if name != "e2e-prompt" {
        return Err(mcp_sdk::Error::Other(format!(
            "Unknown prompt in e2e test: {}",
            name
        )));
    }
    Ok(GetPromptResult {
        description: Some("A test prompt result.".to_string()),
        messages: vec![PromptMessage {
            role: "user".to_string(),
            content: Content::Text {
                text: "This is the prompt content.".to_string(),
            },
        }],
    })
}

// --- Test Setup ---

async fn setup_test_server(server: Server) -> (String, JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = listener.local_addr().unwrap().to_string();

    drop(listener);

    let addr_clone = server_addr.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.listen(&addr_clone).await {
            let error_str = e.to_string();
            if !error_str.contains("os error 10054")
                && !error_str.contains("Connection reset by peer")
                && !error_str.contains("An existing connection was forcibly closed")
            {
                panic!("Server failed to listen: {}", e);
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

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
async fn test_full_prompt_interaction() {
    let test_body = async {
        let server = Server::new("mcp-prompt-test")
            .on_list_prompts(mock_list_prompts_handler)
            .on_get_prompt(mock_get_prompt_handler);

        let (server_addr, _server_handle) = setup_test_server(server).await;
        let client = Client::connect(&server_addr).await.unwrap();

        let list_result = client.list_prompts().await.unwrap();
        assert_eq!(list_result.prompts.len(), 1);
        assert_eq!(list_result.prompts[0].name, "e2e-prompt");

        let get_result = client
            .get_prompt("e2e-prompt".to_string(), None)
            .await
            .unwrap();
        assert_eq!(get_result.messages.len(), 1);
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
        // UPDATED: Check for the new error message format.
        assert!(error_message.contains("JSON-RPC error"));
        assert!(error_message.contains("has no registered handler"));
    };

    tokio::time::timeout(Duration::from_secs(6), test_body)
        .await
        .expect("Test timed out after 6 seconds");
}
