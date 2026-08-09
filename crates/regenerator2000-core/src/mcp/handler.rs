//! Central MCP request handler and tool dispatcher for Model Context Protocol interactions.
//!
//! This module processes incoming JSON-RPC protocol requests ([`McpRequest`]), handles initialization,
//! lists available tools and resources, and dispatches tool calls across analysis, navigation, modification,
//! and session tool submodules.

use crate::mcp::tools::{
    analysis_tools, common::make_mcp_error, modification_tools, navigation_tools, session_tools,
};
use crate::mcp::types::{McpError, McpRequest, McpResponse};
use crate::state::AppState;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Main request handler for processing incoming Model Context Protocol [`McpRequest`] calls.
///
/// Handles `initialize`, `notifications/initialized`, `tools/list`, `resources/list`,
/// `tools/call`, and `resources/read` request methods.
///
/// # Arguments
///
/// * `req` - Reference to the incoming [`McpRequest`].
/// * `app_state` - Mutable reference to the application state [`AppState`].
/// * `view_state` - Mutable reference to the UI viewport state [`CoreViewState`].
///
/// # Returns
///
/// An [`McpResponse`] containing either the JSON result payload or an error structure [`McpError`].
#[must_use]
pub fn handle_request(
    req: &McpRequest,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> McpResponse {
    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "regenerator2000-core-mcp",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "An interactive disassembler for the MOS 6502 assembly."
            },
            "instructions": "You are an expert in MOS 6502 assembly programmer. Always assume the code is 6502 assembly unless stated otherwise.",
            "capabilities": {
                "tools": {},
                "resources": {}
            }
        })),
        "notifications/initialized" => Ok(json!(true)),
        "tools/list" => list_tools(&app_state.system_config),
        "resources/list" => list_resources(),
        "tools/call" => handle_tool_call(&req.params, app_state, view_state),
        "resources/read" => handle_resource_read(&req.params, app_state),
        _ => Err(make_mcp_error(
            -32601,
            format!("Method not found: {}", req.method),
        )),
    };

    match result {
        Ok(val) => McpResponse {
            result: Some(val),
            error: None,
        },
        Err(err) => McpResponse {
            result: None,
            error: Some(err),
        },
    }
}

/// Aggregates tool definitions across all tool submodules into a JSON list response.
///
/// # Arguments
///
/// * `system_config` - Reference to the system configuration [`SystemConfig`](crate::config::SystemConfig).
///
/// # Returns
///
/// A JSON [`Value`] object containing the aggregated tool schemas array under the `"tools"` key.
///
/// # Errors
///
/// Returns [`McpError`] if schema generation fails.
fn list_tools(system_config: &crate::config::SystemConfig) -> Result<Value, McpError> {
    let mut tools = Vec::new();

    tools.extend(analysis_tools::tool_schemas(system_config));
    tools.extend(navigation_tools::tool_schemas());
    tools.extend(modification_tools::tool_schemas());
    tools.extend(session_tools::tool_schemas());

    tools.push(json!({
        "name": "r2000_batch_execute",
        "description": "Executes multiple tool calls sequentially in a single request. Use only when you have 5+ independent operations to perform at once.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "calls": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Name of the tool to execute." },
                            "arguments": { "type": "object", "description": "Arguments for the tool." }
                        },
                        "required": ["name", "arguments"]
                    },
                    "description": "List of tool calls to execute sequentially."
                }
            },
            "required": ["calls"]
        }
    }));

    Ok(json!({ "tools": tools }))
}

/// Handles `tools/call` JSON-RPC requests by extracting parameters and invoking tool execution.
///
/// # Arguments
///
/// * `params` - JSON [`Value`] holding the tool name and arguments.
/// * `app_state` - Mutable reference to the application state [`AppState`].
/// * `view_state` - Mutable reference to the UI viewport state [`CoreViewState`].
///
/// # Returns
///
/// A JSON [`Value`] containing the result payload returned by the dispatched tool execution handler.
///
/// # Errors
///
/// Returns [`McpError`] if `name` is missing or if tool execution returns an error.
fn handle_tool_call(
    params: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Result<Value, McpError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name' in tools/call"))?;

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    handle_tool_call_internal(name, &args, app_state, view_state)
}

