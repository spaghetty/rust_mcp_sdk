//! An example of a simple MCP tool server built using the mcp-sdk.
//! This server provides a single tool called `fetch`.

use anyhow::Result;
// FIX: Import types from the crate root, not the private `types` module.
use mcp_sdk::{CallToolResult, Content, Server, TextContent, Tool};
use serde_json::{json, Value};

/// A mock handler for the `tools/list` request.
/// It returns a static list containing our "fetch" tool.
async fn list_fetch_tool() -> Result<Vec<Tool>> {
    println!("[Server] Handler invoked: list_fetch_tool");
    Ok(vec![Tool {
        name: "fetch".to_string(),
        description: Some("Fetches a website and returns its content".to_string()),
        input_schema: json!({
            "type": "object",
            "required": ["url"],
            "properties": { "url": { "type": "string", "description": "URL to fetch" } },
        }),
        annotations: None,
    }])
}

/// A mock handler for the `tools/call` request.
/// It checks if the tool name is "fetch" and returns a mock result.
async fn call_fetch_tool(name: String, args: Value) -> Result<CallToolResult> {
    println!(
        "[Server] Handler invoked: call_fetch_tool with name='{}'",
        name
    );
    if name != "fetch" {
        // In a real implementation, we would return a proper JSON-RPC error.
        return Err(anyhow::anyhow!("Unknown tool called: {}", name));
    }

    let url = args
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required 'url' argument"))?;

    println!("[Server] Simulating fetch for URL: {}", url);

    Ok(CallToolResult {
        // FIX: Now that `Content` is in scope, we can use it directly.
        content: vec![Content::Text(TextContent {
            r#type: "text".to_string(),
            text: format!("Mock content of {}", url),
        })],
        is_error: false,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:8080";

    // 1. Create a new server instance using the builder pattern.
    let server = Server::new("mcp-fetch-example-server")
        .on_list_tools(list_fetch_tool)
        .on_call_tool(call_fetch_tool);

    println!("[Server] Starting on {}...", addr);

    // 2. Start the server's main listen loop.
    // This will run forever, accepting and handling client connections.
    server.listen(addr).await?;

    Ok(())
}
