// src/network_adapter/stdio.rs

use super::r#trait::NetworkAdapter;
use crate::error::{Error, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};

/// A NetworkAdapter implementation that uses process stdin/stdout.
pub struct StdioAdapter {
    writer: Stdout,
    reader: BufReader<Stdin>,
}

impl StdioAdapter {
    pub fn new() -> Self {
        Self {
            writer: tokio::io::stdout(),
            reader: BufReader::new(tokio::io::stdin()),
        }
    }
}

#[async_trait]
impl NetworkAdapter for StdioAdapter {
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
