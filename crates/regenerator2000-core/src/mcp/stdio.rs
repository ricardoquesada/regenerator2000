//! Standard input/output (stdio) transport loops for the MCP server.
//!
//! This module reads newline-delimited JSON-RPC 2.0 requests from `stdin`, dispatches them
//! to the MCP handler, and writes JSON-RPC 2.0 responses to `stdout`.

use crate::mcp::handler::handle_method;
use crate::mcp::types::{McpRequest, McpResponse};
use crate::state::AppState;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

/// Parsed fields of a JSON-RPC 2.0 message line.
struct ParsedRpcMessage {
    /// Request identifier (`None` when the message is a JSON-RPC 2.0 notification).
    id: Option<Value>,
    /// Method name string.
    method: String,
    /// Method parameter payload.
    params: Value,
}

/// Parses a single line of JSON-RPC 2.0 input into a [`ParsedRpcMessage`].
#[must_use]
fn parse_rpc_line(line: &str) -> Option<ParsedRpcMessage> {
    let payload: Value = serde_json::from_str(line).ok()?;
    let method = payload.get("method")?.as_str()?.to_string();
    let id = payload.get("id").cloned();
    let params = payload.get("params").cloned().unwrap_or(Value::Null);
    Some(ParsedRpcMessage { id, method, params })
}

/// Formats an [`McpResponse`] and request `id` as a JSON-RPC 2.0 response object.
#[must_use]
fn format_rpc_response(id: Value, response: McpResponse) -> Value {
    if let Some(error) = response.error {
        json!({
            "jsonrpc": "2.0",
            "error": {
                "code": error.code,
                "message": error.message,
                "data": error.data
            },
            "id": id
        })
    } else {
        json!({
            "jsonrpc": "2.0",
            "result": response.result,
            "id": id
        })
    }
}

/// Writes a JSON-RPC response line to standard output and flushes the stream.
fn write_rpc_response(id: Value, response: McpResponse) {
    let json_resp = format_rpc_response(id, response);
    println!("{json_resp}");
    let _ = io::stdout().flush();
}

/// Runs the asynchronous MCP stdio loop and forwards requests over `sender` to the main thread.
pub async fn run_stdio_loop(sender: Sender<McpRequest>) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        if let Some(msg) = parse_rpc_line(&line) {
            let (tx, rx) = oneshot::channel();
            let request = McpRequest {
                method: msg.method,
                params: msg.params,
                response_sender: tx,
            };

            if sender.send(request).await.is_ok()
                && let Ok(response) = rx.await
                && let Some(id) = msg.id
            {
                write_rpc_response(id, response);
            }
        }
        line.clear();
    }
}

/// Runs the synchronous headless MCP stdio loop directly against `app_state` and `view_state`.
pub async fn run_headless_stdio_loop(mut app_state: AppState, mut view_state: CoreViewState) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        if let Some(msg) = parse_rpc_line(&line) {
            let response = handle_method(&msg.method, &msg.params, &mut app_state, &mut view_state);
            if let Some(id) = msg.id {
                write_rpc_response(id, response);
            }
        }
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::{make_app_state, make_view_state};

    #[test]
    fn test_parse_rpc_line_request_and_notification() {
        let req = parse_rpc_line(r#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{}}"#)
            .expect("valid request should parse");
        assert_eq!(req.id, Some(json!(7)));
        assert_eq!(req.method, "ping");

        // JSON-RPC notifications omit "id" and must keep `id == None` so no response is emitted.
        let notif = parse_rpc_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .expect("valid notification should parse");
        assert!(notif.id.is_none());
        assert_eq!(notif.method, "notifications/initialized");

        assert!(parse_rpc_line("not json").is_none());
        assert!(parse_rpc_line(r#"{"jsonrpc":"2.0","id":1}"#).is_none());
    }

    #[test]
    fn test_format_rpc_response_success_and_error() {
        let mut app_state = make_app_state(0x1000, 16);
        let mut view_state = make_view_state();

        let ok_resp = handle_method("ping", &Value::Null, &mut app_state, &mut view_state);
        let ok_json = format_rpc_response(json!(1), ok_resp);
        assert_eq!(ok_json["jsonrpc"], "2.0");
        assert_eq!(ok_json["id"], 1);
        assert_eq!(ok_json["result"], json!({}));

        let err_resp = handle_method("unknown", &Value::Null, &mut app_state, &mut view_state);
        let err_json = format_rpc_response(json!(2), err_resp);
        assert_eq!(err_json["jsonrpc"], "2.0");
        assert_eq!(err_json["id"], 2);
        assert_eq!(err_json["error"]["code"], -32601);
    }
}
