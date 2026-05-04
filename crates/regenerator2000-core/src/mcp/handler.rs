use crate::mcp::types::{McpError, McpRequest, McpResponse};
use crate::state::AppState;
use crate::state::types::{Addr, BlockType, ImmediateFormat};
use base64::prelude::*;
use serde_json::{Value, json};

use crate::view_state::CoreViewState;

pub fn handle_request(
    req: &McpRequest,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
) -> McpResponse {
    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "regenerator2000-core-mcp",
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
        "tools/call" => handle_tool_call(&req.params, app_state, view_state),
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
                "name": "r2000_add_scope",
                "description": "Adds a scope covering the specified memory range. Useful for a piece of code that is a routine. Starts a lexical level where all new symbols within this range are in the local lexical level and are accessible from outside only via explicit scope specification. Nested scopes are not supported.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "start_address": { "type": "integer", "description": "Start address of the scope (inclusive), decimal." },
                        "end_address":   { "type": "integer", "description": "End address of the scope (inclusive), decimal." }
                    },
                    "required": ["start_address", "end_address"]
                }
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
                "description": "Returns the origin address, size in bytes, target platform (e.g. 'Commodore 64'), filename, user-provided description, and whether the binary may contain undocumented opcodes (a hint, not guaranteed).",
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
                            "enum": ["text", "hex"],
                            "description": "Encoding for the query. 'text' searches both PETSCII and Screencode. 'hex' for raw byte patterns. Defaults to 'hex' if query looks like hex bytes, otherwise 'text'."
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
                "name": "r2000_get_symbols",
                "description": "Returns defined labels (user and/or platform) and their addresses. With no arguments returns ALL symbols. Provide optional filters to narrow results: 'names' resolves specific label names to addresses, 'start_address'/'end_address' limits to an address range, 'kind' filters by label kind. Filters are combined (AND logic).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "names": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of label names to look up. Only symbols whose name matches one of these strings are returned. Case-sensitive."
                        },
                        "start_address": { "type": "integer", "description": "Optional lower bound (inclusive) of the address range to filter by (decimal)." },
                        "end_address":   { "type": "integer", "description": "Optional upper bound (inclusive) of the address range to filter by (decimal)." },
                        "kind": {
                            "type": "string",
                            "enum": ["user", "platform", "auto"],
                            "description": "Optional filter to return only labels of a given kind. 'user' = user-defined labels, 'platform' = predefined platform labels (e.g. KERNAL, hardware registers), 'auto' = auto-generated labels (e.g. s_C000)."
                        }
                    }
                }
            },
            {
                "name": "r2000_get_comments",
                "description": "Returns user-defined comments and their addresses. Each entry has 'address' (integer), 'type' ('line' or 'side'), and 'comment' (string). With no arguments returns ALL comments. Provide optional filters to narrow results: 'addresses' returns comments at specific addresses, 'start_address'/'end_address' limits to an address range, 'type' filters by comment type. Filters are combined (AND logic).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "addresses": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "description": "Optional list of specific addresses (decimal) to retrieve comments from. Only comments at these addresses are returned."
                        },
                        "start_address": { "type": "integer", "description": "Optional lower bound (inclusive) of the address range to filter by (decimal)." },
                        "end_address":   { "type": "integer", "description": "Optional upper bound (inclusive) of the address range to filter by (decimal)." },
                        "type": {
                            "type": "string",
                            "enum": ["line", "side"],
                            "description": "Optional filter to return only 'line' comments or only 'side' comments."
                        }
                    }
                }
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
    view_state: &mut CoreViewState,
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

    handle_tool_call_internal(name, args, app_state, view_state)
}

