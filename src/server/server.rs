//! Defines the main `Server` struct and its builder API for setting up handlers.

use super::session::{ConnectionHandle, ServerSession};
use crate::{
    error::Result,
    network_adapter::NetworkAdapter,
    protocol::ProtocolConnection,
    types::{
        CallToolResult, GetPromptResult, ListPromptsResult, ReadResourceResult, Resource, Tool,
    },
};
use serde_json::Value;
use std::collections::HashMap;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tracing::{error, info};

use crate::types::Content; // Added for error reporting in typed handlers
use serde::de::DeserializeOwned; // For register_tool_typed

// Type alias for the boxed future returned by handlers
type BoxedFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

// The internal representation of a tool handler
#[allow(clippy::type_complexity)] // To allow complex Boxed dyn Fn types
pub(crate) enum ToolHandler {
    /// Handler for tools registered with raw JSON Value arguments
    Untyped(
        Box<
            dyn Fn(ConnectionHandle, Arc<Value>) -> BoxedFuture<Result<CallToolResult>>
                + Send
                + Sync,
        >,
    ),

    /// Handler for tools registered with strongly-typed arguments.
    Typed(
        Box<
            dyn Fn(ConnectionHandle, Arc<Value>) -> BoxedFuture<Result<CallToolResult>>
                + Send
                + Sync,
        >,
    ),
}

// --- Handler Type Definitions ---
// The old ToolHandler type alias is replaced by the enum above.

pub(crate) type ListResourcesHandler = Arc<
    dyn Fn(ConnectionHandle) -> Pin<Box<dyn Future<Output = Result<Vec<Resource>>> + Send>>
        + Send
        + Sync,
