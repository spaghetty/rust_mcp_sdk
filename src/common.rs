use serde::{Deserialize, Serialize};
use crate::types::{InitializeRequestParams, PaginatedRequestParams};

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
