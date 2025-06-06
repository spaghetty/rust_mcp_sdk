use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;

use crate::protocol::{InitializePayload, ProtocolMessage};
use serde_json::Value;
use tokio::io::{BufReader, BufWriter};

use crate::types::{
    ClientCapabilities, Implementation, InitializeRequest, InitializeRequestParams,
    InitializeResult, ListResourcesRequest, ListResourcesResult, ListToolsRequest, ListToolsResult,
    PaginatedRequestParams, Resource, RootsCapability, SamplingCapability, ServerCapabilities,
    Tool, ToolCallParams, ToolCallRequest, ToolResult,
};
use crate::Result;

pub struct ClientSessionInner {
    pub read_stream: mpsc::Receiver<ProtocolMessage>,
    pub write_stream: mpsc::Sender<ProtocolMessage>,
    pub client_info: Implementation,
    pub _sampling_callback: Option<Box<dyn SamplingCallback + Send + Sync>>,
    pub _list_roots_callback: Option<Box<dyn ListRootsCallback + Send + Sync>>,
    pub _logging_callback: Option<Box<dyn LoggingCallback + Send + Sync>>,
}

impl ClientSessionInner {
    pub async fn send_to_write_stream(&mut self, msg: ProtocolMessage) -> crate::Result<()> {
        self.write_stream
            .send(msg)
            .await
            .map_err(|e| anyhow::anyhow!("Send error: {:?}", e))
    }
    pub async fn recv_from_read_stream(&mut self) -> Option<ProtocolMessage> {
        self.read_stream.recv().await
    }
}

pub struct ClientSession {
    inner: tokio::sync::Mutex<ClientSessionInner>,
}

