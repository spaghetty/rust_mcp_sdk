// src/network_adapter/trait.rs
use crate::error::Result;
use async_trait::async_trait;

#[async_trait]
pub trait NetworkAdapter: Send + Sync {
    async fn send(&mut self, msg: &str) -> Result<()>;
    async fn recv(&mut self) -> Result<Option<String>>;
}
