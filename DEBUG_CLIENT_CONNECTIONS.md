# MCP Client Connection Debugging Guide

This guide provides comprehensive tools and techniques for debugging client connection scenarios in the MCP Rust SDK.

## Overview

Client connection issues are among the most common problems when working with MCP. This guide covers:

- **Common connection scenarios** and their debugging approaches
- **Network adapter issues** (NDJSON, LSP, STDIO)
- **Handshake failures** and protocol mismatches
- **Request ID errors** and JSON-RPC compliance
- **Timeout and connection stability** problems
- **Multiple connection management**

## Quick Start

### Using the Debug Client Tool

We've created a comprehensive debugging tool that can test various connection scenarios:

```bash
# Build the debug client
cd examples/debug_client
cargo build

# Run all debugging scenarios
cargo run -- all

# Test specific connection types
cargo run -- tcp-ndjson --address 127.0.0.1:8080
cargo run -- tcp-lsp --address 127.0.0.1:8081

# Test timeout scenarios
cargo run -- timeout

# Test multiple connections
cargo run -- multiple

# Analyze error logs
cargo run -- analyze-logs --log-file path/to/server.log
```

## Common Connection Scenarios

### 1. TCP Connection with NDJSON Adapter

**Scenario**: Basic TCP connection using line-delimited JSON messages.

```rust
use mcp_sdk::{Client, NdjsonAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to server
    let adapter = NdjsonAdapter::connect("127.0.0.1:8080").await?;
    let client = Client::new(adapter).await?;
    
    // Test basic operations
    let tools = client.list_tools().await?;
    println!("Available tools: {:?}", tools);
    
    Ok(())
}
```

**Common Issues:**
- Server not running on specified port
- Messages not terminated with newlines
- JSON malformation

**Debug Tips:**
```bash
# Test server availability
telnet 127.0.0.1 8080

# Monitor network traffic
tcpdump -i lo0 -A port 8080

# Use our debug tool
cargo run -- tcp-ndjson --address 127.0.0.1:8080
```

### 2. TCP Connection with LSP Adapter

**Scenario**: TCP connection using Language Server Protocol message format.

```rust
use mcp_sdk::{Client, LspAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let adapter = LspAdapter::connect("127.0.0.1:8081").await?;
    let client = Client::new(adapter).await?;
    
    // Client is ready for use
    Ok(())
}
```

**Common Issues:**
- Missing Content-Length headers
- Incorrect header format
- Message body length mismatch

**Debug Tips:**
```bash
# Test LSP connection
cargo run -- tcp-lsp --address 127.0.0.1:8081

# Verify message format manually
echo -e "Content-Length: 45\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"test\"}" | nc 127.0.0.1 8081
```

### 3. STDIO Connection

**Scenario**: Process-based communication using stdin/stdout.

```rust
use mcp_sdk::{Client, StdioAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let adapter = StdioAdapter::new();
    let client = Client::new(adapter).await?;
    
    // Client communicates via stdin/stdout
    Ok(())
}
```

**Common Issues:**
- Process not reading/writing correctly
- Buffer flushing problems
- EOF handling

### 4. Multiple Connections

**Scenario**: Managing connections to multiple servers simultaneously.

```rust
use mcp_sdk::{Client, ClientSessionGroup, NdjsonAdapter, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let group = ClientSessionGroup::new();
    
    // Connect to multiple servers
    let adapter1 = NdjsonAdapter::connect("127.0.0.1:8080").await?;
    let client1 = Client::new(adapter1).await?;
    group.add_client("server1".to_string(), client1).await?;
    
    let adapter2 = NdjsonAdapter::connect("127.0.0.1:8081").await?;
    let client2 = Client::new(adapter2).await?;
    group.add_client("server2".to_string(), client2).await?;
    
    // Aggregate operations across servers
    let all_tools = group.list_tools_all().await?;
    println!("Total tools from all servers: {}", all_tools.len());
    
    Ok(())
}
```

**Debug with:**
```bash
cargo run -- multiple
```

