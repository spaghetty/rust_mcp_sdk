use mcp::client::ClientSessionGroup;

use std::error::Error;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create client session group
    let mut group = ClientSessionGroup::new();
    let server_url = Url::parse("tcp://127.0.0.1:8000")?;

    println!("[CLIENT] Connecting to server at {}...", server_url);
    // Connect to server
    if let Err(e) = group.connect_to_server(server_url.clone()).await {
        eprintln!(
            "[CLIENT] Failed to connect to MCP server at {}: {}",
            server_url, e
        );
        return Ok(());
    }
    println!("[CLIENT] Connected to server at {}", server_url);

    // List resources
    println!("[CLIENT DEBUG] About to call list_resources");
    let resources_result = group.list_resources(&server_url).await;
    println!(
        "[CLIENT DEBUG] list_resources returned: {:?}",
        resources_result
    );
    match resources_result {
        Ok(resources) => {
            println!("[CLIENT] Received resources response");
            for resource in resources.resources {
                println!("Found resource: {}", resource.name);
                if let Some(description) = resource.description {
                    println!("Description: {}", description);
                }
            }
        }
        Err(e) => {
            eprintln!(
                "[CLIENT] Failed to list resources from server at {}: {}",
                server_url, e
            );
            return Ok(());
        }
    }

    // Try to list tools
    println!("[CLIENT DEBUG] About to call list_tools");
    let tools_result = group.list_tools(&server_url, Default::default()).await;
    println!("[CLIENT DEBUG] list_tools returned: {:?}", tools_result);
    match tools_result {
        Ok(tools_result) => {
            if tools_result.tools.is_empty() {
                println!("[CLIENT] No tools returned by server, exiting.");
                return Ok(());
            }
            println!("[CLIENT] Received tools response");
            for tool in &tools_result.tools {
                println!("Tool: {}", tool.name);
                if let Some(description) = &tool.description {
                    println!("Description: {}", description);
                }
            }
            // Try to call the echo tool if present
            if let Some(_echo_tool) = tools_result.tools.iter().find(|t| t.name == "echo") {
                println!("[CLIENT] Calling 'echo' tool...");
                let mut args = std::collections::HashMap::new();
                args.insert("message".to_string(), "Hello from client!".to_string());
                match group.call_tool(&server_url, "echo".to_string(), args).await {
                    Ok(tool_result) => {
                        println!("[CLIENT] Tool call result: {:?}", tool_result);
                    }
                    Err(e) => {
                        eprintln!("[CLIENT] Tool call failed: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("[CLIENT] tools/list not supported or failed: {}", e);
            println!("[CLIENT] Exiting after listing resources.");
        }
    }
    Ok(())
}
