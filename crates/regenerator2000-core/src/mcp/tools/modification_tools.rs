//! MCP tool handlers for modifying assembly state, labels, symbolic offset expressions,
//! comments, block types, immediate formats, and project enums.
//!
//! This module defines JSON schemas and handlers that mutate [`AppState`] via shared core methods.
//! Tools include `r2000_set_symbolic_offset`, Tier 2 expressions in `r2000_set_label_name`, and
//! signed displacements in `r2000_set_immediate_format`.

use crate::commands::Command;
use crate::cpu::AddressingMode;
use crate::mcp::tools::common::{
    convert_region, get_address, get_optional_address, get_optional_address_range, make_mcp_error,
};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{
    Addr, BlockType, CommentKind, EnumDefinition, ImmediateFormat, LabelKind, OffsetRadix,
    SymbolicOffset,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Returns JSON schemas for memory modification and state-mutation tools.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_disassemble",
            "description": "Run recursive flow disassembly starting at a specified address.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "Start address to disassemble (decimal)." }
                },
                "required": ["address"]
            }
        }),
        json!({
            "name": "r2000_set_label_name",
            "description": "Set or delete a user-defined label (or Tier 2 global symbolic offset expression such as 'zp_ptr + 1') at a specific MOS 6502 memory address. Use this tool to name subroutines, variables, jump targets, or pointer/struct fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address where the label should be set (decimal, e.g. 4096 for $1000)." },
                    "name": { "type": "string", "description": "Label name (e.g. 'init_screen'), symbolic offset expression (e.g. 'zp_ptr + 1'), or '' to delete an existing user label or symbolic offset." }
                },
                "required": ["address", "name"]
            }
        }),
        json!({
            "name": "r2000_set_symbolic_offset",
            "description": "Format an operand, address table (including Lo/Hi RTS tables), or target address as a symbolic offset (e.g. 'zp_ptr + 1', 'sub_C100 - 1'). Pass 'address' for one site or address, or pass 'start_address' and 'end_address' for a table or struct range. Set the offset with 'expression' ('zp_ptr + 1' or '' to clear), 'base' (name or address), or 'offset' (signed integer, 0 to clear).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "Single instruction, table entry, or target memory address (decimal)." },
                    "start_address": { "type": "integer", "description": "Start address (inclusive, decimal) when applying to an address table or struct range." },
                    "end_address": { "type": "integer", "description": "End address (inclusive, decimal) when applying to an address table or struct range." },
                    "expression": { "type": "string", "description": "Symbolic expression such as 'zp_ptr + 1', 'sub_C100 - 1', 'SCREEN_RAM + $28', or '' to clear." },
                    "base": { "type": ["string", "integer"], "description": "Base symbol name (e.g. 'zp_ptr', 'SCREEN_RAM') or base address (e.g. 251 or '$00FB'). Automatically calculates offset = effective_address - base." },
                    "offset": { "type": "integer", "description": "Signed byte displacement from base (e.g. -1 for RTS dispatch tables or 1-based indexed loops, +1 for pointer high byte, 0 to clear). Automatically calculates base = effective_address - offset." },
                    "scope": {
                        "type": "string",
                        "enum": ["auto", "operand", "address"],
                        "description": "'operand' sets Tier 1 on the instruction or table at address. 'address' sets Tier 2 on the target address across all references. Default 'auto' uses 'address' when no operand exists, and 'operand' on instructions or address tables."
                    },
                    "radix": {
                        "type": "string",
                        "enum": ["decimal", "hex"],
                        "description": "Offset radix ('decimal' -> '+ 40', 'hex' -> '+ $28'). Default: 'decimal'."
                    }
                }
            }
        }),
        json!({
            "name": "r2000_set_comment",
            "description": "Set or delete a comment at a specific address. 'line' comments appear on their own line before the instruction (multi-line supported). 'side' comments appear inline on the instruction line. Pass an empty string (or omit 'comment') to delete an existing comment.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address for the comment (decimal, e.g. 4096 for $1000)." },
                    "comment": { "type": "string", "description": "Comment text (omit the leading comment prefix character), or an empty string to delete the existing comment." },
                    "type": {
                        "type": "string",
                        "enum": ["line", "side"],
                        "description": "'line' = comment on its own line before the instruction. 'side' = inline comment on the same line."
                    }
                },
                "required": ["address", "type"]
            }
        }),
        json!({
            "name": "r2000_set_data_type",
            "description": "Set the block data type for a memory region (code, bytes, words, addresses, text, or split tables).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "start_address": { "type": "integer", "description": "Start of the memory region (inclusive), decimal." },
                    "end_address":   { "type": "integer", "description": "End of the memory region (inclusive), decimal." },
                    "data_type": {
                        "type": "string",
                        "enum": [
                            "code", "byte", "word", "address", "petscii", "screencode",
                            "lo_hi_address", "hi_lo_address", "lo_hi_word", "hi_lo_word",
                            "external_file", "undefined"
                        ],
                        "description": "Data type classification string."
                    }
                },
                "required": ["start_address", "end_address", "data_type"]
            }
        }),
        json!({
            "name": "r2000_toggle_splitter",
            "description": "Toggle a block splitter at a specific address. Splitters prevent the disassembler from merging adjacent blocks of the same type.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address where the splitter should be toggled (decimal)." }
                },
                "required": ["address"]
            }
        }),
        json!({
            "name": "r2000_set_immediate_format",
            "description": "Set the operand format for an immediate instruction at a specific address. Supports hexadecimal, decimal, binary, and low/high byte references to a 16-bit address (with optional signed symbolic byte displacement).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The address of the instruction (decimal)." },
                    "format": {
                        "type": "string",
                        "enum": ["hex", "decimal", "binary", "low_byte", "high_byte"],
                        "description": "hex=$00, decimal=0, binary=%00000000, low_byte=#<p_ADDRESS, high_byte=#>p_ADDRESS."
                    },
                    "target_address": { "type": "integer", "description": "The 16-bit target address decimal (e.g. 60000 for $EA31). Required when format is 'low_byte' or 'high_byte'." },
                    "offset": { "type": "integer", "description": "Optional signed byte displacement (such as -1 for #<(sub_C100 - 1)). Accepts a base or effective target_address and checks the immediate operand byte." },
                    "radix": {
                        "type": "string",
                        "enum": ["decimal", "hex"],
                        "description": "Offset radix when offset is non-zero ('decimal' -> '- 1', 'hex' -> '- $1'). Default: 'decimal'."
                    }
                },
                "required": ["address", "format"]
            }
        }),
        json!({
            "name": "r2000_create_project_enum",
            "description": "Create a new project-specific enum definition in the project state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique alphanumeric identifier." },
                    "description": { "type": "string", "description": "Optional summary of the enum purpose." },
                    "variants": {
                        "type": "object",
                        "description": "Variant map where keys are numeric strings (decimal, hex 0x/$, bin 0b/%) and values are variant names."
                    }
                },
                "required": ["name", "variants"]
            }
        }),
        json!({
            "name": "r2000_update_project_enum",
            "description": "Update or rename an existing project-specific enum.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Existing name of the enum to update." },
                    "new_name": { "type": "string", "description": "Optional new name when renaming the enum." },
                    "description": { "type": "string", "description": "Optional updated summary of the enum purpose." },
                    "variants": { "type": "object", "description": "Variant map." }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "r2000_delete_project_enum",
            "description": "Delete a project-specific enum definition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the enum to delete." },
                    "force": { "type": "boolean", "description": "When true, forces deletion even if the enum is in use. Defaults to false." }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "r2000_apply_enum_usage",
            "description": "Apply or clear an enum usage annotation for an immediate operand at a specific address.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The address of the instruction (decimal)." },
                    "name": { "type": "string", "description": "Name of the enum to apply, or empty string to clear." }
                },
                "required": ["address"]
            }
        }),
    ]
}

/// Dispatches state-modification tool calls by `name`.
///
/// Returns `Some(Ok(value))` on success, `Some(Err(McpError))` on failure, or `None`
/// if `name` does not match a modification tool.
pub fn handle_call(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
) -> Option<Result<Value, McpError>> {
    match name {
        "r2000_disassemble" => Some(disassemble_impl(app_state, args)),
        "r2000_set_label_name" => Some(set_label_name_impl(app_state, args)),
        "r2000_set_symbolic_offset" => Some(set_symbolic_offset_impl(app_state, args)),
        "r2000_set_comment" => Some(set_comment_impl(app_state, args)),
        "r2000_set_data_type" => Some(set_data_type_impl(app_state, args)),
        "r2000_toggle_splitter" => Some(toggle_splitter_impl(app_state, args)),
        "r2000_set_immediate_format" => Some(set_immediate_format_mcp_impl(app_state, args)),
        "r2000_create_project_enum" => Some(create_project_enum_mcp(app_state, args)),
        "r2000_update_project_enum" => Some(update_project_enum_mcp(app_state, args)),
        "r2000_delete_project_enum" => Some(delete_project_enum_mcp(app_state, args)),
        "r2000_apply_enum_usage" => Some(apply_enum_usage_impl(app_state, args)),
        _ => None,
    }
}

/// Runs recursive flow disassembly from `args["address"]` and records an undoable batch command.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"` is missing, invalid, or outside the loaded binary range.
fn disassemble_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    if app_state.is_external(address) {
        return Err(make_mcp_error(
            -32602,
            format!(
                "Address ${address:04X} is out of bounds (Origin: ${:04X})",
                app_state.origin.0
            ),
        ));
    }

    let ranges = crate::analyzer::flow_analyze(app_state, address);
    if ranges.is_empty() {
        return Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("No new code found starting at ${:04X}", address.0)
            }]
        }));
    }

    let old_labels = app_state.labels.clone();
    let old_cross_refs = app_state.cross_refs.clone();
    let max_len = app_state.block_types.len();
    let mut commands = Vec::with_capacity(ranges.len());

    for range in ranges {
        let end = range.end.min(max_len);
        let old_types = app_state.block_types[range.start..end].to_vec();
        for i in range.start..end {
            app_state.block_types[i] = BlockType::Code;
        }
        commands.push(Command::SetBlockType {
            range,
            new_type: BlockType::Code,
            old_types,
        });
    }

    let (_, msg) = app_state.perform_analysis();
    let analysis_cmd = Command::SetAnalysisData {
        labels: app_state.labels.clone(),
        cross_refs: app_state.cross_refs.clone(),
        old_labels,
        old_cross_refs,
    };
    let batch = Command::Batch(commands);
    app_state.push_command(Command::Batch(vec![analysis_cmd, batch]));

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Disassembled successfully starting at ${:04X}. {msg}", address.0)
        }]
    }))
}

