//! Shared utility functions for Model Context Protocol (MCP) tool handlers.
//!
//! This module provides parameter parsers, error constructors, and block conversion
//! helpers for all MCP tool submodules.

use crate::commands::Command;
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{Addr, BlockType};
use serde_json::Value;

/// Creates an [`McpError`] with the specified JSON-RPC error `code` and `message`.
#[must_use]
pub fn make_mcp_error(code: i32, message: impl Into<String>) -> McpError {
    McpError {
        code,
        message: message.into(),
        data: None,
    }
}

/// Parses a 16-bit [`Addr`] from a JSON [`Value`] (integer or hex/decimal string).
///
/// # Errors
///
/// Returns [`McpError`] if `val` is out of the `0..=65535` range or is not a valid address.
pub(crate) fn parse_address_value(val: &Value, key: &str) -> Result<Addr, McpError> {
    if let Some(n) = val.as_u64() {
        if let Ok(addr) = u16::try_from(n) {
            return Ok(Addr(addr));
        }
        return Err(make_mcp_error(
            -32602,
            format!("Address out of range (0-65535): {n}"),
        ));
    }

    if let Some(s) = val.as_str()
        && let Some(addr) = parse_address_string(s)
    {
        return Ok(Addr(addr));
    }

    Err(make_mcp_error(
        -32602,
        format!(
            "Invalid address format for '{key}'. Expected u16 integer or hex string (e.g. 4096 or \"$1000\"/\"0x1000\")"
        ),
    ))
}

/// Extracts and parses a required 16-bit [`Addr`] parameter from `args[key]`.
///
/// This function accepts decimal integers (for example, `4096`) and strings with
/// `0x`, `0X`, or `$` prefixes (for example, `"0x1000"` or `"$1000"`).
///
/// # Errors
///
/// Returns [`McpError`] if `key` is missing, `null`, or cannot be parsed as a 16-bit [`Addr`].
pub fn get_address(args: &Value, key: &str) -> Result<Addr, McpError> {
    let val = args
        .get(key)
        .filter(|v| !v.is_null())
        .ok_or_else(|| make_mcp_error(-32602, format!("Missing required parameter: '{key}'")))?;

    parse_address_value(val, key)
}

/// Extracts and parses an optional 16-bit [`Addr`] parameter from `args[key]`.
///
/// Returns `Ok(None)` when `key` is absent or `null`.
///
/// # Errors
///
/// Returns [`McpError`] if `args[key]` is present and non-null but is not a valid 16-bit [`Addr`].
pub fn get_optional_address(args: &Value, key: &str) -> Result<Option<Addr>, McpError> {
    let Some(val) = args.get(key).filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    parse_address_value(val, key).map(Some)
}

/// Extracts an optional `(start_address, end_address)` pair from `args`.
///
/// Both `start_address` and `end_address` must be provided together and satisfy
/// `start_address <= end_address`.
///
/// # Errors
///
/// Returns [`McpError`] if either address is invalid, if only one endpoint is provided,
/// or if `start_address > end_address`.
pub fn get_optional_address_range(args: &Value) -> Result<Option<(Addr, Addr)>, McpError> {
    let start_opt = get_optional_address(args, "start_address")?;
    let end_opt = get_optional_address(args, "end_address")?;

    match (start_opt, end_opt) {
        (Some(s), Some(e)) if s <= e => Ok(Some((s, e))),
        (Some(_), Some(_)) => Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address",
        )),
        (Some(_), None) | (None, Some(_)) => Err(make_mcp_error(
            -32602,
            "Both start_address and end_address must be provided together",
        )),
        (None, None) => Ok(None),
    }
}

/// Parses a string representation of a 16-bit address (decimal, `0x` hex, `0X` hex, or `$` hex).
#[must_use]
pub fn parse_address_string(s: &str) -> Option<u16> {
    let s = s.trim();
    if let Some(hex) = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .or_else(|| s.strip_prefix('$'))
    {
        u16::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u16>().ok()
    }
}

/// Applies `block_cmd`, records the pre-command analysis snapshot for undo, rebuilds the
/// disassembly, and pushes a single composite [`Command::Batch`] onto the undo stack.
pub(crate) fn apply_block_command_with_analysis(app_state: &mut AppState, block_cmd: Command) {
    let old_labels = app_state.labels.clone();
    let old_cross_refs = app_state.cross_refs.clone();
    block_cmd.apply(app_state);
    let analysis_cmd = Command::SetAnalysisData {
        labels: app_state.labels.clone(),
        cross_refs: app_state.cross_refs.clone(),
        old_labels,
        old_cross_refs,
    };
    app_state.disassemble();
    app_state.push_command(Command::Batch(vec![analysis_cmd, block_cmd]));
}