// SessionMessage is replaced by ProtocolMessage for protocol-level communication.

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
        read_stream: mpsc::Receiver<ProtocolMessage>,
        write_stream: mpsc::Sender<ProtocolMessage>,
        client_info: Implementation,
        sampling_callback: Option<Box<dyn SamplingCallback + Send + Sync>>,
        list_roots_callback: Option<Box<dyn ListRootsCallback + Send + Sync>>,
        logging_callback: Option<Box<dyn LoggingCallback + Send + Sync>>,
    ) -> Self {
        let inner = ClientSessionInner {
            read_stream,
            write_stream,
            client_info,
            _sampling_callback: sampling_callback,
            _list_roots_callback: list_roots_callback,
            _logging_callback: logging_callback,
        };
        Self {
            inner: tokio::sync::Mutex::new(inner),
        }
    }

    pub async fn initialize(&self) -> Result<InitializeResult> {
        // Send protocol handshake
        let client_info_name = {
            let inner = self.inner.lock().await;
            inner.client_info.name.clone()
        };
        let handshake = ProtocolMessage::Initialize(InitializePayload {
            protocol_version: "1.0".to_string(),
            client_info: Some(client_info_name),
            server_info: None,
        });
        {
            let mut inner = self.inner.lock().await;
            inner.send_to_write_stream(handshake).await?;
        }
        // Await handshake response
        let msg = {
            let mut inner = self.inner.lock().await;
            inner.recv_from_read_stream().await
        };
        if let Some(msg) = msg {
            if let ProtocolMessage::Initialize(payload) = msg {
                // Optionally check protocol version, server_info, etc.
                // Continue with real initialization logic if needed
                Ok(InitializeResult {
                    protocol_version: payload.protocol_version,
                    capabilities: ServerCapabilities::default(),
                    server_info: Implementation {
                        name: payload
                            .server_info
                            .unwrap_or_else(|| "unknown-server".to_string()),
                        version: "unknown".to_string(),
                    },
                    instructions: None,
                })
            } else {
                Err(anyhow::anyhow!(
                    "Unexpected protocol message during handshake"
                ))
            }
        } else {
            Err(anyhow::anyhow!("No handshake response from server"))
        }
    }

    pub async fn list_resources(
        &self,
        params: PaginatedRequestParams,
    ) -> Result<ListResourcesResult> {
        let mut inner = self.inner.lock().await;
        let request = ListResourcesRequest::new(params);
        println!("[CLIENT DEBUG] ListResources Sending request: {:?}", request);
        inner
            .send_to_write_stream(ProtocolMessage::Data(serde_json::to_value(request)?))
            .await?;
        // Wait for response
        loop {
            if let Some(message) = inner.recv_from_read_stream().await {
                println!("[CLIENT DEBUG] Protocol message received: {:?}", message);
                match message {
                    ProtocolMessage::Error(e) => {
                        println!("[CLIENT DEBUG] Protocol error: {}", e);
                        return Err(anyhow::anyhow!("Server error: {}", e.message));
                    }
                    ProtocolMessage::Data(data) => {
                        match serde_json::from_value::<ListResourcesResult>(data) {
                            Ok(result) => return Ok(result),
                            Err(e) => {
                                println!("[CLIENT DEBUG] Failed to deserialize message: {}", e);
                                break;
                            }
                        }
                    }
                    _ => break, // Ignore other protocol messages
                }
            } else {
                break;
            }
        }
        Err(anyhow::anyhow!("No response received"))
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
        // Wrap request in ProtocolMessage::Data
        let msg = ProtocolMessage::Data(serde_json::to_value(request)?);
        // Send the request
        {
            let mut inner = self.inner.lock().await;
            inner.send_to_write_stream(msg).await?;
        }
        // Wait for response
        loop {
            let message = {
                let mut inner = self.inner.lock().await;
                inner.recv_from_read_stream().await
            };
            if let Some(message) = message {
                println!("[CLIENT DEBUG] Protocol message received: {:?}", message);
                match message {
                    ProtocolMessage::Error(e) => {
                        println!("[CLIENT DEBUG] Protocol error: {}", e);
                        return Err(anyhow::anyhow!("Server error: {}", e.message));
                    }
                    ProtocolMessage::Data(data) => match serde_json::from_value::<T>(data) {
                        Ok(result) => return Ok(result),
                        Err(e) => {
                            println!("[CLIENT DEBUG] Failed to deserialize message: {}", e);
                            break;
                        }
                    },
                    _ => break, // Ignore other protocol messages
                }
            } else {
                break;
            }
        }
        Err(anyhow::anyhow!("No response received"))
    }

    async fn handle_messages(&mut self) -> Result<()> {
        println!("[CLIENT DEBUG] Starting message loop");
        loop {
            let message = {
                let mut inner = self.inner.lock().await;
                inner.recv_from_read_stream().await
            };
            if let Some(message) = message {
                println!("[CLIENT DEBUG] Protocol message received: {:?}", message);
                match message {
                    ProtocolMessage::Data(data) => match serde_json::from_value::<Value>(data) {
                        Ok(value) => match value.get("method").and_then(|m| m.as_str()) {
                            Some("initialize") => self.handle_initialize(value).await?,
                            Some("resources/list") => self.handle_list_resources(value).await?,
                            Some("tools/list") => self.handle_list_tools(value).await?,
                            Some(method) => self.handle_unknown_method(method, &value).await?,
                            _ => self.handle_unknown_method("<missing>", &value).await?,
                        },
                        Err(_) => {
                            self.handle_unknown_method("<parse error>", &serde_json::Value::Null)
                                .await?
                        }
                    },
                    ProtocolMessage::Error(err) => {
                        println!("[CLIENT] Received protocol error: {}", err);
                    }
                    ProtocolMessage::Initialize(payload) => {
                        println!("[CLIENT] Received handshake/init: {:?}", payload);
                    }
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

    async fn handle_unknown_method(
        &mut self,
        method: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        println!("[CLIENT] Unknown method '{}', payload: {}", method, value);
        Ok(())
    }
}

pub struct ClientSessionGroup {
    sessions: HashMap<Url, Arc<tokio::sync::Mutex<ClientSession>>>,
    _tools: HashMap<String, Tool>,
    _resources: HashMap<String, Resource>,
    _prompts: HashMap<String, Value>,
}

impl ClientSessionGroup {
    /// Only public for integration testing in `tests/`.
    pub async fn list_resources_with_params(
        &mut self,
        server_url: &Url,
        params: PaginatedRequestParams,
    ) -> Result<ListResourcesResult> {
        if let Some(session) = self.sessions.get(server_url) {
            println!("[CLIENT DEBUG] Sending request: {:?}", params);
            let guard = session.lock().await; //<- we have a deadlock here
            println!("[CLIENT DEBUG] Sending request : lock acquired");
            guard.list_resources(params).await
        } else {
            Err(anyhow::anyhow!("No session for the given server_url"))
        }
    }

    pub async fn list_resources(&mut self, server_url: &Url) -> Result<ListResourcesResult> {
        if let Some(session) = self.sessions.get(server_url) {
            let guard = session.lock().await;
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
            _tools: HashMap::new(),
            _resources: HashMap::new(),
            _prompts: HashMap::new(),
        }
    }

    pub async fn connect_to_server(&mut self, server_url: Url) -> Result<()> {
        use std::sync::Arc;

        use tokio::net::TcpStream;
        let addr = match server_url.socket_addrs(|| None) {
            Ok(addrs) => addrs[0],
            Err(_) => return Err(anyhow::anyhow!("Invalid server URL for TCP connection")),
        };
        let stream = TcpStream::connect(addr).await?;
        let (read_half, write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let mut writer = BufWriter::new(write_half);
        let (read_tx, read_rx): (
            mpsc::Sender<ProtocolMessage>,
            mpsc::Receiver<ProtocolMessage>,
        ) = mpsc::channel(100);
        let (write_tx, mut write_rx): (
            mpsc::Sender<ProtocolMessage>,
            mpsc::Receiver<ProtocolMessage>,
        ) = mpsc::channel(100);
        // Spawn read task: socket -> read_tx
        tokio::spawn(async move {
            loop {
                match ProtocolMessage::read_from_stream(&mut reader).await {
                    Ok(msg) => {
                        let _ = read_tx.send(msg).await;
                    }
                    Err(e) => {
                        println!("[CLIENT] TCP read task: error: {:?}", e);
                        break;
                    }
                }
            }
            println!("[CLIENT] TCP read task exiting");
        });
        // Spawn write task: write_rx -> socket
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = msg.write_to_stream(&mut writer).await {
                    println!("[CLIENT] TCP write task: error: {:?}", e);
                    break;
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