/// Sets or deletes a user label (or Tier 2 symbolic offset expression) at `args["address"]`.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"` or `"name"` is missing or if label or expression checks fail.
fn set_label_name_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let label_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing or invalid 'name'"))?;

    let applied_offset = app_state
        .apply_user_label_or_address_offset(address, label_name, false, None)
        .map_err(|msg| make_mcp_error(-32602, msg))?;

    let msg = if let Some(sym) = applied_offset {
        let expr = app_state.format_symbolic_offset(sym, None, address.0 <= 0x00FF);
        format!("Label set at ${address:04X} ({expr})")
    } else {
        format!("Label set at ${address:04X}")
    };

    Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
}

/// Requested scope parameter for `r2000_set_symbolic_offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolicScopeArg {
    Auto,
    Operand,
    Address,
}

/// Resolved target tier for `r2000_set_symbolic_offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedSymbolicScope {
    /// Tier 1 (`AddressEntry::operand_offset`) on referencing instruction/table sites.
    Operand,
    /// Tier 2 (`AddressEntry::address_offset`) globally on target memory addresses.
    Address,
}

/// Normalized symbolic-offset input parsed from MCP arguments.
#[derive(Debug, Clone)]
struct OffsetInputSpec {
    input: String,
    override_radix: Option<OffsetRadix>,
    is_clear: bool,
}

impl OffsetInputSpec {
    /// Parses an [`OffsetInputSpec`] from `args` and rejects ambiguous `'base'` with `'offset': 0`.
    fn from_args(args: &Value, explicit_radix: Option<OffsetRadix>) -> Result<Self, McpError> {
        let expr_opt = match args.get("expression") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => {
                return Err(make_mcp_error(
                    -32602,
                    "Invalid 'expression' (expected a string)",
                ));
            }
        };
        let base_opt = parse_optional_base_token(args)?;
        let offset_opt = parse_optional_i16_offset(args, "offset")?;
        let default_radix = Some(explicit_radix.unwrap_or(OffsetRadix::Decimal));

        match (expr_opt, base_opt, offset_opt) {
            (Some(expr), None, None) => Ok(Self {
                input: expr.to_string(),
                override_radix: explicit_radix,
                is_clear: AppState::is_symbolic_clear_input(expr),
            }),
            (Some(_), _, _) => Err(make_mcp_error(
                -32602,
                "Provide either 'expression' or 'base'/'offset', not both",
            )),
            (None, Some(base), None) => Ok(Self {
                input: base,
                override_radix: default_radix,
                is_clear: false,
            }),
            (None, None, Some(0)) => Ok(Self {
                input: String::new(),
                override_radix: None,
                is_clear: true,
            }),
            (None, None, Some(offset)) => Ok(Self {
                input: format!("{offset:+}"),
                override_radix: default_radix,
                is_clear: false,
            }),
            (None, Some(_), Some(0)) => Err(make_mcp_error(
                -32602,
                "Do not combine 'base' with 'offset' 0. Pass 'offset': 0 or 'expression': '' to clear.",
            )),
            (None, Some(base), Some(offset)) => {
                let sign = if offset >= 0 { '+' } else { '-' };
                let mag = offset.unsigned_abs();
                let radix = explicit_radix.unwrap_or(OffsetRadix::Decimal);
                let input = match radix {
                    OffsetRadix::Decimal => format!("{base} {sign} {mag}"),
                    OffsetRadix::Hex => format!("{base} {sign} ${mag:x}"),
                };
                Ok(Self {
                    input,
                    override_radix: Some(radix),
                    is_clear: false,
                })
            }
            (None, None, None) => Err(make_mcp_error(
                -32602,
                "Missing offset specification: provide 'expression', 'base', or 'offset'",
            )),
        }
    }

    /// Evaluates the offset specification for `effective_addr` via
    /// [`AppState::parse_symbolic_offset_input_cached`].
    fn evaluate(
        &self,
        app_state: &AppState,
        effective_addr: Addr,
        is_zero_page_mode: bool,
        current_scope: Option<(Addr, Addr)>,
        cache: &mut crate::state::app_state::SymbolicBaseCache,
    ) -> Result<Option<SymbolicOffset>, McpError> {
        if self.is_clear && self.input.is_empty() {
            return Ok(None);
        }
        let mut parsed = app_state
            .parse_symbolic_offset_input_cached(
                &self.input,
                effective_addr,
                is_zero_page_mode,
                current_scope,
                cache,
            )
            .map_err(|msg| make_mcp_error(-32602, msg))?;
        if let (Some(sym), Some(r)) = (&mut parsed, self.override_radix) {
            sym.radix = r;
        }
        Ok(parsed)
    }
}

/// Parses an optional signed 16-bit displacement from `args[key]` strictly via [`Value::as_i64`].
///
/// Never uses `Value::as_u64` as the primary parser because negative JSON integers (such as `-1`
/// for `RTS` dispatch tables and 1-based loops) return `None` from `as_u64()`.
///
/// # Errors
///
/// Returns [`McpError`] (`code == -32602`) if `args[key]` is non-null and either is not an integer
/// or overflows `i16::MIN..=i16::MAX`.
fn parse_optional_i16_offset(args: &Value, key: &str) -> Result<Option<i16>, McpError> {
    let Some(val) = args.get(key).filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let raw = val.as_i64().ok_or_else(|| {
        if val.as_u64().is_some() {
            make_mcp_error(
                -32602,
                format!(
                    "Offset in '{key}' overflows signed 16-bit displacement range (-32768..=32767)"
                ),
            )
        } else {
            make_mcp_error(
                -32602,
                format!("Invalid '{key}': expected a signed integer (e.g. -1, 0, 1)"),
            )
        }
    })?;
    let offset = i16::try_from(raw).map_err(|_| {
        make_mcp_error(
            -32602,
            format!(
                "Offset {raw} in '{key}' overflows signed 16-bit displacement range (-32768..=32767)"
            ),
        )
    })?;
    Ok(Some(offset))
}

/// Parses an optional `"radix"` parameter (`"decimal"` or `"hex"`).
fn parse_optional_radix(args: &Value) -> Result<Option<OffsetRadix>, McpError> {
    let Some(val) = args.get("radix").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let s = val.as_str().ok_or_else(|| {
        make_mcp_error(-32602, "Invalid 'radix' (expected \"decimal\" or \"hex\")")
    })?;
    match s.trim().to_ascii_lowercase().as_str() {
        "decimal" | "dec" => Ok(Some(OffsetRadix::Decimal)),
        "hex" | "hexadecimal" => Ok(Some(OffsetRadix::Hex)),
        other => Err(make_mcp_error(
            -32602,
            format!("Invalid 'radix' value \"{other}\": expected \"decimal\" or \"hex\""),
        )),
    }
}

/// Parses an optional `"scope"` parameter (`"auto"`, `"operand"`, or `"address"`).
fn parse_symbolic_scope_arg(args: &Value) -> Result<SymbolicScopeArg, McpError> {
    let Some(val) = args.get("scope").filter(|v| !v.is_null()) else {
        return Ok(SymbolicScopeArg::Auto);
    };
    let s = val.as_str().ok_or_else(|| {
        make_mcp_error(
            -32602,
            "Invalid 'scope' (expected \"auto\", \"operand\", or \"address\")",
        )
    })?;
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(SymbolicScopeArg::Auto),
        "operand" => Ok(SymbolicScopeArg::Operand),
        "address" => Ok(SymbolicScopeArg::Address),
        other => Err(make_mcp_error(
            -32602,
            format!(
                "Invalid 'scope' value \"{other}\": expected \"auto\", \"operand\", or \"address\""
            ),
        )),
    }
}

/// Parses an optional `"base"` parameter (16-bit integer address or symbol/hex string).
fn parse_optional_base_token(args: &Value) -> Result<Option<String>, McpError> {
    let Some(val) = args.get("base").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    if let Some(i) = val.as_i64() {
        let addr = u16::try_from(i).map_err(|_| {
            make_mcp_error(-32602, format!("Base address out of range (0-65535): {i}"))
        })?;
        return Ok(Some(format!("${addr:04X}")));
    }
    if let Some(u) = val.as_u64() {
        let addr = u16::try_from(u).map_err(|_| {
            make_mcp_error(-32602, format!("Base address out of range (0-65535): {u}"))
        })?;
        return Ok(Some(format!("${addr:04X}")));
    }
    if let Some(s) = val.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(make_mcp_error(-32602, "Parameter 'base' cannot be empty"));
        }
        if trimmed.contains(['+', '-']) {
            return Err(make_mcp_error(
                -32602,
                "Parameter 'base' must be a symbol name or base address without '+'/'-' (use 'expression' for full expressions)",
            ));
        }
        return Ok(Some(trimmed.to_string()));
    }
    Err(make_mcp_error(
        -32602,
        "Invalid 'base': expected a symbol name string or 16-bit integer address",
    ))
}

