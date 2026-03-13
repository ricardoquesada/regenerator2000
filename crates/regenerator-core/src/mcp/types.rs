use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Debug)]
pub struct McpRequest {
    pub method: String,
    pub params: Value,
    pub response_sender: oneshot::Sender<McpResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub result: Option<Value>,
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}
