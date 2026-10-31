use rmcp::{
    ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation,
        InitializeRequestParams, InitializeResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
        ReadResourceResult, ServerCapabilities, ServerConfig, SubscriptionFilter,
    },
    service::{RequestContext, RoleServer},
    transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService},
};
use serde_json::Value;

use axum;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::mcp::handler::{MCP_SERVER_DESCRIPTION, MCP_SERVER_INSTRUCTIONS, MCP_SERVER_NAME};
use crate::mcp::types::McpRequest;

#[derive(Clone)]
pub struct RegeneratorOps {
    sender: Sender<McpRequest>,
}

impl RegeneratorOps {
    #[must_use]
    pub fn new(sender: Sender<McpRequest>) -> Self {
        Self { sender }
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value, String> {
        let (tx, rx) = oneshot::channel();
        let request = McpRequest {
            method: method.to_string(),
            params,
            response_sender: tx,
        };

        self.sender
            .send(request)
            .await
            .map_err(|e| format!("Failed to send request: {e}"))?;

        let response = rx
            .await
            .map_err(|e| format!("Failed to receive response: {e}"))?;

        if let Some(error) = response.error {
            Err(error.message)
        } else {
            Ok(response.result.unwrap_or(Value::Null))
        }
    }
}

impl ServerHandler for RegeneratorOps {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(
            Implementation::new(MCP_SERVER_NAME, env!("CARGO_PKG_VERSION"))
                .with_description(MCP_SERVER_DESCRIPTION),
        )
        .with_instructions(MCP_SERVER_INSTRUCTIONS)
    }

    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        Some(requested.clone())
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        context.peer.set_peer_info(request.clone());
        self.send_request(
            "initialize",
            serde_json::to_value(&request).unwrap_or(Value::Null),
        )
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;
        self.negotiate_initialize(&request)
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let params = serde_json::to_value(request).unwrap_or(Value::Null);
        match self.send_request("tools/list", params).await {
            Ok(val) => serde_json::from_value(val).map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to parse tools list: {e}"), None)
            }),
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let params = serde_json::json!({
            "name": request.name,
            "arguments": request.arguments
        });

        match self.send_request("tools/call", params).await {
            Ok(val) => {
                let result: CallToolResult = serde_json::from_value(val).map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to parse tool result: {e}"),
                        None,
                    )
                })?;
                Ok(CallToolResponse::Complete(result))
            }
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        let params = serde_json::to_value(request).unwrap_or(Value::Null);
        match self.send_request("resources/list", params).await {
            Ok(val) => serde_json::from_value(val).map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed to parse resources list: {e}"),
                    None,
                )
            }),
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, rmcp::ErrorData> {
        let params = serde_json::json!({ "uri": request.uri });
        match self.send_request("resources/read", params).await {
            Ok(val) => {
                let result: ReadResourceResult = serde_json::from_value(val).map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to parse read resource result: {e}"),
                        None,
                    )
                })?;
                Ok(ReadResourceResponse::Complete(result))
            }
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }
}

/// Runs the MCP HTTP server on the specified port.
///
/// # Errors
/// Returns an error if the server fails to bind to the port or encounters an error while serving.
pub async fn run_server(port: u16, sender: Sender<McpRequest>) -> std::io::Result<()> {
    let (_tx, rx) = tokio::sync::oneshot::channel();
    run_server_with_shutdown(port, sender, rx).await
}

/// Runs the MCP HTTP server on the specified port with graceful shutdown support.
///
/// # Errors
/// Returns an error if the server fails to bind to the port or encounters an error while serving.
pub async fn run_server_with_shutdown(
    port: u16,
    sender: Sender<McpRequest>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> std::io::Result<()> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_listener_with_shutdown(listener, sender, shutdown_rx).await
}

