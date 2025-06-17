//! MCP Client Connection Debugging Tool
//!
//! This example demonstrates how to debug various client connection scenarios
//! and provides comprehensive testing for different network adapters and failure modes.

use mcp_sdk::{
    client::ClientSessionGroup, CallToolResult, Client, Content, LspAdapter, NdjsonAdapter, Result,
    Server, Tool,
};

use clap::{Parser, Subcommand};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "debug-client")]
#[command(about = "A tool for debugging MCP client connections")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all debugging scenarios
    All,
    /// Test TCP connection with NDJSON adapter
    TcpNdjson {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        address: String,
    },
    /// Test TCP connection with LSP adapter
    TcpLsp {
        #[arg(short, long, default_value = "127.0.0.1:8081")]
        address: String,
    },
    /// Test connection timeout scenarios
    Timeout,
    /// Test multiple simultaneous connections
    Multiple,
    /// Analyze common error patterns from logs
    AnalyzeLogs {
        #[arg(short, long)]
        log_file: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::All => {
            info!("🔍 Running comprehensive client connection debugging...");
            run_all_debug_scenarios().await?
        }
        Commands::TcpNdjson { address } => {
            info!("🔌 Testing TCP NDJSON connection to {}", address);
            test_tcp_ndjson_connection(&address).await?
        }
        Commands::TcpLsp { address } => {
            info!("🔌 Testing TCP LSP connection to {}", address);
            test_tcp_lsp_connection(&address).await?
        }
        Commands::Timeout => {
            info!("⏱️ Testing connection timeout scenarios...");
            test_timeout_scenarios().await?
        }
        Commands::Multiple => {
            info!("🔗 Testing multiple simultaneous connections...");
            test_multiple_connections().await?
        }
        Commands::AnalyzeLogs { log_file } => {
            info!("📊 Analyzing error patterns from logs...");
            analyze_error_logs(log_file).await?
        }
    }

    Ok(())
}

/// Run all debugging scenarios
async fn run_all_debug_scenarios() -> Result<()> {
    let mut results = Vec::new();

    // Test 1: TCP NDJSON Connection
    info!("\n📋 Test 1: TCP NDJSON Connection");
    match test_tcp_ndjson_with_mock_server().await {
        Ok(_) => {
            info!("✅ TCP NDJSON test passed");
            results.push(("TCP NDJSON", true));
        }
        Err(e) => {
            error!("❌ TCP NDJSON test failed: {}", e);
            results.push(("TCP NDJSON", false));
        }
    }

    // Test 2: Connection Timeout
    info!("\n📋 Test 2: Connection Timeout");
    match test_connection_timeout().await {
        Ok(_) => {
            info!("✅ Timeout test passed");
            results.push(("Timeout", true));
        }
        Err(e) => {
            error!("❌ Timeout test failed: {}", e);
            results.push(("Timeout", false));
        }
    }

    // Test 3: Multiple Connections
    info!("\n📋 Test 3: Multiple Connections");
    match test_multiple_connections().await {
        Ok(_) => {
            info!("✅ Multiple connections test passed");
            results.push(("Multiple Connections", true));
        }
        Err(e) => {
            error!("❌ Multiple connections test failed: {}", e);
            results.push(("Multiple Connections", false));
        }
    }

    // Test 4: Handshake Validation
    info!("\n📋 Test 4: Handshake Validation");
    match test_handshake_validation().await {
        Ok(_) => {
            info!("✅ Handshake validation test passed");
            results.push(("Handshake", true));
        }
        Err(e) => {
            error!("❌ Handshake validation test failed: {}", e);
            results.push(("Handshake", false));
        }
    }

    // Print summary
    print_test_summary(&results);

    Ok(())
}

