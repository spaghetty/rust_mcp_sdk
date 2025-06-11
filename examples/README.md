# MCP Rust SDK Examples
This directory contains several runnable examples demonstrating how to use the ```mcp-sdk``` crate to build clients and servers.

All commands should be run from the root of the ```rust-mcp-sdk``` workspace directory.

## 1. Configurable Server
The ```simple-server-example``` is a single, configurable server that provides all the features needed for the different clients (tools, resources, and prompts). You can run it with flags to change its behavior, which is especially useful for testing the ```group-client```.

### To Run a Single, Default Server:
This is all you need for the ```simple-client```, ```notification-client```, and ```prompt-client``` examples. Open a terminal and run:

```bash
cargo run -p simple-server-example
```

This will start the server on the default port ```127.0.0.1:8080```.

## 2. Client Examples
Basic Tool and Resource Client (```simple-client```)
This client connects to the server and demonstrates basic ```tools/list``` and ```tools/call``` methods.

**Setup:**

In one terminal, start the default server:

```bash
cargo run -p simple-server-example
```

In a second terminal, run the simple client:

```bash
cargo run -p simple-client-example
```

### Notification Client (```notification-client```)

This client demonstrates how to register a handler for server-sent notifications. It calls a specific tool that triggers the server to send a ```tools/listChanged``` notification back to the client.

**Setup:**

In one terminal, start the default server:

```bash
cargo run -p simple-server-example
```

In a second terminal, run the notification client:

```bash
cargo run -p notification-client-example
```

### Prompt Client (```prompt-client```)
This client demonstrates how to list and fetch prompt templates from the server.

**Setup:**

In one terminal, start the default server:

```bash
cargo run -p simple-server-example
```

In a second terminal, run the prompt client:

```bash
cargo run -p prompt-client-example
```

## 3. Client Session Group Example (```group-client```)
This example demonstrates the ```ClientSessionGroup``` by connecting to two different servers simultaneously and aggregating their tools. To run this, you must start two instances of the configurable simple-server on different ports.

**Setup:**

Start Server 1: In your first terminal, start the server on port 8081 with the suffix _1:

```bash
cargo run -p simple-server-example -- --port 8081 --suffix _1
```

Start Server 2: In a second terminal, start another server on port 8082 with the suffix _2:

```bash
cargo run -p simple-server-example -- --port 8082 --suffix _2
```

Run the Group Client: In a third terminal, run the group client example. It is hardcoded to connect to ports 8081 and 8082.

```bash
cargo run -p group-client-example
```

You will see the client connect to both servers and print the aggregated list of tools (```fetch_1``` and ```fetch_2```).