## Error Analysis

### Missing Request ID Errors

**Symptom**: `Serialization error: missing field 'id'`

**Cause**: JSON-RPC requests without proper ID field.

**Example Error Log:**
```
2025-06-16T15:10:05.376325Z ERROR mcp_sdk::server::session: [Server] Error dispatching request: Serialization error: missing field `id`
```

**Solutions:**
1. Ensure all requests include an `id` field
2. Use proper RequestId enum variants:
   ```rust
   use mcp_sdk::types::RequestId;
   
   let id = RequestId::Num(1);  // for numeric IDs
   let id = RequestId::Str("req-123".to_string());  // for string IDs
   ```
3. Initialize request must use `id: 0`

**Debug Analysis:**
```bash
# Analyze your logs for patterns
cargo run -- analyze-logs --log-file logs/server.log.2025-06-16
```

### Connection Timeout Issues

**Symptoms:**
- Connection hangs indefinitely
- "Connection refused" errors
- Timeout exceptions

**Debug Steps:**
1. **Verify server is running:**
   ```bash
   netstat -an | grep 8080
   ```

2. **Test basic connectivity:**
   ```bash
   telnet 127.0.0.1 8080
   ```

3. **Use timeout debugging:**
   ```bash
   cargo run -- timeout
   ```

4. **Check firewall/network settings:**
   ```bash
   # On macOS
   sudo pfctl -sr | grep 8080
   
   # On Linux
   sudo iptables -L | grep 8080
   ```

### Handshake Failures

**Symptoms:**
- Client fails during initialization
- Protocol version mismatches
- Capability negotiation errors

**Common Causes:**
1. **Incompatible protocol versions**
2. **Server doesn't support client capabilities**
3. **Malformed initialization messages**

**Debug Approach:**
```rust
// Enable detailed logging
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

// The handshake details will be logged
let client = Client::new(adapter).await?;
```

### Network Adapter Mismatches

**Issue**: Client and server using different protocols.

| Client Adapter | Server Must Support | Message Format |
|----------------|--------------------|-----------------|
| `NdjsonAdapter` | NDJSON | `{"jsonrpc":"2.0",...}\n` |
| `LspAdapter` | LSP | `Content-Length: 45\r\n\r\n{...}` |
| `StdioAdapter` | STDIO | Line-based via stdin/stdout |

**Verification:**
```bash
# Test each adapter type
cargo run -- tcp-ndjson --address 127.0.0.1:8080
cargo run -- tcp-lsp --address 127.0.0.1:8080
```

## Monitoring and Observability

### Enable Comprehensive Logging

```rust
// In your application
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .with_target(true)
    .with_thread_ids(true)
    .with_file(true)
    .with_line_number(true)
    .init();
```

### Key Log Messages to Monitor

| Log Level | Component | Message Pattern | Indicates |
|-----------|-----------|-----------------|-----------|
| INFO | `mcp_sdk::client::client` | "Handshake successful" | Successful connection |
| ERROR | `mcp_sdk::server::session` | "Error dispatching request" | Request processing failure |
| ERROR | `mcp_sdk::server::session` | "missing field 'id'" | JSON-RPC compliance issue |
| INFO | `mcp_sdk::server::session` | "Initialize handshake started" | Connection attempt |

### Network-Level Debugging

**Capture traffic:**
```bash
# macOS
sudo tcpdump -i lo0 -A port 8080

# Linux
sudo tcpdump -i lo -A port 8080

# Save to file for analysis
sudo tcpdump -i lo0 -w capture.pcap port 8080
```

**Analyze with Wireshark:**
- Filter: `tcp.port == 8080`
- Look for malformed JSON
- Check message boundaries
- Verify protocol compliance

## Performance Debugging

### Connection Latency

```rust
use std::time::Instant;

let start = Instant::now();
let adapter = NdjsonAdapter::connect("127.0.0.1:8080").await?;
let connect_time = start.elapsed();
info!("Connection took: {:?}", connect_time);

let start = Instant::now();
let client = Client::new(adapter).await?;
let handshake_time = start.elapsed();
info!("Handshake took: {:?}", handshake_time);
```

