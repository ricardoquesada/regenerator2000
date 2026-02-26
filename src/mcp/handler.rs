use crate::mcp::types::{McpError, McpRequest, McpResponse};
use crate::state::AppState;
use crate::state::types::{BlockType, ImmediateFormat};
use base64::prelude::*;
use serde_json::{Value, json};

use crate::ui_state::UIState;

pub fn handle_request(
    req: &McpRequest,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> McpResponse {
    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "regenerator2000-mcp",
                "version": env!("CARGO_PKG_VERSION"),
                "description": "An interactive disassembler for the MOS 6502 assembly."
            },
            "instructions": "You are an expert in MOS 6502 assembly programmer. Always assume the code is 6502 assembly unless stated otherwise.",
            "capabilities": {
                "tools": {},
                "resources": {}
            }
        })),
        "notifications/initialized" => Ok(json!(true)),
        "tools/list" => list_tools(),
        "resources/list" => list_resources(),
        // Tools
        "tools/call" => handle_tool_call(&req.params, app_state, ui_state),
        // Resources
        "resources/read" => handle_resource_read(&req.params, app_state),

        _ => Err(McpError {
            code: -32601,
            message: format!("Method not found: {}", req.method),
            data: None,
        }),
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

fn list_tools() -> Result<Value, McpError> {
    Ok(json!({
        "tools": [
            {
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
            },
            {
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
            },
            {
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
                                "code",
                                "byte",
                                "word",
                                "address",
                                "petscii",
                                "screencode",
                                "lo_hi_address",
                                "hi_lo_address",
                                "lo_hi_word",
                                "hi_lo_word",
                                "external_file",
                                "undefined"
                            ],
                            "description": "code=MOS 6502 instructions; byte=raw 8-bit data (sprites, charset, tables, unknowns); word=16-bit LE values; address=16-bit LE pointers (creates X-Refs, use for jump tables/vectors); petscii=PETSCII text; screencode=Screen code text (data written to $0400); lo_hi_address=split address table, low bytes first then high bytes (even count required); hi_lo_address=split address table, high bytes first (even count required); lo_hi_word=split word table, low bytes first (e.g. SID freq tables); hi_lo_word=split word table, high bytes first; external_file=large binary blob (SID, bitmap, charset) to export as-is; undefined=reset region to unknown state."
                        }
                    },
                    "required": ["start_address", "end_address", "data_type"]
                }
            },
            {
                "name": "r2000_toggle_splitter",
                "description": "Toggles a Splitter at a specific address. Splitters prevent the auto-merger from combining adjacent blocks of the same type. Crucial for separating adjacent Lo/Hi table halves.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "The memory address where the splitter should be toggled (decimal)." }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "r2000_undo",
                "description": "Undoes the latest operation.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_redo",
                "description": "Redoes the latest undone operation.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_read_region",
                "description": "Get disassembly or hexdump text for a specific memory range.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "start_address": { "type": "integer", "description": "Start address (inclusive), decimal." },
                        "end_address":   { "type": "integer", "description": "End address (inclusive), decimal." },
                        "view": {
                            "type": "string",
                            "enum": ["disasm", "hexdump"],
                            "description": "The view to return. Default: 'disasm'."
                        }
                    },
                    "required": ["start_address", "end_address"]
                }
            },
            {
                "name": "r2000_read_selected",
                "description": "Get disassembly or hexdump for the range currently selected in the UI. If nothing is selected, returns the instruction/row under the cursor.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "view": {
                            "type": "string",
                            "enum": ["disasm", "hexdump"],
                            "description": "The view to return. Default: 'disasm'."
                        }
                    }
                }
            },
            {
                "name": "r2000_get_binary_info",
                "description": "Returns the origin address, size in bytes, target platform (e.g. 'Commodore 64'), filename, and user-provided description of the loaded binary.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_get_analyzed_blocks",
                "description": "Returns the list of memory blocks as analyzed, including their range and type. Respects splitters.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "block_type": {
                            "type": "string",
                            "description": "Optional filter to return only blocks of a specific type. Case-insensitive."
                        }
                    }
                }
            },
            {
                "name": "r2000_get_address_details",
                "description": "Returns detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "The memory address to inspect (decimal)." }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "r2000_get_disassembly_cursor",
                "description": "Returns the memory address of the current cursor position in the disassembly view.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_jump_to_address",
                "description": "Moves the disassembly cursor to a specific memory address and scrolls the view to make it visible. Also keeps the jump history.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "The target address to jump to (decimal)." }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "r2000_search_memory",
                "description": "Search for a sequence of bytes or a text string in the memory. Returns a list of addresses where the sequence is found.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query. For hex: space-separated bytes, e.g. 'A9 00'. For text: plain string."
                        },
                        "encoding": {
                            "type": "string",
                            "enum": ["ascii", "petscii", "screencode", "hex"],
                            "description": "Encoding for the query. Defaults to 'hex' if query looks like hex bytes, otherwise 'ascii'."
                        }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "r2000_get_cross_references",
                "description": "Get a list of addresses that reference the given address (e.g. JSRs, JMPs, loads).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "The target address to find references to (decimal)." }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "r2000_set_operand_format",
                "description": "Sets the display format for immediate values (operands) at a specific address. Useful for visualizing bitmasks.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer", "description": "The address of the instruction (decimal)." },
                        "format": {
                            "type": "string",
                            "enum": ["hex", "decimal", "binary"],
                            "description": "hex=$00, decimal=0, binary=%00000000."
                        }
                    },
                    "required": ["address", "format"]
                }
            },
            {
                "name": "r2000_get_symbol_table",
                "description": "Returns a list of all defined labels (user and system) and their addresses.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_get_all_comments",
                "description": "Returns all user-defined comments (line and side) and their addresses. Each entry has 'address' (integer), 'type' ('line' or 'side'), and 'comment' (string).",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_save_project",
                "description": "Saves the current project state to the existing .regen2000proj file. Only works if the project was previously loaded from or saved to a project file.",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "r2000_batch_execute",
                "description": "Executes multiple tool calls sequentially in a single request. Use only when you have 5+ independent operations to perform at once (e.g. marking many regions, renaming many labels). Do not use for operations that depend on each other's results.",
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
            }
        ]
    }))
}

