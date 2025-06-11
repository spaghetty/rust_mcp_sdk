//! An example demonstrating the use of `ClientSessionGroup` to connect to
//! multiple, externally-run MCP servers.
use mcp_sdk::{client::ClientSessionGroup, error::Result};

#[tokio::main]
async fn main() -> Result<()> {
    // These addresses correspond to the servers the user is instructed
    // to run in the examples/README.md file.
    let server1_addr = "127.0.0.1:8081";
    let server2_addr = "127.0.0.1:8082";

    println!("[GroupClient] This example requires you to run two instances of the simple-server-example in separate terminals:");
    println!("  - Terminal 1: cargo run -p simple-server-example -- --port 8081 --suffix _1");
    println!("  - Terminal 2: cargo run -p simple-server-example -- --port 8082 --suffix _2");

    // 1. Create a new ClientSessionGroup.
    let group = ClientSessionGroup::new();
    println!("\n[GroupClient] Connecting to servers and adding to group...");

    // 2. Add clients for both external servers to the group.
    group.add_client(server1_addr).await?;
    println!("[GroupClient] Connected to {}", server1_addr);

    group.add_client(server2_addr).await?;
    println!("[GroupClient] Connected to {}", server2_addr);

    // 3. Use the group to list and aggregate tools from all connected servers.
    println!("\n[GroupClient] Calling list_tools_all() to aggregate tools...");
    let all_tools = group.list_tools_all().await?;

    println!("\n✅ Success! Aggregated tools from all servers:");
    for tool in &all_tools {
        println!(
            "  - {} (from {})",
            tool.name,
            tool.description.as_deref().unwrap_or("unknown")
        );
    }

    // 4. Verify the results.
    assert_eq!(all_tools.len(), 4);
    assert!(all_tools.iter().any(|t| t.name == "fetch_1"));
    assert!(all_tools.iter().any(|t| t.name == "fetch_2"));
    assert_eq!(
        all_tools
            .iter()
            .filter(|t| t.name == "trigger_notification")
            .count(),
        2
    );
    println!("\n[GroupClient] Example finished successfully.");
    Ok(())
}
