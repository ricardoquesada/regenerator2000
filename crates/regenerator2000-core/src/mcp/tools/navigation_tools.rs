//! MCP tool handlers for navigation and disassembly cursor queries.
//!
//! This module defines schemas and execution handlers that read the active disassembly
//! cursor address and jump the cursor to a target address.

use crate::mcp::tools::common::{get_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Returns JSON schemas for navigation and cursor tools.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_get_disassembly_cursor",
            "description": "Return the memory address of the current cursor position in the disassembly view.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_jump_to_address",
            "description": "Move the disassembly cursor to a specific memory address, scroll the view to make it visible, and record the previous position in the navigation history.",
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

/// Dispatches navigation tool calls by `name`.
///
/// Returns `Some(Ok(value))` on success, `Some(Err(McpError))` on failure, or `None`
/// if `name` does not match a navigation tool.
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

/// Returns the 6502 address at the active disassembly cursor and sub-cursor indices.
fn get_disassembly_cursor_impl(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Result<Value, McpError> {
    let idx = view_state.cursor_index;
    let line = app_state
        .disassembly
        .get(idx)
        .ok_or_else(|| make_mcp_error(-32602, "Cursor out of bounds"))?;
    let address = line.address_at_sub_index(app_state, view_state.sub_cursor_index);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("${address:04X}")
        }]
    }))
}

/// Moves the disassembly cursor to the target `address` in `args`.
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
    use crate::state::types::Addr;

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
    fn test_r2000_jump_to_mid_instruction_and_external_equate_updates_cursor_address() {
        let mut app_state = make_app_state(0x1000, 16);
        app_state.settings.all_labels = true;
        // JSR $FFD2 at $1000..$1002 creates an external equate for $FFD2 and spans $1000..=$1002.
        app_state.raw_data[0..3].copy_from_slice(&[0x20, 0xD2, 0xFF]);
        app_state.block_types[0..3].fill(crate::state::BlockType::Code);
        app_state.labels.insert(
            Addr(0x1001),
            vec![crate::state::Label {
                name: "mid_label".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        app_state.perform_analysis();

        let mut view_state = make_view_state();
        handle_call(
            "r2000_jump_to_address",
            &json!({ "address": "$1001" }),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match")
        .expect("Jump to mid-instruction label should succeed");

        // The instruction line starts at $1000, while sub_cursor_index points to $1001.
        assert_eq!(
            app_state.disassembly[view_state.cursor_index].address,
            Addr(0x1000)
        );
        let cursor_res = handle_call(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match")
        .expect("Cursor query should succeed");
        assert_eq!(cursor_res["content"][0]["text"], "$1001");

        handle_call(
            "r2000_jump_to_address",
            &json!({ "address": "$FFD2" }),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match")
        .expect("Jump to external equate should succeed");

        let ext_cursor_res = handle_call(
            "r2000_get_disassembly_cursor",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("Handler should match")
        .expect("Cursor query should succeed");
        assert_eq!(ext_cursor_res["content"][0]["text"], "$FFD2");
        assert_eq!(
            app_state.disassembly[view_state.cursor_index].external_label_address,
            Some(Addr(0xFFD2))
        );
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
