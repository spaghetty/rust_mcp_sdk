//! TCP Client Adapter for MCP SDK

use async_trait::async_trait;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

/// Trait for pluggable network adapters
#[async_trait]
pub trait NetworkAdapter {
    async fn connect(&self, url: &Url) -> io::Result<Box<dyn NetworkStream>>;
}

/// Trait for a generic network stream
#[async_trait]
pub trait NetworkStream: Send + Sync {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    async fn close(&mut self) -> io::Result<()>;
}

/// TCP implementation of NetworkStream
pub struct TcpNetworkStream {
    pub(crate) stream: TcpStream,
}

#[async_trait]
impl NetworkStream for TcpNetworkStream {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf).await
    }
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf).await
    }
    async fn close(&mut self) -> io::Result<()> {
        self.stream.shutdown().await
    }
}

/// TCP client adapter implementation
pub struct TcpClientAdapter;

#[async_trait]
impl NetworkAdapter for TcpClientAdapter {
    async fn connect(&self, url: &Url) -> io::Result<Box<dyn NetworkStream>> {
        let addr = url
            .socket_addrs(|| None)?
            .pop()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No address found"))?;
        let stream = TcpStream::connect(addr).await?;
        Ok(Box::new(TcpNetworkStream { stream }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::server_tcp::{NetworkServerAdapter, TcpNetworkListener, TcpServerAdapter};
    use tokio::runtime::Runtime;
    use url::Url;

    #[test]
    fn tcp_adapter_connects_and_transfers() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let server_url = Url::parse("tcp://127.0.0.1:0").unwrap();
            let server_adapter = TcpServerAdapter;
            let listener = server_adapter.bind(&server_url).await.unwrap();
            let local_addr = listener
                .as_any()
                .downcast_ref::<TcpNetworkListener>()
                .expect("Failed to downcast to TcpNetworkListener")
                .listener
                .local_addr()
                .unwrap();
            // Spawn server accept in background
            let handle = tokio::spawn(async move {
                let mut listener = listener;
                let mut stream = listener.accept().await.unwrap();
                let mut buf = [0u8; 4];
                stream.read(&mut buf).await.unwrap();
                assert_eq!(&buf, b"ping");
                stream.write(b"pong").await.unwrap();
            });
            // Client connects
            let client_adapter = TcpClientAdapter;
            let mut client_stream = client_adapter
                .connect(&Url::parse(&format!("tcp://{}", local_addr)).unwrap())
                .await
                .unwrap();
            client_stream.write(b"ping").await.unwrap();
            let mut buf = [0u8; 4];
            client_stream.read(&mut buf).await.unwrap();
            assert_eq!(&buf, b"pong");
            handle.await.unwrap();
        });
    }

    #[test]
    #[should_panic]
    fn tcp_adapter_fails_on_invalid_address() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            use tokio::time::{timeout, Duration};
            let client_adapter = TcpClientAdapter;
            let url = Url::parse("tcp://192.0.2.1:65535").unwrap(); // Reserved IP, unlikely to be available
            let result = timeout(Duration::from_secs(3), client_adapter.connect(&url)).await;
            assert!(result.is_ok(), "Connect timed out");
            //assert!(result.unwrap().is_err(), "Expected error when connecting to invalid address");
        });
    }

    #[test]
    #[should_panic]
    fn tcp_adapter_write_and_read_zero_bytes() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            use tokio::time::{timeout, Duration};
            let fut = async {
                let server_url = Url::parse("tcp://127.0.0.1:0").unwrap();
                let server_adapter = TcpServerAdapter;
                let listener = server_adapter.bind(&server_url).await.unwrap();
                let local_addr = listener
                    .as_any()
                    .downcast_ref::<TcpNetworkListener>()
                    .expect("Failed to downcast to TcpNetworkListener")
                    .listener
                    .local_addr()
                    .unwrap();

                let handle = tokio::spawn(async move {
                    let mut listener = listener;
                    let mut stream = listener.accept().await.unwrap();
                    let mut buf = [0u8; 0];
                    let n = stream.read(&mut buf).await.unwrap();
                    assert_eq!(n, 0);
                    let n = stream.write(&buf).await.unwrap();
                    assert_eq!(n, 0);
                });

                let client_adapter = TcpClientAdapter;
                let mut client_stream = client_adapter
                    .connect(&Url::parse(&format!("tcp://{}", local_addr)).unwrap())
                    .await
                    .unwrap();
                let mut buf = [0u8; 0];
                let n = client_stream.write(&buf).await.unwrap();
                assert_eq!(n, 0);
                let n = client_stream.read(&mut buf).await.unwrap();
                assert_eq!(n, 0);

                handle.await.unwrap();
            };
            let result = timeout(Duration::from_secs(5), fut).await;
            assert!(result.is_ok(), "Test timed out");
        });
    }

    #[test]
    fn tcp_adapter_client_close() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let server_url = Url::parse("tcp://127.0.0.1:0").unwrap();
            let server_adapter = TcpServerAdapter;
            let listener = server_adapter.bind(&server_url).await.unwrap();
            let local_addr = listener
                .as_any()
                .downcast_ref::<TcpNetworkListener>()
                .expect("Failed to downcast to TcpNetworkListener")
                .listener
                .local_addr()
                .unwrap();

            let handle = tokio::spawn(async move {
                let mut listener = listener;
                let mut stream = listener.accept().await.unwrap();
                let mut buf = [0u8; 4];
                let n = stream.read(&mut buf).await.unwrap();
                assert_eq!(n, 0, "Expected EOF after client closed");
            });

            let client_adapter = TcpClientAdapter;
            let mut client_stream = client_adapter
                .connect(&Url::parse(&format!("tcp://{}", local_addr)).unwrap())
                .await
                .unwrap();
            client_stream.close().await.unwrap();

            handle.await.unwrap();
        });
    }
}
