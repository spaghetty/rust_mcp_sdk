#[cfg(test)]
mod typed_tool_integration_tests {
    use async_trait::async_trait;
    use mcp_sdk::{
        error::{Error as SdkError, Result as SdkResult},
        network_adapter::NetworkAdapter,
        protocol::ProtocolConnection,
        server::{ConnectionHandle as ServerConnectionHandle, Server, ServerSession},
        types::{
            CallToolParams, CallToolResult, Content, JSONRPCResponse, Request, RequestId, Tool,
        }, // Removed unused Notification, Response
        ToolArguments, // Removed unused ToolArgumentsDescriptor
    };
    use serde::Deserialize; // Removed unused Serialize, de::DeserializeOwned
    use serde_json::{json, Value};
    use std::{
        collections::VecDeque,
        // Removed unused Future, Pin
        sync::{Arc, Mutex},
    };

    // --- Test Structs ---
    #[derive(ToolArguments, Deserialize, Debug, PartialEq, Clone)]
    struct SimpleTypedArgs {
        message: String,
        count: i32,
    }

    #[derive(ToolArguments, Deserialize, Debug, PartialEq, Clone)]
    struct OptionalTypedArgs {
        id: String,
        #[tool_arg(required = false)]
        value: Option<i32>,
    }

    // --- Mock Infrastructure (adapted from server/session.rs tests) ---
    #[derive(Default, Clone)]
    struct MockAdapter {
        incoming: Arc<Mutex<VecDeque<String>>>,
        outgoing: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockAdapter {
        fn new() -> Self {
            Default::default()
        }

        fn push_incoming(&self, msg: String) {
            self.incoming.lock().unwrap().push_back(msg);
        }

        fn pop_outgoing(&self) -> Option<String> {
            self.outgoing.lock().unwrap().pop_front()
        }

        #[allow(dead_code)] // May be useful for debugging later
        fn outgoing_len(&self) -> usize {
            self.outgoing.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl NetworkAdapter for MockAdapter {
        async fn send(&mut self, msg: &str) -> SdkResult<()> {
            self.outgoing.lock().unwrap().push_back(msg.to_string());
            Ok(())
        }
        async fn recv(&mut self) -> SdkResult<Option<String>> {
            Ok(self.incoming.lock().unwrap().pop_front())
        }
    }

    struct TestServerHarness {
        server: Arc<Server>,
    }

    impl TestServerHarness {
        fn new(server: Server) -> Self {
            Self {
                server: Arc::new(server),
            }
        }

        async fn simulate_request(
            &self,
            request_json: String, // Full JSON-RPC request string
        ) -> SdkResult<Option<String>> {
            // Returns raw JSON string of the first response after init

            let adapter = MockAdapter::new();

            // Standard Initialize Request
            let init_req_params = json!({
                "protocolVersion": mcp_sdk::types::LATEST_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "test-harness-client", "version": "0.1.0"}
            });
            let init_req = Request {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Num(0),
                method: "initialize".to_string(),
                params: init_req_params,
            };
            let init_req_json = serde_json::to_string(&init_req)?;
            adapter.push_incoming(init_req_json);

            // Actual request from the test
            adapter.push_incoming(request_json);

            let session_runner = ServerSession::new(
                ProtocolConnection::new(adapter.clone()), // Clone adapter for session
                Arc::clone(&self.server),
            );

            match tokio::time::timeout(std::time::Duration::from_secs(3), session_runner.run())
                .await
            {
                Ok(Ok(_)) => {
                    // Session completed
                    let _init_response_json = adapter.pop_outgoing(); // Pop init response
                                                                      // assert!(_init_response_json.is_some(), "Harness expected init response");
                    Ok(adapter.pop_outgoing()) // Return the actual tool response
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(SdkError::Other(
                    "TestServerHarness: session run timed out".to_string(),
                )),
            }
        }

        async fn call_tool(
            &self,
            tool_name: &str,
            args: Value,
            request_id_num: i64, // Simplified to use numeric IDs for tests
        ) -> SdkResult<Option<String>> {
            let call_params = CallToolParams {
                name: tool_name.to_string(),
                arguments: args,
            };
            let request = Request {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Num(request_id_num),
                method: "tools/call".to_string(),
                params: call_params,
            };
            let request_json = serde_json::to_string(&request)?;
            self.simulate_request(request_json).await
        }
    }

    // --- Test Cases ---
    #[tokio::test]
    async fn test_typed_tool_successful_call() {
        let server = Server::new("test-server-typed-success").register_tool_typed(
            Tool::from_args::<SimpleTypedArgs>("echo_simple", Some("Echoes simple args.")),
            |_handle: ServerConnectionHandle, args: SimpleTypedArgs| async move {
                Ok(CallToolResult {
                    content: vec![Content::Text {
                        text: format!("msg: {}, count: {}", args.message, args.count),
                    }],
                    is_error: false,
                })
            },
        );

        let harness = TestServerHarness::new(server);
        let response_json_str = harness
            .call_tool(
                "echo_simple",
                json!({"message": "hello", "count": 42}),
                1, // request_id
            )
            .await
            .unwrap()
            .expect("Expected a response for echo_simple tool call");

        let response_value: JSONRPCResponse<CallToolResult> =
            serde_json::from_str(&response_json_str).unwrap();

        match response_value {
            JSONRPCResponse::Success(res) => {
                assert_eq!(res.id, RequestId::Num(1));
                let tool_result = res.result;
                assert!(!tool_result.is_error);
                assert_eq!(
                    tool_result.content,
                    vec![Content::Text {
                        text: "msg: hello, count: 42".into()
                    }]
                );
            }
            JSONRPCResponse::Error(err) => {
                panic!("Expected success, got error: {:?}", err);
            }
        }
    }

    #[tokio::test]
    async fn test_typed_tool_missing_required_arg() {
        let server = Server::new("test-server-typed-missing").register_tool_typed(
            Tool::from_args::<SimpleTypedArgs>("check_simple_missing", Some("Checks simple args.")),
            |_handle: ServerConnectionHandle, _args: SimpleTypedArgs| async move {
                Ok(CallToolResult::default())
            },
        );

        let harness = TestServerHarness::new(server);
        let response_json_str = harness
            .call_tool(
                "check_simple_missing",
                json!({"message": "hello"}), // "count" is missing
                2,                           // request_id
            )
            .await
            .unwrap()
            .expect("Expected a response for check_simple_missing tool call");

        let response_value: JSONRPCResponse<CallToolResult> =
            serde_json::from_str(&response_json_str).unwrap();

        match response_value {
            JSONRPCResponse::Success(res) => {
                let tool_result = res.result;
                assert!(
                    tool_result.is_error,
                    "Expected CallToolResult.is_error to be true for missing args"
                );
                if let Some(Content::Text { text }) = tool_result.content.get(0) {
                    assert!(
                        text.contains("Invalid arguments for tool 'check_simple_missing'"),
                        "Error message prefix mismatch. Got: {}",
                        text
                    );
                    assert!(
                        text.contains("missing field `count`"),
                        "Error message detail mismatch. Got: {}",
                        text
                    );
                } else {
                    panic!(
                        "Expected text error content for missing args. Got: {:?}",
                        tool_result.content
                    );
                }
            }
            JSONRPCResponse::Error(err) => {
                panic!("Expected CallToolResult with is_error=true, but got JSON-RPC ErrorResponse: {:?}", err);
            }
        }
    }

    #[tokio::test]
    async fn test_typed_tool_wrong_arg_type() {
        let server = Server::new("test-server-typed-wrongtype").register_tool_typed(
            Tool::from_args::<SimpleTypedArgs>(
                "check_simple_type_wrong",
                Some("Checks simple args type."),
            ),
            |_handle: ServerConnectionHandle, _args: SimpleTypedArgs| async move {
                Ok(CallToolResult::default())
            },
        );

        let harness = TestServerHarness::new(server);
        let response_json_str = harness
            .call_tool(
                "check_simple_type_wrong",
                json!({"message": "hello", "count": "not-a-number"}),
                3, // request_id
            )
            .await
            .unwrap()
            .expect("Expected a response for check_simple_type_wrong tool call");

        let response_value: JSONRPCResponse<CallToolResult> =
            serde_json::from_str(&response_json_str).unwrap();

        match response_value {
            JSONRPCResponse::Success(res) => {
                let tool_result = res.result;
                assert!(
                    tool_result.is_error,
                    "Expected CallToolResult.is_error to be true for wrong arg type"
                );
                if let Some(Content::Text { text }) = tool_result.content.get(0) {
                    assert!(
                        text.contains("Invalid arguments for tool 'check_simple_type_wrong'"),
                        "Error message prefix mismatch. Got: {}",
                        text
                    );
                    assert!(
                        text.contains("invalid type: string \"not-a-number\", expected i32"),
                        "Error message detail mismatch. Got: {}",
                        text
                    );
                } else {
                    panic!(
                        "Expected text error content for wrong arg type. Got: {:?}",
                        tool_result.content
                    );
                }
            }
            JSONRPCResponse::Error(err) => {
                panic!("Expected CallToolResult with is_error=true, but got JSON-RPC ErrorResponse: {:?}", err);
            }
        }
    }

    #[tokio::test]
    async fn test_typed_tool_optional_args() {
        let server_config = Server::new("test-server-typed-optional") // Define server config once
            .register_tool_typed(
                Tool::from_args::<OptionalTypedArgs>(
                    "echo_optional",
                    Some("Echoes optional args."),
                ),
                |_handle: ServerConnectionHandle, args: OptionalTypedArgs| async move {
                    let val_str = args.value.map_or("None".to_string(), |v| v.to_string());
                    Ok(CallToolResult {
                        content: vec![Content::Text {
                            text: format!("id: {}, value: {}", args.id, val_str),
                        }],
                        is_error: false,
                    })
                },
            );

        // Test case 1: value present
        let harness1 = TestServerHarness::new(server_config.clone()); // Clone server_config for harness
        let response1_json_str = harness1
            .call_tool(
                "echo_optional",
                json!({"id": "id1", "value": 123}),
                4, // request_id
            )
            .await
            .unwrap()
            .expect("Expected a response for echo_optional (value present)");

        let response1_value: JSONRPCResponse<CallToolResult> =
            serde_json::from_str(&response1_json_str).unwrap();
        match response1_value {
            JSONRPCResponse::Success(res) => {
                assert!(!res.result.is_error);
                assert_eq!(
                    res.result.content,
                    vec![Content::Text {
                        text: "id: id1, value: 123".into()
                    }]
                );
            }
            JSONRPCResponse::Error(err) => {
                panic!("Expected success for value present, got error: {:?}", err)
            }
        }

        // Test case 2: value absent
        let harness2 = TestServerHarness::new(server_config); // Use server_config (can be cloned again or moved if last use)
        let response2_json_str = harness2
            .call_tool(
                "echo_optional",
                json!({"id": "id2"}), // value is absent
                5,                    // request_id
            )
            .await
            .unwrap()
            .expect("Expected a response for echo_optional (value absent)");

        let response2_value: JSONRPCResponse<CallToolResult> =
            serde_json::from_str(&response2_json_str).unwrap();
        match response2_value {
            JSONRPCResponse::Success(res) => {
                assert!(!res.result.is_error);
                assert_eq!(
                    res.result.content,
                    vec![Content::Text {
                        text: "id: id2, value: None".into()
                    }]
                );
            }
            JSONRPCResponse::Error(err) => {
                panic!("Expected success for value absent, got error: {:?}", err)
            }
        }
    }
}