### Memory Usage

```rust
// Monitor connection count in ClientSessionGroup
let group = ClientSessionGroup::new();
// ... add clients ...

// Check number of active connections
let session_count = group.sessions.read().await.len();
info!("Active sessions: {}", session_count);
```

## Troubleshooting Checklist

### Basic Connectivity
- [ ] Server is running and listening on expected port
- [ ] No firewall blocking the connection
- [ ] Correct host/port configuration
- [ ] Network interface accessible

### Protocol Compliance
- [ ] Client and server using compatible adapters
- [ ] JSON-RPC 2.0 message format
- [ ] All requests include unique `id` field
- [ ] Initialize request uses `id: 0`

### MCP-Specific
- [ ] Protocol version compatibility
- [ ] Proper capability negotiation
- [ ] Tool/resource registration on server
- [ ] Handler implementations don't panic

### Performance
- [ ] Connection pooling if needed
- [ ] Appropriate timeout values
- [ ] Proper resource cleanup
- [ ] Memory leak monitoring

## Advanced Debugging Techniques

### Custom Network Adapter for Testing

```rust
use mcp_sdk::network_adapter::NetworkAdapter;
use async_trait::async_trait;

#[derive(Debug)]
struct DebugAdapter {
    inner: NdjsonAdapter,
    message_count: Arc<AtomicUsize>,
}

#[async_trait]
impl NetworkAdapter for DebugAdapter {
    async fn send(&mut self, msg: &str) -> Result<()> {
        let count = self.message_count.fetch_add(1, Ordering::SeqCst);
        println!("[DEBUG] Sending message #{}: {}", count, msg);
        self.inner.send(msg).await
    }
    
    async fn recv(&mut self) -> Result<Option<String>> {
        let result = self.inner.recv().await;
        if let Ok(Some(ref msg)) = result {
            println!("[DEBUG] Received message: {}", msg);
        }
        result
    }
}
```

### Mock Server for Testing

```rust
use mcp_sdk::{Server, Tool, CallToolResult, Content};

async fn create_test_server() -> Server {
    Server::new("test-server")
        .register_tool(
            Tool {
                name: "debug-tool".to_string(),
                description: Some("Tool for debugging".to_string()),
                ..Default::default()
            },
            |_handle, args| async move {
                println!("[DEBUG] Tool called with args: {:?}", args);
                Ok(CallToolResult {
                    content: vec![Content::Text {
                        text: format!("Debug response: {:?}", args),
                    }],
                    is_error: false,
                })
            },
        )
}
```

## Integration with External Tools

### JSON Schema Validation

```bash
# Install jsonschema tool
npm install -g ajv-cli

# Validate messages against MCP schema
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
    ajv validate -s mcp-schema.json
```

### Message Recording and Replay

```rust
// Record messages for later analysis
struct RecordingAdapter {
    inner: NdjsonAdapter,
    log_file: tokio::fs::File,
}

// Implementation records all traffic to file
// Can replay later for consistent testing
```

## Getting Help

If you're still experiencing issues after following this guide:

1. **Enable verbose logging** and capture the full output
2. **Run the debug client tool** with all scenarios
3. **Analyze your logs** with the built-in analyzer
4. **Create a minimal reproduction case**
5. **Check the issue tracker** for similar problems

### Useful Commands Summary

```bash
# Quick connection test
cargo run --example debug_client -- tcp-ndjson

# Full diagnostic
cargo run --example debug_client -- all

# Log analysis
cargo run --example debug_client -- analyze-logs --log-file your-log.txt

# Network debugging
netstat -an | grep 8080
tcpdump -i lo0 -A port 8080

# Process debugging
lsof -i :8080
ps aux | grep mcp
```

With these tools and techniques, you should be able to diagnose and resolve most client connection issues in the MCP Rust SDK.