fn list_resources() -> Result<Value, McpError> {
    Ok(json!({
        "resources": [
            {
                "uri": "disasm://main",
                "name": "Full Disassembly Info",
                "mimeType": "text/plain",
                "description": "Information about accessing the full disassembly. Direct reading is not supported; use region resources instead."
            },
            {
                "uri": "binary://main",
                "name": "Full Binary",
                "mimeType": "application/octet-stream",
                "description": "The full raw binary with 2-byte load address header (PRG format)."
            }
        ]
    }))
}

fn handle_tool_call(
    params: &Value,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> Result<Value, McpError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError {
            code: -32602,
            message: "Missing 'name' in tools/call".to_string(),
            data: None,
        })?;

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    handle_tool_call_internal(name, args, app_state, ui_state)
}

fn handle_tool_call_internal(
    name: &str,
    args: Value,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> Result<Value, McpError> {
    match name {
        "r2000_batch_execute" => {
            let calls = args
                .get("calls")
                .and_then(|v| v.as_array())
                .ok_or_else(|| McpError {
                    code: -32602,
                    message: "Missing 'calls' array".to_string(),
                    data: None,
                })?;

            let mut results = Vec::new();
            for call in calls {
                let tool_name =
                    call.get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| McpError {
                            code: -32602,
                            message: "Missing 'name' in call".to_string(),
                            data: None,
                        })?;

                let tool_args = call.get("arguments").cloned().unwrap_or(json!({}));

                match handle_tool_call_internal(tool_name, tool_args, app_state, ui_state) {
                    Ok(val) => results.push(json!({ "status": "success", "result": val })),
                    Err(err) => results.push(json!({ "status": "error", "error": err })),
                }
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&results).unwrap()
                }]
            }))
        }
        "r2000_set_label_name" => {
            let address = get_address(&args, "address")?;
            let label_name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError {
                    code: -32602,
                    message: "Missing or invalid 'name'".to_string(),
                    data: None,
                })?;

            let label = crate::state::Label {
                name: label_name.to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            };

            let command = crate::commands::Command::SetLabel {
                address,
                new_label: Some(vec![label]),
                old_label: app_state.labels.get(&address).cloned(),
            };

            command.apply(app_state);
            app_state.push_command(command);
            app_state.disassemble();

            Ok(
                json!({ "content": [{ "type": "text", "text": format!("Label set at ${:04X}", address) }] }),
            )
        }
        "r2000_set_comment" => {
            let address = get_address(&args, "address")?;
            let comment = args
                .get("comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let comment_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("line");

            let command = if comment_type == "side" {
                crate::commands::Command::SetUserSideComment {
                    address,
                    new_comment: comment.clone(),
                    old_comment: app_state.user_side_comments.get(&address).cloned(),
                }
            } else {
                crate::commands::Command::SetUserLineComment {
                    address,
                    new_comment: comment.clone(),
                    old_comment: app_state.user_line_comments.get(&address).cloned(),
                }
            };

            command.apply(app_state);
            app_state.push_command(command);
            app_state.disassemble();
            Ok(
                json!({ "content": [{ "type": "text", "text": format!("Comment set at ${:04X}", address) }] }),
            )
        }
        "r2000_set_data_type" => {
            let data_type_str =
                args.get("data_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError {
                        code: -32602,
                        message: "Missing 'data_type'".to_string(),
                        data: None,
                    })?;

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
                    return Err(McpError {
                        code: -32602,
                        message: format!("Unknown data_type: '{}'", data_type_str),
                        data: None,
                    });
                }
            };

            convert_region(app_state, &args, block_type)
        }

        "r2000_toggle_splitter" => {
            let address = get_address(&args, "address")?;
            let command = crate::commands::Command::ToggleSplitter { address };
            command.apply(app_state);
            app_state.push_command(command);
            app_state.disassemble();
            Ok(
                json!({ "content": [{ "type": "text", "text": format!("Splitter toggled at ${:04X}", address) }] }),
            )
        }

        "r2000_undo" => {
            let msg = app_state.undo_last_command();
            app_state.disassemble();
            Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
        }

        "r2000_redo" => {
            let msg = app_state.redo_last_command();
            app_state.disassemble();
            Ok(json!({ "content": [{ "type": "text", "text": msg }] }))
        }

        "r2000_read_region" => {
            let start_addr = get_address(&args, "start_address")?;
            let end_addr = get_address(&args, "end_address")?;
            let view = args
                .get("view")
                .and_then(|v| v.as_str())
                .unwrap_or("disasm");
            let text = if view == "hexdump" {
                get_hexdump_text(app_state, start_addr, end_addr)
            } else {
                get_disassembly_text(app_state, start_addr, end_addr)
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }

        "r2000_read_selected" => {
            let view = args
                .get("view")
                .and_then(|v| v.as_str())
                .unwrap_or("disasm");
            let text = if view == "hexdump" {
                let (start, end) = get_selection_range_hexdump(app_state, ui_state)?;
                get_hexdump_text(app_state, start, end)
            } else {
                let (start, end) = get_selection_range_disasm(app_state, ui_state)?;
                get_disassembly_text(app_state, start, end)
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }

        "r2000_get_binary_info" => {
            let origin = app_state.origin;
            let size = app_state.raw_data.len();
            let platform = &app_state.settings.platform;
            let filename = app_state
                .file_path
                .as_ref()
                .or(app_state.project_path.as_ref())
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&json!({
                        "origin": origin,
                        "size": size,
                        "platform": platform,
                        "filename": filename,
                        "description": app_state.settings.description
                    })).unwrap()
                }]
            }))
        }

        "r2000_get_analyzed_blocks" => {
            let filter = args.get("block_type").and_then(|v| v.as_str());
            let blocks = get_analyzed_blocks_impl(app_state, filter);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&blocks).unwrap()
                }]
            }))
        }

        "r2000_get_address_details" => {
            let address = get_address(&args, "address")?;
            let details = get_address_details_impl(app_state, address)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&details).unwrap()
                }]
            }))
        }

        "r2000_get_disassembly_cursor" => {
            let idx = ui_state.cursor_index;
            if let Some(line) = app_state.disassembly.get(idx) {
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("${:04X}", line.address)
                    }]
                }))
            } else {
                Err(McpError {
                    code: -32602,
                    message: "Cursor out of bounds".to_string(),
                    data: None,
                })
            }
        }

        "r2000_jump_to_address" => {
            let address = get_address(&args, "address")?;
            if app_state
                .get_line_index_containing_address(address)
                .or_else(|| app_state.get_line_index_for_address(address))
                .is_some()
            {
                crate::ui::menu::perform_jump_to_address(app_state, ui_state, address);

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Jumped to ${:04X}", address)
                    }]
                }))
            } else {
                Err(McpError {
                    code: -32602,
                    message: format!(
                        "Address ${:04X} not found in disassembly (might be hidden or invalid)",
                        address
                    ),
                    data: None,
                })
            }
        }

        "r2000_search_memory" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError {
                    code: -32602,
                    message: "Missing 'query'".to_string(),
                    data: None,
                })?;
            let encoding = args.get("encoding").and_then(|v| v.as_str());
            let matches = search_memory_impl(app_state, query, encoding)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&matches).unwrap()
                }]
            }))
        }

        "r2000_get_cross_references" => {
            let address = get_address(&args, "address")?;
            let refs = get_cross_references_impl(app_state, address);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&refs).unwrap()
                }]
            }))
        }

        "r2000_set_operand_format" => {
            let address = get_address(&args, "address")?;
            let format_str =
                args.get("format")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError {
                        code: -32602,
                        message: "Missing 'format'".to_string(),
                        data: None,
                    })?;

            set_operand_format_impl(app_state, address, format_str)?;

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Operand format at ${:04X} set to {}", address, format_str)
                }]
            }))
        }

        "r2000_get_symbol_table" => {
            let symbols = get_symbol_table_impl(app_state);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&symbols).unwrap()
                }]
            }))
        }

        "r2000_get_all_comments" => {
            let comments = get_all_comments_impl(app_state);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&comments).unwrap()
                }]
            }))
        }
        "r2000_save_project" => {
            if app_state.project_path.is_none() {
                return Err(McpError {
                    code: -32603,
                    message: "No active project path. Project must be loaded from or saved to a .regen2000proj file before it can be saved.".to_string(),
                    data: None,
                });
            }

            let ctx = create_save_context(app_state, ui_state);
            app_state.save_project(ctx, true).map_err(|e| McpError {
                code: -32603,
                message: format!("Failed to save project: {}", e),
                data: None,
            })?;

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Project saved to {}", app_state.project_path.as_ref().unwrap().display())
                }]
            }))
        }

        _ => Err(McpError {
            code: -32601,
            message: format!("Tool not found: {}", name),
            data: None,
        }),
    }
}

