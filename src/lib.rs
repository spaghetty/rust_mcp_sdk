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

pub mod client;
pub mod error;
pub mod network_adapter;
pub(crate) mod protocol;
pub mod server;
pub mod types;

// --- Public API Re-exports ---
pub use client::Client;
pub use error::{Error, Result};
pub use network_adapter::{LspAdapter, NdjsonAdapter, NetworkAdapter, StdioAdapter};
pub use protocol::ProtocolConnection;
pub use server::{ConnectionHandle, Server};
pub use types::{
    BlobResourceContents, CallToolResult, Content, GetPromptResult, ListPromptsResult,
    ListToolsChangedParams, Notification, Prompt, PromptArgument, PromptMessage,
    ReadResourceResult, Resource, ResourceContents, TextResourceContents, Tool, ToolAnnotations,
};
