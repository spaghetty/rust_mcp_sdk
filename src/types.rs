//! Contains all the core data structures and types for the Model Context Protocol (MCP).
//!
//! These types are Rust translations of the Pydantic models from the Python SDK,
//! designed to be serialized to and deserialized from JSON according to the MCP specification.
//! We use the `serde` library for robust and efficient JSON handling.

use crate::ToolArgumentsDescriptor;
use serde::{Deserialize, Serialize};
use serde_json::Value; // Removed json here, as it's not used in this file anymore

// --- Base MCP Message Trait ---
/// A trait for all MCP messages that have a `method` field.
pub trait MCPMessage {
    fn method(&self) -> &str;
}

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
            input_schema: Value::Null, // Changed from Value::Object(Default::default())
            annotations: None,
        }
    }
}

impl Tool {
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(|s| s.into()),
            input_schema,
            annotations: None, // Defaulting annotations to None
        }
    }

    /// Creates a new `Tool` instance, automatically deriving its `input_schema`
    /// from a type `T` that implements the `ToolArgumentsDescriptor` trait.
    ///
    /// This is the preferred way to create `Tool` instances when using strongly-typed
    /// arguments, as it ensures consistency between the tool's advertised schema
    /// and the actual Rust types used by its handler.
    ///
    /// # Type Parameters
    ///
    /// * `T`: A type that implements `ToolArgumentsDescriptor`. This is typically
    ///   achieved by deriving `#[derive(ToolArguments)]` on a struct.
    ///
    /// # Arguments
    ///
    /// * `name`: The programmatic name of the tool (e.g., "get_weather"). This should
    ///   be unique within the server's toolset.
    /// * `description`: An optional human-readable description of what the tool does.
    ///   This can be used by clients to understand the tool's purpose.
    ///
    /// # Returns
    ///
    /// A new `Tool` instance with its `input_schema` populated by calling
    /// `T::mcp_input_schema()`. The `annotations` field is defaulted to `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mcp_sdk::types::Tool;
    /// use mcp_sdk::{ToolArguments, ToolArgumentsDescriptor}; // For derive and trait
    /// use serde::Deserialize; // For use with typed handlers
    /// use serde_json::json;
    ///
    /// #[derive(ToolArguments, Deserialize)]
    /// struct WeatherArgs {
    ///     #[tool_arg(desc = "The city for which to get the weather.")]
    ///     city: String,
    ///     unit: Option<String>, // e.g., "celsius" or "fahrenheit"
    /// }
    ///
    /// let weather_tool = Tool::from_args::<WeatherArgs>(
    ///     "get_weather",
    ///     Some("Fetches the current weather for a specified city.")
    /// );
    ///
    /// assert_eq!(weather_tool.name, "get_weather");
    /// assert!(weather_tool.input_schema["properties"].get("city").is_some());
    /// ```
    pub fn from_args<T: ToolArgumentsDescriptor>(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(|s| s.into()),
            input_schema: T::mcp_input_schema(),
            annotations: None, // Defaulting annotations to None
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

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

impl<T> MCPMessage for Request<T> {
    fn method(&self) -> &str {
        &self.method
    }
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

impl<T> MCPMessage for Notification<T> {
    fn method(&self) -> &str {
        &self.method
    }
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
    use crate::ToolArgumentsDescriptor;
    use serde_json::json; // Ensure json macro is available for all tests in this module // For test_tool_from_args

    // Moved tests from outside the module into here
    #[test]
    fn test_mcp_message_trait_request_moved() {
        // Renamed to avoid conflict if original is not removed by diff
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Num(1),
            method: "test/method".to_string(),
            params: CallToolParams {
                name: "test_tool".to_string(),
                arguments: json!({}),
            },
        };
        assert_eq!(request.method(), "test/method");
    }

    #[test]
    fn test_mcp_message_trait_notification_moved() {
        let notification = Notification::<ListToolsChangedParams> {
            jsonrpc: "2.0".to_string(),
            method: "test/notification".to_string(),
            params: None,
        };
        assert_eq!(notification.method(), "test/notification");
    }

    #[test]
    fn test_tool_new_helper_moved() {
        let tool_name = "my_tool";
        let tool_desc = "A description for my tool.";
        let input_schema = json!({"type": "string"});

        let tool1 = Tool::new(tool_name, Some(tool_desc), input_schema.clone());
        assert_eq!(tool1.name, tool_name);
        assert_eq!(tool1.description, Some(tool_desc.to_string()));
        assert_eq!(tool1.input_schema, input_schema);
        assert!(tool1.annotations.is_none());

        let tool2 = Tool::new("another_tool", None::<String>, json!({}));
        assert_eq!(tool2.name, "another_tool");
        assert!(tool2.description.is_none());
        assert_eq!(tool2.input_schema, json!({}));
        assert!(tool2.annotations.is_none());
    }

    #[test]
    fn test_tool_default_input_schema_moved() {
        let default_tool = Tool::default();
        assert_eq!(default_tool.input_schema, Value::Null);
    }

    // Dummy struct for testing Tool::from_args
    struct MyTestArgs;
    impl ToolArgumentsDescriptor for MyTestArgs {
        fn mcp_input_schema() -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "test_prop": { "type": "string" }
                },
                "required": ["test_prop"]
            })
        }
    }

    #[test]
    fn test_tool_from_args() {
        let tool = Tool::from_args::<MyTestArgs>(
            "my_test_tool_from_args",
            Some("A tool created with from_args."),
        );

        assert_eq!(tool.name, "my_test_tool_from_args");
        assert_eq!(
            tool.description,
            Some("A tool created with from_args.".to_string())
        );

        let expected_schema = json!({
            "type": "object",
            "properties": {
                "test_prop": { "type": "string" }
            },
            "required": ["test_prop"]
        });
        assert_eq!(tool.input_schema, expected_schema);
        assert!(tool.annotations.is_none());
    }

    // Original tests continue from here
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

// Ensure the loose tests are removed if they were not part of the SEARCH block
// This diff assumes the loose tests were exactly as previously generated.
// If their content was different, they might not be removed by this diff alone.
