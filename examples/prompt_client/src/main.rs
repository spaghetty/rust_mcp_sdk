//! An example MCP client that demonstrates prompt handling.

use mcp_sdk::{Client, Content, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:8080";

    println!("[Client] Connecting to server at {}...", addr);
    let client = Client::connect(addr).await?;
    println!("[Client] Successfully connected.");

    // 1. List the prompts available on the server.
    println!("\n[Client] Calling 'prompts/list'...");
    let list_result = client.list_prompts().await?;
    let prompts = list_result.prompts;
    println!("[Client] Received prompts: {:#?}", prompts);

    if prompts.is_empty() {
        println!("\n[Client] No prompts available on the server.");
        return Ok(());
    }

    // 2. Get the first prompt from the list.
    let prompt_to_get = &prompts[0].name;
    println!(
        "\n[Client] Calling 'prompts/get' for '{}'...",
        prompt_to_get
    );
    let get_result = client.get_prompt(prompt_to_get.clone(), None).await?;

    println!("[Client] Received prompt result: {:#?}", get_result);

    if let Some(first_message) = get_result.messages.first() {
        if let Content::Text { text } = &first_message.content {
            println!("\n✅ Success! Got prompt content: '{}'", text);
        }
    } else {
        println!("\n✅ Success! Got prompt with no messages.");
    }

    Ok(())
}
