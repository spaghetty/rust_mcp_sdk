//! Defines the protocol layer for handling MCP message serialization and deserialization.
//!
//! This layer sits on top of the `NetworkAdapter` and provides a strongly-typed interface
//! for sending and receiving MCP messages. It is responsible for all `serde_json`
//! operations, keeping the client/server logic clean and focused on application tasks.

use crate::adapter::NetworkAdapter;
use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

/// A connection that handles MCP protocol logic over a generic `NetworkAdapter`.
pub struct ProtocolConnection<A: NetworkAdapter> {
    adapter: A,
}

impl<A: NetworkAdapter> ProtocolConnection<A> {
    /// Creates a new `ProtocolConnection` that will use the given adapter for communication.
    pub fn new(adapter: A) -> Self {
        Self { adapter }
    }

    /// Serializes a message struct into a JSON string and sends it via the adapter.
    pub async fn send_serializable<T: Serialize + Send + Sync>(&mut self, msg: T) -> Result<()> {
        let json_string = serde_json::to_string(&msg)?;
        self.adapter.send(&json_string).await
    }

    /// Sends a raw, already-serialized JSON string over the adapter.
    pub async fn send_raw(&mut self, json_string: &str) -> Result<()> {
        self.adapter.send(json_string).await
    }

    /// Receives a raw JSON string from the adapter and deserializes it into a message struct.
    pub async fn recv_message<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        match self.adapter.recv().await? {
            Some(json_string) => {
                let msg = serde_json::from_str::<T>(&json_string)?;
                Ok(Some(msg))
            }
            None => Ok(None), // Connection was closed
        }
    }
}