/// Applies or clears Tier 1 (`operand_offset`) or Tier 2 (`address_offset`) symbolic offsets
/// across a single address or inclusive address range.
///
/// # Errors
///
/// Returns [`McpError`] when address or range arguments conflict or when no offset input is given.
/// Also returns [`McpError`] when any site or address fails bounds, zero-page base, or expression checks.
fn set_symbolic_offset_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let single_addr = get_optional_address(args, "address")?;
    let has_any_range_endpoint = args.get("start_address").filter(|v| !v.is_null()).is_some()
        || args.get("end_address").filter(|v| !v.is_null()).is_some();

    if single_addr.is_some() && has_any_range_endpoint {
        return Err(make_mcp_error(
            -32602,
            "Provide either 'address' or 'start_address'/'end_address', not both",
        ));
    }

    let range_opt = get_optional_address_range(args)?;
    let (start, end) = match (single_addr, range_opt) {
        (Some(addr), None) => (addr, addr),
        (None, Some((s, e))) => (s, e),
        (Some(_), Some(_)) => {
            return Err(make_mcp_error(
                -32602,
                "Provide either 'address' or 'start_address'/'end_address', not both",
            ));
        }
        (None, None) => {
            return Err(make_mcp_error(
                -32602,
                "Provide either 'address' or both 'start_address' and 'end_address'",
            ));
        }
    };

    let scope_arg = parse_symbolic_scope_arg(args)?;
    let explicit_radix = parse_optional_radix(args)?;
    let spec = OffsetInputSpec::from_args(args, explicit_radix)?;

    let range_is_external = app_state.is_external(start) || app_state.is_external(end);
    let extracted_targets = if range_is_external {
        Vec::new()
    } else {
        app_state.effective_targets_with_pair_in_range(start, end)
    };

    let resolved_scope = match scope_arg {
        SymbolicScopeArg::Operand => ResolvedSymbolicScope::Operand,
        SymbolicScopeArg::Address => ResolvedSymbolicScope::Address,
        SymbolicScopeArg::Auto => {
            if spec.is_clear && start == end && !range_is_external {
                let has_tier1 = app_state
                    .annotations
                    .get(start)
                    .and_then(|e| e.operand_offset)
                    .is_some()
                    || extracted_targets.first().is_some_and(|t| {
                        app_state
                            .annotations
                            .get(t.site_addr)
                            .and_then(|e| e.operand_offset)
                            .is_some()
                            || t.second_pair_addr
                                .and_then(|s| app_state.annotations.get(s))
                                .and_then(|e| e.operand_offset)
                                .is_some()
                    });
                let has_tier2 = app_state
                    .annotations
                    .get(start)
                    .and_then(|e| e.address_offset)
                    .is_some()
                    || extracted_targets.iter().any(|t| {
                        crate::disassembler::symbols::has_active_address_offset(
                            t.raw_target,
                            &app_state.labels,
                            &app_state.annotations,
                        )
                    });
                if has_tier1 {
                    ResolvedSymbolicScope::Operand
                } else if has_tier2 {
                    ResolvedSymbolicScope::Address
                } else if !extracted_targets.is_empty() {
                    ResolvedSymbolicScope::Operand
                } else {
                    ResolvedSymbolicScope::Address
                }
            } else if !range_is_external && !extracted_targets.is_empty() {
                ResolvedSymbolicScope::Operand
            } else {
                ResolvedSymbolicScope::Address
            }
        }
    };

    let mut base_cache = crate::state::app_state::SymbolicBaseCache::default();

    match resolved_scope {
        ResolvedSymbolicScope::Operand => {
            if range_is_external {
                return Err(make_mcp_error(
                    -32602,
                    format!(
                        "Operand scope requires addresses inside loaded binary range, got ${start:04X}..=${end:04X}"
                    ),
                ));
            }
            if extracted_targets.is_empty() {
                if spec.is_clear
                    && start == end
                    && app_state
                        .annotations
                        .get(start)
                        .and_then(|e| e.operand_offset)
                        .is_some()
                {
                    app_state.apply_operand_offsets(&[(start, None)]);
                    return Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Cleared operand symbolic offset at ${start:04X}")
                        }]
                    }));
                }
                return Err(make_mcp_error(
                    -32602,
                    format!(
                        "No instruction operands or address table entries found in ${start:04X}..=${end:04X}"
                    ),
                ));
            }

            let is_multi_sites = extracted_targets.len() > 1;
            let mut applied_site_count = 0usize;
            let mut updates = Vec::with_capacity(extracted_targets.len() * 2);
            for t in &extracted_targets {
                let current_scope = app_state.enclosing_scope_for_address(t.site_addr);
                let parsed = spec.evaluate(
                    app_state,
                    t.raw_target,
                    t.is_zero_page_mode,
                    current_scope,
                    &mut base_cache,
                )?;
                if !spec.is_clear && parsed.is_none() {
                    if is_multi_sites {
                        continue;
                    }
                    return Err(make_mcp_error(
                        -32602,
                        format!(
                            "Base matches target address ${:04X} with offset 0. Pass 'offset': 0 or 'expression': '' to clear.",
                            t.raw_target.0
                        ),
                    ));
                }
                applied_site_count += 1;
                updates.push((t.site_addr, parsed));
                if let Some(second_addr) = t.second_pair_addr
                    && second_addr != t.site_addr
                    && app_state
                        .annotations
                        .get(second_addr)
                        .and_then(|e| e.operand_offset)
                        .is_some()
                {
                    updates.push((second_addr, None));
                }
            }

            if !spec.is_clear && applied_site_count == 0 {
                return Err(make_mcp_error(
                    -32602,
                    format!(
                        "Base matches every target in ${start:04X}..=${end:04X} with offset 0."
                    ),
                ));
            }

            app_state.apply_operand_offsets(&updates);
            let action = if spec.is_clear { "Cleared" } else { "Set" };
            let msg = format!(
                "{action} operand symbolic offset on {applied_site_count} site(s) in ${start:04X}..=${end:04X}"
            );
            Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
        }
        ResolvedSymbolicScope::Address => {
            let clear_direct_start = spec.is_clear
                && start == end
                && app_state
                    .annotations
                    .get(start)
                    .and_then(|e| e.address_offset)
                    .is_some()
                && extracted_targets.iter().all(|t| {
                    app_state
                        .annotations
                        .get(t.raw_target)
                        .and_then(|e| e.address_offset)
                        .is_none()
                });

            let (items, site_addrs) =
                if !range_is_external && !extracted_targets.is_empty() && !clear_direct_start {
                    let mut target_reqs = BTreeMap::<Addr, (Addr, bool)>::new();
                    let mut sites = Vec::with_capacity(extracted_targets.len() * 2);
                    for t in &extracted_targets {
                        let zp = t.is_zero_page_mode || t.raw_target.0 <= 0x00FF;
                        target_reqs
                            .entry(t.raw_target)
                            .and_modify(|(_, existing_zp)| *existing_zp |= zp)
                            .or_insert((t.site_addr, zp));
                        sites.push(t.site_addr);
                        if let Some(second_addr) = t.second_pair_addr {
                            sites.push(second_addr);
                        }
                    }
                    let reqs: Vec<_> = target_reqs
                        .into_iter()
                        .map(|(target, (site, is_zp))| {
                            let scope = app_state
                                .enclosing_scope_for_address(site)
                                .or_else(|| app_state.enclosing_scope_for_address(target));
                            (target, scope, is_zp)
                        })
                        .collect();
                    (reqs, sites)
                } else {
                    let reqs = (start.0..=end.0)
                        .map(|addr_u16| {
                            let addr = Addr(addr_u16);
                            (
                                addr,
                                app_state.enclosing_scope_for_address(addr),
                                addr.0 <= 0x00FF,
                            )
                        })
                        .collect();
                    (reqs, Vec::new())
                };

            let is_multi = items.len() > 1;
            let mut updates = Vec::with_capacity(items.len());
            for (addr, current_scope, is_zp) in items {
                let parsed =
                    spec.evaluate(app_state, addr, is_zp, current_scope, &mut base_cache)?;
                if !spec.is_clear && parsed.is_none() {
                    if is_multi {
                        continue;
                    }
                    return Err(make_mcp_error(
                        -32602,
                        format!(
                            "Base matches target address ${addr:04X} with offset 0. Pass 'offset': 0 or 'expression': '' to clear."
                        ),
                    ));
                }
                updates.push((addr, parsed));
            }

            if !spec.is_clear && updates.is_empty() {
                return Err(make_mcp_error(
                    -32602,
                    format!(
                        "Base matches every address in ${start:04X}..=${end:04X} with offset 0."
                    ),
                ));
            }

            let count = updates.len();
            app_state.apply_address_offsets_for_sites(&updates, &site_addrs);
            let action = if spec.is_clear { "Cleared" } else { "Set" };
            let msg = format!(
                "{action} global symbolic offset on {count} address(es) in ${start:04X}..=${end:04X}"
            );
            Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
        }
    }
}

/// Sets or deletes a user line or side comment at `args["address"]`.
///
/// # Errors
///
/// Returns [`McpError`] if parameters are missing or invalid, or if `"type"` is not `"line"` or `"side"`.
fn set_comment_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    if args.get("text").is_some() {
        return Err(make_mcp_error(
            -32602,
            "Unexpected parameter 'text': use 'comment' instead",
        ));
    }
    let comment = match args.get("comment") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        Some(_) => {
            return Err(make_mcp_error(
                -32602,
                "Invalid 'comment' (expected a string)",
            ));
        }
    };
    let comment_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'type' (expected \"line\" or \"side\")"))?;

    let kind = match comment_type {
        "side" => CommentKind::Side,
        "line" => CommentKind::Line,
        other => {
            return Err(make_mcp_error(
                -32602,
                format!("Invalid 'type' value \"{other}\": expected \"line\" or \"side\""),
            ));
        }
    };

    let command = app_state.create_set_comment_command(address, comment, kind);
    let is_clearing = matches!(
        &command,
        Command::SetUserSideComment {
            new_comment: None,
            ..
        } | Command::SetUserLineComment {
            new_comment: None,
            ..
        }
    );

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();
    let msg = if is_clearing {
        format!("Comment removed at ${address:04X}")
    } else {
        format!("Comment set at ${address:04X}")
    };
    Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
}

/// Sets the [`BlockType`] classification for an address range in `args`.
///
/// # Errors
///
/// Returns [`McpError`] if `"data_type"` is unknown or if the region conversion fails.
fn set_data_type_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let data_type_str = args
        .get("data_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'data_type'"))?;

    let block_type = match data_type_str {
        "code" => BlockType::Code,
        "byte" => BlockType::DataByte,
        "word" => BlockType::DataWord,
        "address" => BlockType::Address,
        "petscii" => BlockType::PetsciiText,
        "screencode" => BlockType::ScreencodeText,
        "lo_hi_address" => BlockType::LoHiAddress,
        "hi_lo_address" => BlockType::HiLoAddress,
        "lo_hi_word" => BlockType::LoHiWord,
        "hi_lo_word" => BlockType::HiLoWord,
        "external_file" => BlockType::ExternalFile,
        "undefined" => BlockType::Undefined,
        _ => {
            return Err(make_mcp_error(
                -32602,
                format!("Unknown data_type: '{data_type_str}'"),
            ));
        }
    };

    convert_region(app_state, args, block_type)
}

