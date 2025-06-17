# Setting Up MCP Rust SDK Server with Warp Terminal

This guide shows you how to configure the `unsafe-sql-server` example as an MCP server in Warp Terminal.

## Prerequisites

1. **Warp Terminal** with MCP support enabled
2. **Rust toolchain** installed
3. **Built MCP server binary**

## Step 1: Build the Server

First, ensure your MCP server is built:

```bash
cd /Users/spaghetty/Projects/mcp/rust_mcp_sdk
cargo build --package unsafe-sql-server-example
```

Verify the binary exists:
```bash
ls -la target/debug/unsafe-sql-server-example
```

## Step 2: Test the Server Manually

Before configuring Warp, test that the server works correctly:

```bash
# Test the initialization handshake
echo '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test-client","version":"1.0"},"capabilities":{}}}' | \
  RUST_LOG=info ./target/debug/unsafe-sql-server-example
```

You should see a JSON response like:
```json
{"jsonrpc":"2.0","id":0,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"unsafe-sql-server","version":"0.1.0"}}}
```

## Step 3: Locate Warp's MCP Configuration

Warp stores MCP server configurations in one of these locations:

### macOS:
```bash
# Check these locations (in order of preference):
~/.config/warp/mcp_servers.json
~/Library/Application Support/warp/mcp_servers.json
```

### Create the config directory if it doesn't exist:
```bash
mkdir -p ~/.config/warp
```

## Step 4: Configure the MCP Server

Create or edit the MCP configuration file:

```bash
nano ~/.config/warp/mcp_servers.json
```

Add the following configuration:

```json
{
  "mcpServers": {
    "unsafe-sql-server": {
      "command": "/Users/spaghetty/Projects/mcp/rust_mcp_sdk/target/debug/unsafe-sql-server-example",
      "args": ["--db-file", "/Users/spaghetty/Projects/mcp/local_database.db"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

**Important**: Update the `command` path to match your actual project location!

## Step 5: Restart Warp

After saving the configuration:

1. **Completely quit Warp** (Cmd+Q)
2. **Restart Warp**
3. **Check the MCP server status** in Warp's settings or via command palette

## Step 6: Test the Integration

Once Warp is restarted:

1. **Open Warp Terminal**
2. **Access MCP tools** (usually via command palette or MCP menu)
3. **Look for your server**: You should see "unsafe-sql-server" listed
4. **Test the tools**:
   - `get_schema` - Returns the database schema
   - `execute_sql` - Executes SQL queries

## Troubleshooting

### Server Not Appearing in Warp

1. **Check the configuration file path**:
   ```bash
   cat ~/.config/warp/mcp_servers.json
   ```

2. **Verify the binary path is correct**:
   ```bash
   /Users/spaghetty/Projects/mcp/rust_mcp_sdk/target/debug/unsafe-sql-server-example --help
   ```

3. **Check Warp's logs** (if available) for error messages

4. **Test manual connection** using our debug client:
   ```bash
   cd /Users/spaghetty/Projects/mcp/rust_mcp_sdk/examples/debug_client
   cargo run -- tcp-ndjson --address 127.0.0.1:8080
   ```

### Common Issues

#### Issue: "Binary not found"
**Solution**: Ensure the full absolute path to the binary is correct:
```bash
which unsafe-sql-server-example
# Or
realpath target/debug/unsafe-sql-server-example
```

#### Issue: "Permission denied"
**Solution**: Make the binary executable:
```bash
chmod +x target/debug/unsafe-sql-server-example
```

#### Issue: "Database errors"
**Solution**: Ensure the database directory is writable:
```bash
mkdir -p /Users/spaghetty/Projects/mcp
touch /Users/spaghetty/Projects/mcp/local_database.db
```

#### Issue: "Server crashes on startup"
**Solution**: Test with verbose logging:
```bash
RUST_LOG=debug ./target/debug/unsafe-sql-server-example
```

### Debug Server Communication

If the server appears to connect but doesn't work properly:

1. **Enable debug logging** in the configuration:
   ```json
   {
     "mcpServers": {
       "unsafe-sql-server": {
         "command": "/path/to/unsafe-sql-server-example",
         "args": ["--db-file", "/path/to/local_database.db"],
         "env": {
           "RUST_LOG": "debug"
         }
       }
     }
   }
   ```

2. **Check the server logs**:
   ```bash
   tail -f /Users/spaghetty/Projects/mcp/rust_mcp_sdk/logs/server.log.*
   ```

3. **Use our analysis tool**:
   ```bash
   cd /Users/spaghetty/Projects/mcp/rust_mcp_sdk/examples/debug_client
   cargo run -- analyze-logs --log-file ../../logs/server.log.2025-06-16
   ```

## Advanced Configuration

### Multiple Servers

You can configure multiple MCP servers:

```json
{
  "mcpServers": {
    "unsafe-sql-server": {
      "command": "/path/to/unsafe-sql-server-example",
      "args": ["--db-file", "/path/to/db1.db"]
    },
    "another-server": {
      "command": "/path/to/another-server",
      "args": ["--config", "/path/to/config.json"]
    }
  }
}
```

### Environment Variables

Use environment variables for configuration:

```json
{
  "mcpServers": {
    "unsafe-sql-server": {
      "command": "/path/to/unsafe-sql-server-example",
      "args": ["--db-file", "${HOME}/mcp_database.db"],
      "env": {
        "RUST_LOG": "info",
        "DATABASE_URL": "${HOME}/mcp_database.db"
      }
    }
  }
}
```

## Using the SQL Server

Once configured and working, you can:

### Get Database Schema
1. Open Warp
2. Access MCP tools menu
3. Select "get_schema" from unsafe-sql-server
4. View the current database structure

### Execute SQL Queries
1. Access MCP tools menu
2. Select "execute_sql" from unsafe-sql-server
3. Enter your SQL query (e.g., `SELECT * FROM tasks`)
4. View results

### Example Queries
```sql
-- View all tasks
SELECT * FROM tasks;

-- Add a new task
INSERT INTO tasks (title, status) VALUES ('Learn MCP', 'pending');

-- Update task status
UPDATE tasks SET status = 'completed' WHERE id = 1;

-- Create a new table
CREATE TABLE notes (id INTEGER PRIMARY KEY, content TEXT, created_at DATETIME DEFAULT CURRENT_TIMESTAMP);
```

## Security Notes

⚠️  **Important**: This is called "unsafe-sql-server" for a reason!

- **Do not use in production**
- **Do not connect to important databases**
- **Only use for testing and development**
- **The server executes raw SQL without validation**

## Getting Help

If you're still having issues:

1. **Check our debug guide**: [DEBUG_CLIENT_CONNECTIONS.md](./DEBUG_CLIENT_CONNECTIONS.md)
2. **Run comprehensive diagnostics**:
   ```bash
   cd examples/debug_client
   cargo run -- all
   ```
3. **Analyze server logs**:
   ```bash
   cargo run -- analyze-logs --log-file ../../logs/server.log.2025-06-16
   ```

The server logs all connections and requests, which should help identify any issues with the Warp integration.

