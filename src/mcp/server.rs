use axum::extract::Query;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tokio::sync::oneshot;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

use crate::mcp::types::{McpRequest, McpResponse};

type SseSender = UnboundedSender<Result<Event, Infallible>>;
type SessionMap = Arc<RwLock<HashMap<String, SseSender>>>;

#[derive(Clone)]
struct ServerState {
    sender: Sender<McpRequest>,
    sessions: SessionMap,
}

pub async fn run_server(port: u16, sender: Sender<McpRequest>) {
    let state = ServerState {
        sender,
        sessions: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/sse", get(handle_sse))
        .route("/messages", post(handle_messages))
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

async fn handle_sse(
    State(state): State<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let stream = UnboundedReceiverStream::new(rx);
    let session_id = Uuid::new_v4().to_string();

    {
        let mut sessions = state.sessions.write().unwrap();
        sessions.insert(session_id.clone(), tx.clone());
    }

    // Send the endpoint event immediately
    let _ = tx.send(Ok(Event::default()
        .event("endpoint")
        .data(format!("/messages?sessionId={}", session_id))));

    // We need to remove the session when the stream disconnects?
    // Axum doesn't easily notify on disconnect here without extended logic,
    // but for now let's just keep it in the map. A real implementation should have cleanup.

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
struct MessageQuery {
    #[serde(rename = "sessionId")]
    session_id: String,
}

async fn handle_messages(
    State(state): State<ServerState>,
    Query(query): Query<MessageQuery>,
    Json(payload): Json<Value>,
) -> axum::http::StatusCode {
    let session_id = query.session_id;

    // Check if session exists first
    let has_session = {
        let sessions = state.sessions.read().unwrap();
        sessions.contains_key(&session_id)
    };

    if !has_session {
        return axum::http::StatusCode::NOT_FOUND;
    }

    // Process request asynchronously
    let sender = state.sender.clone();
    let sessions = state.sessions.clone(); // Access to sessions for responding

    tokio::spawn(async move {
        // Extract method/params similar to handle_jsonrpc
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
                let json_resp = to_jsonrpc_response(id, response);
                // Send back via SSE
                if let Ok(sessions_lock) = sessions.read()
                    && let Some(sse_tx) = sessions_lock.get(&session_id)
                {
                    let _ = sse_tx.send(Ok(Event::default()
                        .event("message")
                        .data(json_resp.to_string())));
                }
            }
        }
    });

    axum::http::StatusCode::ACCEPTED
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