/// Toggles a block splitter at `args["address"]`.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"` is missing or invalid.
fn toggle_splitter_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let command = Command::ToggleSplitter { address };
    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();
    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Splitter toggled at ${address:04X}") }] }),
    )
}

/// Extracts parameters for `r2000_set_immediate_format` and applies the immediate operand format.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"`, `"format"`, `"target_address"`, or `"offset"` is invalid.
fn set_immediate_format_mcp_impl(
    app_state: &mut AppState,
    args: &Value,
) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let format_str = args
        .get("format")
        .or_else(|| args.get("format_type"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'format'"))?;
    let target_address = get_optional_address(args, "target_address")?;
    let offset = parse_optional_i16_offset(args, "offset")?;
    let radix = parse_optional_radix(args)?.unwrap_or(OffsetRadix::Decimal);

    set_immediate_format_with_offset_impl(
        app_state,
        address,
        format_str,
        target_address,
        offset,
        radix,
    )?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Immediate format set at ${address:04X}") }] }),
    )
}

/// Ranks defined symbols at `addr` (`0 = User`, `1 = System`, `2 = Auto`, `3 = unlabelled`)
/// to disambiguate base vs. effective target addresses on immediate byte operators.
#[must_use]
fn immediate_base_symbol_rank(app_state: &AppState, addr: Addr) -> u8 {
    if crate::disassembler::symbols::has_active_address_offset(
        addr,
        &app_state.labels,
        &app_state.annotations,
    ) {
        return 3;
    }
    app_state
        .effective_labels_at(addr)
        .iter()
        .map(|l| match l.kind {
            LabelKind::User => 0,
            LabelKind::System => 1,
            LabelKind::Auto => 2,
        })
        .min()
        .unwrap_or(3)
}

/// Reads the immediate operand byte at `address` after checking that `address` decodes to a
/// 2-byte [`AddressingMode::Immediate`] instruction.
fn extract_immediate_operand_byte(app_state: &AppState, address: Addr) -> Result<u8, McpError> {
    if app_state.is_external(address) {
        return Err(make_mcp_error(
            -32602,
            format!("Address ${address:04X} is outside loaded binary range"),
        ));
    }
    let pc = address.offset_from(app_state.origin);
    if pc + 1 >= app_state.raw_data.len() {
        return Err(make_mcp_error(
            -32602,
            format!("Address ${address:04X} does not have an immediate operand byte"),
        ));
    }
    let opcode_byte = app_state.raw_data[pc];
    let is_immediate = app_state
        .disassembler
        .opcodes
        .get(usize::from(opcode_byte))
        .and_then(|op| op.as_ref())
        .filter(|op| !op.illegal || app_state.settings.use_illegal_opcodes)
        .is_some_and(|op| op.mode == AddressingMode::Immediate);
    if !is_immediate {
        return Err(make_mcp_error(
            -32602,
            format!("Instruction at ${address:04X} does not use Immediate addressing mode"),
        ));
    }
    Ok(app_state.raw_data[pc + 1])
}

/// Sets the [`ImmediateFormat`] (and optional [`SymbolicOffset`]) for the instruction at `address`.
///
/// # Errors
///
/// Returns [`McpError`] if `format_str` is unknown, if `target_address` is missing for
/// `"low_byte"` or `"high_byte"` formats, or if `offset` does not match the instruction's
/// immediate operand byte.
pub(crate) fn set_immediate_format_with_offset_impl(
    app_state: &mut AppState,
    address: Addr,
    format_str: &str,
    target_address: Option<Addr>,
    offset: Option<i16>,
    radix: OffsetRadix,
) -> Result<(), McpError> {
    let normalized_fmt = format_str.to_lowercase();
    let is_byte_operator = match normalized_fmt.as_str() {
        "hex" | "decimal" | "dec" | "binary" | "bin" => false,
        "low_byte" | "low" | "lo" | "high_byte" | "high" | "hi" => true,
        _ => {
            return Err(make_mcp_error(
                -32602,
                format!("Unknown format: {format_str}"),
            ));
        }
    };

    if !is_byte_operator {
        if let Some(non_zero) = offset.filter(|&o| o != 0) {
            return Err(make_mcp_error(
                -32602,
                format!(
                    "Parameter 'offset' ({non_zero}) is only supported for 'low_byte' and 'high_byte' formats"
                ),
            ));
        }
        let format = match normalized_fmt.as_str() {
            "hex" => ImmediateFormat::Hex,
            "decimal" | "dec" => ImmediateFormat::Decimal,
            _ => ImmediateFormat::Binary,
        };
        app_state.apply_immediate_format(address, Some(format));
        return Ok(());
    }

    let is_low = matches!(normalized_fmt.as_str(), "low_byte" | "low" | "lo");
    let raw_target_arg = target_address.ok_or_else(|| {
        make_mcp_error(
            -32602,
            if is_low {
                "Missing 'target_address' parameter for low_byte format"
            } else {
                "Missing 'target_address' parameter for high_byte format"
            },
        )
    })?;

    let extract_byte = |addr: Addr| -> u8 {
        if is_low {
            (addr.0 & 0xFF) as u8
        } else {
            (addr.0 >> 8) as u8
        }
    };

    match offset {
        None | Some(0) => {
            if offset == Some(0) {
                let imm_byte = extract_immediate_operand_byte(app_state, address)?;
                let expected = extract_byte(raw_target_arg);
                if expected != imm_byte {
                    return Err(make_mcp_error(
                        -32602,
                        format!(
                            "Immediate byte #${imm_byte:02X} at ${address:04X} does not match {} byte (${expected:02X}) of ${raw_target_arg:04X}",
                            if is_low { "low" } else { "high" }
                        ),
                    ));
                }
            }
            let fmt = if is_low {
                ImmediateFormat::LowByte(raw_target_arg)
            } else {
                ImmediateFormat::HighByte(raw_target_arg)
            };
            app_state.apply_immediate_format_with_offset(address, Some(fmt), None);
            Ok(())
        }
        Some(non_zero_offset) => {
            let imm_byte = extract_immediate_operand_byte(app_state, address)?;

            // Candidate A: `raw_target_arg` is `sym.base` and `raw_target_arg + offset` is effective.
            let cand_a = {
                let eff_i32 = i32::from(raw_target_arg.0) + i32::from(non_zero_offset);
                u16::try_from(eff_i32)
                    .ok()
                    .map(|eff| (raw_target_arg, Addr(eff)))
                    .filter(|&(_, eff)| extract_byte(eff) == imm_byte)
            };

            // Candidate B: `raw_target_arg` is effective and `raw_target_arg - offset` is `sym.base`.
            let cand_b = {
                let base_i32 = i32::from(raw_target_arg.0) - i32::from(non_zero_offset);
                u16::try_from(base_i32)
                    .ok()
                    .map(|base| (Addr(base), raw_target_arg))
                    .filter(|&(_, eff)| extract_byte(eff) == imm_byte)
            };

            let (resolved_base, resolved_effective) = match (cand_a, cand_b) {
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (Some(a), Some(b)) => {
                    // Both interpretations share the same extracted byte (for example, `high_byte`
                    // `#>$C1` with `target_address = $C105, offset = -1` matches both `$C105 - 1 = $C104`
                    // and `$C106 - 1 = $C105`). Prefer the candidate whose `base` carries the
                    // higher-priority symbol (`User` < `System` < `Auto` < unlabelled), defaulting
                    // to Candidate A (`target_address` = `sym.base`) on ties.
                    let rank_a = immediate_base_symbol_rank(app_state, a.0);
                    let rank_b = immediate_base_symbol_rank(app_state, b.0);
                    if rank_b < rank_a { b } else { a }
                }
                (None, None) => {
                    return Err(make_mcp_error(
                        -32602,
                        format!(
                            "Immediate byte #${imm_byte:02X} at ${address:04X} matches neither ${raw_target_arg:04X} {:+} nor ${raw_target_arg:04X} ({} byte)",
                            non_zero_offset,
                            if is_low { "low" } else { "high" }
                        ),
                    ));
                }
            };

            let fmt = if is_low {
                ImmediateFormat::LowByte(resolved_effective)
            } else {
                ImmediateFormat::HighByte(resolved_effective)
            };
            let sym = SymbolicOffset {
                base: resolved_base,
                offset: non_zero_offset,
                radix,
            };
            app_state.apply_immediate_format_with_offset(address, Some(fmt), Some(sym));
            Ok(())
        }
    }
}

/// Extracts parameters for `r2000_create_project_enum` and creates a project enum.
///
/// # Errors
///
/// Returns [`McpError`] if `"name"` or `"variants"` is missing or invalid.
fn create_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let description = args.get("description").and_then(|v| v.as_str());
    let variants = args
        .get("variants")
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'variants'"))?;

    let trimmed_name = enum_name.trim();
    create_project_enum_impl(app_state, trimmed_name, description, variants)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{trimmed_name}' created successfully.") }] }),
    )
}

/// Creates a new project-specific [`EnumDefinition`] in `app_state.enums`.
///
/// # Errors
///
/// Returns [`McpError`] if `name` fails validation or if `variants_val` is not a string map.
pub(crate) fn create_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    description: Option<&str>,
    variants_val: &Value,
) -> Result<(), McpError> {
    let name = name.trim();
    if let Err(err_msg) = app_state.validate_new_enum_name(name) {
        return Err(make_mcp_error(
            -32602,
            format!("Invalid enum name: {err_msg}"),
        ));
    }

    let raw_map: BTreeMap<String, String> = serde_json::from_value(variants_val.clone())
        .map_err(|e| make_mcp_error(-32602, format!("Invalid variants parameter: {e}")))?;

    let parsed_variants = EnumDefinition::parse_variants(raw_map);

    let definition = EnumDefinition {
        name: name.to_string(),
        description: description.map(String::from),
        variants: parsed_variants,
        source_file: None,
    };

    let command = Command::SetEnumDefinition {
        name: name.to_string(),
        new_definition: Some(definition),
        old_definition: None,
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

/// Extracts parameters for `r2000_update_project_enum` and updates a project enum.
///
/// # Errors
///
/// Returns [`McpError`] if `"name"` is missing or if the enum update fails.
fn update_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str());
    let description = args.get("description").and_then(|v| v.as_str());
    let variants = args.get("variants");

    let trimmed_name = enum_name.trim();
    update_project_enum_impl(app_state, trimmed_name, new_name, description, variants)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{trimmed_name}' updated successfully.") }] }),
    )
}

