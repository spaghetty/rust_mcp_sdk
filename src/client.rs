use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use url::Url;

use serde_json::Value;

use crate::types::{
    ClientCapabilities, Implementation, InitializeRequest, InitializeRequestParams,
    InitializeResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest, ListToolsResult,
    PaginatedRequestParams, Resource, RootsCapability, SamplingCapability, Tool, ToolCallParams,
    ToolCallRequest, ToolResult,
};
use crate::Result;

pub struct ClientSession {
    read_stream: mpsc::Receiver<SessionMessage>,
    write_stream: mpsc::Sender<SessionMessage>,
    client_info: Implementation,
    sampling_callback: Option<Box<dyn SamplingCallback + Send + Sync>>,
    list_roots_callback: Option<Box<dyn ListRootsCallback + Send + Sync>>,
    logging_callback: Option<Box<dyn LoggingCallback + Send + Sync>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMessage {
    pub message: Value,
}

#[async_trait::async_trait]
pub trait SamplingCallback {
    async fn on_sampling(&self, params: &Value) -> Result<()>;
}

#[async_trait::async_trait]
pub trait ListRootsCallback {
    async fn on_list_roots(&self, params: &Value) -> Result<()>;
}

#[async_trait::async_trait]
pub trait LoggingCallback {
    async fn on_log(&self, params: &Value) -> Result<()>;
}

impl ClientSession {
    pub fn new(
        read_stream: mpsc::Receiver<SessionMessage>,
        write_stream: mpsc::Sender<SessionMessage>,
        client_info: Implementation,
        sampling_callback: Option<Box<dyn SamplingCallback + Send + Sync>>,
        list_roots_callback: Option<Box<dyn ListRootsCallback + Send + Sync>>,
        logging_callback: Option<Box<dyn LoggingCallback + Send + Sync>>,
    ) -> Self {
        Self {
            read_stream,
            write_stream,
            client_info,
            sampling_callback,
            list_roots_callback,
            logging_callback,
        }
    }

    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeRequestParams {
            protocol_version: "1.0".to_string(),
            capabilities: ClientCapabilities {
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
            client_info: self.client_info.clone(),
            extra: HashMap::new(),
        };

        let request = InitializeRequest::new(params);
        self.send_request(request).await
    }

    pub async fn list_resources(
        &mut self,
        params: PaginatedRequestParams,
    ) -> Result<ListResourcesResult> {
        let request = ListResourcesRequest::new(params);
        self.send_request(request).await
    }

    pub async fn list_tools(&mut self, params: PaginatedRequestParams) -> Result<ListToolsResult> {
        let request = ListToolsRequest::new(params);
        self.send_request(request).await
    }

    pub async fn call_tool(
        &mut self,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<ToolResult> {
        let params = ToolCallParams { name, arguments };
        let request = ToolCallRequest::new(params);
        self.send_request(request).await
    }

    async fn send_request<T: for<'de> serde::Deserialize<'de>>(
        &mut self,
        request: impl serde::Serialize,
    ) -> Result<T> {
        // Send request
        self.write_stream
            .send(SessionMessage {
                message: serde_json::to_value(request)?,
            })
            .await?;

        // Wait for response from self.read_stream
        loop {
            if let Some(message) = self.read_stream.recv().await {
                println!("[CLIENT DEBUG] Raw message received: {:?}", message);
                // Check for error field in the response
                if let Some(error_msg) = message.message.get("error").and_then(|e| e.as_str()) {
                    return Err(anyhow::anyhow!("Server error: {}", error_msg));
                }
                match serde_json::from_value::<T>(message.message.clone()) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        println!(
                            "[CLIENT DEBUG] Failed to deserialize message: {}\nMessage: {:?}",
                            e, message.message
                        );
                        continue; // Skip non-matching messages
                    }
                }
            } else {
                break;
            }
        }
        Err(anyhow::anyhow!("No response received"))
    }

