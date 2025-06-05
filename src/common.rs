use crate::types::{InitializeRequestParams, PaginatedRequestParams};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub message: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    pub params: InitializeRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResourcesRequest {
    pub params: PaginatedRequestParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsRequest {
    pub params: PaginatedRequestParams,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_session_message_serialization() {
        let msg = SessionMessage {
            message: serde_json::json!({"foo": "bar"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: SessionMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.message, deserialized.message);
    }
}
