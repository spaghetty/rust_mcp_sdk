# Dependency Mapping: Python to Rust
This document tracks the translation of Python dependencies from pyproject.toml to their corresponding crates in the Rust ecosystem. This serves as a reference for the architectural choices made for the MCP Rust SDK.

|Python Library|Rust Equivalent|Purpose & Rationale|Status|
|--------------|------------------|-------------------|------|
|anyio, uvicorn|tokio|The core asynchronous runtime for the entire SDK.<br /> It will manage all non-blocking I/O, concurrency, and task scheduling.|Chosen|
|pydantic, pydantic-settings|serde with serde_json|The de-facto standard for high-performance JSON (and other formats) serialization and deserialization in Rust.<br /> It provides powerful derive macros to make Rust structs easily convertible to/from the MCP JSON format.|Chosen|
httpx|reqwest|A powerful and ergonomic asynchronous HTTP client. It's built on tokio and will be used to implement the network adapter for the MCP client.|Chosen|
|starlette, python-multipart|axum|A modern, modular, and highly performant web framework built by the tokio team. Its handler-based design and extractor pattern are a perfect fit for implementing the MCP server-side logic and routing as described in the design document.|Chosen|
|httpx-sse, sse-starlette| |Built-in streaming in reqwest and axumModern Rust web libraries have first-class support for streaming data. We can implement Server-Sent Events (SSE) directly using the response streaming features of axum and reqwest, avoiding the need for a separate library.|Chosen|
|websockets (optional)|tokio-tungstenite|he standard, go-to library for implementing WebSocket communication on top of tokio. We will use this to build the WebSocket network adapter if required.|Chosen|
|typer, python-dotenv (dev)|clap|The most popular and powerful command-line argument parsing library in Rust. It will be used if we build a companion CLI application for the SDK.|Chosen|
|pytest, etc. (dev)|Native Rust testing, rstest|Rust has excellent built-in support for unit and integration testing (#[test]). We can enhance this with crates like rstest for more complex scenarios.|Chosen|
|rich (optional)|ratatui, crossterm|If a rich terminal user interface (TUI) is needed for examples or CLIs, ratatui is the leading library for building them.|To be implemented|
