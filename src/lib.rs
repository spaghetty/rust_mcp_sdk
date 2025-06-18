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
pub mod protocol; // Made public for integration tests
pub mod server;
pub mod types;

// --- ToolArguments Proc Macro ---
/// Derives the `ToolArgumentsDescriptor` trait for a struct, enabling automatic
/// JSON schema generation for its fields. This schema is used by tools to
/// define their expected input arguments.
///
/// The macro also generates an inherent method `pub fn mcp_input_schema() -> serde_json::Value`
/// on the struct that implements the trait method.
///
/// Structs deriving `ToolArguments` typically also need to derive `serde::Deserialize`
/// to be usable with `Server::register_tool_typed`.
///
/// # Usage
///
/// ```rust
/// use mcp_sdk::ToolArguments;
/// use serde::Deserialize; // Required for typed handlers
///
/// #[derive(ToolArguments, Deserialize)]
/// struct MyToolArgs {
///     message: String,
///     count: i32,
///     optional_flag: Option<bool>,
/// }
/// ```
///
/// ## Field Attributes (`#[tool_arg(...)]`)
///
/// Field-level attributes can be used to customize the generated schema:
///
/// - `#[tool_arg(desc = "description")]`: Adds a "description" to the field's schema.
/// - `#[tool_arg(rename = "newName")]`: Uses "newName" as the property name in the JSON schema
///   instead of the Rust field name.
/// - `#[tool_arg(skip)]`: Excludes the field from the generated schema.
/// - `#[tool_arg(required = true/false)]`: Overrides the default requirement behavior.
///   By default, `Option<T>` fields are not required, and other fields are required.
///
/// ### Example with Field Attributes:
///
/// ```rust
/// use mcp_sdk::ToolArguments;
/// use serde::Deserialize;
///
/// #[derive(ToolArguments, Deserialize)]
/// struct AdvancedArgs {
///     #[tool_arg(desc = "The unique user identifier.")]
///     id: String,
///
///     #[tool_arg(rename = "userEmail", desc = "User's email address.", required = true)]
///     email: Option<String>, // Made explicitly required despite being Option
///
///     #[tool_arg(skip)]
///     internal_processing_flag: bool, // This field will not appear in the schema
///
///     #[tool_arg(required = false)]
///     non_optional_but_not_required: String,
/// }
/// ```
pub use mcp_sdk_macros::ToolArguments;

// Define the ToolArgumentsDescriptor trait
/// A trait for types that can describe their structure as a JSON schema
/// suitable for MCP tool arguments.
///
/// This trait is typically derived automatically using `#[derive(ToolArguments)]`.
/// It provides the mechanism for `Tool::from_args` and typed tool registration
/// to understand the expected input for a tool.
pub trait ToolArgumentsDescriptor {
    /// Returns the JSON schema for the implementing type.
    ///
    /// The schema defines the expected "properties" and "required" fields
    /// according to JSON Schema conventions, specifically tailored for MCP.
    fn mcp_input_schema() -> serde_json::Value;
}

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
