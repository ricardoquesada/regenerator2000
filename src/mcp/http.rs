use rmcp::{
    ServerHandler,
    model::*,
    service::{RequestContext, RoleServer},
    transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService},
};
use serde_json::Value;

use axum;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::mcp::types::McpRequest;

#[derive(Clone)]
pub struct RegeneratorOps {
    sender: Sender<McpRequest>,
}

impl RegeneratorOps {
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
            .map_err(|e| format!("Failed to send request: {}", e))?;

        let response = rx
            .await
            .map_err(|e| format!("Failed to receive response: {}", e))?;

        if let Some(error) = response.error {
            Err(error.message)
        } else {
            Ok(response.result.unwrap_or(Value::Null))
        }
    }
}

impl ServerHandler for RegeneratorOps {
    // ... (rest of implementation remains same, just ensuring imports are correct)
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        match self
            .send_request("initialize", serde_json::to_value(request).unwrap())
            .await
        {
            Ok(val) => {
                let result: InitializeResult = serde_json::from_value(val).map_err(|e| {
                    rmcp::ErrorData::internal_error(
                        format!("Failed to parse initialize result: {}", e),
                        None,
                    )
                })?;
                Ok(result)
            }
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let params = serde_json::to_value(request).unwrap_or(Value::Null);
        match self.send_request("tools/list", params).await {
            Ok(val) => serde_json::from_value(val).map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to parse tools list: {}", e), None)
            }),
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let params = serde_json::json!({
            "name": request.name,
            "arguments": request.arguments
        });

        match self.send_request("tools/call", params).await {
            Ok(val) => serde_json::from_value(val).map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to parse tool result: {}", e), None)
            }),
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
                    format!("Failed to parse resources list: {}", e),
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
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let params = serde_json::json!({ "uri": request.uri });
        match self.send_request("resources/read", params).await {
            Ok(val) => serde_json::from_value(val).map_err(|e| {
                rmcp::ErrorData::internal_error(
                    format!("Failed to parse read resource result: {}", e),
                    None,
                )
            }),
            Err(e) => Err(rmcp::ErrorData::internal_error(e, None)),
        }
    }
}

pub async fn run_server(port: u16, sender: Sender<McpRequest>) -> std::io::Result<()> {
    let handler = RegeneratorOps::new(sender);
    let handler_clone = handler.clone();

    // Create the session manager
    let session_manager = Arc::new(
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default(),
    );

    // Create the service
    let service = StreamableHttpService::new(
        move || Ok(handler_clone.clone()),
        session_manager,
        StreamableHttpServerConfig::default(),
    );

    // Nest the MCP service into an Axum router
    let app = axum::Router::new().nest_service("/mcp", service);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("MCP Live Server active on http://127.0.0.1:{}/mcp", port);

    axum::serve(listener, app).await?;

    Ok(())
}
