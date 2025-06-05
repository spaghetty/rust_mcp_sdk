pub mod client;
pub mod common;
pub mod server;
pub mod types;

// Explicitly re-export only unique types to avoid ambiguous glob re-exports
// (Clippy: ambiguous glob re-exports)
pub use common::{SessionMessage, InitializeRequest, ListResourcesRequest, ListToolsRequest};
pub use types::{Implementation, RequestParams, NotificationParams, InitializeRequestParams, InitializeResult, ClientCapabilities, ServerCapabilities, SamplingCapability, RootsCapability, ResourceContents, TextResourceContents, BlobResourceContents, PaginatedRequestParams, ListToolsResult, Tool, ToolResult, ToolResultData, TextContent, ToolCallParams, ToolCallRequest, ListResourcesResult, Resource, ProgressNotificationParams, LoggingMessageNotificationParams, LoggingLevel, ResourceUpdatedNotificationParams, CancelledNotificationParams};

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
