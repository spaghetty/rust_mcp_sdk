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
    use reqwest; // For HTTP client
    use serde_json::Value;
    use tokio::sync::OnceCell; // For async OnceCell
    use tracing::info; // error is no longer used in this module directly

    // The official URL for the raw JSON schema file.
    // This needs to be accessible by get_or_init_schema, so ensure correct path.
    // It was `super::SCHEMA_URL` in prompt, but `SCHEMA_URL` is in the same module.
    const SCHEMA_URL_VAL: &str = "https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/**/schema.json";


    // Use tokio::sync::OnceCell for async initialization
    // Assuming jsonschema::Validator is the correct type alias or struct for the compiled schema object.
    // If jsonschema::validator_for returns jsonschema::JSONSchema, this should be jsonschema::JSONSchema.
    // Let's assume jsonschema::Validator is an alias or the type returned by validator_for.
    static ASYNC_INIT_SCHEMA: OnceCell<jsonschema::Validator> = OnceCell::const_new();

    async fn get_or_init_schema() -> &'static jsonschema::Validator {
        ASYNC_INIT_SCHEMA.get_or_init(|| async {
            info!("[Validator] Fetching and compiling official MCP schema from URL (async)...");
            let schema_url = String::from(SCHEMA_URL_VAL).replace("**", LATEST_PROTOCOL_VERSION);

            // Perform blocking HTTP get and JSON parsing in spawn_blocking
            let schema_value = match tokio::task::spawn_blocking(move || {
                reqwest::blocking::get(schema_url)?
                    .json::<Value>() // Value is serde_json::Value
            }).await {
                Ok(Ok(val)) => val,
                Ok(Err(e)) => panic!("Failed to fetch or parse schema JSON: {}", e), // Or convert to your Error type
                Err(join_err) => panic!("Failed to join spawn_blocking for schema fetch: {}", join_err),
            };

            // Perform blocking schema compilation in spawn_blocking
            let compiled_validator = match tokio::task::spawn_blocking(move || {
                jsonschema::validator_for(&schema_value) // Using the original API call
                    .expect("Failed to compile official MCP schema (from spawn_blocking)")
            }).await {
                Ok(validator) => validator,
                Err(join_err) => panic!("Spawn_blocking for schema compilation panicked: {}", join_err),
            };

            info!("[Validator] Schema successfully compiled (async).");
            compiled_validator
        }).await
    }

    /// Validates a given JSON-RPC message (Request, Response, etc.) against the root schema.
    pub async fn validate_message(value: &Value) -> Result<()> {
        let validator_instance = get_or_init_schema().await;
        match validator_instance.validate(value) {
            Ok(_) => Ok(()),
            Err(validation_error) => { // validation_error is a single ValidationError struct
                Err(Error::Other(format!(
                    "Schema validation failed: {}",
                    validation_error.to_string() // Convert the single error to string
                )))
            }
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
            match validator::validate_message(&value).await {
                Ok(_) => {
                    info!("[Validator] Message is valid after async validation: {}", value);
                }
                Err(e) => {
                    // Log the detailed error here before returning it
                    error!("[Validator] Schema validation failed for value {}: {}", value, e);
                    return Err(e); // Propagate the error
                }
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
