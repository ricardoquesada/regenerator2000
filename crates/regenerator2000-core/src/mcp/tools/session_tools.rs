//! MCP tool handlers for 6502 binary unpacking, project saving, and undo/redo operations.
//!
//! This module defines schemas and execution handlers that unpack compressed 6502 binaries,
//! save `.regen2000proj` project files, and step backward or forward through the undo stack.

use crate::mcp::tools::common::{get_optional_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
#[cfg(test)]
use crate::state::types::Addr;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Returns JSON schemas for session, project, and undo/redo tools.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_unpack_binary",
            "description": "Run dynamic 6502 unpacking analysis on the loaded binary payload.",
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
            "description": "Save the current project state to the active .regen2000proj file.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_undo",
            "description": "Undo the latest operation.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_redo",
            "description": "Redo the latest undone operation.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
    ]
}

/// Dispatches session, project save, and undo/redo tool calls by `name`.
///
/// Returns `Some(Ok(value))` on success, `Some(Err(McpError))` on failure, or `None`
/// if `name` does not match a session tool.
pub fn handle_call(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> Option<Result<Value, McpError>> {
    match name {
        "r2000_unpack_binary" => Some(unpack_binary_impl(app_state, view_state, args)),
        "r2000_save_project" => Some(save_project_impl(app_state, view_state)),
        "r2000_undo" => Some(Ok(undo_impl(app_state))),
        "r2000_redo" => Some(Ok(redo_impl(app_state))),
        _ => None,
    }
}

/// Runs dynamic 6502 unpacking emulation and loads the unpacked image into `app_state`.
///
/// # Errors
///
/// Returns [`McpError`] if an optional address parameter is invalid or if unpacking fails.
fn unpack_binary_impl(
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
    args: &Value,
) -> Result<Value, McpError> {
    let mut config = crate::unpacker::UnpackConfig {
        forced_entry: get_optional_address(args, "entry_point")?.map(|a| a.0),
        forced_ret_addr: get_optional_address(args, "return_address")?.map(|a| a.0),
        forced_dep_addr: get_optional_address(args, "depacker_address")?.map(|a| a.0),
        ..crate::unpacker::UnpackConfig::default()
    };

    if let Some(max_inst) = args.get("max_instructions").and_then(Value::as_u64) {
        config.max_instructions = max_inst;
    }

    let result = crate::unpacker::unpack(&app_state.raw_data, app_state.origin.0, &config, None)
        .map_err(|e| make_mcp_error(-32603, format!("Unpack failed: {e}")))?;

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

/// Saves the active project state to its `.regen2000proj` file on disk.
///
/// # Errors
///
/// Returns [`McpError`] if `app_state.project_path` is `None` or if writing to disk fails.
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

/// Executes one undo step on `app_state.undo_stack`.
#[must_use]
fn undo_impl(app_state: &mut AppState) -> Value {
    let mut stack = std::mem::take(&mut app_state.undo_stack);
    let msg_opt = stack.undo(app_state);
    app_state.undo_stack = stack;

    let text = msg_opt.map_or_else(
        || "Nothing to undo".to_string(),
        |msg| format!("Undo: {msg}"),
    );
    json!({ "content": [{ "type": "text", "text": text }] })
}

/// Executes one redo step on `app_state.undo_stack`.
#[must_use]
fn redo_impl(app_state: &mut AppState) -> Value {
    let mut stack = std::mem::take(&mut app_state.undo_stack);
    let msg_opt = stack.redo(app_state);
    app_state.undo_stack = stack;

    let text = msg_opt.map_or_else(
        || "Nothing to redo".to_string(),
        |msg| format!("Redo: {msg}"),
    );
    json!({ "content": [{ "type": "text", "text": text }] })
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