    async fn handle_messages(&mut self) -> Result<()> {
        loop {
            if let Some(message) = self.read_stream.recv().await {
                match serde_json::from_value::<Value>(message.message) {
                    Ok(value) => match value.get("method").and_then(|m| m.as_str()) {
                        Some("initialize") => self.handle_initialize(value).await?,
                        Some("resources/list") => self.handle_list_resources(value).await?,
                        Some("tools/list") => self.handle_list_tools(value).await?,
                        _ => self.handle_notification(value).await?,
                    },
                    Err(_) => continue,
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    async fn handle_initialize(&mut self, _value: Value) -> Result<()> {
        // Handle initialize message
        Ok(())
    }

    async fn handle_list_resources(&mut self, _value: Value) -> Result<()> {
        // Handle list resources message
        Ok(())
    }

    async fn handle_list_tools(&mut self, _value: Value) -> Result<()> {
        // Handle list tools message
        Ok(())
    }

    async fn handle_notification(&mut self, _value: Value) -> Result<()> {
        // Handle notification message
        Ok(())
    }
}

pub struct ClientSessionGroup {
    sessions: HashMap<Url, Arc<tokio::sync::Mutex<ClientSession>>>,
    tools: HashMap<String, Tool>,
    resources: HashMap<String, Resource>,
    prompts: HashMap<String, Value>,
}

impl ClientSessionGroup {
    pub async fn list_resources(&mut self, server_url: &Url) -> Result<ListResourcesResult> {
        if let Some(session) = self.sessions.get(server_url) {
            let mut guard = session.lock().await;
            guard.list_resources(Default::default()).await
        } else {
            Err(anyhow::anyhow!("No session for the given server_url"))
        }
    }

    pub async fn list_tools(
        &mut self,
        server_url: &Url,
        params: PaginatedRequestParams,
    ) -> Result<ListToolsResult> {
        if let Some(session) = self.sessions.get(server_url) {
            let mut guard = session.lock().await;
            guard.list_tools(params).await
        } else {
            Err(anyhow::anyhow!("No session for the given server_url"))
        }
    }

    pub async fn call_tool(
        &mut self,
        server_url: &Url,
        name: String,
        arguments: HashMap<String, String>,
    ) -> Result<ToolResult> {
        if let Some(session) = self.sessions.get(server_url) {
            let mut guard = session.lock().await;
            guard.call_tool(name, arguments).await
        } else {
            Err(anyhow::anyhow!("No session for the given server_url"))
        }
    }
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            tools: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
        }
    }

    pub async fn connect_to_server(&mut self, server_url: Url) -> Result<()> {
        use std::sync::Arc;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
        use tokio::net::TcpStream;
        let addr = match server_url.socket_addrs(|| None) {
            Ok(addrs) => addrs[0],
            Err(_) => return Err(anyhow::anyhow!("Invalid server URL for TCP connection")),
        };
        let stream = TcpStream::connect(addr).await?;
        let (read_half, write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let mut writer = BufWriter::new(write_half);
        let (read_tx, read_rx): (mpsc::Sender<SessionMessage>, mpsc::Receiver<SessionMessage>) =
            mpsc::channel(100);
        let (write_tx, mut write_rx): (
            mpsc::Sender<SessionMessage>,
            mpsc::Receiver<SessionMessage>,
        ) = mpsc::channel(100);
        // Spawn read task
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        println!("[CLIENT TCP DEBUG] Received line: {:?}", line);
                        if let Ok(msg) = serde_json::from_str::<SessionMessage>(&line) {
                            println!("[CLIENT TCP DEBUG] Parsed as SessionMessage: {:?}", msg);
                            let _ = read_tx.send(msg).await;
                        } else {
                            println!(
                                "[CLIENT TCP DEBUG] Failed to parse line as SessionMessage: {:?}",
                                line
                            );
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        // Spawn write task
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    println!("[CLIENT TCP DEBUG] Sending: {}", json);
                    let _ = writer.write_all(json.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                    let _ = writer.flush().await;
                    println!("[CLIENT TCP DEBUG] Flushed message to server");
                }
            }
        });
        // Create session
        let session = Arc::new(tokio::sync::Mutex::new(ClientSession::new(
            read_rx,
            write_tx,
            Implementation {
                name: "mcp-rust-client".to_string(),
                version: "0.1.0".to_string(),
            },
            None,
            None,
            None,
        )));
        // Spawn message handling loop
        let session_clone = session.clone();
        tokio::spawn(async move {
            let _ = session_clone.lock().await.handle_messages().await;
        });
        self.sessions.insert(server_url, session);
        Ok(())
    }
}
