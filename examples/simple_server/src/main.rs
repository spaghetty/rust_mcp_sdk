//! An example MCP server that provides multiple tools and resource handling.

use anyhow::Result;
use mcp_sdk::{
    CallToolResult, ConnectionHandle, Content, ListToolsChangedParams, Notification,
    ReadResourceResult, Resource, ResourceContents, Server, TextContent, TextResourceContents,
    Tool,
};
use serde_json::Value;

// --- Tool Handler Implementations ---

// UPDATED: This handler now lists all available tools.
async fn list_tools_handler(_handle: ConnectionHandle) -> Result<Vec<Tool>> {
    println!("[Server] Handler invoked: list_tools_handler");
    Ok(vec![
        Tool {
            name: "fetch".to_string(),
            description: Some("Fetches a website and returns its content".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["url"],
                "properties": { "url": { "type": "string", "description": "URL to fetch" } },
            }),
            annotations: None,
        },
        Tool {
            name: "trigger_notification".to_string(),
            description: Some(
                "Asks the server to send a 'tools/listChanged' notification.".to_string(),
            ),
            input_schema: serde_json::json!({ "type": "object" }),
            annotations: None,
        },
    ])
}

// UPDATED: This single handler now dispatches based on the tool name.
async fn call_tool_handler(
    handle: ConnectionHandle,
    name: String,
    args: Value,
) -> Result<CallToolResult> {
    println!(
        "[Server] Handler invoked: call_tool_handler with name='{}'",
        name
    );

    match name.as_str() {
        "fetch" => {
            let url = args.get("url").and_then(Value::as_str).unwrap_or("Unknown");
            println!("[Server] Simulating fetch for URL: {}", url);
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Mock content of {}", url),
                })],
                is_error: false,
            })
        }
        "trigger_notification" => {
            println!("[Server] Sending 'tools/listChanged' notification...");
            handle
                .send_notification(Notification {
                    jsonrpc: "2.0".to_string(),
                    method: "notifications/tools/list_changed".to_string(),
                    params: ListToolsChangedParams {},
                })
                .await?;
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: "Notification sent!".to_string(),
                })],
                is_error: false,
            })
        }
        _ => Err(anyhow::anyhow!("Unknown tool called: {}", name)),
    }
}

// --- Resource Handler Implementations (Unchanged) ---

async fn list_resources_handler(_handle: ConnectionHandle) -> Result<Vec<Resource>> {
    Ok(vec![Resource {
        uri: "mcp://example/hello.txt".to_string(),
        name: "hello.txt".to_string(),
        description: Some("An example resource file.".to_string()),
        mime_type: Some("text/plain".to_string()),
    }])
}

async fn read_resource_handler(
    _handle: ConnectionHandle,
    uri: String,
) -> Result<ReadResourceResult> {
    if uri != "mcp://example/hello.txt" {
        return Err(anyhow::anyhow!("Unknown resource URI: {}", uri));
    }
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::Text(TextResourceContents {
            uri,
            mime_type: Some("text/plain".to_string()),
            text: "Hello from a resource!".to_string(),
        })],
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:8080";

    // UPDATED: Register the new combined handlers.
    let server = Server::new("mcp-multi-tool-server")
        .on_list_tools(list_tools_handler)
        .on_call_tool(call_tool_handler)
        .on_list_resources(list_resources_handler)
        .on_read_resource(read_resource_handler);

    println!("[Server] All handlers (tools and resources) are enabled.");
    println!("[Server] Starting on {}...", addr);

    server.listen(addr).await?;

    Ok(())
}