/// Serves the MCP HTTP router on an already-bound [`tokio::net::TcpListener`] with graceful shutdown support.
///
/// Separating socket binding from request serving allows callers (such as the TUI background
/// server thread) to verify that the TCP port bound successfully before emitting a "server started"
/// event, and enables deterministic ephemeral-port (`127.0.0.1:0`) testing without port collisions.
///
/// # Errors
/// Returns an error if [`axum::serve`] encounters an I/O error while serving.
pub async fn serve_listener_with_shutdown(
    listener: tokio::net::TcpListener,
    sender: Sender<McpRequest>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> std::io::Result<()> {
    let handler = RegeneratorOps::new(sender);
    let handler_clone = handler.clone();

    // Create the session manager.
    // Disable the default 5-minute keep_alive timeout: Regenerator 2000 is a
    // local desktop app where MCP sessions can be idle for extended periods.
    let session_manager = Arc::new({
        use rmcp::transport::streamable_http_server::session::local::SessionConfig;
        let mut mgr =
            rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default();
        let mut sc = SessionConfig::default();
        sc.keep_alive = None;
        mgr.session_config = sc;
        mgr
    });

    // Create the service
    let service = StreamableHttpService::new(move || Ok(handler_clone.clone()), session_manager, {
        let mut config = StreamableHttpServerConfig::default();
        // Disable SSE priming events (retry:) — clients like Antigravity
        // try to parse every SSE event's data as JSON-RPC, and priming
        // events have no `data:` field, causing "unexpected end of JSON input".
        config.sse_retry = None;
        config
    });

    // Nest the MCP service into an Axum router
    let app = axum::Router::new().nest_service("/mcp", service);

    if let Ok(local_addr) = listener.local_addr() {
        log::info!(
            "MCP Live Server active on http://127.0.0.1:{}/mcp",
            local_addr.port()
        );
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{ClientCapabilities, DiscoverResult, ProtocolVersion};

    fn dummy_ops() -> RegeneratorOps {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        RegeneratorOps::new(tx)
    }

    #[test]
    fn get_info_advertises_tools_resources_and_metadata() {
        let ops = dummy_ops();
        let info = ops.get_info();

        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.resources.is_some());
        assert_eq!(info.server_info.name, MCP_SERVER_NAME);
        assert_eq!(
            info.server_info.description.as_deref(),
            Some(MCP_SERVER_DESCRIPTION)
        );
        assert_eq!(info.instructions.as_deref(), Some(MCP_SERVER_INSTRUCTIONS));
    }

    #[test]
    fn discover_result_from_server_info_preserves_capabilities() {
        let ops = dummy_ops();
        let discover = DiscoverResult::from_server_info(
            ops.supported_protocol_versions().into_owned(),
            ops.get_info(),
        );

        assert!(
            discover
                .supported_versions
                .contains(&ProtocolVersion::V_2026_07_28)
        );
        assert!(discover.capabilities.tools.is_some());
        assert!(discover.capabilities.resources.is_some());
        assert_eq!(
            discover.instructions.as_deref(),
            Some(MCP_SERVER_INSTRUCTIONS)
        );
        let server_info = discover.server_info().expect("server_info in _meta");
        assert_eq!(server_info.name, MCP_SERVER_NAME);
    }

    #[test]
    fn negotiate_initialize_supports_legacy_protocol_versions() {
        let ops = dummy_ops();
        let req = InitializeRequestParams::new(
            ClientCapabilities::default(),
            Implementation::new("test-client", "1.0.0"),
        )
        .with_protocol_version(ProtocolVersion::V_2024_11_05);

        let result = ops
            .negotiate_initialize(&req)
            .expect("negotiate_initialize should succeed");
        assert_eq!(result.protocol_version, ProtocolVersion::V_2024_11_05);
        assert!(result.capabilities.tools.is_some());
        assert!(result.capabilities.resources.is_some());
        assert_eq!(result.server_info.name, MCP_SERVER_NAME);
    }

    #[test]
    fn accepted_subscription_filter_accepts_client_subscriptions() {
        let ops = dummy_ops();
        let mut filter = SubscriptionFilter::default();
        filter.tools_list_changed = Some(true);

        let accepted = ops
            .accepted_subscription_filter(&filter)
            .expect("subscriptions/listen must be accepted");
        assert_eq!(accepted.tools_list_changed, Some(true));
    }
}