>;
pub(crate) type ReadResourceHandler = Arc<
    dyn Fn(
            ConnectionHandle,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<ReadResourceResult>> + Send>>
        + Send
        + Sync,
>;
pub(crate) type ListPromptsHandler = Arc<
    dyn Fn(ConnectionHandle) -> Pin<Box<dyn Future<Output = Result<ListPromptsResult>> + Send>>
        + Send
        + Sync,
>;
pub(crate) type GetPromptHandler = Arc<
    dyn Fn(
            ConnectionHandle,
            String,
            Option<Value>,
        ) -> Pin<Box<dyn Future<Output = Result<GetPromptResult>> + Send>>
        + Send
        + Sync,
>;

/// A high-level, asynchronous server for handling MCP requests.
///
/// This struct uses a builder pattern to register handlers for different MCP methods.
/// After configuration, the [`Self::tcp_listen`] method is called to start the server.
/// The server will then listen for incoming TCP connections, spawning a new
/// asynchronous task for each client to handle them concurrently.
///
/// # Example
///
/// ```no_run
/// use mcp_sdk::server::{ConnectionHandle, Server};
/// use mcp_sdk::types::{Tool, CallToolResult, Content};
/// use mcp_sdk::network_adapter::NdjsonAdapter;
/// use mcp_sdk::Result;
/// use serde_json::Value;
///
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let server = Server::new("test").register_tool(
///         Tool {
///             name: "test-tool".to_string(),
///             ..Default::default()
///         },
///         |_handle, _args| async {
///             Ok(CallToolResult {
///                 content: vec![Content::Text {
///                     text: "Success!".to_string(),
///                 }],
///                 is_error: false,
///             })
///         },
///     );
///
///     // This runs forever, handling connections until the process is stopped.
///     server.tcp_listen::<NdjsonAdapter>("127.0.0.1:8080").await?;
///
///     Ok(())
/// }
/// ```
#[derive(Default, Clone)]
pub struct Server {
    pub(crate) name: String,
    // Consolidated tools and handlers: tool_name -> (Tool_metadata, Arc_to_handler_enum)
    pub(crate) tools_and_handlers: HashMap<String, (Tool, Arc<ToolHandler>)>,
    pub(crate) list_resources_handler: Option<ListResourcesHandler>,
    pub(crate) read_resource_handler: Option<ReadResourceHandler>,
    pub(crate) list_prompts_handler: Option<ListPromptsHandler>,
    pub(crate) get_prompt_handler: Option<GetPromptHandler>,
}

impl Server {
    /// Creates a new `Server` builder.
    ///
    /// # Arguments
    ///
    /// * `name` - A name for the server implementation, e.g., "my-tool-server". This
    ///   is sent to the client during the initialization handshake.
    pub fn new(name: &str) -> Self {
        Server {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Registers a tool, its metadata, and its execution handler at the same time.
    pub fn register_tool<F, Fut>(mut self, tool: Tool, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CallToolResult>> + Send + 'static,
    {
        let tool_name = tool.name.clone();
        let handler_arc = Arc::new(ToolHandler::Untyped(Box::new(
            move |conn_handle, json_args_arc| {
                // The original handler expects Value, not Arc<Value>.
                // We clone from Arc if the original handler needs ownership,
                // or pass a reference if it can work with &Value.
                // For now, let's assume the original handler took Value by value.
                // The public API of register_tool took `Value`, not `Arc<Value>`.
                // So, the closure it stores needs to match the new internal ToolHandler::Untyped signature.
                // This means the outer Box<dyn Fn...> for Untyped should match the new signature.
                // The handler passed to `register_tool` is `F: Fn(ConnectionHandle, Value) -> Fut`
                // The new Untyped signature is `Fn(ConnectionHandle, Arc<Value>) -> BoxedFuture<...>`
                // So we need to adapt:
                let value_clone = (*json_args_arc).clone(); // Clone Value from Arc<Value>
                Box::pin(handler(conn_handle, value_clone))
            },
        )));
        self.tools_and_handlers
            .insert(tool_name, (tool, handler_arc));
        self
    }

    /// Registers a tool with a handler that accepts strongly-typed arguments.
    ///
    /// This method is preferred for new tool implementations as it provides better
    /// type safety and ergonomics compared to `register_tool`, which uses raw
    /// `serde_json::Value` for arguments.
    ///
    /// The `tool` definition, including its `input_schema`, should be created using
    /// `Tool::from_args<Args>()`, where `Args` is the struct defining the arguments
    /// for this tool. The `Args` struct must derive `mcp_sdk::ToolArguments` (which
    /// provides the schema) and `serde::Deserialize` (to allow the server to
    /// deserialize the incoming JSON arguments into the struct).
    ///
    /// # Type Parameters
    ///
    /// * `Args`: The struct type representing the arguments for this tool. It must
    ///   implement `serde::de::DeserializeOwned` and be `Send + Sync + 'static`.
    ///   Typically, this struct will also derive `mcp_sdk::ToolArguments`.
    /// * `Fut`: The type of future returned by the handler, which resolves to
    ///   `Result<CallToolResult>`.
    /// * `F`: The type of the handler closure. It takes a `ConnectionHandle` and an
    ///   instance of `Args`, and returns `Fut`.
    ///
    /// # Arguments
    ///
    /// * `tool`: The `Tool` metadata. The `input_schema` of this tool should match
    ///   the schema generated from `Args`. Using `Tool::from_args::<Args>(...)`
    ///   ensures this.
    /// * `handler`: An asynchronous function or closure that will be called when the
    ///   tool is invoked. It receives a `ConnectionHandle` (for sending notifications)
    ///   and the deserialized, typed arguments of type `Args`.
    ///
    /// # Panics
    ///
    /// This method internally handles argument deserialization. If deserialization fails
    /// (e.g., due to missing required fields or type mismatches in the client's request),
    /// an error `CallToolResult` is automatically generated and sent to the client.
    /// The provided `handler` will not be called in such cases.
    ///
    /// # Example
    ///
    /// ```rust
    /// use mcp_sdk::server::{Server, ConnectionHandle};
    /// use mcp_sdk::types::{Tool, CallToolResult, Content};
    /// use mcp_sdk::{Result as SdkResult, ToolArguments};
    /// use serde::Deserialize;
    ///
    /// #[derive(ToolArguments, Deserialize)]
    /// struct MyTypedToolArgs {
    ///     #[tool_arg(desc = "A message to echo.")]
    ///     message: String,
    ///     repeat: Option<i32>,
    /// }
    ///
    /// async fn my_typed_handler(
    ///     _handle: ConnectionHandle,
    ///     args: MyTypedToolArgs,
    /// ) -> SdkResult<CallToolResult> {
    ///     let repeated_message = args.message.repeat(args.repeat.unwrap_or(1) as usize);
    ///     Ok(CallToolResult {
    ///         content: vec![Content::Text { text: repeated_message }],
    ///         is_error: false,
    ///     })
    /// }
    ///
    /// let server = Server::new("my-server")
    ///     .register_tool_typed(
    ///         Tool::from_args::<MyTypedToolArgs>("echo", Some("Echoes a message.")),
    ///         my_typed_handler
    ///     );
    /// // Server is now ready to listen for connections and handle "echo" tool calls.
    /// ```
    pub fn register_tool_typed<Args, Fut, F>(mut self, tool: Tool, handler: F) -> Self
    where
        Args: DeserializeOwned + Send + Sync + 'static,
        Fut: Future<Output = Result<CallToolResult>> + Send + 'static,
        F: Fn(ConnectionHandle, Args) -> Fut + Send + Sync + 'static,
    {
        let user_handler_arc = Arc::new(handler);
        let tool_name_clone_for_error = tool.name.clone(); // For error messages
        let tool_input_schema_clone_for_error = tool.input_schema.clone(); // For error messages

        let wrapped_handler = Arc::new(ToolHandler::Typed(Box::new(
            move |conn_handle: ConnectionHandle, json_args: Arc<Value>| {
                let user_handler = Arc::clone(&user_handler_arc);
                let tool_name = tool_name_clone_for_error.clone();
                let input_schema = tool_input_schema_clone_for_error.clone();

                Box::pin(async move {
                    match serde_json::from_value::<Args>((*json_args).clone()) {
                        Ok(typed_args) => (user_handler)(conn_handle, typed_args).await,
                        Err(e) => {
                            error!(tool_name = %tool_name, error = %e, "Failed to deserialize arguments for tool");
                            Ok(CallToolResult {
                                content: vec![Content::Text {
                                    text: format!(
                                        "Invalid arguments for tool '{}': {}. Expected schema: {}",
                                        tool_name,
                                        e,
                                        serde_json::to_string_pretty(&input_schema)
                                            .unwrap_or_default()
                                    ),
                                }],
                                is_error: true,
                            })
                        }
                    }
                })
            },
        )));

        self.tools_and_handlers
            .insert(tool.name.clone(), (tool, wrapped_handler));
        self
    }

    /// Registers a handler for the `resources/list` request.
    pub fn on_list_resources<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Resource>>> + Send + 'static,
    {
        self.list_resources_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    /// Registers a handler for the `resources/read` request.
    pub fn on_read_resource<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult>> + Send + 'static,
    {
        self.read_resource_handler =
            Some(Arc::new(move |handle, uri| Box::pin(handler(handle, uri))));
        self
    }

    /// Registers a handler for the `prompts/list` request.
    pub fn on_list_prompts<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ListPromptsResult>> + Send + 'static,
    {
        self.list_prompts_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    /// Registers a handler for the `prompts/get` request.
    pub fn on_get_prompt<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String, Option<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<GetPromptResult>> + Send + 'static,
    {
        self.get_prompt_handler = Some(Arc::new(move |handle, name, args| {
            Box::pin(handler(handle, name, args))
        }));
        self
    }
    /// Takes a single, pre-existing network adapter and runs a session for it.
    /// This is the core logic block used by both `serve` and `tcp_listen`.
    pub async fn handle_connection<A>(&self, adapter: A) -> Result<()>
    where
        A: NetworkAdapter + 'static,
    {
        let conn = ProtocolConnection::new(adapter);
        let session = ServerSession::new(conn, Arc::new(self.clone()));
        session.run().await
    }

    /// Starts the TCP listener and enters the main server loop.
    ///
    /// This method binds a TCP listener to the given address. For each incoming
    /// client connection, it spawns a new asynchronous task to handle that
    /// connection's entire lifecycle, allowing the server to handle multiple
    /// clients concurrently.
    ///
    /// This method runs indefinitely until the process is terminated or an
    /// unrecoverable error occurs.
    ///
    /// # Arguments
    ///
    /// * `addr` - The network address to listen on (e.g., "127.0.0.1:8080").
    ///
    /// # Errors
    ///
    /// This function will return an error if the server fails to bind the TCP
    /// listener to the specified address. This can happen if the port is already
    /// in use or if the application lacks the necessary permissions to bind to
    pub async fn tcp_listen<A>(self, addr: &str) -> Result<()>
    where
        A: NetworkAdapter + From<TcpStream> + 'static,
    {
        let listener = TcpListener::bind(addr).await?;
        info!("[Server] Listening on {}", addr);
        let server = Arc::new(self);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            info!("[Server] Accepted connection from: {}", client_addr);
            let server_clone = Arc::clone(&server);

            tokio::spawn(async move {
                let adapter = A::from(stream);
                if let Err(e) = server_clone.handle_connection(adapter).await {
                    error!("[Server] Session failed for {}: {}", client_addr, e);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CallToolResult, ListPromptsResult};
    use serde_json::json;

    #[tokio::test]
    async fn test_handler_registration() {
        // 1. Setup
        // Create a dummy tool definition
        let dummy_tool = Tool {
            name: "my-test-tool".to_string(),
            description: Some("A tool for testing.".to_string()),
            input_schema: json!({ "type": "object" }),
            annotations: None,
        };

        // Create a dummy handler
        let dummy_handler =
            |_handle: ConnectionHandle, _args: Value| async { Ok(CallToolResult::default()) };
        // 2. Action
        // Create a server and register the tool
        let server = Server::new("test-server")
            .register_tool(dummy_tool.clone(), dummy_handler)
            .on_list_prompts(|_| async { Ok(ListPromptsResult { prompts: vec![] }) });

        assert_eq!(server.tools_and_handlers.len(), 1);
        assert!(server.tools_and_handlers.contains_key("my-test-tool"));
        let (registered_tool, _handler) = server.tools_and_handlers.get("my-test-tool").unwrap();
        assert_eq!(registered_tool.name, dummy_tool.name);

        assert!(server.list_prompts_handler.is_some());
    }
}
