use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestParams {
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationParams {
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequestParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: Implementation,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: Implementation,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    pub sampling: Option<SamplingCapability>,
    pub roots: Option<RootsCapability>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub sampling: Option<SamplingCapability>,
    pub roots: Option<RootsCapability>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl ServerCapabilities {
    pub fn default() -> Self {
        ServerCapabilities {
            sampling: Some(SamplingCapability {
                sample_size: 100,
                extra: HashMap::new(),
            }),
            roots: Some(RootsCapability {
                list_changed: true,
                extra: HashMap::new(),
            }),
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapability {
    pub sample_size: u32,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsCapability {
    pub list_changed: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContents {
    pub uri: Url,
    pub mime_type: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextResourceContents {
    #[serde(flatten)]
    pub base: ResourceContents,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobResourceContents {
    #[serde(flatten)]
    pub base: ResourceContents,
    pub blob: String, // base64-encoded
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaginatedRequestParams {
    pub cursor: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// MCP ToolResult definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub result: Option<ToolResultData>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolResultData {
    TextContent(TextContent),
    // Add other variants as needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    #[serde(rename = "method")]
    method: &'static str,
    pub params: InitializeRequestParams,
}
impl InitializeRequest {
    pub fn new(params: InitializeRequestParams) -> Self {
        Self {
            method: "initialize",
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourcesRequest {
    #[serde(rename = "method")]
    method: &'static str,
    pub params: PaginatedRequestParams,
}
impl ListResourcesRequest {
    pub fn new(params: PaginatedRequestParams) -> Self {
        Self {
            method: "resources/list",
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsRequest {
    #[serde(rename = "method")]
    method: &'static str,
    pub params: PaginatedRequestParams,
}
impl ListToolsRequest {
    pub fn new(params: PaginatedRequestParams) -> Self {
        Self {
            method: "tools/list",
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    pub arguments: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    #[serde(rename = "method")]
    method: &'static str,
    pub params: ToolCallParams,
}
impl ToolCallRequest {
    pub fn new(params: ToolCallParams) -> Self {
        Self {
            method: "tool/call",
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourcesResult {
    pub resources: Vec<Resource>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: Url,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotificationParams {
    pub progress_token: String,
    pub progress: f64,
    pub total: Option<f64>,
    pub message: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingMessageNotificationParams {
    pub level: LoggingLevel,
    pub logger: Option<String>,
    pub data: serde_json::Value,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoggingLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdatedNotificationParams {
    pub uri: Url,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelledNotificationParams {
    pub request_id: String,
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implementation_fields() {
        let imp = Implementation {
            name: "test-sdk".to_string(),
            version: "0.1.0".to_string(),
        };
        assert_eq!(imp.name, "test-sdk");
        assert_eq!(imp.version, "0.1.0");
    }
}
