//! An example MCP server that provides a `fetch` tool and resource handling.

use anyhow::Result;
use mcp_sdk::{
    CallToolResult, Content, ReadResourceResult, Resource, ResourceContents, Server, TextContent,
    TextResourceContents, Tool,
};
use serde_json::{json, Value};

// --- Tool Handler Implementations ---

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

async fn call_fetch_tool(name: String, args: Value) -> Result<CallToolResult> {
    println!(
        "[Server] Handler invoked: call_fetch_tool with name='{}'",
        name
    );
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

// --- Resource Handler Implementations ---

async fn list_resources_handler() -> Result<Vec<Resource>> {
    println!("[Server] Handler invoked: list_resources_handler");
    Ok(vec![Resource {
        uri: "mcp://example/hello.txt".to_string(),
        name: "hello.txt".to_string(),
        description: Some("An example resource file.".to_string()),
        mime_type: Some("text/plain".to_string()),
    }])
}

async fn read_resource_handler(uri: String) -> Result<ReadResourceResult> {
    println!(
        "[Server] Handler invoked: read_resource_handler for uri: '{}'",
        uri
    );
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

    // Create a server and register all handlers unconditionally.
    let server = Server::new("mcp-unified-example-server")
        .on_list_tools(list_fetch_tool)
        .on_call_tool(call_fetch_tool)
        .on_list_resources(list_resources_handler)
        .on_read_resource(read_resource_handler);

    println!("[Server] All handlers (tools and resources) are enabled.");
    println!("[Server] Starting on {}...", addr);

    server.listen(addr).await?;

    Ok(())
}
