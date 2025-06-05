use std::collections::HashMap;

use url::Url;

use mcp::server::Server;
use mcp::types::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create server
    let mut server = Server::new();

    // Add resource listing handler
    server.list_resources(|_| {
        println!("[SERVER] Received resources/list request");
        let resources = vec![
            Resource {
                uri: Url::parse("file:///greeting.txt").unwrap(),
                name: "greeting".to_string(),
                description: Some("A sample text resource".to_string()),
                mime_type: Some("text/plain".to_string()),
                extra: HashMap::new(),
            },
            Resource {
                uri: Url::parse("file:///help.txt").unwrap(),
                name: "help".to_string(),
                description: Some("Server help information".to_string()),
                mime_type: Some("text/plain".to_string()),
                extra: HashMap::new(),
            },
        ];
        println!("[SERVER] Returning resources: {:?}", resources);
        resources
    });

    println!("[SERVER] Starting server on 127.0.0.1:8000");
    server.run("127.0.0.1:8000").await?;
    Ok(())
}
