# MCP Rust SDK Design Document

## Table of Contents
- [Overview](#overview)
- [Structural Design](#structural-design)
  - [Key Components](#key-components)
  - [File Layout](#file-layout)
- [Logical Representation](#logical-representation)
  - [Component Responsibilities](#component-responsibilities)
  - [Interactions & Message Flow](#interactions--message-flow)
- [Public API](#public-api)
  - [Client API](#client-api)
  - [Server API](#server-api)
  - [Types & Data Structures](#types--data-structures)
- [Error Handling & Isolation](#error-handling--isolation)
- [Extensibility & Future Work](#extensibility--future-work)

---

## Overview

The MCP Rust SDK provides a robust, async, and type-safe implementation of the MCP protocol for both client and server applications. It is designed for reliability, extensibility, and clear separation of concerns, with a focus on protocol compliance and robust error handling.

---

## Structural Design

### Layered Architecture & Key Components

The SDK is structured into four primary layers, each with clear responsibilities and file mappings. **This document is the reference for all future design and testing decisions.**

1. **Network Adapter Layer**
   - **Files:** `src/adapter_client_tcp.rs`, `src/adapter_server_tcp.rs`, (future: `src/adapter_client_http.rs`, etc.)
   - **Responsibilities:**
     - Handles raw transport (TCP, HTTP, etc.), async IO, message framing.
     - Implements a common `NetworkAdapter` trait for pluggability.
     - Enables easy addition of new transports (WebSocket, HTTP, Unix sockets, etc).

2. **Protocol Layer**
   - **Files:** `src/types.rs`, `src/common.rs`, `src/protocol.rs`
   - **Responsibilities:**
     - Defines all MCP protocol message types, requests, responses, and error structures.
     - Handles serialization/deserialization (JSON <-> Rust types).
     - Validates protocol rules (required fields, method names, versioning).
     - Generates protocol-compliant error responses for invalid messages.

3. **Routing Layer**
   - **Files:** `src/routing.rs`
   - **Responsibilities:**
     - Receives validated protocol messages and dispatches them to the correct handler (e.g., `resources/list`, `tools/list`, `tool/call`).
     - Handles method-level logic and response matching.

4. **Application Layer**
   - **Files:** User code, handler registration in examples or main app
   - **Responsibilities:**
     - Implements business logic (tool/resource handlers, callbacks).
     - Should not deal with protocol or network details directly.

### Adapter Pattern Example

```rust
// In src/adapter_server_tcp.rs (similar for client)
#[async_trait::async_trait]
pub trait NetworkAdapter {
    async fn send(&mut self, msg: &str) -> anyhow::Result<()>;
    async fn recv(&mut self) -> anyhow::Result<Option<String>>;
}
```
- Each adapter implements this trait for its transport.
- Routing and protocol logic use the trait, not concrete adapters.

### File Layout

```
/ rust-sdk
  /src
    adapter_client_tcp.rs    # TCP-specific client network logic
    adapter_server_tcp.rs    # TCP-specific server network logic
    adapter_client_http.rs   # (future) HTTP client
    adapter_server_http.rs   # (future) HTTP server
    routing.rs               # Routing logic, protocol dispatch
    protocol.rs              # Protocol message (de)serialization, validation
    types.rs                 # Data structures and protocol types
    client.rs                # High-level client API (uses adapters)
    server.rs                # High-level server API (uses adapters)
    common.rs                # Shared helpers/utilities
  /examples
    simple_client.rs
    simple_resource.rs
    simple_tool.rs
  /docs
    SDK_DESIGN.md   # <-- This document
```

---

## Logical Representation

### Layered Responsibilities

| Layer           | Responsibilities                                         | Example Files                              |
|-----------------|----------------------------------------------------------|--------------------------------------------|
| High-Level API  | User-facing API for building clients and servers         | client.rs, server.rs                       |
| Adapter         | TCP/HTTP/WebSocket, async IO, framing                    | adapter_client_tcp.rs, adapter_server_tcp.rs, etc. |
| Protocol        | (De)serialization, validation, protocol errors           | types.rs, common.rs, protocol.rs           |
| Routing         | Dispatch to handlers, method lookup                      | routing.rs                                 |
| Application     | Business logic, tool/resource handlers                   | user code, examples/                       |

### Component Responsibilities

- **High-Level API Layer:** Provides ergonomic, user-facing interfaces for building MCP clients and servers, abstracting over adapters, protocol, and routing (see `client.rs`, `server.rs`).
- **Adapter Layer:** Handles all raw IO, connection setup, and message framing for various transports.
- **Protocol Layer:** Converts bytes/lines to strongly-typed messages, validates protocol rules, and generates error responses.
- **Routing Layer:** Receives valid protocol messages and dispatches to the appropriate handler based on method.
- **Application Layer:** Implements handler logic for tools/resources, returning results or application-level errors.

### Interactions & Message Flow

1. **Network:** Receives a line over TCP.
2. **Protocol:** Parses JSON, validates message, checks protocol rules (method, fields, version).
   - If invalid: sends protocol error response.
   - If valid: passes message to router.
3. **Routing:** Looks up handler for the method, dispatches.
4. **Application:** Executes handler, returns result or error.
5. **Protocol:** Wraps result/error in a protocol-compliant response.
6. **Network:** Serializes response, sends over TCP.

- Both sides use line-delimited JSON for message framing.
- Asynchronous tasks and Tokio channels decouple IO from business logic.

---

## Public API

### Client API (`client.rs`)

```rust
impl ClientSessionGroup {
    pub async fn connect_to_server(&mut self, server_url: Url) -> Result<()>;
    pub async fn list_resources(&mut self, server_url: &Url) -> Result<ListResourcesResult>;
    pub async fn list_tools(&mut self, server_url: &Url, params: PaginatedRequestParams) -> Result<ListToolsResult>;
    pub async fn call_tool(&mut self, server_url: &Url, name: String, arguments: HashMap<String, String>) -> Result<ToolResult>;
}
```

- **Session Management:** Multiple connections, each handled as a session.
- **Request Methods:** List resources, list tools, call tool (with arguments).
- **Async:** All methods are async and return `Result<T>`.

### Server API (`server.rs`)

```rust
impl Server {
    pub fn new() -> Self;
    pub fn list_resources<F>(&mut self, handler: F)
        where F: Fn(Value) -> Vec<Resource> + Send + Sync + 'static;
    pub fn register_tool_handler<F>(&mut self, handler: F)
        where F: Fn(String, HashMap<String, String>) -> Result<ToolResult> + Send + Sync + 'static;
    pub async fn run(&self, bind_addr: &str) -> Result<()>;
}
```

- **Resource/Tool Registration:** Register handlers for resources and tools.
- **Async Run:** Accepts connections and spawns sessions.

### Types & Data Structures (`types.rs`)

- **Requests:** `ListResourcesRequest`, `ListToolsRequest`, `ToolCallRequest`, etc.
- **Responses:** `ListResourcesResult`, `ListToolsResult`, `ToolResult`, etc.
- **Core Types:** `Resource`, `Tool`, `PaginatedRequestParams`, etc.

---

## Error Handling & Isolation

- **IO errors** (e.g., connection lost) terminate sessions (Network Layer).
- **Protocol errors** (bad message, unsupported method, invalid fields) are caught in the Protocol Layer and result in structured error responses, not panics or dropped connections.
- **Routing errors** (unknown method, handler not found) are caught and reported as protocol errors.
- **Application errors** (handler failures) are returned as error fields in responses, not as panics.
- **Isolation:** Each layer catches and reports its own errors; only fatal IO errors terminate sessions. Errors in one session do not affect others. Handlers are wrapped to catch and report errors without crashing the server/client.

---

## Extensibility & Future Work

- **Pluggable Handlers:** Easy to register new tools/resources.
- **Streaming & Pagination:** Protocol supports cursor-based pagination for large lists.
- **Authentication/Authorization:** Can be added at the session or handler level.
- **Protocol Extensions:** New methods and message types can be added with minimal disruption.
- **Improved Error Types:** Move toward custom error enums for protocol/application errors.

---

## Testing Strategy

**Testing is mandatory for every function and module.**

### 1. Unit Tests
- Each function, adapter, and protocol type must have unit tests in a `mod tests` section within the same file.
- Use mocks for adapters and protocol boundaries to isolate logic.

### 2. Integration Tests
- Tests in `/tests` directory cover end-to-end scenarios (client-server interaction, error propagation, protocol compliance).
- Each adapter should have integration tests to verify real network behavior.

### 3. Adapter Tests
- Each network adapter (TCP, HTTP, etc.) must have tests for connection setup, message send/receive, error handling, and edge cases (timeouts, disconnects).

### 4. Protocol Tests
- Serialization/deserialization roundtrips for all message types.
- Validation of required fields, error response generation, and version compatibility.

### 5. Routing & Application Tests
- Ensure correct handler dispatch for all protocol methods.
- Application handlers should be tested with both valid and invalid inputs.

---

## Appendix: Example Usage

See the `examples/` directory for working code illustrating client-server interaction, resource listing, and tool calling.

---

### Layer Summary Table

| Layer      | Example File(s)              | Responsibilities                                      |
|------------|-----------------------------|-------------------------------------------------------|
| Adapter    | adapter_client_tcp.rs, ...   | TCP/HTTP/WebSocket, async IO, framing                 |
| Protocol   | types.rs, common.rs, protocol.rs | Message (de)serialization, validation, protocol errors|
| Routing    | routing.rs                   | Dispatch to handlers, method lookup                   |
| Application| User code, examples/         | Business logic, tool/resource handlers                |

*This document is a living specification and must be kept up-to-date as the SDK evolves. All new features and modules must include appropriate tests as described above.*