fn convert_region(
    app_state: &mut AppState,
    args: &Value,
    block_type: BlockType,
) -> Result<Value, McpError> {
    let start_addr = get_address(args, "start_address")?;
    let end_addr = get_address(args, "end_address")?;

    if start_addr > end_addr {
        return Err(McpError {
            code: -32602,
            message: "start_address must be <= end_address".to_string(),
            data: None,
        });
    }

    let origin = app_state.origin;
    let max_len = app_state.block_types.len() as u16;

    // Bounds check
    if start_addr < origin || end_addr >= origin.wrapping_add(max_len) {
        return Err(McpError {
            code: -32602,
            message: format!(
                "Region ${:04X}-${:04X} out of bounds (Origin: ${:04X})",
                start_addr, end_addr, origin
            ),
            data: None,
        });
    }

    let start_idx = (start_addr - origin) as usize;
    let end_idx = (end_addr - origin) as usize;

    // Range is inclusive on both ends, Command::SetBlockType uses start..end+1
    let range = start_idx..(end_idx + 1);

    let old_types = app_state.block_types[range.clone()].to_vec();

    let command = crate::commands::Command::SetBlockType {
        range,
        new_type: block_type,
        old_types,
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(
        json!({ "content": [{ "type": "text", "text": format!("Region ${:04X}-${:04X} converted to {:?}", start_addr, end_addr, block_type) }] }),
    )
}

fn get_address(args: &Value, key: &str) -> Result<u16, McpError> {
    let val = args.get(key).ok_or_else(|| McpError {
        code: -32602,
        message: format!("Missing '{}'", key),
        data: None,
    })?;

    if let Some(n) = val.as_u64() {
        return Ok(n as u16);
    }

    // Still accept string formats for robustness / backwards compat with older clients
    if let Some(s) = val.as_str()
        && let Some(addr) = parse_address_string(s)
    {
        return Ok(addr);
    }

    Err(McpError {
        code: -32602,
        message: format!(
            "Invalid address format for '{}'. Expected a decimal integer.",
            key
        ),
        data: None,
    })
}

fn parse_address_string(s: &str) -> Option<u16> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(hex_part) = s.strip_prefix('$') {
        return u16::from_str_radix(hex_part, 16).ok();
    }

    if let Some(hex_part) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u16::from_str_radix(hex_part, 16).ok();
    }

    s.parse::<u16>().ok()
}