/// Test TCP NDJSON connection to a specific address
async fn test_tcp_ndjson_connection(address: &str) -> Result<()> {
    info!(
        "Attempting to connect to {} with NDJSON adapter...",
        address
    );

    let adapter = match NdjsonAdapter::connect(address).await {
        Ok(adapter) => {
            info!("✅ Successfully connected to {}", address);
            adapter
        }
        Err(e) => {
            error!("❌ Failed to connect to {}: {}", address, e);
            return Err(e);
        }
    };

    let client = match Client::new(adapter).await {
        Ok(client) => {
            info!("✅ Client handshake successful");
            client
        }
        Err(e) => {
            error!("❌ Client handshake failed: {}", e);
            return Err(e);
        }
    };

    // Test basic operations
    match client.list_tools().await {
        Ok(tools) => {
            info!("✅ Listed {} tools", tools.len());
            for tool in &tools {
                info!("   🔧 Tool: {}", tool.name);
            }
        }
        Err(e) => {
            warn!("⚠️ Failed to list tools: {}", e);
        }
    }

    Ok(())
}

/// Test TCP NDJSON connection with a mock server
async fn test_tcp_ndjson_with_mock_server() -> Result<()> {
    let (server_addr, _server_handle) = start_test_server().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    test_tcp_ndjson_connection(&server_addr).await
}

/// Test TCP LSP connection to a specific address
async fn test_tcp_lsp_connection(address: &str) -> Result<()> {
    info!("Attempting to connect to {} with LSP adapter...", address);

    let adapter = match LspAdapter::connect(address).await {
        Ok(adapter) => {
            info!("✅ Successfully connected to {}", address);
            adapter
        }
        Err(e) => {
            error!("❌ Failed to connect to {}: {}", address, e);
            return Err(e);
        }
    };

    let _client = match Client::new(adapter).await {
        Ok(client) => {
            info!("✅ LSP client handshake successful");
            client
        }
        Err(e) => {
            error!("❌ LSP client handshake failed: {}", e);
            return Err(e);
        }
    };

    Ok(())
}

/// Test connection timeout scenarios
async fn test_timeout_scenarios() -> Result<()> {
    info!("Testing connection to non-existent server...");

    let timeout_result = timeout(
        Duration::from_millis(2000),
        NdjsonAdapter::connect("127.0.0.1:9999"), // Non-existent port
    )
    .await;

    match timeout_result {
        Ok(Err(e)) => {
            info!("✅ Connection properly failed: {}", e);
        }
        Err(_) => {
            info!("✅ Connection timed out as expected");
        }
        Ok(Ok(_)) => {
            warn!("⚠️ Unexpected successful connection to non-existent server");
        }
    }

    Ok(())
}

/// Test connection timeout scenarios
async fn test_connection_timeout() -> Result<()> {
    test_timeout_scenarios().await
}

