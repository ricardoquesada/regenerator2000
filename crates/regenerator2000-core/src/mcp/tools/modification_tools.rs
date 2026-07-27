//! MCP tool handlers for modifying assembly state, labels, comments, block types, immediate formats, and project enums.
//!
//! This module provides tool definitions and execution handlers for mutating project state,
//! adding user labels and comments, changing memory block classifications, configuring operand
//! formatting, and creating or applying project-level enum definitions.

use crate::mcp::tools::common::{convert_region, get_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{Addr, BlockType, EnumDefinition, ImmediateFormat};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Returns JSON schemas for memory modification and mutation tools.
///
/// # Returns
///
/// A vector of JSON [`Value`] objects defining schemas for `r2000_disassemble`, `r2000_set_label_name`,
/// `r2000_set_comment`, `r2000_set_data_type`, `r2000_toggle_splitter`, `r2000_set_immediate_format`,
/// `r2000_create_project_enum`, `r2000_update_project_enum`, `r2000_delete_project_enum`, and `r2000_apply_enum_usage`.
#[must_use]
pub fn tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_disassemble",
            "description": "Performs recursive disassembly starting at a specified address.",
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
            "description": "Sets a user-defined label at a specific MOS 6502 memory address. Use this to name functions, variables, or jump targets to make the disassembly more readable.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address where the label should be set (decimal, e.g. 4096 for $1000)." },
                    "name": { "type": "string", "description": "The label name (e.g. 'init_screen', 'loop_start')." }
                },
                "required": ["address", "name"]
            }
        }),
        json!({
            "name": "r2000_set_comment",
            "description": "Adds a comment at a specific address. 'line' comments appear on their own line before the instruction (supports multi-line). 'side' comments appear inline on the same line as the instruction.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address for the comment (decimal, e.g. 4096 for $1000)." },
                    "comment": { "type": "string", "description": "The comment text. Do not include the ';' prefix." },
                    "type": {
                        "type": "string",
                        "enum": ["line", "side"],
                        "description": "'line' = comment on its own line before the instruction. 'side' = inline comment on the same line."
                    }
                },
                "required": ["address", "comment", "type"]
            }
        }),
        json!({
            "name": "r2000_set_data_type",
            "description": "Sets the data type for a memory region. Use this to mark regions as code, bytes, addresses, text, split tables, etc.",
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
            "description": "Toggles a Splitter at a specific address. Splitters prevent the auto-merger from combining adjacent blocks of the same type.",
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
            "description": "Sets the display format for immediate values (operands) at a specific address. Supports hexadecimal, decimal, binary, and Low/High byte references to a 16-bit address.",
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
            "description": "Creates a new project-specific enum definition embedded in the project file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique alphanumeric identifier." },
                    "description": { "type": "string", "description": "Optional summary explaining the enum's purpose." },
                    "variants": {
                        "type": "object",
                        "description": "Variant mapping where keys are numeric strings (decimal, hex 0x/$, bin 0b/%) and values are variant names."
                    }
                },
                "required": ["name", "variants"]
            }
        }),
        json!({
            "name": "r2000_update_project_enum",
            "description": "Updates or renames an existing project-specific enum.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Existing name of the enum to update." },
                    "new_name": { "type": "string", "description": "Optional new name if renaming the enum." },
                    "description": { "type": "string", "description": "Optional updated summary explaining the enum's purpose." },
                    "variants": { "type": "object", "description": "Variant mapping." }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "r2000_delete_project_enum",
            "description": "Deletes a project-specific enum definition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the enum to delete." },
                    "force": { "type": "boolean", "description": "When true, forces deletion even if enum is in use. Defaults to false." }
                },
                "required": ["name"]
            }
        }),
        json!({
            "name": "r2000_apply_enum_usage",
            "description": "Applies or removes an enum usage for an immediate value at a specific address.",
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

/// Handles execution of modification tool calls.
///
/// # Arguments
///
/// * `name` - Tool identifier string.
/// * `args` - Tool arguments JSON [`Value`].
/// * `app_state` - Mutable reference to application state [`AppState`].
///
/// # Returns
///
/// `Some(Ok(value))` on successful mutation, `Some(Err(McpError))` on error, or `None` if unrecognized.
///
/// # Errors
///
/// Returns [`McpError`] if tool argument validation fails or execution breaks constraints.
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

/// Private helper to perform recursive disassembly starting at a specified address.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address`.
///
/// # Returns
///
/// A JSON [`Value`] containing execution result text describing disassembly status.
///
/// # Errors
///
/// Returns [`McpError`] if `address` is outside loaded binary range.
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
    let mut commands = Vec::new();
    for range in ranges {
        let old_types = app_state.block_types[range.start..range.end].to_vec();
        commands.push(crate::commands::Command::SetBlockType {
            range,
            new_type: BlockType::Code,
            old_types,
        });
    }
    if !commands.is_empty() {
        let batch = crate::commands::Command::Batch(commands);
        batch.apply(app_state);
        let (analysis_cmd, msg) = app_state.perform_analysis();
        let final_cmd = crate::commands::Command::Batch(vec![batch, analysis_cmd]);
        app_state.push_command(final_cmd);
        app_state.disassemble();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Disassembled successfully starting at ${:04X}. {msg}", address.0)
            }]
        }))
    } else {
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("No new code found starting at ${:04X}", address.0)
            }]
        }))
    }
}