/// Converts a memory region to `target_type`, records the undo snapshot, and updates disassembly.
///
/// # Errors
///
/// Returns [`McpError`] if address parsing fails, if `start_address > end_address`,
/// if the range falls outside the loaded binary bounds, or if a 2-byte split word/address
/// block type has an odd byte length.
pub fn convert_region(
    app_state: &mut AppState,
    args: &Value,
    target_type: BlockType,
) -> Result<Value, McpError> {
    let start = get_address(args, "start_address")?;
    let end = get_address(args, "end_address")?;

    if start > end {
        return Err(make_mcp_error(
            -32602,
            format!("Invalid address range: start ({start}) > end ({end})"),
        ));
    }

    if app_state.is_external(start) || app_state.is_external(end) {
        return Err(make_mcp_error(
            -32602,
            format!("Address range {start}..={end} is outside loaded binary range"),
        ));
    }

    let origin = app_state.origin;
    let start_idx = start.offset_from(origin);
    let end_idx = end.offset_from(origin);
    let range = start_idx..end_idx + 1;

    if matches!(
        target_type,
        BlockType::LoHiAddress | BlockType::HiLoAddress | BlockType::LoHiWord | BlockType::HiLoWord
    ) && !range.len().is_multiple_of(2)
    {
        return Err(make_mcp_error(
            -32602,
            "Address/Word block requires even number of bytes",
        ));
    }

    let old_types = app_state.block_types[range.clone()].to_vec();
    let command = Command::SetBlockType {
        range,
        new_type: target_type,
        old_types,
    };

    apply_block_command_with_analysis(app_state, command);

    Ok(serde_json::json!({
        "content": [{
            "type": "text",
            "text": format!("Region {start}-{end} converted to {target_type:?}")
        }]
    }))
}

/// Creates a mock [`AppState`] with the specified `origin` address and byte `size` for unit tests.
#[cfg(test)]
#[must_use]
pub fn make_app_state(origin: u16, size: usize) -> AppState {
    let mut state = AppState::new();
    state.origin = Addr(origin);
    state.raw_data = vec![0u8; size];
    state.block_types = vec![BlockType::Code; size];
    state.disassemble();
    state
}

/// Creates a default [`CoreViewState`](crate::view_state::CoreViewState) for unit tests.
#[cfg(test)]
#[must_use]
pub fn make_view_state() -> crate::view_state::CoreViewState {
    crate::view_state::CoreViewState::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Label, LabelKind, LabelType};

    #[test]
    fn test_parse_required_param_success() {
        let args = serde_json::json!({
            "address": 4096,
            "name": "main_loop"
        });
        let addr = get_address(&args, "address").expect("Should parse address");
        assert_eq!(addr, Addr(4096));
    }

    #[test]
    fn test_parse_required_param_missing() {
        let args = serde_json::json!({
            "other_param": 123
        });
        let err = get_address(&args, "address").expect_err("Should return error for missing param");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing required parameter"));
    }

    #[test]
    fn test_parse_address_hex_and_decimal() {
        assert_eq!(parse_address_string("4096"), Some(4096));
        assert_eq!(parse_address_string("$1000"), Some(0x1000));
        assert_eq!(parse_address_string("0x1000"), Some(0x1000));
        assert_eq!(parse_address_string("0X1000"), Some(0x1000));
        assert_eq!(parse_address_string("invalid"), None);
    }

    #[test]
    fn test_get_optional_address_range_supports_hex_and_rejects_invalid() {
        let args = serde_json::json!({
            "start_address": "$1000",
            "end_address": "0x1010"
        });
        assert_eq!(
            get_optional_address_range(&args).unwrap(),
            Some((Addr(0x1000), Addr(0x1010)))
        );

        let bad_hex = serde_json::json!({
            "start_address": "$ZZZZ",
            "end_address": "$1010"
        });
        assert!(get_optional_address_range(&bad_hex).is_err());

        let out_of_range = serde_json::json!({
            "start_address": 70000,
            "end_address": 70010
        });
        assert!(get_optional_address_range(&out_of_range).is_err());
    }

    #[test]
    fn test_convert_region_rejects_odd_length_split_word_and_preserves_auto_labels_on_undo() {
        let mut app_state = make_app_state(0x1000, 16);
        app_state.labels.insert(
            Addr(0x1005),
            vec![Label {
                name: "a1005".to_string(),
                label_type: LabelType::AbsoluteAddress,
                kind: LabelKind::Auto,
            }],
        );

        let odd_err = convert_region(
            &mut app_state,
            &serde_json::json!({ "start_address": 0x1000, "end_address": 0x1002 }),
            BlockType::LoHiAddress,
        )
        .expect_err("odd byte length must fail for LoHiAddress");
        assert!(odd_err.message.contains("even number of bytes"));

        convert_region(
            &mut app_state,
            &serde_json::json!({ "start_address": 0x1000, "end_address": 0x1003 }),
            BlockType::DataByte,
        )
        .expect("DataByte conversion should succeed");
        assert!(!app_state.labels.contains_key(&Addr(0x1005)));

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(
            app_state.labels.contains_key(&Addr(0x1005)),
            "undoing convert_region must restore pre-command auto labels"
        );
    }
}