/// Dispatches an MCP tool call to the appropriate submodule (`analysis_tools`, `navigation_tools`,
/// `modification_tools`, or `session_tools`). Handles batch tool invocation recursively.
///
/// # Arguments
///
/// * `name` - The tool name (e.g. `"r2000_get_disassembly"`).
/// * `args` - Reference to the tool arguments as a JSON [`Value`].
/// * `app_state` - Mutable reference to the application state [`AppState`].
/// * `view_state` - Mutable reference to the UI viewport state [`CoreViewState`].
///
/// # Returns
///
/// A JSON [`Value`] containing execution results for single or batch tool invocations.
///
/// # Errors
///
/// Returns [`McpError`] if `name` is unrecognized or if the tool call fails.
pub(crate) fn handle_tool_call_internal(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Result<Value, McpError> {
    if name == "r2000_batch_execute" {
        let calls = args
            .get("calls")
            .and_then(|v| v.as_array())
            .ok_or_else(|| make_mcp_error(-32602, "Missing 'calls' array"))?;

        let mut results = Vec::new();
        let default_args = json!({});
        for call in calls {
            let tool_name = call
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| make_mcp_error(-32602, "Missing 'name' in call"))?;

            let tool_args = call.get("arguments").unwrap_or(&default_args);

            match handle_tool_call_internal(tool_name, tool_args, app_state, view_state) {
                Ok(val) => results.push(json!({ "status": "success", "result": val })),
                Err(err) => results.push(json!({ "status": "error", "error": err })),
            }
        }

        return Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results).unwrap_or_default()
            }]
        }));
    }

    if let Some(res) = analysis_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    if let Some(res) = navigation_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    if let Some(res) = modification_tools::handle_call(name, args, app_state) {
        return res;
    }

    if let Some(res) = session_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    Err(make_mcp_error(-32601, format!("Unknown tool: {name}")))
}

/// Returns a list of available MCP resource definitions.
///
/// # Returns
///
/// A JSON [`Value`] object containing the list of available resource definitions under the `"resources"` key.
///
/// # Errors
///
/// Returns [`McpError`] if resource listing fails.
fn list_resources() -> Result<Value, McpError> {
    Ok(json!({
        "resources": [
            {
                "uri": "binary://main",
                "name": "Main Binary Data",
                "description": "The raw PRG binary payload (including 2-byte load address header) encoded as base64 blob.",
                "mimeType": "application/octet-stream"
            },
            {
                "uri": "disassembly://current",
                "name": "Current Disassembly View",
                "description": "The complete disassembled text of the active binary file.",
                "mimeType": "text/x-asm"
            },
            {
                "uri": "hexdump://current",
                "name": "Current Hexdump View",
                "description": "The formatted memory hexdump of the active binary file.",
                "mimeType": "text/plain"
            },
            {
                "uri": "symbols://current",
                "name": "Defined Symbols List",
                "description": "All labels, functions, and symbols defined in the current disassembly session.",
                "mimeType": "application/json"
            },
            {
                "uri": "comments://current",
                "name": "Defined Comments List",
                "description": "All user line and side comments in the current disassembly session.",
                "mimeType": "application/json"
            },
            {
                "uri": "blocks://current",
                "name": "Memory Blocks List",
                "description": "All memory block classifications and their byte ranges.",
                "mimeType": "application/json"
            }
        ]
    }))
}

