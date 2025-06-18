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
    use serde_json::Value;
    use tokio::sync::OnceCell;
    use tracing::{info, warn}; // Added warn

    // Conditional imports
    #[cfg(not(test))]
    use reqwest;
    #[cfg(test)]
    use std::{fs, path::Path}; // reqwest only needed for non-test

    const SCHEMA_URL_CONST: &str = "https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/main/schema/**/schema.json";

    static ASYNC_INIT_SCHEMA: OnceCell<jsonschema::Validator> = OnceCell::const_new();

    async fn get_or_init_schema() -> &'static jsonschema::Validator {
        ASYNC_INIT_SCHEMA.get_or_init(|| async {
            info!("[Validator] Initializing schema (async)...");

            let schema_content_loader = || -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
                let schema_url_val = String::from(SCHEMA_URL_CONST).replace("**", LATEST_PROTOCOL_VERSION);

                #[cfg(test)]
                {
                    let version = LATEST_PROTOCOL_VERSION;
                    let local_schema_path_str = format!("schemas/{}/schema.json", version);
                    let local_schema_path = Path::new(&local_schema_path_str);

                    info!("[Validator] TEST MODE: Attempting to load schema from local file: {}", local_schema_path_str);
                    if local_schema_path.exists() {
                        match fs::read_to_string(local_schema_path) {
                            Ok(file_content) => {
                                match serde_json::from_str::<Value>(&file_content) {
                                    Ok(schema_value) => {
                                        info!("[Validator] TEST MODE: Successfully loaded schema from local file: {}", local_schema_path_str);
                                        return Ok(schema_value);
                                    }
                                    Err(e) => {
                                        warn!("[Validator] TEST MODE: Failed to parse local schema JSON from '{}'. Error: {}. Falling back to network fetch.", local_schema_path_str, e);
                                        // Proceed to network fetch below
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("[Validator] TEST MODE: Failed to read local schema file '{}'. Error: {}. Falling back to network fetch.", local_schema_path_str, e);
                                // Proceed to network fetch below
                            }
                        }
                    } else {
                        info!("[Validator] TEST MODE: Local schema file not found at '{}'. Falling back to network fetch.", local_schema_path_str);
                        // Proceed to network fetch below
                    }
                }

                // Network fetch (executes if not test, or if test mode failed to return Ok above)
                info!("[Validator] Fetching schema from URL: {}", schema_url_val);
                let fetched_value = reqwest::blocking::get(schema_url_val)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                    .json::<Value>()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
                Ok(fetched_value)
            };

            let schema_value = match tokio::task::spawn_blocking(schema_content_loader).await {
                Ok(Ok(val)) => val,
                Ok(Err(e)) => panic!("Schema content loader failed: {}", e),
                Err(join_err) => panic!("Spawn_blocking for schema content loader panicked: {}", join_err),
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
            Err(validation_error) => {
                // validation_error is a single ValidationError struct
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
                    info!(
                        "[Validator] Message is valid after async validation: {}",
                        value
                    );
                }
                Err(e) => {
                    // Log the detailed error here before returning it
                    error!(
                        "[Validator] Schema validation failed for value {}: {}",
                        value, e
                    );
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
