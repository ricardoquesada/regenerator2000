use crate::mcp::handler::handle_request;
use crate::mcp::types::McpRequest;
use crate::state::AppState;
use crate::ui_state::UIState;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

pub async fn run_stdio_loop(sender: Sender<McpRequest>) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    while reader.read_line(&mut line).unwrap() > 0 {
        let payload: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                line.clear();
                continue;
            }
        };

        let id = payload.get("id").cloned().unwrap_or(Value::Null);
        let method = payload
            .get("method")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let params = payload.get("params").cloned().unwrap_or(Value::Null);

        if let Some(method) = method {
            let (tx, rx) = oneshot::channel();
            let request = McpRequest {
                method,
                params,
                response_sender: tx,
            };

            if sender.send(request).await.is_ok()
                && let Ok(response) = rx.await
            {
                let json_resp = if let Some(error) = response.error {
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
                };
                println!("{}", json_resp);
                io::stdout().flush().unwrap();
            }
        }
        line.clear();
    }
}

// For truly headless mode where we don't bridge to a TUI thread
pub async fn run_headless_stdio_loop(mut app_state: AppState, mut ui_state: UIState) {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut line = String::new();

    while reader.read_line(&mut line).unwrap() > 0 {
        let payload: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                line.clear();
                continue;
            }
        };

        let id = payload.get("id").cloned().unwrap_or(Value::Null);
        let method = payload
            .get("method")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let params = payload.get("params").cloned().unwrap_or(Value::Null);

        if let Some(method) = method {
            let request = McpRequest {
                method,
                params,
                // In headless mode we don't need the oneshot because we handle it synchronously here
                response_sender: oneshot::channel().0,
            };

            // We need a slightly different handle_request or just use the existing one but handle the oneshot manually?
            // Actually, handle_request returns McpResponse directly.
            let response = handle_request(&request, &mut app_state, &mut ui_state);

            let json_resp = if let Some(error) = response.error {
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
            };
            println!("{}", json_resp);
            io::stdout().flush().unwrap();
        }
        line.clear();
    }
}