fn handle_tool_call_internal(
    name: &str,
    args: Value,
    app_state: &mut AppState,
    view_state: &mut CoreViewState,
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

                match handle_tool_call_internal(tool_name, tool_args, app_state, view_state) {
                    Ok(val) => results.push(json!({ "status": "success", "result": val })),
                    Err(err) => results.push(json!({ "status": "error", "error": err })),
                }
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&results).unwrap_or_default()
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

            let command = app_state
                .create_set_user_label_command(address, label_name, false)
                .map_err(|msg| McpError {
                    code: -32602,
                    message: msg,
                    data: None,
                })?;

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
                .map(std::string::ToString::to_string)
                .ok_or_else(|| McpError {
                    code: -32602,
                    message: "Missing or invalid 'comment' (expected a string)".to_string(),
                    data: None,
                })?;
            let comment_type =
                args.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError {
                        code: -32602,
                        message: "Missing 'type' (expected \"line\" or \"side\")".to_string(),
                        data: None,
                    })?;

            let command = match comment_type {
                "side" => crate::commands::Command::SetUserSideComment {
                    address,
                    new_comment: Some(comment),
                    old_comment: app_state.user_side_comments.get(&address).cloned(),
                },
                "line" => crate::commands::Command::SetUserLineComment {
                    address,
                    new_comment: Some(comment),
                    old_comment: app_state.user_line_comments.get(&address).cloned(),
                },
                other => {
                    return Err(McpError {
                        code: -32602,
                        message: format!(
                            "Invalid 'type' value \"{other}\": expected \"line\" or \"side\""
                        ),
                        data: None,
                    });
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
                        message: format!("Unknown data_type: '{data_type_str}'"),
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

        "r2000_add_scope" => {
            let start_addr = get_address(&args, "start_address")?;
            let end_addr = get_address(&args, "end_address")?;

            if start_addr > end_addr {
                return Err(McpError {
                    code: -32602,
                    message: "start_address must be <= end_address".to_string(),
                    data: None,
                });
            }

            let mut overlaps = false;
            for (&s, &e) in &app_state.scopes {
                if start_addr <= e && end_addr >= s {
                    overlaps = true;
                    break;
                }
            }

            if overlaps {
                return Err(McpError {
                    code: -32602,
                    message: "Cannot create scope: overlaps with an existing scope".to_string(),
                    data: None,
                });
            }

            let mut commands = Vec::new();

            // Generate a default label for the scope if one does not exist
            let has_label = app_state
                .labels
                .get(&start_addr)
                .is_some_and(|l| !l.is_empty());
            if !has_label {
                let label = crate::state::Label {
                    name: format!("scope_{:04X}", start_addr.0),
                    kind: crate::state::LabelKind::User,
                    label_type: crate::state::LabelType::UserDefined,
                };
                commands.push(crate::commands::Command::SetLabel {
                    address: start_addr,
                    new_label: Some(vec![label]),
                    old_label: None,
                });
            }

            let old_end = app_state.scopes.get(&start_addr).copied();
            commands.push(crate::commands::Command::AddScope {
                start: start_addr,
                end: end_addr,
                old_end,
            });

            let command = if commands.len() == 1 {
                commands.remove(0)
            } else {
                crate::commands::Command::Batch(commands)
            };

            command.apply(app_state);

            let (analysis_cmd, msg) = app_state.perform_analysis();
            app_state.push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));
            app_state.disassemble();

            Ok(
                json!({ "content": [{ "type": "text", "text": format!("Added Scope from ${:04X} to ${:04X}. {}", start_addr.0, end_addr.0, msg) }] }),
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
            let text = match view {
                "disasm" => get_disassembly_text(app_state, start_addr, end_addr),
                "hexdump" => get_hexdump_text(app_state, start_addr, end_addr),
                other => {
                    return Err(McpError {
                        code: -32602,
                        message: format!(
                            "Invalid 'view' value \"{other}\": expected \"disasm\" or \"hexdump\""
                        ),
                        data: None,
                    });
                }
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }] }))
        }

        "r2000_read_selected" => {
            let view = args
                .get("view")
                .and_then(|v| v.as_str())
                .unwrap_or("disasm");
            let text = match view {
                "disasm" => {
                    let (start, end) = get_selection_range_disasm(app_state, view_state)?;
                    get_disassembly_text(app_state, start, end)
                }
                "hexdump" => {
                    let (start, end) = get_selection_range_hexdump(app_state, view_state)?;
                    get_hexdump_text(app_state, start, end)
                }
                other => {
                    return Err(McpError {
                        code: -32602,
                        message: format!(
                            "Invalid 'view' value \"{other}\": expected \"disasm\" or \"hexdump\""
                        ),
                        data: None,
                    });
                }
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
                .map(std::string::ToString::to_string);

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&json!({
                        "origin": origin,
                        "size": size,
                        "platform": platform,
                        "filename": filename,
                        "description": app_state.settings.description,
                        "may_contain_undocumented_opcodes": app_state.settings.use_illegal_opcodes
                    })).unwrap_or_default()
                }]
            }))
        }

        "r2000_get_analyzed_blocks" => {
            let filter = args.get("block_type").and_then(|v| v.as_str());
            let blocks = get_analyzed_blocks_impl(app_state, filter);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&blocks).unwrap_or_default()
                }]
            }))
        }

        "r2000_get_address_details" => {
            let address = get_address(&args, "address")?;
            let details = get_address_details_impl(app_state, address)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&details).unwrap_or_default()
                }]
            }))
        }

        "r2000_get_disassembly_cursor" => {
            let idx = view_state.cursor_index;
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
                crate::navigation::perform_jump_to_address(app_state, view_state, address);

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
                        "Address ${address:04X} not found in disassembly (might be hidden or invalid)"
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
            let matches = crate::state::search::search_memory_raw(app_state, query, encoding, 100)
                .map_err(|msg| McpError {
                    code: -32602,
                    message: msg,
                    data: None,
                })?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&matches).unwrap_or_default()
                }]
            }))
        }

        "r2000_get_cross_references" => {
            let address = get_address(&args, "address")?;
            let refs = get_cross_references_impl(app_state, address);
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&refs).unwrap_or_default()
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

        "r2000_get_symbols" => {
            let symbols = get_symbols_impl(app_state, &args)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&symbols).unwrap_or_default()
                }]
            }))
        }

        "r2000_get_comments" => {
            let comments = get_comments_impl(app_state, &args)?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&comments).unwrap_or_default()
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

            let ctx = crate::navigation::create_save_context(app_state, view_state);
            app_state.save_project(ctx, true).map_err(|e| McpError {
                code: -32603,
                message: format!("Failed to save project: {e}"),
                data: None,
            })?;

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Project saved to {}", app_state.project_path.as_ref().map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string()))
                }]
            }))
        }

        _ => Err(McpError {
            code: -32601,
            message: format!("Tool not found: {name}"),
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

    // Bounds check — use is_external() which correctly handles wrapping around u16::MAX
    if app_state.is_external(start_addr) || app_state.is_external(end_addr) {
        return Err(McpError {
            code: -32602,
            message: format!(
                "Region ${start_addr:04X}-${end_addr:04X} out of bounds (Origin: ${origin:04X})"
            ),
            data: None,
        });
    }

    let start_idx = start_addr.offset_from(origin);
    let end_idx = end_addr.offset_from(origin);

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

fn get_address(args: &Value, key: &str) -> Result<Addr, McpError> {
    let val = args.get(key).ok_or_else(|| McpError {
        code: -32602,
        message: format!("Missing '{key}'"),
        data: None,
    })?;

    if let Some(n) = val.as_u64() {
        return Ok(Addr(n as u16));
    }

    // Still accept string formats for robustness / backwards compat with older clients
    if let Some(s) = val.as_str()
        && let Some(addr) = parse_address_string(s)
    {
        return Ok(Addr(addr));
    }

    Err(McpError {
        code: -32602,
        message: format!("Invalid address format for '{key}'. Expected a decimal integer."),
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

fn get_disassembly_text(app_state: &AppState, start: Addr, end: Addr) -> String {
    let mut output = String::new();
    output.push_str(&format!("* = ${start:04X}\n"));

    for line in &app_state.disassembly {
        if line.address >= start && line.address <= end {
            // Line comments
            if let Some(comment) = &line.line_comment {
                for line in comment.lines() {
                    output.push_str(&format!("; {line}\n"));
                }
            }

            if let Some(label) = &line.label
                && !label.is_empty()
            {
                output.push_str(&format!("{label}:\n"));
            }

            let instruction = if line.operand.is_empty() {
                line.mnemonic.clone()
            } else {
                format!("{} {}", line.mnemonic, line.operand)
            };

            // Side comments
            if line.comment.is_empty() {
                output.push_str(&format!("${:04X} {}\n", line.address, instruction));
            } else {
                output.push_str(&format!(
                    "${:04X} {:<20} ; {}\n",
                    line.address, instruction, line.comment
                ));
            }
        }
    }
    output
}

fn get_selection_range_disasm(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Result<(Addr, Addr), McpError> {
    let cursor_idx = view_state.cursor_index;
    let selection_idx = view_state.selection_start;

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
    view_state: &CoreViewState,
) -> Result<(Addr, Addr), McpError> {
    let cursor_row = view_state.hex_cursor_index;
    let selection_row = view_state.hex_selection_start;

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

    let alignment_padding = (origin.0 % 16) as usize;
    let aligned_origin = (origin.0 as usize) - alignment_padding;

    let start_addr = Addr((aligned_origin + start_row * bytes_per_row) as u16);
    let end_addr = Addr((aligned_origin + (end_row + 1) * bytes_per_row - 1) as u16);

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

fn get_hexdump_text(app_state: &AppState, start_addr: Addr, end_addr: Addr) -> String {
    let mut output = String::new();
    let origin = app_state.origin;
    for addr_val in start_addr.0..=end_addr.0 {
        let addr = Addr(addr_val);
        if addr < origin || addr >= origin.wrapping_add(app_state.raw_data.len() as u16) {
            continue;
        }
        let idx = addr.offset_from(origin);
        let byte = app_state.raw_data[idx];
        if (addr.0.wrapping_sub(start_addr.0)).is_multiple_of(16) {
            if addr != start_addr {
                output.push('\n');
            }
            output.push_str(&format!("${addr:04X}: "));
        }
        output.push_str(&format!("{byte:02X} "));
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

fn get_cross_references_impl(app_state: &AppState, address: Addr) -> Vec<Addr> {
    if let Some(refs) = app_state.cross_refs.get(&address) {
        let mut sorted_refs = refs.clone();
        sorted_refs.sort_unstable();
        sorted_refs.dedup();
        sorted_refs
    } else {
        Vec::new()
    }
}

fn set_operand_format_impl(
    app_state: &mut AppState,
    address: Addr,
    format_str: &str,
) -> Result<(), McpError> {
    let format = match format_str.to_lowercase().as_str() {
        "hex" => ImmediateFormat::Hex,
        "decimal" | "dec" => ImmediateFormat::Decimal,
        "binary" | "bin" => ImmediateFormat::Binary,
        _ => {
            return Err(McpError {
                code: -32602,
                message: format!("Unknown format: {format_str}"),
                data: None,
            });
        }
    };

    let command = crate::commands::Command::SetImmediateFormat {
        address,
        new_format: Some(format),
        old_format: app_state.immediate_value_formats.get(&address).copied(),
    };

    command.apply(app_state);
    app_state.push_command(command);
    app_state.disassemble();

    Ok(())
}

fn get_symbols_impl(app_state: &AppState, args: &Value) -> Result<Vec<Value>, McpError> {
    // Parse optional filters
    let name_filter: Option<Vec<&str>> = args
        .get("names")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());

    let start_addr = args
        .get("start_address")
        .and_then(|v| v.as_u64())
        .map(|n| Addr(n as u16));
    let end_addr = args
        .get("end_address")
        .and_then(|v| v.as_u64())
        .map(|n| Addr(n as u16));

    let kind_filter = match args.get("kind").and_then(|v| v.as_str()) {
        Some("user") => Some("user"),
        Some("platform") | Some("system") => Some("platform"),
        Some("auto") => Some("auto"),
        Some(other) => {
            return Err(McpError {
                code: -32602,
                message: format!(
                    "Invalid 'kind' value \"{other}\": expected \"user\", \"platform\", or \"auto\""
                ),
                data: None,
            });
        }
        None => None,
    };

    // Validate: if one range bound is given, both must be present
    if start_addr.is_some() != end_addr.is_some() {
        return Err(McpError {
            code: -32602,
            message: "Both 'start_address' and 'end_address' must be provided together."
                .to_string(),
            data: None,
        });
    }
    if let (Some(s), Some(e)) = (start_addr, end_addr)
        && s > e
    {
        return Err(McpError {
            code: -32602,
            message: "start_address must be <= end_address".to_string(),
            data: None,
        });
    }

    let mut symbols = Vec::new();
    for (addr, labels) in &app_state.labels {
        // Address range filter
        if let (Some(s), Some(e)) = (start_addr, end_addr)
            && (*addr < s || *addr > e)
        {
            continue;
        }

        for label in labels {
            // Kind filter
            if let Some(k) = kind_filter {
                let label_kind = format!("{:?}", label.kind);
                if !label_kind.eq_ignore_ascii_case(k) {
                    continue;
                }
            }

            // Name filter
            if let Some(ref names) = name_filter
                && !names.contains(&label.name.as_str())
            {
                continue;
            }

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
        let addr_a = a["address"].as_u64().unwrap_or(0);
        let addr_b = b["address"].as_u64().unwrap_or(0);
        addr_a.cmp(&addr_b)
    });
    Ok(symbols)
}

fn get_comments_impl(app_state: &AppState, args: &Value) -> Result<Vec<Value>, McpError> {
    // Parse optional filters
    let addr_filter: Option<Vec<Addr>> =
        args.get("addresses").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64())
                .map(|n| Addr(n as u16))
                .collect()
        });

    let start_addr = args
        .get("start_address")
        .and_then(|v| v.as_u64())
        .map(|n| Addr(n as u16));
    let end_addr = args
        .get("end_address")
        .and_then(|v| v.as_u64())
        .map(|n| Addr(n as u16));

    let type_filter = match args.get("type").and_then(|v| v.as_str()) {
        Some("line") => Some("line"),
        Some("side") => Some("side"),
        Some(other) => {
            return Err(McpError {
                code: -32602,
                message: format!("Invalid 'type' value \"{other}\": expected \"line\" or \"side\""),
                data: None,
            });
        }
        None => None,
    };

    // Validate: if one range bound is given, both must be present
    if start_addr.is_some() != end_addr.is_some() {
        return Err(McpError {
            code: -32602,
            message: "Both 'start_address' and 'end_address' must be provided together."
                .to_string(),
            data: None,
        });
    }
    if let (Some(s), Some(e)) = (start_addr, end_addr)
        && s > e
    {
        return Err(McpError {
            code: -32602,
            message: "start_address must be <= end_address".to_string(),
            data: None,
        });
    }

    let mut comments = Vec::new();

    // Helper closure to check whether an address passes the filters
    let addr_passes = |addr: &Addr| -> bool {
        if let (Some(s), Some(e)) = (start_addr, end_addr)
            && (*addr < s || *addr > e)
        {
            return false;
        }
        if let Some(ref addrs) = addr_filter
            && !addrs.contains(addr)
        {
            return false;
        }
        true
    };

    if type_filter != Some("side") {
        for (addr, comment) in &app_state.user_line_comments {
            if addr_passes(addr) {
                comments.push(json!({
                    "address": addr,
                    "type": "line",
                    "comment": comment
                }));
            }
        }
    }

    if type_filter != Some("line") {
        for (addr, comment) in &app_state.user_side_comments {
            if addr_passes(addr) {
                comments.push(json!({
                    "address": addr,
                    "type": "side",
                    "comment": comment
                }));
            }
        }
    }

    // Sort by address
    comments.sort_by(|a, b| {
        let addr_a = a["address"].as_u64().unwrap_or(0);
        let addr_b = b["address"].as_u64().unwrap_or(0);
        addr_a.cmp(&addr_b)
    });

    Ok(comments)
}

fn get_address_details_impl(app_state: &AppState, address: Addr) -> Result<Value, McpError> {
    let origin = app_state.origin;
    if address < origin || address >= origin.wrapping_add(app_state.raw_data.len() as u16) {
        return Ok(json!({
            "address": address,
            "type": "OutOfRange",
            "message": "Address is outside the loaded binary range."
        }));
    }

    let idx = address.offset_from(origin);
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
                            Some(u16::from(line.bytes[2]) << 8 | u16::from(line.bytes[1]))
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::ZeroPage
                    | crate::cpu::AddressingMode::ZeroPageX
                    | crate::cpu::AddressingMode::ZeroPageY => {
                        if line.bytes.len() >= 2 {
                            Some(u16::from(line.bytes[1]))
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::Indirect => {
                        if line.bytes.len() >= 3 {
                            Some(u16::from(line.bytes[2]) << 8 | u16::from(line.bytes[1]))
                        } else {
                            None
                        }
                    }
                    crate::cpu::AddressingMode::IndirectX
                    | crate::cpu::AddressingMode::IndirectY => {
                        if line.bytes.len() >= 2 {
                            Some(u16::from(line.bytes[1]))
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
        comments.push(format!("[User Line] {c}"));
    }
    if let Some(c) = app_state.user_side_comments.get(&address) {
        comments.push(format!("[User Side] {c}"));
    }
    if let Some(c) = app_state.platform_comments.get(&address) {
        comments.push(format!("[Platform] {c}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::BlockType;

    /// Create a minimal AppState with the given origin and data size.
    fn make_app_state(origin: u16, size: usize) -> AppState {
        let mut state = AppState::new();
        state.origin = Addr(origin);
        state.raw_data = vec![0u8; size];
        state.block_types = vec![BlockType::Code; size];
        state.disassemble();
        state
    }

    fn make_view_state() -> CoreViewState {
        CoreViewState::new()
    }

    // -----------------------------------------------------------------------
    // convert_region bounds tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_convert_region_valid_range() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x1000, "end_address": 0x10FF});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_ok(), "Expected Ok, got: {result:?}");
    }

    #[test]
    fn test_convert_region_single_byte() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x1000, "end_address": 0x1000});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_ok(), "Expected Ok, got: {result:?}");
    }

    #[test]
    fn test_convert_region_last_byte() {
        let mut app_state = make_app_state(0x1000, 256);
        // Last valid address is 0x1000 + 255 = 0x10FF
        let args = json!({"start_address": 0x10FF, "end_address": 0x10FF});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_ok(), "Expected Ok, got: {result:?}");
    }

    #[test]
    fn test_convert_region_out_of_bounds_below_origin() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x0FFF, "end_address": 0x1010});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_err(), "Expected Err for start below origin");
    }

    #[test]
    fn test_convert_region_out_of_bounds_above_end() {
        let mut app_state = make_app_state(0x1000, 256);
        // 0x1100 is one byte past the end
        let args = json!({"start_address": 0x1000, "end_address": 0x1100});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_err(), "Expected Err for end past binary");
    }

    #[test]
    fn test_convert_region_completely_outside() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x2000, "end_address": 0x2010});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_err(), "Expected Err for range completely outside");
    }

    #[test]
    fn test_convert_region_reversed_range() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x1010, "end_address": 0x1000});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_err(), "Expected Err for start > end");
    }

    #[test]
    fn test_convert_region_wrapping_origin_valid() {
        // Binary at $FF00, 256 bytes → wraps around to $0000
        let mut app_state = make_app_state(0xFF00, 256);
        // Address at the start of the binary
        let args = json!({"start_address": 0xFF00, "end_address": 0xFF0F});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(
            result.is_ok(),
            "Expected Ok for valid range in wrapping binary, got: {result:?}"
        );
    }

    #[test]
    fn test_convert_region_wrapping_origin_outside() {
        // Binary at $FF00, 256 bytes → valid range $FF00-$FFFF
        let mut app_state = make_app_state(0xFF00, 256);
        // $FE00 is before the origin
        let args = json!({"start_address": 0xFE00, "end_address": 0xFE10});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(
            result.is_err(),
            "Expected Err for range before wrapping origin"
        );
    }

    #[test]
    fn test_convert_region_origin_zero_valid() {
        // Binary loaded at $0000 (e.g. a raw .bin file)
        let mut app_state = make_app_state(0x0000, 1024);
        let args = json!({"start_address": 0x0000, "end_address": 0x03FF});
        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(
            result.is_ok(),
            "Expected Ok for origin=0 binary, got: {result:?}"
        );
    }

    #[test]
    fn test_convert_region_applies_block_type() {
        let mut app_state = make_app_state(0x1000, 256);
        let args = json!({"start_address": 0x1010, "end_address": 0x101F});

        let result = convert_region(&mut app_state, &args, BlockType::DataByte);
        assert!(result.is_ok());

        // Verify block types were actually changed
        for i in 0x10..=0x1F {
            assert_eq!(app_state.block_types[i], BlockType::DataByte);
        }
        // Verify surrounding bytes are untouched
        assert_eq!(app_state.block_types[0x0F], BlockType::Code);
        assert_eq!(app_state.block_types[0x20], BlockType::Code);
    }

    // -----------------------------------------------------------------------
    // handle_tool_call_internal: set_data_type tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_data_type_all_types() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        let types = [
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
            "undefined",
        ];

        for data_type in &types {
            let args = json!({
                "start_address": 0x1000,
                "end_address": 0x100F,
                "data_type": data_type
            });
            let result = handle_tool_call_internal(
                "r2000_set_data_type",
                args,
                &mut app_state,
                &mut view_state,
            );
            assert!(
                result.is_ok(),
                "Expected Ok for data_type={data_type}, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_set_data_type_unknown_type_returns_error() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        let args = json!({
            "start_address": 0x1000,
            "end_address": 0x100F,
            "data_type": "invalid_type_xyz"
        });
        let result =
            handle_tool_call_internal("r2000_set_data_type", args, &mut app_state, &mut view_state);
        assert!(result.is_err(), "Expected Err for unknown data_type");
    }

    #[test]
    fn test_set_data_type_missing_data_type() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();

        let args = json!({
            "start_address": 0x1000,
            "end_address": 0x100F
        });
        let result =
            handle_tool_call_internal("r2000_set_data_type", args, &mut app_state, &mut view_state);
        assert!(result.is_err(), "Expected Err when data_type is missing");
    }
}
