//! Protocol layer for MCP Rust SDK
// Handles message framing, protocol handshake, and error serialization.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// Enum of all protocol-level messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum ProtocolMessage {
    /// Initialization handshake
    Initialize(InitializePayload),
    /// Regular message
    Data(Value),
    /// Protocol error
    Error(ProtocolError),
}

/// Payload for initialization handshake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializePayload {
    pub protocol_version: String,
    pub client_info: Option<String>,
    pub server_info: Option<String>,
}

/// Protocol error message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolError {
    pub code: u32,
    pub message: String,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProtocolError {{ code: {}, message: {} }}",
            self.code, self.message
        )
    }
}

/// Trait for handling protocol messages.
pub trait ProtocolHandler {
    /// Handle a protocol message and return a response (if any) or an error.
    fn handle_message(
        &mut self,
        msg: ProtocolMessage,
    ) -> Result<Option<ProtocolMessage>, ProtocolError>;
}

impl ProtocolMessage {
    /// Reads a ProtocolMessage from a buffered async reader (expects one message per line).
    pub async fn read_from_stream<R: AsyncBufReadExt + Unpin>(
        reader: &mut R,
    ) -> std::io::Result<ProtocolMessage> {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "stream closed",
            ));
        }
        let msg: ProtocolMessage = serde_json::from_str(&line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(msg)
    }

    /// Writes this ProtocolMessage to a buffered async writer (as a single line).
    pub async fn write_to_stream<W: AsyncWriteExt + Unpin>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        let json = serde_json::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writer.write_all(json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    }
}

/// Helper to construct a protocol handshake message.
pub fn handshake_init(protocol_version: &str, client_info: Option<String>) -> ProtocolMessage {
    ProtocolMessage::Initialize(InitializePayload {
        protocol_version: protocol_version.to_string(),
        client_info,
        server_info: None,
    })
}

/// Helper to construct a protocol handshake response.
pub fn handshake_response(protocol_version: &str, server_info: Option<String>) -> ProtocolMessage {
    ProtocolMessage::Initialize(InitializePayload {
        protocol_version: protocol_version.to_string(),
        client_info: None,
        server_info,
    })
}

/// Helper to construct a protocol error message.
pub fn protocol_error(code: u32, message: &str) -> ProtocolMessage {
    ProtocolMessage::Error(ProtocolError {
        code,
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_initialize() {
        let msg = ProtocolMessage::Initialize(InitializePayload {
            protocol_version: "1.0".to_string(),
            client_info: Some("client-x".to_string()),
            server_info: None,
        });
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ProtocolMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_error() {
        let err = ProtocolMessage::Error(ProtocolError {
            code: 42,
            message: "bad stuff".to_string(),
        });
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: ProtocolMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_data() {
        let data = ProtocolMessage::Data(serde_json::json!({"foo": 123}));
        let json = serde_json::to_string(&data).unwrap();
        let deserialized: ProtocolMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
