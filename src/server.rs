use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use serde_json::Value;
use url::Url;

use crate::common::*;
use crate::types::ToolResult;
use crate::types::*;
use crate::Result;

pub struct ServerSession {
    tool_handler: Option<
        Arc<
            StdMutex<
                Option<
                    Box<
                        dyn Fn(String, HashMap<String, String>) -> Result<ToolResult> + Send + Sync,
                    >,
                >,
            >,
        >,
    >,

    read_stream: mpsc::Receiver<SessionMessage>,
    write_stream: mpsc::Sender<SessionMessage>,
    initialized: bool,
    client_params: Option<InitializeRequestParams>,
    client_url: Url,
    list_resources_handler:
        Option<Arc<StdMutex<Option<Box<dyn Fn(Value) -> Vec<Resource> + Send + Sync>>>>>,
}

// impl Clone for ServerSession {
//     fn clone(&self) -> Self {
//         Self {
//             read_stream: self.read_stream.clone(), // mpsc::Receiver cannot be cloned
//             write_stream: self.write_stream.clone(),
//             initialized: self.initialized,
//             client_params: self.client_params.clone(),
//             client_url: self.client_url.clone(),
//         }
//     }
// }
// Removed: mpsc::Receiver cannot be cloned. If cloning is needed, use a different pattern (e.g., broadcast channel).

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

impl ServerSession {
    pub fn new(
        read_stream: mpsc::Receiver<SessionMessage>,
        write_stream: mpsc::Sender<SessionMessage>,
        client_url: Url,
        list_resources_handler: Option<
            Arc<StdMutex<Option<Box<dyn Fn(Value) -> Vec<Resource> + Send + Sync>>>>,
        >,
    ) -> Self {
        Self {
            read_stream,
            write_stream,
            initialized: false,
            client_params: None,
            client_url,
            list_resources_handler,
            tool_handler: None,
        }
    }

    pub fn from_socket(
        socket: TcpStream,
        list_resources_handler: Option<
            Arc<StdMutex<Option<Box<dyn Fn(Value) -> Vec<Resource> + Send + Sync>>>>,
        >,
        tool_handler: Option<
            Arc<
                StdMutex<
                    Option<
                        Box<
                            dyn Fn(String, HashMap<String, String>) -> Result<ToolResult>
                                + Send
                                + Sync,
                        >,
                    >,
                >,
            >,
        >,
    ) -> Self {
        use tokio::io::{split, BufReader, BufWriter};
        let (read_half, write_half) = split(socket);
        let mut reader = BufReader::new(read_half);
        let mut writer = BufWriter::new(write_half);
        let (write_tx, mut write_rx) = mpsc::channel(16);
        let (read_tx, read_rx) = mpsc::channel(16);

        // Spawn read task: socket -> read_tx
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        println!(
                            "[SERVER] TCP read task: connection closed (read_line returned 0)"
                        );
                        break;
                    }
                    Ok(_) => {
                        println!("[SERVER] Received raw line: {}", line.trim_end());
                        if let Ok(msg) = serde_json::from_str::<SessionMessage>(&line) {
                            let _ = read_tx.send(msg).await;
                        }
                    }
                    Err(e) => {
                        println!("[SERVER] TCP read task: error: {:?}", e);
                        break;
                    }
                }
            }
            println!("[SERVER] TCP read task exiting");
        });

        // Spawn write task: write_rx -> socket
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = writer.write_all(json.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                    let _ = writer.flush().await;
                }
            }
        });

        Self {
            read_stream: read_rx,
            write_stream: write_tx,
            initialized: false,
            client_params: None,
            client_url: Url::parse("tcp://localhost").unwrap(),
            list_resources_handler,
            tool_handler,
        }
    }

    pub fn with_streams(
        read_stream: mpsc::Receiver<SessionMessage>,
        write_stream: mpsc::Sender<SessionMessage>,
    ) -> Self {
        Self {
            read_stream,
            write_stream,
            initialized: false,
            client_params: None,
            client_url: Url::parse("http://localhost").unwrap(),
            list_resources_handler: None,
            tool_handler: None,
        }
    }

    async fn handle_messages(&mut self) -> Result<()> {
        println!("[SERVER] handle_messages: starting message loop");
        loop {
            if let Some(message) = self.read_stream.recv().await {
                match serde_json::from_value::<Value>(message.message) {
                    Ok(value) => match value.get("method").and_then(|m| m.as_str()) {
                        Some("initialize") => self.handle_initialize(value).await?,
                        Some("resources/list") => self.handle_list_resources(value).await?,
                        Some("tools/list") => self.handle_list_tools(value).await?,
                        Some("tool/call") => self.handle_call_tool(value).await?,
                        Some(method) => self.handle_unknown_method(method, &value).await?,
                        _ => self.handle_unknown_method("<missing>", &value).await?,
                    },
                    Err(_) => continue,
                }
            } else {
                println!(
                    "[SERVER] handle_messages: read_stream channel closed, exiting message loop"
                );
                break;
            }
        }
        println!("[SERVER] handle_messages: message loop exited");
        Ok(())
    }

    async fn handle_initialize(&mut self, request: Value) -> Result<()> {
        let params = request
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
        let params: InitializeRequestParams = serde_json::from_value(params.clone())?;

        self.client_params = Some(params.clone());
        self.initialized = true;

        let response = InitializeResult {
            protocol_version: "1.0".to_string(),
            capabilities: ServerCapabilities {
                sampling: Some(SamplingCapability {
                    sample_size: 100,
                    extra: HashMap::new(),
                }),
                roots: Some(RootsCapability {
                    list_changed: true,
                    extra: HashMap::new(),
                }),
                extra: HashMap::new(),
            },
            server_info: Implementation {
                name: "mcp-rust-server".to_string(),
                version: "0.1.0".to_string(),
            },
            instructions: Some("Welcome to the MCP Rust server".to_string()),
        };

        self.send_response(response).await
    }

    async fn handle_list_resources(&mut self, request: Value) -> Result<()> {
        println!("[SERVER] handle_list_resources called");
        let params = request
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
        let params: PaginatedRequestParams = serde_json::from_value(params.clone())?;

        let resources = if let Some(handler_arc) = &self.list_resources_handler {
            let guard = handler_arc.lock().unwrap();
            if let Some(handler) = &*guard {
                handler(request.clone())
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let response = ListResourcesResult {
            resources,
            cursor: params.cursor,
        };

        self.send_response(response).await
    }

    async fn handle_list_tools(&mut self, _request: Value) -> Result<()> {
        println!("[SERVER] handle_list_tools called (not implemented)");
        let error_message = "Method 'tools/list' not implemented";
        let error_response = serde_json::json!({
            "error": error_message
        });
        self.send_response(error_response).await
    }

    async fn handle_call_tool(&mut self, request: Value) -> Result<()> {
        // This expects request["params"] to have "name" and "arguments"
        let params = request
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("Missing params"))?;
        let params: crate::types::ToolCallParams = serde_json::from_value(params.clone())?;
        let result = if let Some(handler_arc) = &self.tool_handler {
            let guard = handler_arc.lock().unwrap();
            if let Some(handler) = &*guard {
                handler(params.name, params.arguments)
            } else {
                Err(anyhow::anyhow!("No tool handler registered"))
            }
        } else {
            Err(anyhow::anyhow!("No tool handler registered"))
        };
        // Respond with the result
        // (You may need to adapt this to your protocol)
        match result {
            Ok(tool_result) => self.send_response(tool_result).await,
            Err(e) => {
                self.send_response(ToolResult {
                    result: None,
                    error: Some(e.to_string()),
                })
                .await
            }
        }
    }

    async fn handle_unknown_method(&mut self, method: &str, _value: &Value) -> Result<()> {
        println!(
            "[SERVER] handle_unknown_method called for method: {}",
            method
        );
        // Send an error response for unknown/unimplemented methods
        let error_message = format!("Method '{}' not implemented", method);
        let error_response = serde_json::json!({
            "error": error_message
        });
        self.send_response(error_response).await
    }

    async fn handle_notification(&mut self, _notification: Value) -> Result<()> {
        // TODO: Implement notification handling
        Ok(())
    }

    async fn send_response<T: serde::Serialize>(&mut self, response: T) -> Result<()> {
        let message = SessionMessage {
            message: serde_json::to_value(response)?,
        };
        println!("[SERVER] send_response: {:?}", message);
        if message.message.get("error").is_some() {
            println!("[SERVER] Sending ERROR response to client: {:?}", message);
        }
        self.write_stream.send(message).await?;
        Ok(())
    }
}

