//! Central Model Context Protocol (MCP) request handler and tool dispatcher.
//!
//! This module processes JSON-RPC protocol requests ([`McpRequest`]), initializes sessions,
//! lists tools and resources, and dispatches tool calls to analysis, navigation, modification,
//! and session tool submodules.

use crate::mcp::tools::{
    analysis_tools, common::make_mcp_error, modification_tools, navigation_tools, session_tools,
};
use crate::mcp::types::{McpError, McpRequest, McpResponse};
use crate::state::AppState;
use crate::state::types::Addr;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Canonical MCP server name for initialization and discovery responses.
pub const MCP_SERVER_NAME: &str = "regenerator2000-core-mcp";

/// Human-readable description of the Regenerator 2000 MCP server.
pub const MCP_SERVER_DESCRIPTION: &str = "An interactive disassembler for MOS 6502 assembly.";

/// System instructions for MCP clients that connect to Regenerator 2000.
pub const MCP_SERVER_INSTRUCTIONS: &str = "You are an expert MOS 6502 assembly programmer. Assume the code is 6502 assembly unless stated otherwise.";

/// Processes a Model Context Protocol [`McpRequest`] and returns an [`McpResponse`].
///
/// This function handles the `initialize`, `notifications/initialized`, `ping`, `tools/list`,
/// `resources/list`, `tools/call`, and `resources/read` methods.
#[must_use]
pub fn handle_request(
    req: &McpRequest,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> McpResponse {
    handle_method(&req.method, &req.params, app_state, view_state)
}

/// Processes a JSON-RPC method call directly without an [`McpRequest`] channel wrapper.
#[must_use]
pub(crate) fn handle_method(
    method: &str,
    params: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> McpResponse {
    let result = match method {
        "initialize" => {
            let protocol_version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05");
            Ok(json!({
                "protocolVersion": protocol_version,
                "serverInfo": {
                    "name": MCP_SERVER_NAME,
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": MCP_SERVER_DESCRIPTION
                },
                "instructions": MCP_SERVER_INSTRUCTIONS,
                "capabilities": {
                    "tools": {},
                    "resources": {}
                }
            }))
        }
        "notifications/initialized" => Ok(json!(true)),
        "ping" => Ok(json!({})),
        "tools/list" => list_tools(&app_state.system_config),
        "resources/list" => list_resources(),
        "tools/call" => handle_tool_call(params, app_state, view_state),
        "resources/read" => handle_resource_read(params, app_state),
        _ => Err(make_mcp_error(
            -32601,
            format!("Method not found: {method}"),
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

/// Collects tool schemas from all tool submodules into a JSON list response.
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
        "description": "Execute multiple tool calls sequentially in a single request. Use this tool only when you have 5 or more independent operations to run at once.",
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

/// Extracts parameters from a `tools/call` request and executes the requested tool.
///
/// # Errors
///
/// Returns [`McpError`] if `"name"` is missing or if tool execution fails.
fn handle_tool_call(
    params: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Result<Value, McpError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name' in tools/call"))?;

    let default_args = json!({});
    let args = params.get("arguments").unwrap_or(&default_args);

    handle_tool_call_internal(name, args, app_state, view_state)
}

/// Dispatches an MCP tool call to the matching submodule or executes a batch call recursively.
///
/// # Errors
///
/// Returns [`McpError`] if `name` is unknown or if the tool execution fails.
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

        let mut results = Vec::with_capacity(calls.len());
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

    if name == "r2000_add_scope" {
        let anchor = crate::action_handlers::DisassemblyViewAnchor::capture(app_state, view_state);
        if let Some(res) = analysis_tools::handle_call(name, args, app_state, view_state) {
            if res.is_ok() {
                anchor.restore(app_state, view_state);
            }
            return res;
        }
    } else if let Some(res) = analysis_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    if let Some(res) = navigation_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    {
        let anchor = crate::action_handlers::DisassemblyViewAnchor::capture(app_state, view_state);
        if let Some(res) = modification_tools::handle_call(name, args, app_state) {
            if res.is_ok() {
                anchor.restore(app_state, view_state);
            }
            return res;
        }
    }

    if matches!(name, "r2000_undo" | "r2000_redo") {
        let anchor = crate::action_handlers::DisassemblyViewAnchor::capture(app_state, view_state);
        if let Some(res) = session_tools::handle_call(name, args, app_state, view_state) {
            if res.is_ok() {
                anchor.restore(app_state, view_state);
            }
            return res;
        }
    } else if let Some(res) = session_tools::handle_call(name, args, app_state, view_state) {
        return res;
    }

    Err(make_mcp_error(-32601, format!("Unknown tool: {name}")))
}

/// Returns the list of available MCP resource definitions.
///
/// # Errors
///
/// Returns [`McpError`] if resource list construction fails.
fn list_resources() -> Result<Value, McpError> {
    Ok(json!({
        "resources": [
            {
                "uri": "binary://main",
                "name": "Main Binary Data",
                "description": "The raw PRG binary payload (with a 2-byte load address header) encoded as a base64 blob.",
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

/// Returns the inclusive `(start, end)` address bounds for the loaded binary in `app_state`.
#[must_use]
fn binary_address_bounds(app_state: &AppState) -> (Addr, Addr) {
    let start = app_state.origin;
    let end = app_state
        .raw_data
        .len()
        .checked_sub(1)
        .map_or(start, |len_minus_1| {
            #[allow(clippy::cast_possible_truncation)]
            start.wrapping_add(len_minus_1 as u16)
        });
    (start, end)
}

/// Processes `resources/read` JSON-RPC requests for registered resource URIs.
///
/// # Errors
///
/// Returns [`McpError`] if `"uri"` is missing or unknown.
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
            let (start, end) = binary_address_bounds(app_state);
            (
                "text/x-asm",
                analysis_tools::get_disassembly_text(app_state, start, end),
            )
        }
        "hexdump://current" => {
            let (start, end) = binary_address_bounds(app_state);
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

    #[test]
    fn test_mcp_mutating_calls_reanchor_cursor_and_selection_when_preamble_shrinks() {
        let mut app_state = make_app_state(0x1000, 5);
        app_state.settings.all_labels = true;
        // $1000: LDA ($FB),Y (B1 FB), $1002: STA $FC (85 FC), $1004: RTS (60)
        app_state.raw_data = vec![0xB1, 0xFB, 0x85, 0xFC, 0x60];
        app_state.block_types = vec![crate::state::BlockType::Code; 5];
        app_state.labels.insert(
            Addr(0x00FB),
            vec![crate::state::Label {
                name: "zp_ptr".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::ZeroPagePointer,
            }],
        );
        let _ = app_state.perform_analysis();

        let mut view_state = make_view_state();

        // Jump to the final body instruction `$1004` (`RTS`).
        handle_tool_call_internal(
            "r2000_jump_to_address",
            &json!({ "address": 0x1004 }),
            &mut app_state,
            &mut view_state,
        )
        .expect("jump to $1004 should succeed");
        let len_before = app_state.disassembly.len();
        assert_eq!(view_state.cursor_index, len_before - 1);

        // Suppress preamble equate `zpa_FC = $fc`, shrinking `app_state.disassembly`.
        handle_tool_call_internal(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x00FC, "base": "zp_ptr" }),
            &mut app_state,
            &mut view_state,
        )
        .expect("setting Tier 2 offset should succeed");
        assert!(app_state.disassembly.len() < len_before);

        // Follow-up `r2000_get_disassembly_cursor` and `r2000_read_selected` stay on `$1004`.
        let cur_val = handle_tool_call_internal(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("cursor query after preamble shrink must succeed");
        assert_eq!(cur_val["content"][0]["text"], "$1004");

        let sel_val = handle_tool_call_internal(
            "r2000_read_selected",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("read_selected after preamble shrink must succeed");
        assert!(
            sel_val["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("$1004")
        );

        // `r2000_undo` and `r2000_redo` also keep the cursor anchored at `$1004`.
        handle_tool_call_internal("r2000_undo", &json!({}), &mut app_state, &mut view_state)
            .expect("undo must succeed");
        let cur_undo = handle_tool_call_internal(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("cursor query after undo must succeed");
        assert_eq!(cur_undo["content"][0]["text"], "$1004");

        handle_tool_call_internal("r2000_redo", &json!({}), &mut app_state, &mut view_state)
            .expect("redo must succeed");
        let cur_redo = handle_tool_call_internal(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("cursor query after redo must succeed");
        assert_eq!(cur_redo["content"][0]["text"], "$1004");
    }
}