/// Private helper to set or remove a user label name at a memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address` and `name`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming label update.
///
/// # Errors
///
/// Returns [`McpError`] if `address` is missing or label creation validation fails.
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

/// Private helper to set line or side comments at a memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address`, `comment`, and `type`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming comment update.
///
/// # Errors
///
/// Returns [`McpError`] if parameters are missing or `type` is unrecognized.
fn set_comment_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let comment = args
        .get("comment")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| {
            make_mcp_error(-32602, "Missing or invalid 'comment' (expected a string)")
        })?;
    let comment_type = args
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'type' (expected \"line\" or \"side\")"))?;

    let command = match comment_type {
        "side" => crate::commands::Command::SetUserSideComment {
            address,
            new_comment: Some(comment),
            old_comment: app_state
                .annotations
                .get(address)
                .and_then(|e| e.user_side_comment.clone()),
        },
        "line" => crate::commands::Command::SetUserLineComment {
            address,
            new_comment: Some(comment),
            old_comment: app_state
                .annotations
                .get(address)
                .and_then(|e| e.user_line_comment.clone()),
        },
        other => {
            return Err(make_mcp_error(
                -32602,
                format!("Invalid 'type' value \"{other}\": expected \"line\" or \"side\""),
            ));
        }
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();
    Ok(json!({ "content": [{ "type": "text", "text": format!("Comment set at ${address:04X}") }] }))
}

/// Private helper to set data block type classification for a memory region.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `start_address`, `end_address`, and `data_type`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming block data type classification update.
///
/// # Errors
///
/// Returns [`McpError`] if `data_type` string is invalid or region bounds are invalid.
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

/// Private helper to toggle a block splitter at a specific memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming splitter toggle at target address.
///
/// # Errors
///
/// Returns [`McpError`] if address parameter is missing or invalid.
fn toggle_splitter_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let command = crate::commands::Command::ToggleSplitter { address };
    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();
    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Splitter toggled at ${address:04X}") }] }),
    )
}

/// Private helper wrapper for setting immediate formatting via MCP.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address`, `format`, and optional `target_address`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming immediate operand formatting change.
///
/// # Errors
///
/// Returns [`McpError`] if formatting options are invalid.
fn set_immediate_format_mcp_impl(
    app_state: &mut AppState,
    args: &Value,
) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let format_str = args
        .get("format")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'format'"))?;
    let target_address = get_address(args, "target_address").ok();

    set_immediate_format_impl(app_state, address, format_str, target_address)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Immediate format set at ${address:04X}") }] }),
    )
}

/// Helper function to set immediate operand formatting [`ImmediateFormat`] at an address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `address` - Target instruction memory address [`Addr`].
/// * `format_str` - Format identifier string (`"hex"`, `"decimal"`, `"binary"`, `"low_byte"`, `"high_byte"`).
/// * `target_address` - Optional 16-bit target address [`Addr`] required for low/high byte formatting.
///
/// # Returns
///
/// Returns `Ok(())` on successful immediate format application.
///
/// # Errors
///
/// Returns [`McpError`] if `format_str` is unknown or required `target_address` is missing.
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

    let command = crate::commands::Command::SetImmediateFormat {
        address,
        new_format: Some(format),
        old_format: app_state
            .annotations
            .get(address)
            .and_then(|e| e.immediate_format),
    };

    command.apply(app_state);
    let (analysis_cmd, _) = app_state.perform_analysis();
    app_state.push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));
    app_state.disassemble();

    Ok(())
}

/// Private helper wrapper for creating a project enum via MCP.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `name`, optional `description`, and `variants`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming project enum creation.
///
/// # Errors
///
/// Returns [`McpError`] if creation parameters are missing or invalid.
fn create_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let description = args.get("description").and_then(|v| v.as_str());
    let variants = args
        .get("variants")
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'variants'"))?;

    create_project_enum_impl(app_state, enum_name, description, variants)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{enum_name}' created successfully.") }] }),
    )
}