/// Test multiple simultaneous connections
async fn test_multiple_connections() -> Result<()> {
    let group = ClientSessionGroup::new();

    // Start multiple test servers
    let (server1_addr, _handle1) = start_test_server().await?;
    let (server2_addr, _handle2) = start_test_server().await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect to both servers
    info!("Connecting to server 1 at {}", server1_addr);
    let adapter1 = NdjsonAdapter::connect(&server1_addr).await?;
    let client1 = Client::new(adapter1).await?;
    group.add_client(server1_addr.clone(), client1).await?;

    info!("Connecting to server 2 at {}", server2_addr);
    let adapter2 = NdjsonAdapter::connect(&server2_addr).await?;
    let client2 = Client::new(adapter2).await?;
    group.add_client(server2_addr.clone(), client2).await?;

    // Test aggregated operations
    match group.list_tools_all().await {
        Ok(tools) => {
            info!("✅ Listed {} tools from multiple servers", tools.len());
            for tool in &tools {
                info!("   🔧 Tool: {}", tool.name);
            }
        }
        Err(e) => {
            error!("❌ Failed to list tools from multiple servers: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Test handshake validation
async fn test_handshake_validation() -> Result<()> {
    let (server_addr, _server_handle) = start_test_server().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let adapter = NdjsonAdapter::connect(&server_addr).await?;
    let _client = Client::new(adapter).await?;

    info!("✅ Handshake validation completed successfully");
    Ok(())
}

/// Start a test server for debugging
async fn start_test_server() -> Result<(String, tokio::task::JoinHandle<()>)> {
    let server = Server::new("debug-server").register_tool(
        Tool {
            name: "test-tool".to_string(),
            description: Some("A test tool for debugging".to_string()),
            ..Default::default()
        },
        |_handle, _args| async {
            Ok(CallToolResult {
                content: vec![Content::Text {
                    text: "Test tool executed successfully".to_string(),
                }],
                is_error: false,
            })
        },
    );

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = listener.local_addr()?.to_string();
    drop(listener);

    let server_addr_clone = server_addr.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = server.tcp_listen::<NdjsonAdapter>(&server_addr_clone).await {
            if !e.to_string().contains("reset by peer") {
                error!("Test server error: {}", e);
            }
        }
    });

    Ok((server_addr, handle))
}

/// Analyze error patterns from log files
async fn analyze_error_logs(log_file: Option<String>) -> Result<()> {
    let log_path = log_file.unwrap_or_else(|| "logs/server.log.2025-06-16".to_string());

    info!("Analyzing log file: {}", log_path);

    let content = match tokio::fs::read_to_string(&log_path).await {
        Ok(content) => content,
        Err(e) => {
            error!("❌ Failed to read log file {}: {}", log_path, e);
            return Err(e.into());
        }
    };

    let mut error_patterns = std::collections::HashMap::new();
    let mut total_errors = 0;

    for line in content.lines() {
        if line.contains("ERROR") {
            total_errors += 1;

            if line.contains("missing field `id`") {
                *error_patterns
                    .entry("Missing Request ID".to_string())
                    .or_insert(0) += 1;
            } else if line.contains("Serialization error") {
                *error_patterns
                    .entry("Serialization Error".to_string())
                    .or_insert(0) += 1;
            } else if line.contains("Connection") {
                *error_patterns
                    .entry("Connection Error".to_string())
                    .or_insert(0) += 1;
            } else {
                *error_patterns.entry("Other Error".to_string()).or_insert(0) += 1;
            }
        }
    }

    println!("\n📊 Log Analysis Results");
    println!("=======================\n");
    println!("Total errors found: {}", total_errors);
    println!();

    for (pattern, count) in &error_patterns {
        println!("• {}: {} occurrences", pattern, count);
    }

    if error_patterns.get("Missing Request ID").unwrap_or(&0) > &0 {
        println!("\n🆔 Missing Request ID Issues:");
        println!("   • All JSON-RPC requests must include an 'id' field");
        println!("   • Use RequestId::Num(n) for numeric IDs");
        println!("   • Use RequestId::Str(s) for string IDs");
        println!("   • The initialize request must have id: 0");
    }

    Ok(())
}

/// Print test summary
fn print_test_summary(results: &[(&str, bool)]) {
    println!("\n🎯 Test Summary");
    println!("===============\n");

    let total = results.len();
    let passed = results.iter().filter(|(_, success)| *success).count();
    let failed = total - passed;

    println!("Total tests: {}", total);
    println!("Passed: {} ✅", passed);
    println!("Failed: {} ❌", failed);
    println!();

    for (test_name, success) in results {
        let status = if *success { "✅" } else { "❌" };
        println!("{} {}", status, test_name);
    }

    if failed > 0 {
        println!("\n🔧 Troubleshooting Guide:");
        println!("=========================\n");
        print_troubleshooting_guide();
    }
}

/// Print troubleshooting guide
fn print_troubleshooting_guide() {
    println!("🚨 Connection Issues:");
    println!("   • Ensure server is running and accessible");
    println!("   • Check host and port configuration");
    println!("   • Verify firewall and network settings\n");

    println!("🤝 Handshake Problems:");
    println!("   • Check protocol version compatibility");
    println!("   • Ensure proper initialization sequence");
    println!("   • Verify adapter type matches server\n");

    println!("🆔 Request ID Errors:");
    println!("   • All requests need unique 'id' field");
    println!("   • Initialize request must use id: 0");
    println!("   • Use proper RequestId enum variants\n");

    println!("📡 Network Adapter Issues:");
    println!("   • NDJSON: One JSON object per line");
    println!("   • LSP: Content-Length header required");
    println!("   • STDIO: For subprocess communication\n");
}
