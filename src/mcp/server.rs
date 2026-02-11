use axum::{Json, Router, extract::State, routing::post};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::mcp::types::{McpRequest, McpResponse};

#[derive(Clone)]
struct ServerState {
    sender: Sender<McpRequest>,
}

pub async fn run_server(port: u16, sender: Sender<McpRequest>) {
    let state = ServerState { sender };

    let app = Router::new()
        .route("/jsonrpc", post(handle_jsonrpc))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    log::info!("MCP Server listening on {}", addr);

    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                log::error!("MCP Server error: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to bind MCP server port: {}", e);
        }
    };
}

async fn handle_jsonrpc(
    State(state): State<ServerState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
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

        if state.sender.send(request).await.is_ok() {
            match rx.await {
                Ok(response) => return Json(to_jsonrpc_response(id, response)),
                Err(_) => {
                    return Json(json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32603,
                            "message": "Internal error: processing cancelled",
                        },
                        "id": id
                    }));
                }
            }
        }
    }

    Json(json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32600,
            "message": "Invalid Request",
        },
        "id": id
    }))
}

fn to_jsonrpc_response(id: Value, response: McpResponse) -> Value {
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
