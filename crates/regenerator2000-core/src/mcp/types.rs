//! Request, response, and error types for the Model Context Protocol (MCP) transport layer.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;

/// Represents an incoming JSON-RPC request for the MCP dispatcher.
#[derive(Debug)]
pub struct McpRequest {
    /// JSON-RPC method name (for example, `"tools/call"` or `"resources/read"`).
    pub method: String,
    /// JSON parameter payload for the method.
    pub params: Value,
    /// Oneshot channel sender that returns the [`McpResponse`] to the transport caller.
    pub response_sender: oneshot::Sender<McpResponse>,
}

/// Represents the result or error returned by the MCP request handler.
#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    /// JSON result value when the request succeeds.
    pub result: Option<Value>,
    /// Error details when the request fails.
    pub error: Option<McpError>,
}

/// Represents a JSON-RPC 2.0 error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    /// Numeric JSON-RPC 2.0 error code (for example, `-32601` or `-32602`).
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured error data.
    pub data: Option<Value>,
}