/// Updates or renames an existing project [`EnumDefinition`] and rewrites matching `enum_usage` annotations.
///
/// # Errors
///
/// Returns [`McpError`] if `name` does not exist in `app_state.enums` or if `new_name` is invalid.
pub(crate) fn update_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    new_name: Option<&str>,
    description: Option<&str>,
    variants_val: Option<&Value>,
) -> Result<(), McpError> {
    let name = name.trim();
    let new_name = new_name.map(str::trim).filter(|s| !s.is_empty());

    let old_definition = app_state.enums.get(name).cloned().ok_or_else(|| {
        make_mcp_error(
            -32602,
            format!("Enum '{name}' not found in project-specific enums"),
        )
    })?;

    if let Some(n) = new_name
        && n != name
        && let Err(err_msg) = app_state.validate_new_enum_name(n)
    {
        return Err(make_mcp_error(
            -32602,
            format!("Invalid new enum name: {err_msg}"),
        ));
    }

    let variants = if let Some(v_val) = variants_val {
        let raw_map: BTreeMap<String, String> = serde_json::from_value(v_val.clone())
            .map_err(|e| make_mcp_error(-32602, format!("Invalid variants parameter: {e}")))?;
        EnumDefinition::parse_variants(raw_map)
    } else {
        old_definition.variants.clone()
    };

    let desc = if let Some(d) = description {
        if d.trim().is_empty() {
            None
        } else {
            Some(d.to_string())
        }
    } else {
        old_definition.description.clone()
    };

    let updated_def = EnumDefinition {
        name: new_name.unwrap_or(name).to_string(),
        description: desc,
        variants,
        source_file: old_definition.source_file.clone(),
    };

    let cmd = if let Some(n) = new_name
        && n != name
    {
        let mut batch_cmds = Vec::new();
        for (addr, entry) in app_state.annotations.iter() {
            if entry.enum_usage.as_deref() == Some(name) {
                batch_cmds.push(Command::SetEnumUsage {
                    address: addr,
                    new_enum: Some(n.to_string()),
                    old_enum: Some(name.to_string()),
                });
            }
        }
        batch_cmds.push(Command::SetEnumDefinition {
            name: name.to_string(),
            new_definition: None,
            old_definition: Some(old_definition),
        });
        batch_cmds.push(Command::SetEnumDefinition {
            name: n.to_string(),
            new_definition: Some(updated_def),
            old_definition: None,
        });
        Command::Batch(batch_cmds)
    } else {
        Command::SetEnumDefinition {
            name: name.to_string(),
            new_definition: Some(updated_def),
            old_definition: Some(old_definition),
        }
    };

    cmd.apply(app_state);
    app_state.push_command(cmd);
    app_state.disassemble();

    Ok(())
}

/// Extracts parameters for `r2000_delete_project_enum` and deletes a project enum.
///
/// # Errors
///
/// Returns [`McpError`] if `"name"` is missing or if enum deletion fails.
fn delete_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let trimmed_name = enum_name.trim();
    delete_project_enum_impl(app_state, trimmed_name, force)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{trimmed_name}' deleted successfully.") }] }),
    )
}

