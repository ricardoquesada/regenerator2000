//! MCP tool handlers for 6502 unpacking analysis, project saving, and undo/redo operations.
//!
//! This module provides tool definitions and execution handlers for emulating packed binary payload depacking,
//! saving project state, and performing undo/redo command stack operations.

use crate::mcp::tools::common::{get_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
#[cfg(test)]
use crate::state::types::Addr;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Returns JSON schemas for session, project, and undo/redo tools.
///
/// # Returns
///
/// A vector of JSON [`Value`] objects defining schemas for `r2000_unpack_binary`,
/// `r2000_save_project`, `r2000_undo`, and `r2000_redo`.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_unpack_binary",
            "description": "Executes dynamic 6502 unpacking analysis on the loaded binary payload.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entry_point": { "type": "string", "description": "Optional forced entry point address (hex string or integer)." },
                    "return_address": { "type": "string", "description": "Optional forced return address." },
                    "depacker_address": { "type": "string", "description": "Optional forced depacker address." },
                    "max_instructions": { "type": "integer", "description": "Maximum instructions to emulate." }
                }
            }
        }),
        json!({
            "name": "r2000_save_project",
            "description": "Saves current project state to existing .regen2000proj file.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_undo",
            "description": "Undoes the latest operation.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_redo",
            "description": "Redoes the latest undone operation.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
    ]
}

/// Handles execution of session management, project state, and undo/redo tools.
///
/// # Arguments
///
/// * `name` - Tool identifier string.
/// * `args` - Tool argument JSON [`Value`].
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `view_state` - Mutable reference to view state [`CoreViewState`].
///
/// # Returns
///
/// `Some(Ok(value))` on success, `Some(Err(McpError))` on error, or `None` if unrecognized.
///
/// # Errors
///
/// Returns [`McpError`] if saving, unpacking, or execution fails.
pub fn handle_call(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Option<Result<Value, McpError>> {
    match name {
        "r2000_unpack_binary" => Some(unpack_binary_impl(app_state, view_state, args)),
        "r2000_save_project" => Some(save_project_impl(app_state, view_state)),
        "r2000_undo" => Some(undo_impl(app_state, view_state)),
        "r2000_redo" => Some(redo_impl(app_state, view_state)),
        _ => None,
    }
}

/// Private helper to run dynamic 6502 unpacking emulation and reload payload into app state.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `view_state` - Mutable reference to view state [`CoreViewState`].
/// * `args` - JSON [`Value`] containing optional forced entry point, return address, depacker address, and max instructions limit.
///
/// # Returns
///
/// A JSON [`Value`] response containing summary details of the unpacking result.
///
/// # Errors
///
/// Returns [`McpError`] if unpacker emulation or payload loading fails.
fn unpack_binary_impl(
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
    args: &Value,
) -> Result<Value, McpError> {
    let load_addr = app_state.origin.0;
    let raw_data = app_state.raw_data.clone();
    let mut config = crate::unpacker::UnpackConfig::default();

    if args.get("entry_point").is_some() {
        config.forced_entry = Some(get_address(args, "entry_point")?.0);
    }
    if args.get("return_address").is_some() {
        config.forced_ret_addr = Some(get_address(args, "return_address")?.0);
    }
    if args.get("depacker_address").is_some() {
        config.forced_dep_addr = Some(get_address(args, "depacker_address")?.0);
    }
    if let Some(max_inst) = args.get("max_instructions").and_then(|v| v.as_u64()) {
        config.max_instructions = max_inst;
    }

    match crate::unpacker::unpack(&raw_data, load_addr, &config, None) {
        Ok(result) => {
            let outcome = app_state.apply_unpack_result(&result);
            crate::navigation::perform_jump_to_address_no_history(
                app_state,
                view_state,
                outcome.entry_point,
            );

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": outcome.summary
                }]
            }))
        }
        Err(e) => Err(make_mcp_error(-32603, format!("Unpack failed: {e}"))),
    }
}

/// Private helper to save active project state to disk file.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `view_state` - Reference to view state [`CoreViewState`].
///
/// # Returns
///
/// A JSON [`Value`] response text confirming file save destination.
///
/// # Errors
///
/// Returns [`McpError`] if project path is missing or file save operation fails.
fn save_project_impl(
    app_state: &mut AppState,
    view_state: &CoreViewState,
) -> Result<Value, McpError> {
    if app_state.project_path.is_none() {
        return Err(make_mcp_error(
            -32603,
            "No active project path. Project must be loaded from or saved to a .regen2000proj file before it can be saved.",
        ));
    }

    let ctx = crate::navigation::create_save_context(app_state, view_state);
    app_state
        .save_project(ctx, true)
        .map_err(|e| make_mcp_error(-32603, format!("Failed to save project: {e}")))?;

    let path_str = app_state
        .project_path
        .as_ref()
        .map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string());

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Project saved to {path_str}")
        }]
    }))
}

