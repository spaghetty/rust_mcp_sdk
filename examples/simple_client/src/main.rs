//! An example MCP client that interacts with the fetch tool server and optional resources.

use anyhow::Result;
use clap::Parser;
use mcp_sdk::{Client, Content, ResourceContents};
use serde_json::json;

// --- Command-Line Argument Parsing ---
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Attempt to use resource handling methods
    #[arg(long)]
    with_resources: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr = "127.0.0.1:8080";

    println!("[Client] Connecting to server at {}...", addr);
    let client = Client::connect(addr).await?;
    println!("[Client] Successfully connected.");

    // --- Always test tool methods ---
    println!("\n--- Phase 1: Testing Tool Methods ---");
    let tools = client.list_tools().await?;
    println!("[Client] Received tools: {:#?}", tools);

    let url_to_fetch = "https://example.com";
    println!("\n[Client] Calling 'fetch' with URL: {}", url_to_fetch);
    let result = client
        .call_tool("fetch".to_string(), json!({ "url": url_to_fetch }))
        .await?;
    println!("[Client] Received result from 'fetch': {:#?}", result);

    // --- Conditionally test resource methods ---
    if args.with_resources {
        println!("\n--- Phase 2: Testing Resource Methods ---");
        println!("[Client] Attempting to list resources...");
        match client.list_resources().await {
            Ok(resources) => {
                println!("[Client] Received resources: {:#?}", resources);
                if let Some(resource) = resources.first() {
                    println!("\n[Client] Attempting to read resource: {}", resource.uri);
                    match client.read_resource(resource.uri.clone()).await {
                        Ok(content) => {
                            println!("[Client] Read resource content: {:#?}", content);
                            if let Some(ResourceContents::Text(text)) = content.contents.first() {
                                println!("\n✅ Success! Resource content: '{}'", text.text);
                            }
                        }
                        Err(e) => eprintln!("\n❌ Error reading resource: {}", e),
                    }
                }
            }
            Err(e) => eprintln!(
                "\n❌ Error listing resources: {}. Is the server running with --with-resources?",
                e
            ),
        }
    } else {
        println!("\n--- Phase 2: Skipped ---");
        println!("(Run with --with-resources to test resource handling)");
    }

    Ok(())
}