/// Deletes a project [`EnumDefinition`] and clears any active `enum_usage` annotations when `force` is `true`.
///
/// # Errors
///
/// Returns [`McpError`] if `name` is not found in `app_state.enums` or if `force` is `false`
/// while the enum is referenced in `app_state.annotations`.
pub(crate) fn delete_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    force: bool,
) -> Result<(), McpError> {
    let name = name.trim();
    let old_definition = app_state.enums.get(name).cloned().ok_or_else(|| {
        make_mcp_error(-32602, format!("Enum '{name}' not found in project enums"))
    })?;

    let usages: Vec<Addr> = app_state
        .annotations
        .iter()
        .filter(|(_, entry)| entry.enum_usage.as_deref() == Some(name))
        .map(|(addr, _)| addr)
        .collect();

    if !force && !usages.is_empty() {
        let addresses: Vec<String> = usages.iter().map(|a| format!("${a:04X}")).collect();
        return Err(make_mcp_error(
            -32602,
            format!(
                "Cannot delete enum '{name}' because it is currently in use at addresses: {}. Set 'force' to true to override.",
                addresses.join(", ")
            ),
        ));
    }

    let delete_def_cmd = Command::SetEnumDefinition {
        name: name.to_string(),
        new_definition: None,
        old_definition: Some(old_definition),
    };

    let command = if usages.is_empty() {
        delete_def_cmd
    } else {
        let mut batch_cmds: Vec<Command> = usages
            .into_iter()
            .map(|addr| Command::SetEnumUsage {
                address: addr,
                new_enum: None,
                old_enum: Some(name.to_string()),
            })
            .collect();
        batch_cmds.push(delete_def_cmd);
        Command::Batch(batch_cmds)
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

/// Applies or clears an enum usage annotation at `args["address"]`.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"` is invalid, if `"name"` is not a string,
/// or if the requested enum does not exist.
fn apply_enum_usage_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let name_opt = match args.get("name") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Some(_) => {
            return Err(make_mcp_error(-32602, "Invalid 'name' (expected a string)"));
        }
    };

    if let Some(ref enum_name) = name_opt
        && app_state.get_enum(enum_name).is_none()
    {
        return Err(make_mcp_error(
            -32602,
            format!("Enum '{enum_name}' not found in project or system enums"),
        ));
    }

    let command = Command::SetEnumUsage {
        address,
        new_enum: name_opt.clone(),
        old_enum: app_state
            .annotations
            .get(address)
            .and_then(|e| e.enum_usage.clone()),
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": if let Some(n) = name_opt {
                format!("Enum '{n}' applied successfully at ${address:04X}.")
            } else {
                format!("Enum usage cleared successfully at ${address:04X}.")
            }
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::make_app_state;
    use crate::state::{Label, LabelKind, LabelType};

    #[test]
    fn test_r2000_set_label_name_applies_command() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({ "address": 0x1000, "name": "main_entry" });
        let res = set_label_name_impl(&mut app_state, &args).expect("Label set should succeed");
        assert!(
            res["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Label set at $1000")
        );
        assert!(app_state.undo_stack.can_undo());
        assert_eq!(
            app_state.labels.get(&Addr(0x1000)).unwrap()[0].name,
            "main_entry"
        );
    }

    #[test]
    fn test_r2000_set_label_name_remove() {
        let mut app_state = make_app_state(0x1000, 256);
        let args_add = json!({ "address": 0x1000, "name": "main_entry" });
        set_label_name_impl(&mut app_state, &args_add).unwrap();

        let args_rem = json!({ "address": 0x1000, "name": "" });
        set_label_name_impl(&mut app_state, &args_rem).unwrap();
        assert!(
            app_state
                .labels
                .get(&Addr(0x1000))
                .is_none_or(|l| l.is_empty())
        );
    }

    #[test]
    fn test_r2000_set_comment_line_and_side() {
        let mut app_state = make_app_state(0x1000, 256);
        let args_line = json!({ "address": 0x1000, "comment": "Header comment", "type": "line" });
        set_comment_impl(&mut app_state, &args_line).unwrap();

        let args_side = json!({ "address": 0x1000, "comment": "Inline comment", "type": "side" });
        set_comment_impl(&mut app_state, &args_side).unwrap();

        let annotation = app_state.annotations.get(Addr(0x1000)).unwrap();
        assert_eq!(
            annotation.user_line_comment.as_deref(),
            Some("Header comment")
        );
        assert_eq!(
            annotation.user_side_comment.as_deref(),
            Some("Inline comment")
        );
    }

    #[test]
    fn test_r2000_set_comment_remove() {
        let mut app_state = make_app_state(0x1000, 256);

        // 1. Remove side comment via empty string ""
        set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "comment": "Inline comment", "type": "side" }),
        )
        .unwrap();
        let res_empty = set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "comment": "", "type": "side" }),
        )
        .unwrap();
        assert!(
            res_empty["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Comment removed at $1000")
        );
        assert!(app_state.annotations.get(Addr(0x1000)).is_none());
        assert_eq!(app_state.disassembly[0].comment, "");

        // 2. Remove side comment via whitespace-only string "   "
        set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "comment": "Inline comment", "type": "side" }),
        )
        .unwrap();
        let res_ws = set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "comment": "   ", "type": "side" }),
        )
        .unwrap();
        assert!(
            res_ws["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Comment removed at $1000")
        );
        assert!(app_state.annotations.get(Addr(0x1000)).is_none());
        assert_eq!(app_state.disassembly[0].comment, "");

        // 3. Remove line comment by omitting 'comment'
        set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "comment": "Header comment", "type": "line" }),
        )
        .unwrap();
        let res_omit = set_comment_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "type": "line" }),
        )
        .unwrap();
        assert!(
            res_omit["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Comment removed at $1000")
        );
        assert!(app_state.annotations.get(Addr(0x1000)).is_none());
        assert_eq!(app_state.disassembly[0].line_comment, None);

        // 4. Reject legacy 'text' parameter and non-string 'comment'
        assert!(
            set_comment_impl(
                &mut app_state,
                &json!({ "address": 0x1000, "text": "hello", "type": "side" })
            )
            .is_err()
        );
        assert!(
            set_comment_impl(
                &mut app_state,
                &json!({ "address": 0x1000, "comment": 123, "type": "side" })
            )
            .is_err()
        );
    }

    #[test]
    fn test_r2000_set_data_type_range() {
        let mut app_state = make_app_state(0x2000, 256);
        let args = json!({
            "start_address": 0x2000,
            "end_address": 0x200F,
            "data_type": "byte"
        });
        set_data_type_impl(&mut app_state, &args).expect("Data type set should succeed");
        for addr_idx in 0..16 {
            assert_eq!(app_state.block_types[addr_idx], BlockType::DataByte);
        }
    }

    #[test]
    fn test_r2000_disassemble_in_modification_tools() {
        let mut app_state = make_app_state(0x1000, 256);
        // NOP ($EA), RTS ($60) at $1000..$1002
        app_state.raw_data[0] = 0xEA;
        app_state.raw_data[1] = 0x60;
        app_state.block_types[0] = BlockType::Undefined;
        app_state.block_types[1] = BlockType::Undefined;
        app_state.labels.insert(
            Addr(0x1050),
            vec![Label {
                name: "a1050".to_string(),
                label_type: LabelType::AbsoluteAddress,
                kind: LabelKind::Auto,
            }],
        );

        let args = json!({ "address": 0x1000 });
        let res = handle_call("r2000_disassemble", &args, &mut app_state)
            .expect("Tool call should match")
            .expect("Disassemble should succeed");
        assert!(
            res["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Disassembled successfully")
        );
        assert_eq!(app_state.block_types[0], BlockType::Code);
        assert!(!app_state.labels.contains_key(&Addr(0x1050)));

        // Undoing r2000_disassemble must restore both Undefined block types and pre-existing auto labels.
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;
        assert_eq!(app_state.block_types[0], BlockType::Undefined);
        assert!(app_state.labels.contains_key(&Addr(0x1050)));

        // Test out of bounds address
        let invalid_args = json!({ "address": 0x3000 });
        let err = handle_call("r2000_disassemble", &invalid_args, &mut app_state)
            .expect("Tool call should match")
            .expect_err("Out of bounds address should fail");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("out of bounds"));
    }

    #[test]
    fn test_r2000_set_immediate_format_validates_target_address() {
        let mut app_state = make_app_state(0x1000, 16);
        let err = set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1000,
                "format": "low_byte",
                "target_address": "$ZZZZ"
            }),
        )
        .expect_err("invalid target_address must report invalid format error");
        assert!(
            err.message
                .contains("Invalid address format for 'target_address'")
        );
    }

    #[test]
    fn test_project_enum_rename_and_force_delete_keep_enum_usage_consistent() {
        let mut app_state = make_app_state(0x1000, 16);
        create_project_enum_mcp(
            &mut app_state,
            &json!({
                "name": "game_state",
                "variants": { "0": "INIT", "1": "RUN" }
            }),
        )
        .expect("create enum should succeed");

        apply_enum_usage_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "name": "game_state" }),
        )
        .expect("apply existing enum should succeed");

        // Reject applying an unknown enum name.
        assert!(
            apply_enum_usage_impl(
                &mut app_state,
                &json!({ "address": 0x1000, "name": "nonexistent_enum" }),
            )
            .is_err()
        );

        // Renaming the enum must update the annotation at $1000.
        update_project_enum_mcp(
            &mut app_state,
            &json!({ "name": "game_state", "new_name": "mode_state" }),
        )
        .expect("rename enum should succeed");
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.enum_usage.as_deref()),
            Some("mode_state")
        );

        // Force-deleting the enum must clear the dangling `enum_usage` annotation.
        delete_project_enum_mcp(
            &mut app_state,
            &json!({ "name": "mode_state", "force": true }),
        )
        .expect("force delete enum should succeed");
        assert!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.enum_usage.as_deref())
                .is_none()
        );
    }

    #[test]
    fn test_r2000_set_symbolic_offset_single_range_negative_i64_and_clear() {
        let mut app_state = make_app_state(0x1000, 0x41);
        // $1000: STA $FC (85 FC)
        app_state.raw_data[0x00] = 0x85;
        app_state.raw_data[0x01] = 0xFC;
        // $1002: LDA $102F,X (BD 2F 10)
        app_state.raw_data[0x02] = 0xBD;
        app_state.raw_data[0x03] = 0x2F;
        app_state.raw_data[0x04] = 0x10;
        // $1010..=$1013: 2-pair LoHiAddress RTS table ($102F, $1034)
        app_state.raw_data[0x10] = 0x2F;
        app_state.raw_data[0x11] = 0x34;
        app_state.raw_data[0x12] = 0x10;
        app_state.raw_data[0x13] = 0x10;
        for b in &mut app_state.block_types[0x10..=0x13] {
            *b = BlockType::LoHiAddress;
        }
        // $1020..=$1023: struct region
        for b in &mut app_state.block_types[0x20..=0x23] {
            *b = BlockType::DataByte;
        }
        // $1030 & $1035: subroutines
        app_state.raw_data[0x30] = 0x60;
        app_state.raw_data[0x35] = 0x60;

        for (addr, name) in [
            (Addr(0x00FB), "zp_ptr"),
            (Addr(0x1020), "player"),
            (Addr(0x1030), "sub_a"),
            (Addr(0x1035), "sub_b"),
        ] {
            app_state.labels.insert(
                addr,
                vec![Label {
                    name: name.to_string(),
                    label_type: LabelType::UserDefined,
                    kind: LabelKind::User,
                }],
            );
        }
        app_state.perform_analysis();

        // Verify `tools/list` deserializes cleanly into `rmcp::model::ListToolsResult` (HTTP transport).
        let mut view_state = crate::mcp::tools::common::make_view_state();
        let list_resp = crate::mcp::handler::handle_method(
            "tools/list",
            &Value::Null,
            &mut app_state,
            &mut view_state,
        );
        serde_json::from_value::<rmcp::model::ListToolsResult>(
            list_resp.result.expect("tools/list result"),
        )
        .expect("updated MCP schemas must deserialize into rmcp::model::ListToolsResult");

        // 1. External `scope: "auto"` with integer `base` (251 = $00FB) sets Tier 2 `address_offset`.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x00FC, "base": 251 }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("external auto Tier 2 offset should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset),
            Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );
        let line_1000 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("STA $FC line");
        assert_eq!(line_1000.operand, "zp_ptr + 1");

        // 2. Single instruction negative `i64` offset sets Tier 1 `operand_offset` (`sub_a - 1`).
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x1002, "offset": -1 }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("negative i64 operand offset should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(SymbolicOffset {
                base: Addr(0x1030),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        let line_1002 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .expect("LDA $102F,X line");
        assert_eq!(line_1002.operand, "sub_a - 1,x");

        // 3. Split table range sets Tier 1 `operand_offset` on canonical pair addresses $1010 and $1011 only.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "start_address": "$1010", "end_address": "$1013", "offset": -1 }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("split table range offset should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1010))
                .and_then(|e| e.operand_offset),
            Some(SymbolicOffset {
                base: Addr(0x1030),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1011))
                .and_then(|e| e.operand_offset),
            Some(SymbolicOffset {
                base: Addr(0x1035),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x1012))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x1013))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        let line_1010 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1010) && !l.bytes.is_empty())
            .expect("LoHi low row");
        assert_eq!(line_1010.operand, "<(sub_a - 1), <(sub_b - 1)");

        // 4. Struct range skipping `addr == base` keeps `"player"` at $1020 and sets `player + $1..$3`.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1020,
                "end_address": 0x1023,
                "base": "player",
                "scope": "address",
                "radix": "hex"
            }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("struct range should succeed");

        assert!(
            app_state
                .annotations
                .get(Addr(0x1020))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        assert_eq!(
            app_state.labels.get(&Addr(0x1020)).unwrap()[0].name,
            "player"
        );
        for disp in 1i16..=3 {
            let field_addr = Addr(0x1020 + disp as u16);
            assert_eq!(
                app_state
                    .annotations
                    .get(field_addr)
                    .and_then(|e| e.address_offset),
                Some(SymbolicOffset {
                    base: Addr(0x1020),
                    offset: disp,
                    radix: OffsetRadix::Hex,
                })
            );
        }

        // 5. Clearing via `offset: 0` (Tier 1) and `expression: ""` (Tier 2), plus Undo/Redo roundtrip.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x1002, "offset": 0 }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("clear Tier 1 should succeed");
        assert!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );

        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x00FC, "expression": "" }),
            &mut app_state,
        )
        .expect("tool matched")
        .expect("clear Tier 2 should succeed");
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );

        app_state.undo_last_command();
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| s.offset),
            Some(1)
        );
        app_state.undo_last_command();
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .map(|s| s.offset),
            Some(-1)
        );

        app_state.redo_last_command();
        app_state.redo_last_command();
        assert!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
    }

    #[test]
    fn test_r2000_set_symbolic_offset_validation_errors() {
        let mut app_state = make_app_state(0x1000, 16);
        // $1000: LDA $FF,X (B5 FF, 2-byte ZeroPageX)
        app_state.raw_data[0] = 0xB5;
        app_state.raw_data[1] = 0xFF;
        app_state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                label_type: LabelType::UserDefined,
                kind: LabelKind::User,
            }],
        );
        app_state.disassemble();

        let invalid_cases = [
            // (a) neither `address` nor `start_address`/`end_address`
            json!({ "offset": 1 }),
            // (b) both `address` and `start_address`/`end_address`
            json!({ "address": 0x1000, "start_address": 0x1000, "end_address": 0x1002, "offset": 1 }),
            json!({ "address": 0x1000, "start_address": 0x1000, "offset": 1 }),
            // (c) missing `expression`/`base`/`offset` or conflicting `expression` + `offset`
            json!({ "address": 0x00FC }),
            json!({ "address": 0x00FC, "expression": "zp_ptr + 1", "offset": 1 }),
            // (d) `i16` overflow (`offset: 100000` and `-100000`)
            json!({ "address": 0x00FC, "offset": 100_000 }),
            json!({ "address": 0x00FC, "offset": -100_000 }),
            // (e) 16-bit non-wrapping wraparound (`$0000 - 1` at `$FFFF`, `$FFFF + 1` at `$0000`)
            json!({ "address": 0xFFFF, "expression": "$0000 - 1", "scope": "address" }),
            json!({ "address": 0x0000, "expression": "$FFFF + 1", "scope": "address" }),
            // (f) expression mismatch (`zp_ptr + 5` evaluates to `$0100`, expected `$00FC`)
            json!({ "address": 0x00FC, "expression": "zp_ptr + 5" }),
            // (g) 2-byte ZP instruction `LDA $FF,X` (`B5 FF`) with `base: 0x0100` (`> $00FF`)
            json!({ "address": 0x1000, "base": 0x0100 }),
            // (h) invalid `scope`, `radix`, `base` bounds/types, non-integer `offset`, and `operand` bounds
            json!({ "address": 0x00FC, "offset": 1, "scope": "global" }),
            json!({ "address": 0x00FC, "offset": 1, "scope": 123 }),
            json!({ "address": 0x00FC, "offset": 1, "radix": "binary" }),
            json!({ "address": 0x00FC, "offset": 1, "radix": 16 }),
            json!({ "address": 0x00FC, "base": -1 }),
            json!({ "address": 0x00FC, "base": 70_000 }),
            json!({ "address": 0x00FC, "base": "   " }),
            json!({ "address": 0x00FC, "base": "zp_ptr + 1" }),
            json!({ "address": 0x00FC, "base": true }),
            json!({ "address": 0x00FC, "offset": "bad" }),
            json!({ "address": 0x00FC, "offset": 1, "scope": "operand" }),
            json!({ "address": 0x1005, "offset": 1, "scope": "operand" }),
        ];

        for bad_args in invalid_cases {
            let err = handle_call("r2000_set_symbolic_offset", &bad_args, &mut app_state)
                .expect("tool matched")
                .expect_err(&format!("expected validation failure for {bad_args}"));
            assert_eq!(err.code, -32602, "unexpected code for {bad_args}: {err:?}");
        }
        assert!(app_state.annotations.is_empty());
    }

    #[test]
    fn test_r2000_set_label_name_symbolic_expression_routing_and_mutual_exclusion() {
        let mut app_state = make_app_state(0x1000, 16);
        // $1000: STA $FC (85 FC)
        app_state.raw_data[0] = 0x85;
        app_state.raw_data[1] = 0xFC;
        app_state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                label_type: LabelType::UserDefined,
                kind: LabelKind::User,
            }],
        );
        app_state.perform_analysis();

        // 1. Set plain User label `"temp_fc"` at `$00FC`.
        set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x00FC, "name": "temp_fc" }),
        )
        .expect("set temp_fc");
        assert_eq!(
            app_state.labels.get(&Addr(0x00FC)).unwrap()[0].name,
            "temp_fc"
        );

        // 2. `"zp_ptr + 1"` removes `"temp_fc"` (`User`) and sets Tier 2 `address_offset`.
        set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x00FC, "name": "zp_ptr + 1" }),
        )
        .expect("set zp_ptr + 1");
        assert!(!app_state.labels.contains_key(&Addr(0x00FC)));
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset),
            Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );

        // 3. Invalid `"zp_ptr + 99"` returns `Err(McpError)` and leaves `zp_ptr + 1` untouched.
        let bad_err = set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x00FC, "name": "zp_ptr + 99" }),
        )
        .expect_err("mismatched expression must fail");
        assert_eq!(bad_err.code, -32602);
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| s.offset),
            Some(1)
        );

        // 4. `"plain_fc"` clears `address_offset` and sets `User` label `"plain_fc"`.
        set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x00FC, "name": "plain_fc" }),
        )
        .expect("set plain_fc");
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        assert_eq!(
            app_state.labels.get(&Addr(0x00FC)).unwrap()[0].name,
            "plain_fc"
        );

        // 5. Setting `"zp_ptr + 1"` again and then `""` clears `address_offset` too.
        set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x00FC, "name": "zp_ptr + 1" }),
        )
        .expect("re-set zp_ptr + 1");
        set_label_name_impl(&mut app_state, &json!({ "address": 0x00FC, "name": "" }))
            .expect("clear empty string");
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
    }

    #[test]
    fn test_r2000_set_immediate_format_with_offset_base_or_effective_and_stale_cleanup() {
        let imm_schema = tool_schemas()
            .into_iter()
            .find(|s| s["name"] == "r2000_set_immediate_format")
            .expect("r2000_set_immediate_format schema");
        assert!(
            imm_schema["inputSchema"]["properties"]["radix"].is_object(),
            "r2000_set_immediate_format schema must expose optional 'radix' property"
        );

        let mut app_state = make_app_state(0x1000, 16);
        // $1000: LDA #$FF, $1002: LDY #$C0, $1004: LDX #$C1, $1006: LDA #$FF, $1008: RTS
        app_state.raw_data[0..9]
            .copy_from_slice(&[0xA9, 0xFF, 0xA0, 0xC0, 0xA2, 0xC1, 0xA9, 0xFF, 0x60]);
        app_state.load_system_assets();
        app_state.insert_typed_user_labels(&[
            (0xC100, "sub_C100", LabelType::Subroutine),
            (0xC105, "sub_C105", LabelType::Subroutine),
        ]);
        app_state.perform_analysis();

        // 1. Base address form (`target_address: 0xC100, offset: -1`) -> `LowByte($C0FF)` + `sub_C100 - 1`.
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1000,
                "format_type": "low_byte",
                "target_address": 0xC100,
                "offset": -1
            }),
        )
        .expect("base address low_byte with offset -1 should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(ImmediateFormat::LowByte(Addr(0xC0FF)))
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        let line_1000 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("LDA #$FF line");
        assert_eq!(line_1000.operand, "#<(sub_C100 - 1)");

        // 2. Raw effective address form (`target_address: 0xC0FF, offset: -1`) -> `HighByte($C0FF)` + `sub_C100 - 1`.
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1002,
                "format_type": "high_byte",
                "target_address": 0xC0FF,
                "offset": -1
            }),
        )
        .expect("raw effective high_byte with offset -1 should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.immediate_format),
            Some(ImmediateFormat::HighByte(Addr(0xC0FF)))
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        let line_1002 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .expect("LDY #$C0 line");
        assert_eq!(line_1002.operand, "#>(sub_C100 - 1)");

        // Same-page `high_byte` with canonical `"format"` key and `"radix": "hex"`.
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1004,
                "format": "high_byte",
                "target_address": 0xC104,
                "offset": -1,
                "radix": "hex"
            }),
        )
        .expect("same-page raw effective high_byte with hex radix should resolve sub_C105");
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1004))
                .and_then(|e| e.operand_offset)
                .map(|s| s.radix),
            Some(OffsetRadix::Hex)
        );
        let line_1004 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1004) && !l.bytes.is_empty())
            .expect("LDX #$C1 line");
        assert_eq!(line_1004.operand, "#>(sub_C105 - $1)");

        // Unreferenced `system_labels` disambiguation: `LDA #$FF` with `target_address: 0xFFD1, offset: -1`
        // ranks `$FFD2` (`KERNAL_CHROUT` in `system_labels`) above unlabelled `$FFD1`.
        assert!(!app_state.labels.contains_key(&Addr(0xFFD2)));
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1006,
                "format": "high_byte",
                "target_address": 0xFFD1,
                "offset": -1
            }),
        )
        .expect("high_byte should disambiguate unreferenced KERNAL_CHROUT in system_labels");
        let line_1006 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1006) && !l.bytes.is_empty())
            .expect("LDA #$FF line");
        assert_eq!(line_1006.operand, "#>(KERNAL_CHROUT - 1)");

        // 3. Validation errors: byte mismatch, non-zero offset on literal formats, offset: 0 mismatch, non-Immediate opcode.
        for bad_imm in [
            json!({ "address": 0x1000, "format": "low_byte", "target_address": 0xC100, "offset": -2 }),
            json!({ "address": 0x1000, "format": "hex", "offset": 1 }),
            json!({ "address": 0x1000, "format": "decimal", "offset": -1 }),
            json!({ "address": 0x1000, "format": "binary", "offset": 1 }),
            json!({ "address": 0x1000, "format": "low_byte", "target_address": 0xC100, "offset": 0 }),
            json!({ "address": 0x1008, "format": "low_byte", "target_address": 0xC100, "offset": -1 }),
        ] {
            let err = set_immediate_format_mcp_impl(&mut app_state, &bad_imm).expect_err(&format!(
                "expected immediate validation error for {bad_imm}"
            ));
            assert_eq!(err.code, -32602);
        }

        // 4. Stale cleanup & Undo: switching to `"hex"` clears stale `operand_offset` in one undoable batch.
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "format_type": "hex" }),
        )
        .expect("switching to hex should succeed");
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(ImmediateFormat::Hex)
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );

        app_state.undo_last_command();
        let restored_1000 = app_state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("restored LDA #$FF line");
        assert_eq!(restored_1000.operand, "#<(sub_C100 - 1)");

        // Omitting `offset` on `high_byte` also clears stale `operand_offset` at $1002.
        set_immediate_format_mcp_impl(
            &mut app_state,
            &json!({
                "address": 0x1002,
                "format": "high_byte",
                "target_address": 0xC0FF
            }),
        )
        .expect("high_byte without offset should clear stale operand_offset");
        assert!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
    }

    #[test]
    fn test_r2000_set_label_name_rejects_corrupt_plain_label_syntax() {
        let mut app_state = make_app_state(0x1000, 16);

        for bad_name in [
            "(foo)",
            "foo:bar",
            "a..b",
            "123",
            "foo!bar",
            "my label",
            "$0400",
            ".dot_global",
            "@at_global",
        ] {
            let err = set_label_name_impl(
                &mut app_state,
                &json!({ "address": 0x1000, "name": bad_name }),
            )
            .expect_err(&format!(
                "corrupt plain label '{bad_name}' must be rejected"
            ));
            assert_eq!(err.code, -32602);
            assert!(
                !app_state.labels.contains_key(&Addr(0x1000)),
                "rejected label '{bad_name}' must not be inserted"
            );
        }

        // Valid identifiers and empty string clear continue to work cleanly
        set_label_name_impl(
            &mut app_state,
            &json!({ "address": 0x1000, "name": "_valid_ident_1" }),
        )
        .expect("valid identifier should succeed");
        assert_eq!(
            app_state.labels.get(&Addr(0x1000)).unwrap()[0].name,
            "_valid_ident_1"
        );

        set_label_name_impl(&mut app_state, &json!({ "address": 0x1000, "name": "" }))
            .expect("empty string clear should succeed");
        assert!(!app_state.labels.contains_key(&Addr(0x1000)));
    }

    #[test]
    fn test_r2000_set_symbolic_offset_nested_parens_clear_and_unbalanced_rejection() {
        let mut app_state = make_app_state(0x1000, 0x40);
        // $1010..=$1013: 2-pair LoHiAddress table ($102F, $1034)
        app_state.raw_data[0x10..0x14].copy_from_slice(&[0x2F, 0x34, 0x10, 0x10]);
        app_state.block_types[0x10..0x14].fill(BlockType::LoHiAddress);
        // $1020..=$1023: 4-byte struct (`player` at $1020)
        app_state.block_types[0x20..0x24].fill(BlockType::DataByte);
        app_state.insert_user_labels(&[(0x1020, "player")]);
        app_state.perform_analysis();

        // Populate both the LoHiAddress range (Tier 1) and the struct range (Tier 2)
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "start_address": 0x1010, "end_address": 0x1013, "offset": -1 }),
            &mut app_state,
        )
        .unwrap()
        .unwrap();
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1020,
                "end_address": 0x1023,
                "base": "player",
                "scope": "address"
            }),
            &mut app_state,
        )
        .unwrap()
        .unwrap();

        assert!(
            app_state
                .annotations
                .get(Addr(0x1010))
                .and_then(|e| e.operand_offset)
                .is_some()
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x1022))
                .and_then(|e| e.address_offset)
                .is_some()
        );

        // Unbalanced parentheses (`"(0"`, `"0)"`, `"(0) + (0)"`) MUST fail with -32602 rather than
        // masquerading as clear tokens or silently skipping addresses!
        for bad_expr in ["(0", "0)", "(0) + (0)"] {
            let err = handle_call(
                "r2000_set_symbolic_offset",
                &json!({
                    "start_address": 0x1020,
                    "end_address": 0x1023,
                    "expression": bad_expr,
                    "scope": "address"
                }),
                &mut app_state,
            )
            .unwrap()
            .expect_err(&format!("unbalanced '{bad_expr}' must return error"));
            assert_eq!(err.code, -32602);
        }
        assert!(
            app_state
                .annotations
                .get(Addr(0x1022))
                .and_then(|e| e.address_offset)
                .is_some(),
            "Rejected unbalanced expression must leave existing offsets intact"
        );

        // Nested balanced parentheses `"( (0) )"` MUST clear every entry in multi-address ranges!
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1020,
                "end_address": 0x1023,
                "expression": "( (0) )",
                "scope": "address"
            }),
            &mut app_state,
        )
        .unwrap()
        .expect("nested balanced '( (0) )' must clear multi-address Tier 2 range");

        for addr in 0x1020u16..=0x1023u16 {
            assert!(
                app_state
                    .annotations
                    .get(Addr(addr))
                    .and_then(|e| e.address_offset)
                    .is_none(),
                "Expected $102x address ${addr:04X} to be cleared by '( (0) )'"
            );
        }

        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1010,
                "end_address": 0x1013,
                "expression": "( (0) )"
            }),
            &mut app_state,
        )
        .unwrap()
        .expect("nested balanced '( (0) )' must clear multi-entry Tier 1 range");

        for addr in 0x1010u16..=0x1013u16 {
            assert!(
                app_state
                    .annotations
                    .get(Addr(addr))
                    .and_then(|e| e.operand_offset)
                    .is_none(),
                "Expected Tier 1 site ${addr:04X} to be cleared by '( (0) )'"
            );
        }
    }

    #[test]
    fn test_performance_64k_mcp_set_symbolic_offset_large_split_table_and_struct_ranges() {
        let mut app_state = AppState::build_64k_symbolic_offset_fixture();
        app_state.settings.auto_analyze = false;
        for pair_idx in 0..256u16 {
            let base_addr = 0x8000u16 + pair_idx * 16;
            app_state.annotations.update(Addr(0x4100 + pair_idx), |e| {
                e.operand_offset = Some(SymbolicOffset {
                    base: Addr(base_addr),
                    offset: -1,
                    radix: OffsetRadix::Decimal,
                });
            });
        }

        // 1. 256-pair split table Tier 1 range update (also clearing all 256 second-half entries in O(N))
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x4000,
                "end_address": 0x41FF,
                "offset": -1
            }),
            &mut app_state,
        )
        .unwrap()
        .unwrap();

        // 2. 512-byte Tier 2 struct range update resolving `"big_struct"` via `SymbolicBaseCache`
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x6000,
                "end_address": 0x61FF,
                "base": "big_struct",
                "scope": "address"
            }),
            &mut app_state,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x40FF))
                .and_then(|e| e.operand_offset)
                .map(|s| s.offset),
            Some(-1)
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x41FF))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x61FF))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x6000), 511))
        );
    }

    #[test]
    fn test_r2000_set_symbolic_offset_scope_address_on_instruction_and_multi_site_clear() {
        let mut app_state = make_app_state(0x1000, 16);
        // $1000: STA $FC (85 FC), $1002: LDA $FD (A5 FD), $1004: LDA $00FF,X (BD FF 00), $1007: RTS (60)
        app_state.raw_data[0..8].copy_from_slice(&[0x85, 0xFC, 0xA5, 0xFD, 0xBD, 0xFF, 0x00, 0x60]);
        app_state.block_types[0..8].fill(BlockType::Code);
        app_state.insert_typed_user_labels(&[
            (0x1000, "main", LabelType::Subroutine),
            (0x00FB, "zp_ptr", LabelType::ZeroPagePointer),
        ]);
        app_state.perform_analysis();

        // Seed shadowing Tier 1 `operand_offset`s at `$1000` and `$1002`.
        app_state.apply_operand_offsets(&[
            (
                Addr(0x1000),
                Some(SymbolicOffset {
                    base: Addr(0x00FA),
                    offset: 2,
                    radix: OffsetRadix::Decimal,
                }),
            ),
            (
                Addr(0x1002),
                Some(SymbolicOffset {
                    base: Addr(0x00FA),
                    offset: 3,
                    radix: OffsetRadix::Decimal,
                }),
            ),
        ]);

        // Finding 8: `scope: "address"` on instruction `$1000` (`4096`) binds `$00FC` (`zp_ptr + 1`),
        // preserves `main` at `$1000`, and clears the shadowing Tier 1 offset at `$1000`.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 4096, "offset": 1, "scope": "address" }),
            &mut app_state,
        )
        .unwrap()
        .expect("scope: address on instruction should bind target $00FC");

        assert_eq!(app_state.labels.get(&Addr(0x1000)).unwrap()[0].name, "main");
        assert!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        assert!(
            app_state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 1))
        );

        // Finding 6: multi-site `scope: "address"` across `$1000..=$1003` clears Tier 1 on `$1002` too.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1000,
                "end_address": 0x1003,
                "base": "zp_ptr",
                "scope": "address"
            }),
            &mut app_state,
        )
        .unwrap()
        .expect("multi-site scope: address range should succeed");
        assert!(
            app_state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x00FD))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 2))
        );

        // Finding 2: Tier 2 `scope: "address"` rejects `base > $00FF` on `$00FF` both directly and via `$1004`.
        let err_ext = handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x00FF, "expression": "$0100 - 1", "scope": "address" }),
            &mut app_state,
        )
        .unwrap()
        .expect_err("Tier 2 on $00FF must reject base > $00FF");
        assert_eq!(err_ext.code, -32602);

        let err_via_inst = handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 0x1004, "expression": "$0100 - 1", "scope": "address" }),
            &mut app_state,
        )
        .unwrap()
        .expect_err("Tier 2 via 3-byte $1004 targeting $00FF must reject base > $00FF");
        assert_eq!(err_via_inst.code, -32602);
    }

    #[test]
    fn test_r2000_set_symbolic_offset_zero_offset_rejection_and_multi_entry_word_skip() {
        let mut app_state = make_app_state(0x1000, 0x30);
        // $1010..=$1015: 3-entry `.word` table (`[table, table + 2, table + 4]` where `table = $1020`)
        app_state.raw_data[0x10..0x16].copy_from_slice(&[0x20, 0x10, 0x22, 0x10, 0x24, 0x10]);
        app_state.block_types[0x10..0x16].fill(BlockType::Address);
        app_state.insert_typed_user_labels(&[
            (0x00FB, "zp_ptr", LabelType::ZeroPagePointer),
            (0x1020, "table", LabelType::Field),
        ]);
        app_state.perform_analysis();

        // Seed an offset on `$00FC` (`252`) to verify clear behavior.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 252, "base": "zp_ptr" }),
            &mut app_state,
        )
        .unwrap()
        .unwrap();

        // 1. `{"address": 251, "base": "zp_ptr"}` (`base == target`) fails with `-32602`.
        let err_same = handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 251, "base": "zp_ptr" }),
            &mut app_state,
        )
        .unwrap()
        .expect_err("base == target must fail with -32602");
        assert_eq!(err_same.code, -32602);

        // 2. `{"address": 252, "base": "zp_ptr", "offset": 0}` fails upfront with `-32602`.
        let err_combo = handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 252, "base": "zp_ptr", "offset": 0 }),
            &mut app_state,
        )
        .unwrap()
        .expect_err("combining base with offset 0 must fail with -32602");
        assert_eq!(err_combo.code, -32602);
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_some()
        );

        // 3. `{"address": 252, "offset": 0}` clears cleanly.
        handle_call(
            "r2000_set_symbolic_offset",
            &json!({ "address": 252, "offset": 0 }),
            &mut app_state,
        )
        .unwrap()
        .expect("offset: 0 alone must clear cleanly");
        assert!(
            app_state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );

        // 4. Multi-entry `.word` range `[table, table + 2, table + 4]` skips entry 0 (`table`)
        //    and sets `+ 2` and `+ 4` on entries 1 (`$1012`) and 2 (`$1014`).
        let prior_entry0 = SymbolicOffset {
            base: Addr(0x101F),
            offset: 1,
            radix: OffsetRadix::Decimal,
        };
        app_state.apply_operand_offsets(&[(Addr(0x1010), Some(prior_entry0))]);

        handle_call(
            "r2000_set_symbolic_offset",
            &json!({
                "start_address": 0x1010,
                "end_address": 0x1015,
                "base": "table"
            }),
            &mut app_state,
        )
        .unwrap()
        .expect("multi-entry .word range with base: table should succeed");

        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1010))
                .and_then(|e| e.operand_offset),
            Some(prior_entry0)
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1012))
                .and_then(|e| e.operand_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x1020), 2))
        );
        assert_eq!(
            app_state
                .annotations
                .get(Addr(0x1014))
                .and_then(|e| e.operand_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x1020), 4))
        );
    }
}
