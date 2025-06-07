//! Defines the pluggable network transport layer for the MCP SDK.
//!
//! This module contains the `NetworkAdapter` trait, which abstracts away the specifics
//! of the underlying transport protocol (e.g., TCP, WebSockets). It ensures that the
//! higher-level client and server logic can operate without needing to know how
//! messages are actually sent over the wire.

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// A trait for a generic, message-based network transport.
///
/// This abstracts over the transport layer, allowing for different
/// implementations like TCP, WebSockets, or in-memory channels for testing.
/// The protocol uses line-delimited JSON for message framing.
#[async_trait]
pub trait NetworkAdapter: Send + Sync {
    /// Sends a single, complete message string over the transport.
    /// A newline character will be appended to frame the message.
    async fn send(&mut self, msg: &str) -> Result<()>;

    /// Receives a single, complete message string from the transport.
    /// This should handle reading until a newline character is found.
    /// Returns `Ok(None)` if the connection is closed gracefully.
    async fn recv(&mut self) -> Result<Option<String>>;
}

/// A `NetworkAdapter` implementation for a TCP stream.
pub struct TcpAdapter {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

impl TcpAdapter {
    /// Creates a new `TcpAdapter` by connecting to a given address.
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self::new(stream))
    }

    /// Creates a new `TcpAdapter` from an existing `TcpStream`.
    pub fn new(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            writer: write_half,
            reader: BufReader::new(read_half),
        }
    }
}

#[async_trait]
impl NetworkAdapter for TcpAdapter {
    async fn send(&mut self, msg: &str) -> Result<()> {
        // Write the message bytes followed by a newline to frame the message.
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        // Read from the buffered reader until a newline is encountered.
        match self.reader.read_line(&mut line).await {
            // 0 bytes read means the connection was closed by the peer.
            Ok(0) => Ok(None),
            Ok(_) => {
                // Remove the trailing newline character, if it exists.
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                Ok(Some(line))
            }
            Err(e) => Err(e.into()),
        }
    }
}

// --- Unit Tests ---
// Ensures that our network adapter can correctly frame, send, and receive messages.
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::task;

    /// Tests a full send/receive round-trip over a real TCP connection.
    #[tokio::test]
    async fn test_tcp_adapter_send_recv_roundtrip() {
        // 1. Set up a listener on a random local port.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // 2. Spawn a "server" task to accept the connection and respond.
        let server_handle = task::spawn(async move {
            let (server_stream, _) = listener.accept().await.unwrap();
            let mut server_adapter = TcpAdapter::new(server_stream);

            // Receive a message from the client.
            let received = server_adapter.recv().await.unwrap().unwrap();
            assert_eq!(received, "hello from client");

            // Send a message back to the client.
            server_adapter.send("hello from server").await.unwrap();
        });

        // 3. Spawn a "client" task to connect and initiate communication.
        let client_handle = task::spawn(async move {
            let mut client_adapter = TcpAdapter::connect(&addr.to_string()).await.unwrap();

            // Send a message to the server.
            client_adapter.send("hello from client").await.unwrap();

            // Receive the response from the server.
            let received = client_adapter.recv().await.unwrap().unwrap();
            assert_eq!(received, "hello from server");
        });

        // 4. Wait for both tasks to complete successfully.
        server_handle.await.unwrap();
        client_handle.await.unwrap();
    }
}
