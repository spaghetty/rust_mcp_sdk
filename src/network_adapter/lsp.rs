// src/network_adapter/lsp.rs
use super::r#trait::NetworkAdapter; // Use the trait from the parent module
use crate::error::{Error, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub struct LspAdapter {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

impl From<TcpStream> for LspAdapter {
    fn from(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            writer: write_half,
            reader: BufReader::new(read_half),
        }
    }
}

impl LspAdapter {
    /// Creates a new `LspAdapter` by connecting to a given address.
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self::from(stream))
    }
}

#[async_trait]
impl NetworkAdapter for LspAdapter {
    async fn send(&mut self, msg: &str) -> Result<()> {
        let msg_len = msg.len();
        let header = format!("Content-Length: {}\r\n\r\n", msg_len);
        self.writer.write_all(header.as_bytes()).await?;
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        let mut content_length = 0;
        loop {
            let mut header_line = String::new();
            let bytes_read = self.reader.read_line(&mut header_line).await?;
            if bytes_read == 0 {
                return Ok(None);
            }
            if header_line.trim().is_empty() {
                break;
            }
            if let Some(len_str) = header_line.strip_prefix("Content-Length:") {
                if let Ok(len) = len_str.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }
        if content_length == 0 {
            return Err(Error::Other(
                "Received message with no Content-Length header.".into(),
            ));
        }
        let mut body_buf = vec![0; content_length];
        self.reader.read_exact(&mut body_buf).await?;
        let body_str = String::from_utf8(body_buf)
            .map_err(|e| Error::Other(format!("Invalid UTF-8 in message body: {}", e)))?;
        Ok(Some(body_str))
    }
}