fn handle_resource_read(params: &Value, app_state: &mut AppState) -> Result<Value, McpError> {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError {
            code: -32602,
            message: "Missing 'uri'".to_string(),
            data: None,
        })?;

    if uri == "disasm://main" {
        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": "Full disassembly not supported via simple resource read, use regions."
            }]
        }))
    } else if uri == "binary://main" {
        let mut data = Vec::with_capacity(app_state.raw_data.len() + 2);
        let origin = app_state.origin;
        data.push((origin & 0xFF) as u8);
        data.push(((origin >> 8) & 0xFF) as u8);
        data.extend_from_slice(&app_state.raw_data);

        let encoded = BASE64_STANDARD.encode(&data);

        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/octet-stream",
                "blob": encoded
            }]
        }))
    } else {
        Err(McpError {
            code: -32602,
            message: "Resource not found".to_string(),
            data: None,
        })
    }
}

fn get_disassembly_text(app_state: &AppState, start: u16, end: u16) -> String {
    let mut output = String::new();
    output.push_str(&format!("* = ${:04X}\n", start));

    for line in &app_state.disassembly {
        if line.address >= start && line.address <= end {
            // Line comments
            if let Some(comment) = &line.line_comment {
                for line in comment.lines() {
                    output.push_str(&format!("; {}\n", line));
                }
            }

            if let Some(label) = &line.label
                && !label.is_empty()
            {
                output.push_str(&format!("{}:\n", label));
            }

            let instruction = if line.operand.is_empty() {
                line.mnemonic.clone()
            } else {
                format!("{} {}", line.mnemonic, line.operand)
            };

            // Side comments
            if !line.comment.is_empty() {
                output.push_str(&format!(
                    "${:04X} {:<20} ; {}\n",
                    line.address, instruction, line.comment
                ));
            } else {
                output.push_str(&format!("${:04X} {}\n", line.address, instruction));
            }
        }
    }
    output
}

