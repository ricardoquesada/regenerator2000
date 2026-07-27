//! MCP tool handlers for navigation and disassembly cursor retrieval.
//!
//! This module provides tool definitions and execution handlers for retrieving cursor positions
//! and jumping to specific memory addresses.

use crate::mcp::tools::common::{get_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Returns JSON schemas for navigation and cursor tools.
///
/// # Returns
///
/// A vector of JSON [`Value`] objects defining the schemas for `r2000_get_disassembly_cursor`
/// and `r2000_jump_to_address`.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_get_disassembly_cursor",
            "description": "Returns the memory address of the current cursor position in the disassembly view.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_jump_to_address",
            "description": "Moves the disassembly cursor to a specific memory address and scrolls the view to make it visible. Also keeps the jump history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The target address to jump to (decimal)." }
                },
                "required": ["address"]
            }
        }),
    ]
}

/// Handles execution of navigation tools.
///
/// # Arguments
///
/// * `name` - Tool name string to execute.
/// * `args` - Tool argument JSON [`Value`].
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `view_state` - Mutable reference to view state [`CoreViewState`].
///
/// # Returns
///
/// `Some(Ok(value))` on success, `Some(Err(`[`McpError`]`))` on error, or `None` if `name` is unrecognized.
///
/// # Errors
///
/// Returns [`McpError`] if target address is missing or not found in disassembly.
pub fn handle_call(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Option<Result<Value, McpError>> {
    match name {
        "r2000_get_disassembly_cursor" => Some(get_disassembly_cursor_impl(app_state, view_state)),
        "r2000_jump_to_address" => Some(jump_to_address_impl(app_state, view_state, args)),
        _ => None,
    }
}

/// Private helper to get the address at current disassembly cursor index.
fn get_disassembly_cursor_impl(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Result<Value, McpError> {
    let idx = view_state.cursor_index;
    let line = app_state
        .disassembly
        .get(idx)
        .ok_or_else(|| make_mcp_error(-32602, "Cursor out of bounds"))?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("${:04X}", line.address)
        }]
    }))
}

/// Private helper to jump cursor position to a target address in disassembly.
fn jump_to_address_impl(
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
    args: &Value,
) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;

    let line_exists = app_state
        .get_line_index_containing_address(address)
        .or_else(|| app_state.get_line_index_for_address(address))
        .is_some();

    if !line_exists {
        return Err(make_mcp_error(
            -32602,
            format!("Address ${address:04X} not found in disassembly (might be hidden or invalid)"),
        ));
    }

    crate::navigation::perform_jump_to_address(app_state, view_state, address);
    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Jumped to ${address:04X}")
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::{make_app_state, make_view_state};

    #[test]
    fn test_r2000_get_disassembly_cursor_initial() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();
        view_state.cursor_index = 0;

        let res = handle_call(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match tool name")
        .expect("Should return success");

        let text = res["content"][0]["text"]
            .as_str()
            .expect("Text field present");
        assert_eq!(text, "$1000");
    }

    #[test]
    fn test_r2000_jump_to_address() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        let res = handle_call(
            "r2000_jump_to_address",
            &json!({ "address": 0x1010 }),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match tool name")
        .expect("Should return success");

        let text = res["content"][0]["text"]
            .as_str()
            .expect("Text field present");
        assert_eq!(text, "Jumped to $1010");
        assert_eq!(view_state.cursor_index, 16);
    }

    #[test]
    fn test_r2000_get_disassembly_cursor_out_of_bounds() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();
        view_state.cursor_index = 99999;

        let err = handle_call(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match tool name")
        .expect_err("Should return error for out of bounds cursor");

        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Cursor out of bounds"));
    }

    #[test]
    fn test_r2000_jump_to_address_invalid() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        let err = handle_call(
            "r2000_jump_to_address",
            &json!({ "address": 0x9999 }),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match tool name")
        .expect_err("Should return error for invalid jump address");

        assert_eq!(err.code, -32602);
        assert!(err.message.contains("not found in disassembly"));
    }
}
