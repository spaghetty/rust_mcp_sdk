use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use url::Url;

use mcp::types::*;
use mcp::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create server
    let server = Server::new();

    // Add tool handler
    let mut server = server; // Make server mutable
    server.add_tool_handler(|name, arguments| {
        match name.as_str() {
            "echo" => {
                let message = arguments.get("message").ok_or_else(|| 
                    anyhow::anyhow!("Missing message parameter")
                )?;
                Ok(ToolResult {
                    result: Some(ToolResultData::TextContent(
                        TextContent {
                            text: format!("Echo: {}", message),
                        }
                    )),
                    error: None,
                })
            }
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    });

    // List tools
    server.list_tools(|_| async {
        Ok(ListToolsResult {
            tools: vec![Tool {
                name: "echo".to_string(),
                description: Some("Echoes back your message".to_string()),
                extra: HashMap::new(),
            }],
        })
    });

    // Register a minimal list_resources handler
    server.list_resources(|_params| {
        vec![]
    });

    // Start the server
    server.run("127.0.0.1:8000").await?;
    Ok(())
}