fn get_selection_range_disasm(
    app_state: &AppState,
    ui_state: &UIState,
) -> Result<(u16, u16), McpError> {
    let cursor_idx = ui_state.cursor_index;
    let selection_idx = ui_state.selection_start;

    let (start_idx, end_idx) = if let Some(sel_start) = selection_idx {
        if sel_start < cursor_idx {
            (sel_start, cursor_idx)
        } else {
            (cursor_idx, sel_start)
        }
    } else {
        (cursor_idx, cursor_idx)
    };

    let start_line = app_state.disassembly.get(start_idx).ok_or(McpError {
        code: -32602,
        message: "Invalid start index".to_string(),
        data: None,
    })?;

    let end_line = app_state.disassembly.get(end_idx).ok_or(McpError {
        code: -32602,
        message: "Invalid end index".to_string(),
        data: None,
    })?;

    let start_addr = start_line.address;
    let end_addr = end_line.address;

    Ok((start_addr, end_addr))
}

fn get_selection_range_hexdump(
    app_state: &AppState,
    ui_state: &UIState,
) -> Result<(u16, u16), McpError> {
    let cursor_row = ui_state.hex_cursor_index;
    let selection_row = ui_state.hex_selection_start;

    let (start_row, end_row) = if let Some(sel_start) = selection_row {
        if sel_start < cursor_row {
            (sel_start, cursor_row)
        } else {
            (cursor_row, sel_start)
        }
    } else {
        (cursor_row, cursor_row)
    };

    let origin = app_state.origin;
    let bytes_per_row = 16;

    let alignment_padding = (origin % 16) as usize;
    let aligned_origin = (origin as usize) - alignment_padding;

    let start_addr = (aligned_origin + start_row * bytes_per_row) as u16;
    let end_addr = (aligned_origin + (end_row + 1) * bytes_per_row - 1) as u16;

    // Clamp to valid range
    let max_len = app_state.raw_data.len() as u16;
    let end_limit = origin.wrapping_add(max_len).wrapping_sub(1);

    let final_start = if start_addr < origin {
        origin
    } else {
        start_addr
    };
    let final_end = if end_addr > end_limit {
        end_limit
    } else {
        end_addr
    };

    Ok((final_start, final_end))
}

