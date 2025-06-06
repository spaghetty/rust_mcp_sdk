pub mod client;
pub mod common;
pub mod server;
pub mod types;

/// Main library file for MCP Rust SDK
mod adapters;
mod protocol;

// Explicitly re-export only unique types to avoid ambiguous glob re-exports
// (Clippy: ambiguous glob re-exports)
pub use common::{InitializeRequest, ListResourcesRequest, ListToolsRequest, SessionMessage};
pub use types::{
    BlobResourceContents, CancelledNotificationParams, ClientCapabilities, Implementation,
    InitializeRequestParams, InitializeResult, ListResourcesResult, ListToolsResult, LoggingLevel,
    LoggingMessageNotificationParams, NotificationParams, PaginatedRequestParams,
    ProgressNotificationParams, RequestParams, Resource, ResourceContents,
    ResourceUpdatedNotificationParams, RootsCapability, SamplingCapability, ServerCapabilities,
    TextContent, TextResourceContents, Tool, ToolCallParams, ToolCallRequest, ToolResult,
    ToolResultData,
};

pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
