//! An example demonstrating the use of `ClientSessionGroup` to manage
//! connections to multiple servers and aggregate their features.

use anyhow::Result;
use mcp_sdk::{
    client::ClientSessionGroup,
    server::{ConnectionHandle, Server},
    types::Tool,
};
use serde_json::json;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Test helper to create and run a simple mock server in the background.
async fn setup_mock_server(tool_name: &'static str) -> Result<(String, JoinHandle<()>)> {
    let server = Server::new(Box::leak(tool_name.to_string().into_boxed_str())).on_list_tools(
        move |_handle: ConnectionHandle| {
            let tool = Tool {
                name: tool_name.to_string(),
                description: Some("A mock tool".to_string()),
                input_schema: json!({}),
                annotations: None,
            };
            async { Ok(vec![tool]) }
        },
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = listener.local_addr()?.to_string();
    drop(listener);

    let addr_clone = server_addr.clone();
    let handle = tokio::spawn(async move {
        server.listen(&addr_clone).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    Ok((server_addr, handle))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("[GroupClient] Setting up multiple servers...");

    // 1. Setup two mock servers in the background.
    let (server1_addr, _server1_handle) = setup_mock_server("tool-from-server-1").await?;
    let (server2_addr, _server2_handle) = setup_mock_server("tool-from-server-2").await?;
    println!("[GroupClient] Server 1 listening on: {}", server1_addr);
    println!("[GroupClient] Server 2 listening on: {}", server2_addr);

    // 2. Create a new ClientSessionGroup.
    let group = ClientSessionGroup::new();
    println!("\n[GroupClient] Connecting to servers and adding to group...");

    // 3. Add clients for both servers to the group.
    group.add_client(&server1_addr).await?;
    group.add_client(&server2_addr).await?;
    println!("[GroupClient] Successfully connected to 2 servers.");

    // 4. Use the group to list and aggregate tools from all connected servers.
    println!("\n[GroupClient] Calling list_tools_all() to aggregate tools...");
    let all_tools = group.list_tools_all().await?;

    println!("\n✅ Success! Aggregated tools from all servers:");
    for tool in &all_tools {
        println!("  - {}", tool.name);
    }

    assert_eq!(all_tools.len(), 2);
    assert!(all_tools.iter().any(|t| t.name == "tool-from-server-1"));
    assert!(all_tools.iter().any(|t| t.name == "tool-from-server-2"));

    Ok(())
}
