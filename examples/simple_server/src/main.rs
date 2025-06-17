//! An example MCP server that can be configured with command-line flags.
//! This allows running multiple distinct instances for testing the ClientSessionGroup.
//!
//! Run instances like:
//! `cargo run -p simple-server-example -- --port 8081 --suffix _1`
//! `cargo run -p simple-server-example -- --port 8082 --suffix _2`

use clap::Parser;
use mcp_sdk::{
    error::{Error, Result},
    network_adapter::NdjsonAdapter,
    CallToolResult, ConnectionHandle, Content, GetPromptResult, ListPromptsResult,
    ListToolsChangedParams, Notification, Prompt, ReadResourceResult, Resource, ResourceContents,
    Server, TextResourceContents, Tool,
};
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

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

async fn list_resources_handler(
    suffix: String,
    _handle: ConnectionHandle,
) -> Result<Vec<Resource>> {
    println!("[Server{}] Handler invoked: list_resources_handler", suffix);
    Ok(vec![Resource {
        uri: format!("mcp://example/hello{}.txt", suffix),
        name: format!("hello{}.txt", suffix),
        description: Some("An example resource file.".to_string()),
        mime_type: Some("text/plain".to_string()),
    }])
}

async fn read_resource_handler(
    suffix: String,
    _handle: ConnectionHandle,
    uri: String,
) -> Result<ReadResourceResult> {
    println!(
        "[Server{}] Handler invoked: read_resource_handler for uri: '{}'",
        suffix, uri
    );
    let expected_uri = format!("mcp://example/hello{}.txt", suffix);
    if uri != expected_uri {
        return Err(Error::Other(format!("Unknown resource URI: {}", uri)));
    }
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::Text(TextResourceContents {
            uri,
            mime_type: Some("text/plain".to_string()),
            text: format!("Hello from resource {}!", suffix),
        })],
    })
}

async fn list_prompts_handler(
    suffix: String,
    _handle: ConnectionHandle,
) -> Result<ListPromptsResult> {
    println!("[Server{}] Handler invoked: list_prompts_handler", suffix);
    Ok(ListPromptsResult {
        prompts: vec![Prompt {
            name: format!("example-prompt{}", suffix),
            description: Some("An example prompt.".to_string()),
            arguments: None,
        }],
    })
}

async fn get_prompt_handler(
    suffix: String,
    _handle: ConnectionHandle,
    name: String,
    _args: Option<Value>,
) -> Result<GetPromptResult> {
    println!(
        "[Server{}] Handler invoked: get_prompt_handler with name='{}'",
        suffix, name
    );
    Ok(GetPromptResult {
        description: Some(format!("This is the example prompt{}.", suffix)).into(),
        messages: vec![],
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arc::new(Args::parse());
    let addr = format!("127.0.0.1:{}", args.port);

    let server = Server::new("mcp-configurable-server")
        // Register the first tool and its handler logic directly.
        .register_tool(
            Tool {
                name: format!("fetch{}", args.suffix),
                description: Some("Fetches a website and returns its content".to_string()),
                input_schema: json!({ "type": "object", "properties": { "url": { "type": "string" } } }),
                annotations: None,
            },
            {
                // Capture the suffix for use in the handler.
                let suffix = args.suffix.clone();
                move |_handle: ConnectionHandle, args: Value| {
                    let sfx_value = suffix.clone();
                    async move {
                        let url = args.get("url").and_then(Value::as_str).unwrap_or("Unknown");
                        info!("[Server{}] Simulating fetch for URL: {}", sfx_value, url);
                        Ok(CallToolResult {
                            content: vec![Content::Text {
                                text: format!("Mock content of {}", url),
                            }],
                            is_error: false,
                        })
                    }
                }
            },
        )
        .register_tool(
            Tool {
                name: "trigger_notification".to_string(),
                description: Some("Asks the server to send a 'tools/listChanged' notification.".to_string()),
                input_schema: json!({ "type": "object" }),
                annotations: None,
            },
            |handle: ConnectionHandle, _args: Value| async move {
                info!("Sending 'tools/listChanged' notification...");
                handle
                    .send_notification(Notification {
                        jsonrpc: "2.0".to_string(),
                        method: "notifications/tools/list_changed".to_string(),
                        params: Some(ListToolsChangedParams {}),
                    })
                    .await?;
                Ok(CallToolResult {
                    content: vec![Content::Text {
                        text: "Notification sent!".to_string(),
                    }],
                    is_error: false,
                })
            },
        )
        .on_list_resources({
            let args = Arc::clone(&args);
            move |handle| list_resources_handler(args.suffix.clone(), handle)
        })
        .on_read_resource({
            let args = Arc::clone(&args);
            move |handle, uri| read_resource_handler(args.suffix.clone(), handle, uri)
        })
        .on_list_prompts({
            let args = Arc::clone(&args);
            move |handle| list_prompts_handler(args.suffix.clone(), handle)
        })
        .on_get_prompt({
            let args = Arc::clone(&args);
            move |handle, name, args_val| {
                get_prompt_handler(args.suffix.clone(), handle, name, args_val)
            }
        });

    println!("[Server{}] Starting on {}...", args.suffix, addr);

    server.tcp_listen::<NdjsonAdapter>(&addr).await?;

    Ok(())
}
