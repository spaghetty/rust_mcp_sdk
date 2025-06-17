//! Contains all the core data structures and types for the Model Context Protocol (MCP).
//!
//! These types are Rust translations of the Pydantic models from the Python SDK,
//! designed to be serialized to and deserialized from JSON according to the MCP specification.
//! We use the `serde` library for robust and efficient JSON handling.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Protocol Version ---
pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";

// --- Core Public API Types ---

/// Definition for a tool the client can call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ToolAnnotations>,
}

impl Default for Tool {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            input_schema: Value::Object(Default::default()),
            annotations: None,
        }
    }
}

/// A known resource that the server is capable of reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

// --- NEW: Prompt-related types ---

/// A prompt or prompt template that the server offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// An argument for a prompt template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

/// Describes a message returned as part of a prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String, // "user" or "assistant"
    pub content: Content,
}

// --- Result Types ---

/// The server's response to a `tools/call` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(default)]
    pub is_error: bool,
}

/// The server's response to a `resources/read` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceResult {
    pub contents: Vec<ResourceContents>,
}

/// The server's response to a `prompts/list` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListPromptsResult {
    pub prompts: Vec<Prompt>,
}

/// The server's response to a `prompts/get` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

// --- Content and Resource Types ---

// serialization and deserialization, removing the need for separate structs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum Content {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        resource: ResourceContents,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceContents {
    Text(TextResourceContents),
    Blob(BlobResourceContents),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContents {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub blob: String,
}

// --- Annotation and Metadata Types ---

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
}

// --- Foundational JSON-RPC Types ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request<T> {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    pub params: T,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response<T> {
    pub jsonrpc: String,
    pub id: RequestId,
    pub result: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Num(i64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification<T> {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub params: Option<T>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JSONRPCResponse<T> {
    Success(Response<T>),
    Error(ErrorResponse),
}

// --- JSON-RPC Error Types ---
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    pub error: ErrorData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: i32,
    pub message: String,
}

// --- Initialization Handshake Types ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequestParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: Implementation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: Implementation,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

// --- Method-Specific Parameter Types ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsParams {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolParams {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesParams {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceParams {
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsParams {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// Parameters for the `tools/listChanged` notification. Currently empty.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsChangedParams {}

// --- Unit Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_roundtrip() {
        let tool = Tool {
            name: "fetch".to_string(),
            description: Some("Fetches a website".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
            }),
            annotations: Some(ToolAnnotations {
                read_only_hint: Some(true),
                ..Default::default()
            }),
        };
        let json_string = serde_json::to_string(&tool).unwrap();
        let deserialized: Tool = serde_json::from_str(&json_string).unwrap();
        assert_eq!(tool, deserialized);
    }

    #[test]
    fn test_prompt_roundtrip() {
        let prompt = Prompt {
            name: "test-prompt".to_string(),
            description: Some("A test prompt".to_string()),
            arguments: Some(vec![PromptArgument {
                name: "arg1".to_string(),
                description: Some("An argument".to_string()),
                required: Some(true),
            }]),
        };
        let json_string = serde_json::to_string(&prompt).unwrap();
        let deserialized: Prompt = serde_json::from_str(&json_string).unwrap();
        assert_eq!(prompt, deserialized);
    }

    #[test]
    fn test_get_prompt_result_roundtrip() {
        let result = GetPromptResult {
            description: Some("A test prompt".to_string()),
            messages: vec![
                PromptMessage {
                    role: "user".to_string(),
                    content: Content::Text {
                        text: "Hello".to_string(),
                    },
                },
                PromptMessage {
                    role: "assistant".to_string(),
                    content: Content::Image {
                        data: "base64data".to_string(),
                        mime_type: "image/png".to_string(),
                    },
                },
            ],
        };

        let json_string = serde_json::to_string(&result).unwrap();
        let deserialized: GetPromptResult = serde_json::from_str(&json_string).unwrap();
        assert_eq!(result, deserialized);

        // Also check the raw JSON
        let value: Value = serde_json::from_str(&json_string).unwrap();
        assert_eq!(value["messages"][0]["content"]["type"], "text");
        assert_eq!(value["messages"][1]["content"]["type"], "image");
        assert_eq!(value["messages"][1]["content"]["mimeType"], "image/png");
    }

    #[test]
    fn test_resource_roundtrip() {
        let resource = Resource {
            name: "My File".to_string(),
            uri: "file:///path/to/file.txt".to_string(),
            description: Some("A test file".to_string()),
            mime_type: Some("text/plain".to_string()),
        };
        let json_string = serde_json::to_string(&resource).unwrap();
        let deserialized: Resource = serde_json::from_str(&json_string).unwrap();
        assert_eq!(resource, deserialized);
    }

    #[test]
    fn test_read_resource_result_roundtrip() {
        let result = ReadResourceResult {
            contents: vec![
                ResourceContents::Text(TextResourceContents {
                    uri: "file:///doc.txt".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: "Hello".to_string(),
                }),
                ResourceContents::Blob(BlobResourceContents {
                    uri: "file:///img.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    blob: "base64data".to_string(),
                }),
            ],
        };
        let json_string = serde_json::to_string(&result).unwrap();
        let deserialized: ReadResourceResult = serde_json::from_str(&json_string).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_notification_deserialization() {
        let notif_json = r#"
        {
            "jsonrpc": "2.0",
            "method": "notifications/tools/list_changed",
            "params": {}
        }
        "#;
        let notif: Notification<ListToolsChangedParams> = serde_json::from_str(notif_json).unwrap();
        assert_eq!(notif.method, "notifications/tools/list_changed");
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let success_json = r#"
        {
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "status": "ok" }
        }
        "#;
        let response: JSONRPCResponse<Value> = serde_json::from_str(success_json).unwrap();
        match response {
            JSONRPCResponse::Success(s) => {
                assert_eq!(s.id, RequestId::Num(1));
                assert_eq!(s.result, json!({ "status": "ok" }));
            }
            JSONRPCResponse::Error(_) => panic!("Expected success response"),
        }
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let error_json = r#"
        {
            "jsonrpc": "2.0",
            "id": 2,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }
        "#;
        let response: JSONRPCResponse<Value> = serde_json::from_str(error_json).unwrap();
        match response {
            JSONRPCResponse::Success(_) => panic!("Expected error response"),
            JSONRPCResponse::Error(e) => {
                assert_eq!(e.id, RequestId::Num(2));
                assert_eq!(e.error.code, -32601);
                assert_eq!(e.error.message, "Method not found");
            }
        }
    }
}
