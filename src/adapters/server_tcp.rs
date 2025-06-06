//! TCP Server Adapter for MCP SDK

use super::client_tcp::{NetworkStream, TcpNetworkStream};
use async_trait::async_trait;
use std::io;
use tokio::net::TcpListener;
use url::Url;

#[async_trait]
pub trait NetworkServerAdapter {
    async fn bind(&self, url: &Url) -> io::Result<Box<dyn NetworkListener>>;
}

use std::any::Any;

#[async_trait]
pub trait NetworkListener: Send + Sync {
    async fn accept(&mut self) -> io::Result<Box<dyn NetworkStream>>;
    fn as_any(&self) -> &dyn Any;
}

/// TCP server adapter implementation
pub struct TcpServerAdapter;

pub struct TcpNetworkListener {
    pub(crate) listener: TcpListener,
}

#[async_trait]
impl NetworkListener for TcpNetworkListener {
    async fn accept(&mut self) -> io::Result<Box<dyn NetworkStream>> {
        let (stream, _) = self.listener.accept().await?;
        Ok(Box::new(TcpNetworkStream { stream }))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[async_trait]
impl NetworkServerAdapter for TcpServerAdapter {
    async fn bind(&self, url: &Url) -> io::Result<Box<dyn NetworkListener>> {
        let addr = url
            .socket_addrs(|| None)?
            .pop()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No address found"))?;
        let listener = TcpListener::bind(addr).await?;
        Ok(Box::new(TcpNetworkListener { listener }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    use url::Url;

    #[test]
    fn tcp_server_adapter_fails_on_invalid_address() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let server_adapter = TcpServerAdapter;
            let url = Url::parse("tcp://256.256.256.256:12345").unwrap();
            let result = server_adapter.bind(&url).await;
            assert!(
                result.is_err(),
                "Expected error when binding to invalid address"
            );
        });
    }

    #[test]
    fn tcp_server_adapter_accepts_multiple_connections() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let server_adapter = TcpServerAdapter;
            let server_url = Url::parse("tcp://127.0.0.1:0").unwrap();
            let listener = server_adapter.bind(&server_url).await.unwrap();
            let local_addr = listener
                .as_any()
                .downcast_ref::<super::TcpNetworkListener>()
                .unwrap()
                .listener
                .local_addr()
                .unwrap();

            let listener = std::sync::Arc::new(tokio::sync::Mutex::new(listener));

            // Spawn two clients
            let client1 = tokio::spawn({
                let addr = local_addr;
                async move {
                    let _ = tokio::net::TcpStream::connect(addr).await.unwrap();
                }
            });
            let client2 = tokio::spawn({
                let addr = local_addr;
                async move {
                    let _ = tokio::net::TcpStream::connect(addr).await.unwrap();
                }
            });

            // Accept both clients
            let mut listener = listener.lock().await;
            let _stream1 = listener.accept().await.unwrap();
            let _stream2 = listener.accept().await.unwrap();

            client1.await.unwrap();
            client2.await.unwrap();
        });
    }
}