use std::sync::Mutex as StdMutex;

pub struct Server {
    _sessions: Arc<Mutex<HashMap<Url, Arc<tokio::sync::Mutex<ServerSession>>>>>, 
    pub list_resources_handler:
        Arc<StdMutex<Option<Box<dyn Fn(Value) -> Vec<Resource> + Send + Sync>>>>,
    pub tool_handler: Arc<
        StdMutex<
            Option<
                Box<dyn Fn(String, HashMap<String, String>) -> Result<ToolResult> + Send + Sync>,
            >,
        >,
    >,
}

impl Server {
    pub fn add_tool_handler<F>(&mut self, handler: F)
    where
        F: Fn(String, HashMap<String, String>) -> Result<ToolResult> + 'static + Send + Sync,
    {
        let mut guard = self.tool_handler.lock().unwrap();
        *guard = Some(Box::new(handler));
    }

    pub fn list_tools<F, Fut>(&mut self, _handler: F)
    where
        F: Fn(Value) -> Fut + 'static + Send + Sync,
        Fut: std::future::Future<Output = Result<ListToolsResult>> + Send + 'static,
    {
        // For now, this is a stub. If you want to fully implement async handler storage, you can adjust the field and handler logic accordingly.
        // Here, just store a synchronous handler for demonstration, or adapt as needed for your async runtime.
        // This will allow the example to compile and run, but you may want to expand this for full async support.
    }

    pub fn list_resources<F>(&mut self, handler: F)
    where
        F: Fn(Value) -> Vec<Resource> + 'static + Send + Sync,
    {
        let mut guard = self.list_resources_handler.lock().unwrap();
        *guard = Some(Box::new(handler));
    }
    pub fn new() -> Self {
        Self {
            _sessions: Arc::new(Mutex::new(HashMap::new())),
            list_resources_handler: Arc::new(StdMutex::new(None)),
            tool_handler: Arc::new(StdMutex::new(None)),
        }
    }

    pub async fn run(&self, bind_addr: &str) -> Result<()> {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(bind_addr).await?;
        println!("Server listening on {}", bind_addr);
        loop {
            let (socket, addr) = listener.accept().await?;
            println!("New MCP connection from {:?}", addr);
            let list_resources_handler = self.list_resources_handler.clone();
            let tool_handler = self.tool_handler.clone();
            tokio::spawn(async move {
                let mut session = ServerSession::from_socket(
                    socket,
                    Some(list_resources_handler),
                    Some(tool_handler),
                );
                let _ = session.handle_messages().await;
            });
        }
    }
}
