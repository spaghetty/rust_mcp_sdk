//! Defines the main `Server` struct and its builder API for setting up handlers.

use super::session::{ConnectionHandle, ServerSession};
use crate::{
    protocol::ProtocolConnection,
    types::{
        CallToolResult, GetPromptResult, ListPromptsResult, ReadResourceResult, Resource, Tool,
    },
    TcpAdapter,
};
use anyhow::Result;
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpListener;

// --- Handler Type Definitions ---
pub(crate) type ListToolsHandler = Arc<
    dyn Fn(ConnectionHandle) -> Pin<Box<dyn Future<Output = Result<Vec<Tool>>> + Send>>
        + Send
        + Sync,
>;
pub(crate) type CallToolHandler = Arc<
    dyn Fn(
            ConnectionHandle,
            String,
            Value,
        ) -> Pin<Box<dyn Future<Output = Result<CallToolResult>> + Send>>
        + Send
        + Sync,
>;
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
// NEW: Add handlers for prompts
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

/// A high-level server for handling MCP requests.
#[derive(Default)]
pub struct Server {
    pub(crate) name: String,
    pub(crate) list_tools_handler: Option<ListToolsHandler>,
    pub(crate) call_tool_handler: Option<CallToolHandler>,
    pub(crate) list_resources_handler: Option<ListResourcesHandler>,
    pub(crate) read_resource_handler: Option<ReadResourceHandler>,
    // NEW: Add handler fields for prompts
    pub(crate) list_prompts_handler: Option<ListPromptsHandler>,
    pub(crate) get_prompt_handler: Option<GetPromptHandler>,
}

impl Server {
    /// Creates a new `Server`.
    pub fn new(name: &str) -> Self {
        Server {
            name: name.to_string(),
            ..Default::default()
        }
    }

    // --- Builder Methods ---

    pub fn on_list_tools<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Tool>>> + Send + 'static,
    {
        self.list_tools_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    pub fn on_call_tool<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String, Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<CallToolResult>> + Send + 'static,
    {
        self.call_tool_handler = Some(Arc::new(move |handle, name, args| {
            Box::pin(handler(handle, name, args))
        }));
        self
    }

    pub fn on_list_resources<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<Resource>>> + Send + 'static,
    {
        self.list_resources_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

    pub fn on_read_resource<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ReadResourceResult>> + Send + 'static,
    {
        self.read_resource_handler =
            Some(Arc::new(move |handle, uri| Box::pin(handler(handle, uri))));
        self
    }

    // NEW: Add builder methods for prompts
    pub fn on_list_prompts<F, Fut>(mut self, handler: F) -> Self
    where
        F: Fn(ConnectionHandle) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ListPromptsResult>> + Send + 'static,
    {
        self.list_prompts_handler = Some(Arc::new(move |handle| Box::pin(handler(handle))));
        self
    }

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

    /// Starts the server and listens for incoming TCP connections.
    pub async fn listen(self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("[Server] Listening on {}", addr);
        let server = Arc::new(self);

        loop {
            let (stream, client_addr) = listener.accept().await?;
            println!("[Server] Accepted connection from: {}", client_addr);
            let server_clone = Arc::clone(&server);

            tokio::spawn(async move {
                let adapter = TcpAdapter::new(stream);
                let conn = ProtocolConnection::new(adapter);
                let session = ServerSession::new(conn, server_clone);

                if let Err(e) = session.run().await {
                    eprintln!("[Server] Session failed for {}: {}", client_addr, e);
                } else {
                    println!("[Server] Session with {} closed gracefully.", client_addr);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CallToolResult, ListPromptsResult};

    #[tokio::test]
    async fn test_handler_registration() {
        let server = Server::new("test")
            .on_list_tools(|_| async { Ok(vec![]) })
            .on_call_tool(|_, _, _| async { Ok(CallToolResult::default()) })
            .on_list_prompts(|_| async { Ok(ListPromptsResult { prompts: vec![] }) });
        assert!(server.list_tools_handler.is_some());
        assert!(server.call_tool_handler.is_some());
        assert!(server.list_prompts_handler.is_some());
    }
}
