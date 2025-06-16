// src/network_adapter/stdio.rs

use super::r#trait::NetworkAdapter;
use crate::error::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};

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
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            // 0 bytes read means stdin was closed.
            Ok(0) => Ok(None),
            Ok(_) => {
                // Remove the trailing newline character before returning.
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
