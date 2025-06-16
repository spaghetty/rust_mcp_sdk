// src/network_adapter/ndjson.rs
use super::r#trait::NetworkAdapter;
use crate::error::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

pub struct NdjsonAdapter {
    writer: OwnedWriteHalf,
    reader: BufReader<OwnedReadHalf>,
}

impl NdjsonAdapter {
    /// Creates a new `NdjsonAdapter` by connecting to a given address.
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self::from(stream))
    }
}

impl From<TcpStream> for NdjsonAdapter {
    fn from(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            writer: write_half,
            reader: BufReader::new(read_half),
        }
    }
}

#[async_trait]
impl NetworkAdapter for NdjsonAdapter {
    async fn send(&mut self, msg: &str) -> Result<()> {
        self.writer.write_all(msg.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        match self.reader.read_line(&mut line).await {
            Ok(0) => Ok(None),
            Ok(_) => {
                if line.ends_with('\n') {
                    line.pop();
                }
                if line.ends_with('\r') {
                    line.pop();
                }
                Ok(Some(line))
            }
            Err(e) => Err(e.into()),
        }
    }
}
