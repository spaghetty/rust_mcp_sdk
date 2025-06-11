//! An example MCP server that can be configured with command-line flags.
//! This allows running multiple distinct instances for testing the ClientSessionGroup.
//!
//! Run instances like:
//! `cargo run -p simple-server-example -- --port 8081 --suffix _1`
//! `cargo run -p simple-server-example -- --port 8082 --suffix _2`

use anyhow::Result;
use clap::Parser;
use mcp_sdk::{
    CallToolResult, ConnectionHandle, Content, ListToolsChangedParams, Notification, Server, Tool,
};
use serde_json::Value;
use std::sync::Arc;

// --- Command-Line Argument Parsing ---
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// The port number to listen on.
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// A suffix to append to resource names to make them unique.
    #[arg(long, default_value = "")]
    suffix: String,
}

// --- Handler Implementations ---

async fn list_tools_handler(suffix: String, _handle: ConnectionHandle) -> Result<Vec<Tool>> {
    let tool_name = format!("fetch{}", suffix);
    println!("[Server{}] Handler invoked: list_tools_handler", suffix);
    Ok(vec![Tool {
        name: tool_name,
        description: Some("Fetches a website and returns its content".to_string()),
        input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        annotations: None,
    }])
}

async fn call_tool_handler(
    suffix: String,
    handle: ConnectionHandle,
    name: String,
    _args: Value,
) -> Result<CallToolResult> {
    let expected_tool_name = format!("fetch{}", suffix);
    if name != expected_tool_name {
        return Err(anyhow::anyhow!("Unknown tool called: {}", name));
    }

    println!(
        "[Server{}] Handler invoked: call_tool with name='{}'",
        suffix, name
    );

    // Send a notification to demonstrate that the handle works.
    handle
        .send_notification(Notification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/tools/list_changed".to_string(),
            params: ListToolsChangedParams {},
        })
        .await?;

    Ok(CallToolResult {
        content: vec![Content::Text {
            text: format!("Response from fetch{}", suffix),
        }],
        is_error: false,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arc::new(Args::parse());
    let addr = format!("127.0.0.1:{}", args.port);

    let server = Server::new("mcp-configurable-server")
        .on_list_tools({
            let args = Arc::clone(&args);
            move |handle| list_tools_handler(args.suffix.clone(), handle)
        })
        .on_call_tool({
            let args = Arc::clone(&args);
            move |handle, name, value| call_tool_handler(args.suffix.clone(), handle, name, value)
        });

    println!("[Server{}] Starting on {}...", args.suffix, addr);

    server.listen(&addr).await?;

    Ok(())
}
