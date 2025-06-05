pub mod client;
pub mod common;
pub mod server;
pub mod types;

pub use common::*;
pub use types::*;

pub type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("Internal error: {0}")]
    Internal(String),
}
