//! Defines the `ClientSessionGroup` for managing multiple client connections.

use crate::client::Client;
use crate::error::Result;
use crate::types::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages connections to multiple MCP servers simultaneously.
///
/// This struct is designed for client applications that need to connect to more
/// than one MCP server at a time. It provides a convenient way to add, remove,
/// and interact with a collection of `Client` sessions, and to aggregate
/// data (like tools or prompts) from all of them concurrently.
///
/// # Example
///
/// ```no_run
/// use mcp_sdk::client::ClientSessionGroup;
/// use mcp_sdk::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let group = ClientSessionGroup::new();
///
///     // Connect to two different servers
///     group.add_client("127.0.0.1:8081").await?;
///     group.add_client("127.0.0.1:8082").await?;
///
///     // Aggregate all tools from all connected servers
///     let all_tools = group.list_tools_all().await?;
///     println!("All available tools: {:?}", all_tools);
///
///     // The group will automatically clean up connections when it is dropped.
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct ClientSessionGroup {
    sessions: Arc<RwLock<HashMap<String, Arc<Client>>>>,
}

impl ClientSessionGroup {
    /// Creates a new, empty `ClientSessionGroup`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new client to the group by connecting to the given server address.
    ///
    /// This method will establish a new connection and perform the MCP handshake.
    /// If the connection is successful, the new `Client` session is added to the group,
    /// keyed by its server address.
    ///
    /// # Arguments
    ///
    /// * `addr` - The network address of the MCP server (e.g., "127.0.0.1:8080").
    ///
    /// # Errors
    ///
    /// This function will return an error if the connection or handshake fails.
    pub async fn add_client(&self, addr: &str) -> Result<()> {
        let client = Client::connect(addr).await?;
        let mut sessions = self.sessions.write().await;
        sessions.insert(addr.to_string(), Arc::new(client));
        Ok(())
    }

    /// Removes a client from the group by its server address.
    ///
    /// When the client is removed, its connection will be gracefully terminated
    /// by the `Drop` implementation on the `Client` struct.
    ///
    /// # Arguments
    ///
    /// * `addr` - The network address of the client session to remove.
    pub async fn remove_client(&self, addr: &str) {
        let mut sessions = self.sessions.write().await;
        // When the Arc<Client> is dropped, the Client's Drop impl will
        // abort its background connection task.
        sessions.remove(addr);
    }

    /// Fetches a list of all tools from all connected servers and aggregates them.
    ///
    /// This method demonstrates how to dispatch a request to multiple clients
    /// concurrently and combine their results into a single list.
    pub async fn list_tools_all(&self) -> Result<Vec<Tool>> {
        let mut all_tools = Vec::new();
        let mut join_handles = Vec::new();

        let sessions = self.sessions.read().await;

        for client in sessions.values() {
            let client_clone = Arc::clone(client);
            let handle = tokio::spawn(async move { client_clone.list_tools().await });
            join_handles.push(handle);
        }

        // Wait for all the concurrent `list_tools` calls to complete.
        for handle in join_handles {
            match handle.await.unwrap() {
                Ok(tools) => {
                    all_tools.extend(tools);
                }
                Err(e) => {
                    // In a real application, you might want more sophisticated error handling,
                    // like collecting errors or logging them without failing the whole operation.
                    eprintln!("Failed to fetch tools from a server: {}", e);
                }
            }
        }

        Ok(all_tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        protocol::ProtocolConnection,
        server::{session::ServerSession, ConnectionHandle, Server},
        types::Tool,
        TcpAdapter,
    };
    use serde_json::json;
    use std::time::Duration;
    use tokio::{net::TcpListener, task::JoinHandle};

    /// CORRECTED: This test helper is now robust against race conditions.
    /// It creates a real TCP listener and runs an accept loop in the background,
    /// ensuring the server is ready before the test function proceeds.
    async fn setup_mock_server(tool_name: &'static str) -> (String, JoinHandle<()>) {
        let server = Arc::new(Server::new("mock-server").on_list_tools(
            move |_handle: ConnectionHandle| {
                let tool = Tool {
                    name: tool_name.to_string(),
                    description: Some("A mock tool".to_string()),
                    input_schema: json!({}),
                    annotations: None,
                };
                async { Ok(vec![tool]) }
            },
        ));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap().to_string();

        let handle = tokio::spawn(async move {
            loop {
                // This select makes the loop cancellable when the JoinHandle is aborted.
                tokio::select! {
                    res = listener.accept() => {
                        if let Ok((stream, _)) = res {
                            let server_clone = Arc::clone(&server);
                            tokio::spawn(async move {
                                let session = ServerSession::new(
                                    ProtocolConnection::new(TcpAdapter::new(stream)),
                                    server_clone,
                                );
                                if let Err(e) = session.run().await {
                                    if !e.to_string().contains("reset by peer") {
                                        eprintln!("[Test Server] Session error: {}", e);
                                    }
                                }
                            });
                        } else {
                            break; // Error accepting, break loop
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                }
            }
        });

        (server_addr, handle)
    }

    #[tokio::test]
    async fn test_add_and_list_all() {
        let (server1_addr, _server1_handle) = setup_mock_server("tool-from-server-1").await;
        let (server2_addr, _server2_handle) = setup_mock_server("tool-from-server-2").await;

        let group = ClientSessionGroup::new();
        group.add_client(&server1_addr).await.unwrap();
        group.add_client(&server2_addr).await.unwrap();

        assert_eq!(group.sessions.read().await.len(), 2);

        let all_tools = group.list_tools_all().await.unwrap();
        assert_eq!(all_tools.len(), 2);
        assert!(all_tools.iter().any(|t| t.name == "tool-from-server-1"));
        assert!(all_tools.iter().any(|t| t.name == "tool-from-server-2"));
    }

    #[tokio::test]
    async fn test_remove_and_list_all() {
        let (server1_addr, _server1_handle) = setup_mock_server("tool-1").await;
        let (server2_addr, _server2_handle) = setup_mock_server("tool-2").await;

        let group = ClientSessionGroup::new();
        group.add_client(&server1_addr).await.unwrap();
        group.add_client(&server2_addr).await.unwrap();

        group.remove_client(&server1_addr).await;
        assert_eq!(group.sessions.read().await.len(), 1);

        let all_tools = group.list_tools_all().await.unwrap();
        assert_eq!(all_tools.len(), 1);
        assert_eq!(all_tools[0].name, "tool-2");
    }
}
