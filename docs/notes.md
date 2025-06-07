Project Plan: Implementing the MCP Rust SDK
This plan is based on the provided SDK_DESIGN.md. Our goal is to collaboratively implement the specified architecture, moving layer by layer to build a robust, async, and type-safe Rust SDK for the Model Context Protocol.

Our Collaborative Workflow
We will tackle this project in a series of focused phases, mirroring the layers in your design document. For each phase, the process will be:

Input: You provide the relevant Python source code or further specifications for a specific component.

Implementation: I will generate the corresponding idiomatic Rust code, complete with documentation and unit tests, as specified in your design.

Review & Refine: We review the generated code together, and I make any necessary adjustments based on your feedback.

Phase 1: Public API Validation (New Step)
Before implementing the internals, we will define the target public API to ensure the new SDK offers an excellent developer experience that is both idiomatic to Rust and spiritually similar to the Python original.

Goal: Define the exact structs and methods for the high-level Client and Server APIs.

Process:

You will provide key examples from the Python SDK's examples/ directory (e.g., a simple client and a simple server).

We will analyze how they are used.

I will propose a corresponding high-level API in idiomatic Rust. This proposal will become the blueprint for our src/client.rs and src/server.rs files.

What I need to start Phase 1:
Please provide the Python source code for two key examples:

A simple client that connects to a server and makes a call (e.g., examples/clients/simple_client.py).

A simple server that registers a tool or resource (e.g., examples/servers/simple_tool_server.py).

Phase 2: The Foundation - Protocol & Types
With a clear API target, we will build the core data structures.

Files to Create: src/types.rs, src/common.rs, src/protocol.rs

Key Rust Crates: serde, serde_json, tokio, anyhow.

Goal: Define all MCP request/response structs, core types (Resource, Tool), and implement the serialization/deserialization logic based on the Python types.py.

Phase 3: The Network - Pluggable Adapters
Next, we'll build the layer responsible for communication.

Files to Create: src/adapter_server_tcp.rs, src/adapter_client_tcp.rs

Key Rust Crates: tokio, async-trait.

Goal: Define the NetworkAdapter trait and implement the first concrete version for TCP.

Phase 4: The Logic - Routing and API Implementation
With data types and networking in place, we'll connect them with the routing logic and implement the public API we designed in Phase 1.

Files to Create: src/routing.rs, src/client.rs, src/server.rs

Goal: Implement the routing layer and the user-facing Client and Server APIs.

Phase 5: Integration, Examples, and Testing
The final phase is to bring everything together and write the corresponding Rust examples and end-to-end integration tests.

Files to Create: examples/, tests/

Goal: Ensure all layers work together correctly, create clear examples, and validate the full implementation.
