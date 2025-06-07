//! Contains all the core data structures and types for the Model Context Protocol (MCP).
//!
//! These types are Rust translations of the Pydantic models from the Python SDK,
//! designed to be serialized to and deserialized from JSON according to the MCP specification.
//! We use the `serde` library for robust and efficient JSON handling.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Protocol Version ---
pub const LATEST_PROTOCOL_VERSION: &str = "2025-03-26";

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

/// The server's response to a `tools/call` request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<Content>,
    #[serde(default)]
    pub is_error: bool,
}

// --- Content and Resource Types ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(TextContent),
    Image(ImageContent),
    EmbeddedResource(EmbeddedResource),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextContent {
    pub r#type: String, // "text"
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    pub r#type: String, // "image"
    pub data: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedResource {
    pub r#type: String, // "resource"
    pub resource: ResourceContents,
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

// --- JSON-RPC Error Types ---
pub const METHOD_NOT_FOUND: i32 = -32601;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JSONRPCResponse<T> {
    Success(Response<T>),
    Error(ErrorResponse),
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
    // We can add fields here as we support more capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    // We can add fields here as we support more capabilities.
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

// --- Unit Tests ---
// Ensures that our Rust types correctly serialize to and deserialize from the JSON format.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Tests the serialization and deserialization of the `Tool` struct.
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

        // Serialize to JSON string
        let json_string = serde_json::to_string(&tool).unwrap();

        // Deserialize back to a Tool struct
        let deserialized_tool: Tool = serde_json::from_str(&json_string).unwrap();

        // Assert that the deserialized version is identical to the original
        assert_eq!(tool, deserialized_tool);
    }

    /// Tests the serialization and deserialization of a `CallToolResult`.
    #[test]
    fn test_call_tool_result_roundtrip() {
        let result = CallToolResult {
            content: vec![Content::Text(TextContent {
                r#type: "text".to_string(),
                text: "Hello, world!".to_string(),
            })],
            is_error: false,
        };

        let json_string = serde_json::to_string(&result).unwrap();
        let deserialized_result: CallToolResult = serde_json::from_str(&json_string).unwrap();

        assert_eq!(result, deserialized_result);
    }

    /// Tests a full JSON-RPC request roundtrip.
    #[test]
    fn test_json_rpc_request_roundtrip() {
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Num(1),
            method: "tools/call".to_string(),
            params: CallToolParams {
                name: "fetch".to_string(),
                arguments: json!({"url": "https://example.com"}),
            },
        };

        let json_string = serde_json::to_string(&request).unwrap();
        let deserialized_request: Request<CallToolParams> =
            serde_json::from_str(&json_string).unwrap();

        assert_eq!(request, deserialized_request);
    }

    /// Verifies that deserialization handles camelCase correctly.
    #[test]
    fn test_camel_case_deserialization() {
        let json_data = r#"
        {
            "name": "my-tool",
            "inputSchema": { "type": "string" },
            "annotations": {
                "readOnlyHint": true
            }
        }
        "#;

        let tool: Tool = serde_json::from_str(json_data).unwrap();

        assert_eq!(tool.name, "my-tool");
        assert_eq!(tool.annotations.unwrap().read_only_hint, Some(true));
    }

    /// Tests deserialization of a successful JSON-RPC response.
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

    /// Tests deserialization of a JSON-RPC error response.
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
