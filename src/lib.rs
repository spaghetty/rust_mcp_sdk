//! # MCP SDK for Rust
//!
//! This crate provides a robust, async, and type-safe implementation of the
//! Model Context Protocol (MCP) for building both client and server applications in Rust.
//!
//! The design is based on the official Python SDK and the MCP specification, with a
//! focus on creating an idiomatic and ergonomic developer experience for Rustaceans.
//!
//! ## Crate Structure
//!
//! The SDK is organized into several modules:
//!
//! * `types`: Contains all core data structures for MCP messages (requests, responses, etc.).
//! * `adapter`: Contains the pluggable network transport trait and implementations (e.g., TCP).
//! * `protocol`: Handles message serialization/deserialization over an adapter.
//! * `client`: Provides the high-level API for creating MCP clients.
//! * `server`: Provides the high-level API for creating MCP servers.

// --- Module Declarations ---

pub(crate) mod adapter;
pub(crate) mod protocol;
pub(crate) mod types;

// UPDATED: Declare the new client module structure.
pub mod client;
pub mod server;

// --- Public API Re-exports ---
pub use adapter::{NetworkAdapter, TcpAdapter};
pub use client::Client;
pub use protocol::ProtocolConnection;
pub use server::{ConnectionHandle, Server};
pub use types::{
    // Resource-related types
    BlobResourceContents,
    // Tool-related types
    CallToolResult,
    Content,
    EmbeddedResource,
    ImageContent,
    ListToolsChangedParams,
    // General RPC Types
    Notification,

    ReadResourceResult,
    Resource,
    ResourceContents,
    TextContent,
    TextResourceContents,
    Tool,
    ToolAnnotations,
};
