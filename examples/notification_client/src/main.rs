//! An example MCP client that demonstrates receiving a notification from the server.

// CORRECTED: Import ListToolsChangedParams directly from the crate root.
use mcp_sdk::{Client, ListToolsChangedParams, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:8080";

    println!("[Client] Connecting to server at {}...", addr);
    let client = Client::connect(addr).await?;
    println!("[Client] Successfully connected.");

    // 1. Set up a flag and register a handler for the 'tools/listChanged' notification.
    let notification_received = Arc::new(AtomicBool::new(false));
    let notification_received_clone = Arc::clone(&notification_received);

    client.on_tools_list_changed(move |_params: ListToolsChangedParams| {
        println!("\n[Client] <<< Received 'tools/listChanged' notification! >>>\n");
        notification_received_clone.store(true, Ordering::SeqCst);
    });
    println!("[Client] Notification handler registered.");

    // 2. List the tools to show that 'trigger_notification' is available.
    let tools = client.list_tools().await?;
    println!(
        "\n[Client] Server has the following tools: {:#?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // 3. Call the 'trigger_notification' tool.
    println!("\n[Client] Calling 'trigger_notification' tool...");
    let result = client
        .call_tool("trigger_notification".to_string(), serde_json::Value::Null)
        .await?;
    println!(
        "[Client] Received result from 'trigger_notification': {:?}",
        result.content
    );

    // 4. Wait for up to 2 seconds for the notification to arrive and be processed.
    let wait_for_notif = async {
        while !notification_received.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };

    if timeout(Duration::from_secs(2), wait_for_notif)
        .await
        .is_ok()
    {
        println!("\n✅ Success! Notification handler was executed.");
    } else {
        println!("\n❌ Failure! Timed out waiting for notification.");
    }

    Ok(())
}
