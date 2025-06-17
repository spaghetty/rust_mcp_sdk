//! Defines the protocol layer for handling MCP message serialization and deserialization.
//!
//! This layer sits on top of the `NetworkAdapter` and provides a strongly-typed interface
//! for sending and receiving MCP messages. It is responsible for all `serde_json`
//! operations, keeping the client/server logic clean and focused on application tasks.

use crate::error::Result;
use crate::network_adapter::NetworkAdapter;
use serde::{de::DeserializeOwned, Serialize};
#[cfg(feature = "schema-validation")]
use tracing::{error, info};

#[cfg(feature = "schema-validation")]
mod validator {
    use super::*;
    use crate::{types::LATEST_PROTOCOL_VERSION, Error};
    use jsonschema;
    use once_cell::sync::Lazy; // To ensure we only fetch the schema once.
    use reqwest;
    use serde_json::Value;

    // The official URL for the raw JSON schema file.
    const SCHEMA_URL: &str = "https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/**/schema.json";

    // This static variable will fetch and compile the schema exactly once.
    // The first time it's accessed, the code inside the closure will run.
    // Subsequent accesses will just get the cached, compiled schema.
    static COMPILED_SCHEMA: Lazy<jsonschema::Validator> = Lazy::new(|| {
        info!("[Validator] Fetching and compiling official MCP schema from URL...");
        let schema_url = String::from(SCHEMA_URL).replace("**", LATEST_PROTOCOL_VERSION);
        // Use a blocking HTTP client for this one-time fetch.
        let schema_value: Value = reqwest::blocking::get(schema_url)
            .expect("Failed to fetch schema from URL")
            .json()
            .expect("Failed to parse schema JSON");

        let validator = jsonschema::validator_for(&schema_value)
            .expect("Failed to compile official MCP schema");

        info!("[Validator] Schema successfully compiled.");
        validator
    });
    /// Validates a given JSON-RPC message (Request, Response, etc.) against the root schema.
    /// The schema itself contains definitions for all message types.
    pub fn validate_message(value: &Value) -> Result<()> {
        match COMPILED_SCHEMA.validate(value) {
            Ok(_) => Ok(()),
            Err(error) => Err(Error::Other(format!(
                "Schema validation failed: {}",
                error.to_string()
            ))),
        }
    }
}

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
        let value = serde_json::to_value(&msg)?;

        #[cfg(feature = "schema-validation")]
        {
            // If the feature is enabled, validate the JSON value before sending.
            match validator::validate_message(&value) {
                Ok(_) => info!("[Vaidator] Message is valid {}", value),
                Err(e) => error!("[Validator] message {} is not valid for: {}", value, e),
            }
        }
        let json_string = serde_json::to_string(&value)?;
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
                // Return an error if the string is empty, as it's not valid JSON
                if json_string.trim().is_empty() {
                    return Ok(None);
                }
                let msg = serde_json::from_str::<T>(&json_string)?;
                Ok(Some(msg))
            }
            None => Ok(None), // Connection was closed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CallToolParams, Request, RequestId, Response};
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// A mock adapter that uses an in-memory queue instead of a real network.
    /// This allows us to test the `ProtocolConnection` without any I/O.
    struct InMemoryAdapter {
        // We use a Mutex here to allow for safe concurrent access,
        // which is a good practice even in single-threaded tests for async code.
        buffer: Mutex<VecDeque<String>>,
    }

    impl InMemoryAdapter {
        fn new() -> Self {
            Self {
                buffer: Mutex::new(VecDeque::new()),
            }
        }
    }

    #[async_trait]
    impl NetworkAdapter for InMemoryAdapter {
        async fn send(&mut self, msg: &str) -> Result<()> {
            self.buffer.lock().unwrap().push_back(msg.to_string());
            Ok(())
        }

        async fn recv(&mut self) -> Result<Option<String>> {
            Ok(self.buffer.lock().unwrap().pop_front())
        }
    }

    #[tokio::test]
    async fn test_protocol_connection_send_recv() {
        // 1. Setup
        let adapter = InMemoryAdapter::new();
        let mut proto_conn = ProtocolConnection::new(adapter);

        // 2. Create a typed message (a request to call a tool)
        let request = Request {
            jsonrpc: "2.0".to_string(),
            id: RequestId::Num(123),
            method: "tools/call".to_string(),
            params: CallToolParams {
                name: "test-tool".to_string(),
                arguments: json!({ "arg1": "value1" }),
            },
        };

        // 3. Send the message. This will serialize it and put it in the mock adapter's buffer.
        proto_conn.send_serializable(request.clone()).await.unwrap();

        // 4. Receive the message. This will take it from the buffer and deserialize it.
        let received_request: Option<Request<CallToolParams>> =
            proto_conn.recv_message().await.unwrap();

        // 5. Assert that the received message is identical to the one we sent.
        assert_eq!(Some(request), received_request);
    }

    #[tokio::test]
    async fn test_protocol_connection_receives_none_on_empty() {
        let adapter = InMemoryAdapter::new();
        let mut proto_conn = ProtocolConnection::new(adapter);

        // Receive from an empty buffer should yield None, simulating a closed connection.
        let received: Option<Response<()>> = proto_conn.recv_message().await.unwrap();
        assert!(received.is_none());
    }
}
