//! An example of a simple MCP client that connects to a server and uses its tools.

use anyhow::Result;
use mcp_sdk::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    let server_addr = "127.0.0.1:8080";
    println!(
        "[Client] Attempting to connect to server at {}...",
        server_addr
    );

    // 1. Connect to the server. The `connect` function handles the initialize handshake.
    let client = Client::connect(server_addr).await?;
    println!("[Client] Successfully connected and initialized.");

    // 2. List the tools available on the server.
    println!("\n[Client] Requesting list of available tools...");
    let tools = client.list_tools().await?;
    println!("[Client] Received tools: {:#?}", tools);

    // 3. Call the "fetch" tool.
    let tool_name = "fetch";
    let tool_args = json!({ "url": "https://modelcontextprotocol.io" });
    println!(
        "\n[Client] Calling tool '{}' with arguments: {}",
        tool_name, tool_args
    );
    let result = client.call_tool(tool_name.to_string(), tool_args).await?;
    println!("[Client] Received tool result: {:#?}", result);

    Ok(())
}