fn get_hexdump_text(app_state: &AppState, start_addr: u16, end_addr: u16) -> String {
    let mut output = String::new();
    let origin = app_state.origin;
    for addr in start_addr..=end_addr {
        if addr < origin || addr >= origin.wrapping_add(app_state.raw_data.len() as u16) {
            continue;
        }
        let idx = (addr - origin) as usize;
        let byte = app_state.raw_data[idx];
        if (addr - start_addr).is_multiple_of(16) {
            if addr != start_addr {
                output.push('\n');
            }
            output.push_str(&format!("${:04X}: ", addr));
        }
        output.push_str(&format!("{:02X} ", byte));
    }
    output
}

fn get_analyzed_blocks_impl(app_state: &AppState, filter: Option<&str>) -> Vec<Value> {
    let mut blocks = Vec::new();
    let origin = app_state.origin;
    let max_len = app_state.block_types.len();

    if max_len == 0 {
        return blocks;
    }

    let mut start_idx = 0;
    let mut current_type = app_state.block_types[0];

    for i in 1..max_len {
        let addr = origin.wrapping_add(i as u16);
        let type_ = app_state.block_types[i];

        let is_splitter = app_state.splitters.contains(&addr);

        if type_ != current_type || is_splitter {
            // Finish previous block
            let end_idx = i - 1;
            let start_addr = origin.wrapping_add(start_idx as u16);
            let end_addr = origin.wrapping_add(end_idx as u16);

            let type_str = current_type.to_string();
            let should_include = match filter {
                Some(f) => type_str.eq_ignore_ascii_case(f),
                None => true,
            };

            if should_include {
                blocks.push(json!({
                    "start_address": start_addr,
                    "end_address": end_addr,
                    "type": type_str
                }));
            }

            start_idx = i;
            current_type = type_;
        }
    }

    // Last block
    let end_idx = max_len - 1;
    let start_addr = origin.wrapping_add(start_idx as u16);
    let end_addr = origin.wrapping_add(end_idx as u16);
    let type_str = current_type.to_string();
    let should_include = match filter {
        Some(f) => type_str.eq_ignore_ascii_case(f),
        None => true,
    };

    if should_include {
        blocks.push(json!({
            "start_address": start_addr,
            "end_address": end_addr,
            "type": type_str
        }));
    }

    blocks
}

fn search_memory_impl(
    app_state: &AppState,
    query: &str,
    encoding: Option<&str>,
) -> Result<Vec<u16>, McpError> {
    let mut search_bytes = Vec::new();

    // Determine mode
    let mode = if let Some(enc) = encoding {
        enc
    } else {
        // Simple heuristic: if query contains space and hex-like chars, try hex
        if query.contains(' ')
            && query
                .split_whitespace()
                .all(|s| u8::from_str_radix(s, 16).is_ok())
        {
            "hex"
        } else {
            "ascii"
        }
    };

    match mode {
        "hex" => {
            for part in query.split_whitespace() {
                // Remove $ or 0x prefix if present
                let clean_part = part
                    .trim_start_matches("0x")
                    .trim_start_matches("0X")
                    .trim_start_matches('$');
                if let Ok(b) = u8::from_str_radix(clean_part, 16) {
                    search_bytes.push(b);
                }
            }
        }
        "ascii" => {
            search_bytes = query.as_bytes().to_vec();
        }
        "petscii" => {
            for c in query.chars() {
                search_bytes.push(ascii_char_to_petscii(c));
            }
        }
        "screencode" => {
            for c in query.chars() {
                let p = ascii_char_to_petscii(c);
                search_bytes.push(petscii_to_screencode_simple(p));
            }
        }
        _ => {
            return Err(McpError {
                code: -32602,
                message: format!("Unknown encoding: {}", mode),
                data: None,
            });
        }
    }

    if search_bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut found_addresses = Vec::new();
    let data = &app_state.raw_data;
    let origin = app_state.origin;

    if data.len() < search_bytes.len() {
        return Ok(Vec::new());
    }

    for i in 0..=data.len() - search_bytes.len() {
        if data[i..i + search_bytes.len()] == search_bytes[..] {
            found_addresses.push(origin.wrapping_add(i as u16));
            if found_addresses.len() >= 100 {
                break; // Limit results
            }
        }
    }

    Ok(found_addresses)
}