/// Helper function to create a new project-specific enum definition [`EnumDefinition`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `name` - Unique enum identifier string.
/// * `description` - Optional summary description text.
/// * `variants_val` - JSON [`Value`] map of numeric keys to variant names.
///
/// # Returns
///
/// Returns `Ok(())` on successful project enum creation.
///
/// # Errors
///
/// Returns [`McpError`] if enum name fails validation or variant parsing fails.
pub(crate) fn create_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    description: Option<&str>,
    variants_val: &Value,
) -> Result<(), McpError> {
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

    let command = crate::commands::Command::SetEnumDefinition {
        name: name.to_string(),
        new_definition: Some(definition),
        old_definition: None,
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

/// Private helper wrapper for updating a project enum via MCP.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `name`, optional `new_name`, `description`, and `variants`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming project enum update.
///
/// # Errors
///
/// Returns [`McpError`] if enum update fails.
fn update_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let new_name = args.get("new_name").and_then(|v| v.as_str());
    let description = args.get("description").and_then(|v| v.as_str());
    let variants = args.get("variants");

    update_project_enum_impl(app_state, enum_name, new_name, description, variants)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{enum_name}' updated successfully.") }] }),
    )
}

/// Helper function to update or rename an existing project enum definition [`EnumDefinition`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `name` - Target enum identifier string.
/// * `new_name` - Optional new name if renaming.
/// * `description` - Optional updated description string.
/// * `variants_val` - Optional updated variants JSON [`Value`].
///
/// # Returns
///
/// Returns `Ok(())` on successful project enum modification or rename.
///
/// # Errors
///
/// Returns [`McpError`] if target enum is not found or new name is invalid.
pub(crate) fn update_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    new_name: Option<&str>,
    description: Option<&str>,
    variants_val: Option<&Value>,
) -> Result<(), McpError> {
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
        let cmd_remove = crate::commands::Command::SetEnumDefinition {
            name: name.to_string(),
            new_definition: None,
            old_definition: Some(old_definition.clone()),
        };
        let cmd_add = crate::commands::Command::SetEnumDefinition {
            name: n.to_string(),
            new_definition: Some(updated_def),
            old_definition: None,
        };
        crate::commands::Command::Batch(vec![cmd_remove, cmd_add])
    } else {
        crate::commands::Command::SetEnumDefinition {
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

/// Private helper wrapper for deleting a project enum via MCP.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `name` and optional `force` flag.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming project enum deletion.
///
/// # Errors
///
/// Returns [`McpError`] if deletion fails.
fn delete_project_enum_mcp(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let enum_name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'name'"))?;
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    delete_project_enum_impl(app_state, enum_name, force)?;

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Project enum '{enum_name}' deleted successfully.") }] }),
    )
}

/// Helper function to delete a project enum definition [`EnumDefinition`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `name` - Target enum identifier string to delete.
/// * `force` - When true, forces deletion even if enum is in use at memory addresses.
///
/// # Returns
///
/// Returns `Ok(())` on successful project enum removal.
///
/// # Errors
///
/// Returns [`McpError`] if enum is not found or is currently used in annotations (when `force` is false).
pub(crate) fn delete_project_enum_impl(
    app_state: &mut AppState,
    name: &str,
    force: bool,
) -> Result<(), McpError> {
    let old_definition = app_state.enums.get(name).cloned().ok_or_else(|| {
        make_mcp_error(-32602, format!("Enum '{name}' not found in project enums"))
    })?;

    if !force {
        let usages: Vec<Addr> = app_state
            .annotations
            .iter()
            .filter(|(_, entry)| entry.enum_usage.as_deref() == Some(name))
            .map(|(addr, _)| addr)
            .collect();

        if !usages.is_empty() {
            let addresses: Vec<String> = usages.iter().map(|a| format!("${a:04X}")).collect();
            return Err(make_mcp_error(
                -32602,
                format!(
                    "Cannot delete enum '{name}' because it is currently in use at addresses: {}. Set 'force' to true to override.",
                    addresses.join(", ")
                ),
            ));
        }
    }

    let command = crate::commands::Command::SetEnumDefinition {
        name: name.to_string(),
        new_definition: None,
        old_definition: Some(old_definition),
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

/// Private helper to attach or clear enum usage annotations at a memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `address` and optional `name`.
///
/// # Returns
///
/// A JSON [`Value`] notification text confirming enum usage application or clearance.
///
/// # Errors
///
/// Returns [`McpError`] if address parsing fails.
fn apply_enum_usage_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let address = get_address(args, "address")?;
    let name = args.get("name").and_then(|v| v.as_str());
    let name_opt = name.filter(|s| !s.trim().is_empty()).map(String::from);

    let command = crate::commands::Command::SetEnumUsage {
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
        // NOP instruction ($EA) at 0x1000
        app_state.raw_data[0] = 0xEA;
        app_state.block_types[0] = BlockType::Undefined;

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

        // Test out of bounds address
        let invalid_args = json!({ "address": 0x3000 });
        let err = handle_call("r2000_disassemble", &invalid_args, &mut app_state)
            .expect("Tool call should match")
            .expect_err("Out of bounds address should fail");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("out of bounds"));
    }
}