/// Private helper to execute an undo step on the command undo stack.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `_view_state` - Mutable reference to view state [`CoreViewState`].
///
/// # Returns
///
/// A JSON [`Value`] response text describing undone operation or status message.
///
/// # Errors
///
/// Returns [`McpError`] if undo processing fails.
fn undo_impl(app_state: &mut AppState, _view_state: &mut CoreViewState) -> Result<Value, McpError> {
    let mut stack = std::mem::take(&mut app_state.undo_stack);
    let msg_opt = stack.undo(app_state);
    app_state.undo_stack = stack;

    if let Some(msg) = msg_opt {
        Ok(json!({ "content": [{ "type": "text", "text": format!("Undo: {msg}") }] }))
    } else {
        Ok(json!({ "content": [{ "type": "text", "text": "Nothing to undo" }] }))
    }
}

/// Private helper to execute a redo step on the command undo stack.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `_view_state` - Mutable reference to view state [`CoreViewState`].
///
/// # Returns
///
/// A JSON [`Value`] response text describing redone operation or status message.
///
/// # Errors
///
/// Returns [`McpError`] if redo processing fails.
fn redo_impl(app_state: &mut AppState, _view_state: &mut CoreViewState) -> Result<Value, McpError> {
    let mut stack = std::mem::take(&mut app_state.undo_stack);
    let msg_opt = stack.redo(app_state);
    app_state.undo_stack = stack;

    if let Some(msg) = msg_opt {
        Ok(json!({ "content": [{ "type": "text", "text": format!("Redo: {msg}") }] }))
    } else {
        Ok(json!({ "content": [{ "type": "text", "text": "Nothing to redo" }] }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::{make_app_state, make_view_state};

    #[test]
    fn test_r2000_undo_redo_via_mcp() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        assert!(!app_state.undo_stack.can_undo());

        let cmd = app_state
            .create_set_user_label_command(Addr(0x1000), "test_lbl", false)
            .unwrap();
        cmd.apply(&mut app_state);
        app_state.push_command(cmd);

        assert!(app_state.undo_stack.can_undo());

        let undo_res = handle_call("r2000_undo", &json!({}), &mut app_state, &mut view_state)
            .expect("Handler should match r2000_undo")
            .expect("Undo should succeed");
        assert!(
            undo_res["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Undo:")
        );
        assert!(!app_state.undo_stack.can_undo());
        assert!(app_state.undo_stack.can_redo());

        let redo_res = handle_call("r2000_redo", &json!({}), &mut app_state, &mut view_state)
            .expect("Handler should match r2000_redo")
            .expect("Redo should succeed");
        assert!(
            redo_res["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Redo:")
        );
        assert!(app_state.undo_stack.can_undo());
    }

    // T4.2.8
    #[test]
    fn mcp_unpack_binary_summary_contains_provenance() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502/c64_FppScroller.byte_boozer2.prg");
        let mut app_state = AppState::new();
        app_state.load_file(&fixture).expect("fixture should load");
        let mut view_state = make_view_state();

        let res = handle_call(
            "r2000_unpack_binary",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("handler should match r2000_unpack_binary")
        .expect("unpack should succeed");

        let text = res["content"][0]["text"]
            .as_str()
            .expect("response text must be a string");
        assert!(text.contains("ByteBoozer"), "missing packer: {text}");
        assert!(text.contains("exit="), "missing exit reason: {text}");
        assert!(text.contains("start="), "missing start provenance: {text}");
        assert!(text.contains("end="), "missing end provenance: {text}");
        assert!(text.contains("entry="), "missing entry provenance: {text}");
    }

    // T4.2.9
    #[test]
    fn mcp_unpack_binary_error_mapping_unchanged() {
        let mut app_state = AppState::new();
        let mut view_state = make_view_state();

        let err = handle_call(
            "r2000_unpack_binary",
            &json!({}),
            &mut app_state,
            &mut view_state,
        )
        .expect("handler should match r2000_unpack_binary")
        .expect_err("empty binary must fail to unpack");

        assert_eq!(err.code, -32603);
        assert!(
            err.message.starts_with("Unpack failed:"),
            "unexpected error message: {}",
            err.message
        );
    }
}