fn get_cross_references_impl(app_state: &AppState, address: u16) -> Vec<u16> {
    if let Some(refs) = app_state.cross_refs.get(&address) {
        let mut sorted_refs = refs.clone();
        sorted_refs.sort();
        sorted_refs.dedup();
        sorted_refs
    } else {
        Vec::new()
    }
}

fn set_operand_format_impl(
    app_state: &mut AppState,
    address: u16,
    format_str: &str,
) -> Result<(), McpError> {
    let format = match format_str.to_lowercase().as_str() {
        "hex" => ImmediateFormat::Hex,
        "decimal" | "dec" => ImmediateFormat::Decimal,
        "binary" | "bin" => ImmediateFormat::Binary,
        _ => {
            return Err(McpError {
                code: -32602,
                message: format!("Unknown format: {}", format_str),
                data: None,
            });
        }
    };

    let command = crate::commands::Command::SetImmediateFormat {
        address,
        new_format: Some(format),
        old_format: app_state.immediate_value_formats.get(&address).cloned(),
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

fn get_symbol_table_impl(app_state: &AppState) -> Vec<Value> {
    let mut symbols = Vec::new();
    for (addr, labels) in &app_state.labels {
        for label in labels {
            symbols.push(json!({
                "address": addr,
                "name": label.name,
                "kind": format!("{:?}", label.kind),
                "type": format!("{:?}", label.label_type)
            }));
        }
    }
    // Sort by address
    symbols.sort_by(|a, b| {
        let addr_a = a["address"].as_u64().unwrap();
        let addr_b = b["address"].as_u64().unwrap();
        addr_a.cmp(&addr_b)
    });
    symbols
}

fn get_all_comments_impl(app_state: &AppState) -> Vec<Value> {
    let mut comments = Vec::new();

    for (addr, comment) in &app_state.user_line_comments {
        comments.push(json!({
            "address": addr,
            "type": "line",
            "comment": comment
        }));
    }

    for (addr, comment) in &app_state.user_side_comments {
        comments.push(json!({
            "address": addr,
            "type": "side",
            "comment": comment
        }));
    }

    // Sort by address
    comments.sort_by(|a, b| {
        let addr_a = a["address"].as_u64().unwrap();
        let addr_b = b["address"].as_u64().unwrap();
        addr_a.cmp(&addr_b)
    });

    comments
}

fn get_address_details_impl(app_state: &AppState, address: u16) -> Result<Value, McpError> {
    let origin = app_state.origin;
    if address < origin || address >= origin.wrapping_add(app_state.raw_data.len() as u16) {
        return Ok(json!({
            "address": address,
            "type": "OutOfRange",
            "message": "Address is outside the loaded binary range."
        }));
    }

    let idx = (address - origin) as usize;
    let block_type = app_state.block_types[idx];
    let mut details = json!({
        "address": address,
        "type": format!("{:?}", block_type)
    });

    // 1. Instruction Semantics (if Code)
    if block_type == BlockType::Code
        && let Ok(line_idx) = app_state
            .disassembly
            .binary_search_by_key(&address, |l| l.address)
    {
        let line = &app_state.disassembly[line_idx];

        if let Some(opcode) = &line.opcode {
            let instruction_json = json!({
                "mnemonic": opcode.mnemonic,
                "mode": format!("{:?}", opcode.mode),
                "size": opcode.size,
                "cycles": opcode.cycles,
                "description": opcode.description,
                "bytes": line.bytes,
                "operand_text": line.operand
            });

            // Implied Target (Flow control)
            if let Some(target) = line.target_address {
                details["metadata"]["target_address"] = json!(target);
            }
            // Explicit Data Reference (Operand)
            else {
                let ref_addr = match opcode.mode {
                    crate::cpu::AddressingMode::Absolute
                    | crate::cpu::AddressingMode::AbsoluteX
                    | crate::cpu::AddressingMode::AbsoluteY => {
                        if line.bytes.len() >= 3 {
                            Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::ZeroPage
                    | crate::cpu::AddressingMode::ZeroPageX
                    | crate::cpu::AddressingMode::ZeroPageY => {
                        if line.bytes.len() >= 2 {
                            Some(line.bytes[1] as u16)
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::Indirect => {
                        if line.bytes.len() >= 3 {
                            Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::IndirectX
                    | crate::cpu::AddressingMode::IndirectY => {
                        if line.bytes.len() >= 2 {
                            Some(line.bytes[1] as u16)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(addr) = ref_addr {
                    details["metadata"]["referenced_address"] = json!(addr);
                }
            }

            details["instruction"] = instruction_json;
        }
    }

    // 2. Cross References (incoming)
    if let Some(refs) = app_state.cross_refs.get(&address) {
        details["metadata"]["cross_refs_in"] = json!(refs);
    }

    // 3. Labels
    if let Some(labels) = app_state.labels.get(&address) {
        let label_names: Vec<String> = labels.iter().map(|l| l.name.clone()).collect();
        details["metadata"]["labels"] = json!(label_names);
    }

    // 4. Comments
    let mut comments = Vec::new();
    if let Some(c) = app_state.user_line_comments.get(&address) {
        comments.push(format!("[User Line] {}", c));
    }
    if let Some(c) = app_state.user_side_comments.get(&address) {
        comments.push(format!("[User Side] {}", c));
    }
    if let Some(c) = app_state.system_comments.get(&address) {
        comments.push(format!("[System] {}", c));
    }
    if !comments.is_empty() {
        details["metadata"]["comments"] = json!(comments);
    }

    // 5. Operand Format
    if let Some(fmt) = app_state.immediate_value_formats.get(&address) {
        details["metadata"]["operand_format"] = json!(format!("{:?}", fmt));
    }

    Ok(details)
}

fn create_save_context(
    app_state: &AppState,
    ui_state: &UIState,
) -> crate::state::project::ProjectSaveContext {
    let origin = app_state.origin as usize;

    // Cursor address
    let cursor_address = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address);

    // Hex cursor address
    let alignment_padding = origin % 16;
    let aligned_origin = origin - alignment_padding;
    let hex_dump_cursor_address = Some((aligned_origin + ui_state.hex_cursor_index * 16) as u16);

    // Sprites cursor address
    let aligned_sprite_origin = (origin / 64) * 64;
    let sprites_cursor_address =
        Some((aligned_sprite_origin + ui_state.sprites_cursor_index * 64) as u16);

    // Charset cursor address
    let base_alignment = 0x400;
    let aligned_charset_origin = (origin / base_alignment) * base_alignment;
    let charset_cursor_address =
        Some((aligned_charset_origin + ui_state.charset_cursor_index * 8) as u16);

    // Bitmap cursor address
    let aligned_bitmap_origin = (origin / 8192) * 8192;
    let bitmap_cursor_address =
        Some((aligned_bitmap_origin + ui_state.bitmap_cursor_index * 8192) as u16);

    crate::state::project::ProjectSaveContext {
        cursor_address,
        hex_dump_cursor_address,
        sprites_cursor_address,
        right_pane_visible: Some(format!("{:?}", ui_state.right_pane)),
        charset_cursor_address,
        bitmap_cursor_address,
        sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
        charset_multicolor_mode: ui_state.charset_multicolor_mode,
        bitmap_multicolor_mode: ui_state.bitmap_multicolor_mode,
        hexdump_view_mode: ui_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: ui_state.blocks_list_state.selected(),
        bookmarks: app_state.bookmarks.clone(),
    }
}

// Helpers

fn ascii_char_to_petscii(c: char) -> u8 {
    let b = c as u8;
    match b {
        b'a'..=b'z' => b - 32, // 'a' (97) -> 'A' (65) (Unshifted PETSCII)
        b'A'..=b'Z' => b + 32, // 'A' (65) -> 'a' (97) (Shifted PETSCII / Graphics)
        _ => b,                // Numbers, punctuation mostly map 1:1 for basic ASCII
    }
}

fn petscii_to_screencode_simple(petscii: u8) -> u8 {
    match petscii {
        0x40..=0x5F => petscii - 0x40,
        0x20..=0x3F => petscii,
        0x60..=0x7F => petscii - 0x20,
        0xA0..=0xBF => petscii - 0x40,
        _ => petscii, // Fallback
    }
}