/// Handles `resources/read` JSON-RPC requests for registered resource URIs.
///
/// # Arguments
///
/// * `params` - JSON [`Value`] containing `"uri"`.
/// * `app_state` - Mutable reference to the application state [`AppState`].
///
/// # Returns
///
/// A JSON [`Value`] containing the contents payload for the requested resource URI.
///
/// # Errors
///
/// Returns [`McpError`] if `uri` is missing or unrecognized.
fn handle_resource_read(params: &Value, app_state: &mut AppState) -> Result<Value, McpError> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'uri' in resources/read"))?;

    if uri == "binary://main" {
        let mut payload = Vec::with_capacity(2 + app_state.raw_data.len());
        payload.extend_from_slice(&app_state.origin.0.to_le_bytes());
        payload.extend_from_slice(&app_state.raw_data);
        use base64::Engine;
        let blob_b64 = base64::prelude::BASE64_STANDARD.encode(&payload);
        return Ok(json!({
            "contents": [{
                "uri": "binary://main",
                "mimeType": "application/octet-stream",
                "blob": blob_b64
            }]
        }));
    }

    let (mime_type, text) = match uri {
        "disassembly://current" => {
            let start = app_state.origin;
            let end = app_state
                .raw_data
                .len()
                .checked_sub(1)
                .map_or(start, |len_minus_1| start.wrapping_add(len_minus_1 as u16));
            (
                "text/x-asm",
                analysis_tools::get_disassembly_text(app_state, start, end),
            )
        }
        "hexdump://current" => {
            let start = app_state.origin;
            let end = app_state
                .raw_data
                .len()
                .checked_sub(1)
                .map_or(start, |len_minus_1| start.wrapping_add(len_minus_1 as u16));
            (
                "text/plain",
                analysis_tools::get_hexdump_text(app_state, start, end),
            )
        }
        "symbols://current" => {
            let symbols = analysis_tools::get_symbols_impl(app_state, &json!({}))?;
            (
                "application/json",
                serde_json::to_string_pretty(&symbols).unwrap_or_default(),
            )
        }
        "comments://current" => {
            let comments = analysis_tools::get_comments_impl(app_state, &json!({}))?;
            (
                "application/json",
                serde_json::to_string_pretty(&comments).unwrap_or_default(),
            )
        }
        "blocks://current" => {
            let blocks = analysis_tools::get_blocks_impl(app_state, None);
            (
                "application/json",
                serde_json::to_string_pretty(&blocks).unwrap_or_default(),
            )
        }
        _ => {
            return Err(make_mcp_error(-32602, format!("Resource not found: {uri}")));
        }
    };

    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": mime_type,
            "text": text
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::{make_app_state, make_view_state};

    #[test]
    fn test_handle_request_initialize() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let req = McpRequest {
            method: "initialize".to_string(),
            params: json!({}),
            response_sender: tx,
        };

        let resp = handle_request(&req, &mut app_state, &mut view_state);
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_handle_request_list_tools() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let req = McpRequest {
            method: "tools/list".to_string(),
            params: json!({}),
            response_sender: tx,
        };

        let resp = handle_request(&req, &mut app_state, &mut view_state);
        assert!(resp.error.is_none());
        let res = resp.result.unwrap();
        assert!(res.get("tools").unwrap().as_array().unwrap().len() >= 20);
    }

    #[test]
    fn test_binary_main_resource() {
        let mut app_state = make_app_state(0x1000, 4);
        app_state.raw_data = vec![0x01, 0x02, 0x03, 0x04];
        let mut view_state = make_view_state();

        // 1. Verify resource listing includes binary://main
        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        let list_req = McpRequest {
            method: "resources/list".to_string(),
            params: json!({}),
            response_sender: tx1,
        };
        let list_resp = handle_request(&list_req, &mut app_state, &mut view_state);
        assert!(list_resp.error.is_none());
        let list_val = list_resp.result.unwrap();
        let resources = list_val.get("resources").unwrap().as_array().unwrap();
        let binary_res = resources
            .iter()
            .find(|r| r.get("uri").and_then(|u| u.as_str()) == Some("binary://main"))
            .expect("binary://main should be listed in resources");
        assert_eq!(
            binary_res.get("name").unwrap().as_str().unwrap(),
            "Main Binary Data"
        );
        assert_eq!(
            binary_res.get("mimeType").unwrap().as_str().unwrap(),
            "application/octet-stream"
        );

        // 2. Verify reading binary://main returns base64 blob containing origin header + raw data
        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        let read_req = McpRequest {
            method: "resources/read".to_string(),
            params: json!({ "uri": "binary://main" }),
            response_sender: tx2,
        };
        let read_resp = handle_request(&read_req, &mut app_state, &mut view_state);
        assert!(read_resp.error.is_none());
        let read_val = read_resp.result.unwrap();
        let contents = read_val.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
        let content = &contents[0];
        assert_eq!(
            content.get("uri").unwrap().as_str().unwrap(),
            "binary://main"
        );
        assert_eq!(
            content.get("mimeType").unwrap().as_str().unwrap(),
            "application/octet-stream"
        );

        let blob_b64 = content.get("blob").unwrap().as_str().unwrap();
        use base64::Engine;
        let decoded = base64::prelude::BASE64_STANDARD
            .decode(blob_b64)
            .expect("blob should be valid base64");

        // Expected: 2-byte origin 0x1000 (0x00, 0x10) + [0x01, 0x02, 0x03, 0x04]
        assert_eq!(decoded, vec![0x00, 0x10, 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_comments_current_resource() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        app_state
            .annotations
            .update(crate::state::types::Addr(0x1000), |e| {
                e.user_line_comment = Some("Resource comment test".to_string());
            });

        let (tx, _rx) = tokio::sync::oneshot::channel();
        let read_req = McpRequest {
            method: "resources/read".to_string(),
            params: json!({ "uri": "comments://current" }),
            response_sender: tx,
        };

        let resp = handle_request(&read_req, &mut app_state, &mut view_state);
        assert!(resp.error.is_none());
        let val = resp.result.unwrap();
        let contents = val.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents[0]["mimeType"], "application/json");
        assert!(
            contents[0]["text"]
                .as_str()
                .unwrap()
                .contains("Resource comment test")
        );
    }

    #[test]
    fn test_64kb_resource_read_disassembly_and_hexdump() {
        let mut app_state = make_app_state(0x0000, 65536);
        let mut view_state = make_view_state();

        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        let req1 = McpRequest {
            method: "resources/read".to_string(),
            params: json!({ "uri": "disassembly://current" }),
            response_sender: tx1,
        };
        let resp1 = handle_request(&req1, &mut app_state, &mut view_state);
        assert!(resp1.error.is_none());
        let text1 = resp1.result.unwrap()["contents"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text1.contains("* = $0000"));
        assert!(text1.contains("$FFFF"));

        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        let req2 = McpRequest {
            method: "resources/read".to_string(),
            params: json!({ "uri": "hexdump://current" }),
            response_sender: tx2,
        };
        let resp2 = handle_request(&req2, &mut app_state, &mut view_state);
        assert!(resp2.error.is_none());
        let text2 = resp2.result.unwrap()["contents"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text2.contains("$0000:"));
        assert!(text2.contains("$FFF0:"));
    }
}
