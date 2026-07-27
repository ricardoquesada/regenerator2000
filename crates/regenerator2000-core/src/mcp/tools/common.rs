//! Shared utility functions and helpers for Model Context Protocol (MCP) tool implementations.
//!
//! This module provides common parameter extraction routines, address parsing logic,
//! error construction helpers, and region conversion utilities used across all MCP tool modules.

use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{Addr, BlockType};
use serde_json::Value;

/// Constructs an [`McpError`] with the given JSON-RPC error code and message.
///
/// # Arguments
///
/// * `code` - The integer JSON-RPC error code (e.g., `-32602` for invalid params).
/// * `message` - The error description text.
///
/// # Returns
///
/// An [`McpError`] initialized with the given code, message, and `None` data payload.
#[must_use]
pub fn make_mcp_error(code: i32, message: impl Into<String>) -> McpError {
    McpError {
        code,
        message: message.into(),
        data: None,
    }
}

/// Parses an address from a JSON value, supporting numeric integers and hex/decimal strings.
///
/// Supports decimal numbers (e.g. `4096`) as well as string representations starting with
/// `0x`, `0X`, or `$` (e.g. `"0x1000"` or `"$1000"`).
///
/// # Arguments
///
/// * `args` - The JSON [`Value`] object containing tool parameters.
/// * `key` - The parameter key name to inspect within `args`.
///
/// # Errors
///
/// Returns [`McpError`] if the parameter is missing or cannot be parsed into a valid 16-bit address [`Addr`].
pub fn get_address(args: &Value, key: &str) -> Result<Addr, McpError> {
    let val = args
        .get(key)
        .ok_or_else(|| make_mcp_error(-32602, format!("Missing required parameter: '{key}'")))?;

    if let Some(n) = val.as_u64() {
        if n <= 0xFFFF {
            #[allow(clippy::cast_possible_truncation)]
            return Ok(Addr(n as u16));
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

/// Parses a string representation of an address (decimal, `0x` hex, or `$` hex).
///
/// # Arguments
///
/// * `s` - The string slice containing the hex or decimal address representation.
///
/// # Returns
///
/// `Some(u16)` if parsing succeeds, or `None` if the input is malformed or out of range.
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

/// Converts a memory region to a target block type and updates disassembly.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to the application state [`AppState`].
/// * `args` - The JSON [`Value`] containing `start_address` and `end_address` keys.
/// * `target_type` - The target [`BlockType`] to assign to the memory range.
///
/// # Errors
///
/// Returns [`McpError`] if address parsing fails, if start address > end address, or
/// if the range falls outside loaded binary bounds.
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
            format!("Invalid address range: start (${start}) > end (${end})"),
        ));
    }

    if app_state.is_external(start) || app_state.is_external(end) {
        return Err(make_mcp_error(
            -32602,
            format!("Address range ${start}..=${end} is outside loaded binary range"),
        ));
    }

    let origin = app_state.origin;
    let start_idx = start.offset_from(origin);
    let end_idx = end.offset_from(origin);
    let range = start_idx..end_idx + 1;
    let old_types = app_state.block_types[range.clone()].to_vec();

    let command = crate::commands::Command::SetBlockType {
        range,
        new_type: target_type,
        old_types,
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("Converted region ${start}..=${end} to {target_type:?}")
    }))
}

/// Constructs a mock [`AppState`] with specified origin address and memory size for unit tests.
///
/// # Arguments
///
/// * `origin` - The 16-bit base origin memory address.
/// * `size` - The number of raw bytes in the memory buffer.
///
/// # Returns
///
/// An [`AppState`] populated with zeroed raw bytes, code block types, and disassembly.
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

/// Constructs a mock [`CoreViewState`](crate::view_state::CoreViewState) for unit tests.
///
/// # Returns
///
/// A newly initialized [`CoreViewState`](crate::view_state::CoreViewState).
#[cfg(test)]
#[must_use]
pub fn make_view_state() -> crate::view_state::CoreViewState {
    crate::view_state::CoreViewState::new()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
