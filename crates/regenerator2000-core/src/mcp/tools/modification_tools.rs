//! MCP tool handlers for modifying assembly state, labels, comments, block types, immediate formats, and project enums.
//!
//! This module defines schemas and execution handlers that mutate project state,
//! set user labels and comments, change memory block types, configure immediate operand
//! formats, and manage project-level enum definitions.

use crate::commands::Command;
use crate::mcp::tools::common::{
    convert_region, get_address, get_optional_address, make_mcp_error,
};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{Addr, BlockType, CommentKind, EnumDefinition, ImmediateFormat};
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
            "description": "Set or remove a user-defined label at a specific MOS 6502 memory address. Use this tool to name subroutines, variables, or jump targets.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address where the label should be set (decimal, e.g. 4096 for $1000)." },
                    "name": { "type": "string", "description": "The label name (e.g. 'init_screen', 'loop_start'), or an empty string to remove an existing user label." }
                },
                "required": ["address", "name"]
            }
        }),
        json!({
            "name": "r2000_set_comment",
            "description": "Set or remove a comment at a specific address. 'line' comments appear on their own line before the instruction (multi-line supported). 'side' comments appear inline on the instruction line. Pass an empty string (or omit 'comment') to remove an existing comment.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address for the comment (decimal, e.g. 4096 for $1000)." },
                    "comment": { "type": "string", "description": "The comment text (do not include the ';' prefix), or an empty string to remove the existing comment." },
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
            "description": "Set the display format for an immediate operand at a specific address. Supports hexadecimal, decimal, binary, and low/high byte references to a 16-bit address.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The address of the instruction (decimal)." },
                    "format": {
                        "type": "string",
                        "enum": ["hex", "decimal", "binary", "low_byte", "high_byte"],
                        "description": "hex=$00, decimal=0, binary=%00000000, low_byte=#<p_ADDRESS, high_byte=#>p_ADDRESS."
                    },
                    "target_address": { "type": "integer", "description": "The 16-bit target address decimal (e.g. 60000 for $EA31). Required when format is 'low_byte' or 'high_byte'." }
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

    // Snapshot labels and cross-references BEFORE mutating `block_types` so undo restores
    // any prior auto-generated labels, and run `perform_analysis()` only once across all ranges.
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

/// Sets or removes a user label at `args["address"]`.
///
/// # Errors
///
/// Returns [`McpError`] if `"address"` or `"name"` is missing or if label validation fails.
fn set_label_name_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let label_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing or invalid 'name'"))?;

    let command = app_state
        .create_set_user_label_command(address, label_name, false)
        .map_err(|msg| make_mcp_error(-32602, msg))?;

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(json!({ "content": [{ "type": "text", "text": format!("Label set at ${address:04X}") }] }))
}

/// Sets or removes a user line or side comment at `args["address"]`.
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
/// Returns [`McpError`] if `"address"`, `"format"`, or `"target_address"` is invalid.
fn set_immediate_format_mcp_impl(
    app_state: &mut AppState,
    args: &Value,
) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let format_str = args
        .get("format")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'format'"))?;
    let target_address = get_optional_address(args, "target_address")?;

    set_immediate_format_impl(app_state, address, format_str, target_address)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Immediate format set at ${address:04X}") }] }),
    )
}

/// Sets the [`ImmediateFormat`] for the instruction at `address`.
///
/// # Errors
///
/// Returns [`McpError`] if `format_str` is unknown or if `target_address` is missing for
/// `"low_byte"` or `"high_byte"` formats.
pub(crate) fn set_immediate_format_impl(
    app_state: &mut AppState,
    address: Addr,
    format_str: &str,
    target_address: Option<Addr>,
) -> Result<(), McpError> {
    let format = match format_str.to_lowercase().as_str() {
        "hex" => ImmediateFormat::Hex,
        "decimal" | "dec" => ImmediateFormat::Decimal,
        "binary" | "bin" => ImmediateFormat::Binary,
        "low_byte" | "low" | "lo" => {
            let target = target_address.ok_or_else(|| {
                make_mcp_error(
                    -32602,
                    "Missing 'target_address' parameter for low_byte format",
                )
            })?;
            ImmediateFormat::LowByte(target)
        }
        "high_byte" | "high" | "hi" => {
            let target = target_address.ok_or_else(|| {
                make_mcp_error(
                    -32602,
                    "Missing 'target_address' parameter for high_byte format",
                )
            })?;
            ImmediateFormat::HighByte(target)
        }
        _ => {
            return Err(make_mcp_error(
                -32602,
                format!("Unknown format: {format_str}"),
            ));
        }
    };

    let command = Command::SetImmediateFormat {
        address,
        new_format: Some(format),
        old_format: app_state
            .annotations
            .get(address)
            .and_then(|e| e.immediate_format),
    };

    command.apply(app_state);
    let (analysis_cmd, _) = app_state.perform_analysis();
    app_state.push_command(Command::Batch(vec![analysis_cmd, command]));

    Ok(())
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
}
