use crate::mcp::types::{McpError, McpRequest, McpResponse};
use crate::state::AppState;
use crate::state::types::BlockType;
use serde_json::{Value, json};

pub fn handle_request(req: &McpRequest, app_state: &mut AppState) -> McpResponse {
    let result = match req.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "regenerator2000-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {},
                "resources": {}
            }
        })),
        "notifications/initialized" => Ok(json!(true)),
        "tools/list" => list_tools(),
        "resources/list" => list_resources(),
        // Tools
        "tools/call" => handle_tool_call(&req.params, app_state),
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
                "name": "set_label_name",
                "description": "Set a label at a specific address",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer" },
                        "name": { "type": "string" }
                    },
                    "required": ["address", "name"]
                }
            },
            {
                "name": "set_side_comment",
                "description": "Set a side comment at a specific address",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer" },
                        "comment": { "type": "string" }
                    },
                    "required": ["address", "comment"]
                }
            },
             {
                "name": "set_line_comment",
                "description": "Set a line comment at a specific address",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "integer" },
                        "comment": { "type": "string" }
                    },
                    "required": ["address", "comment"]
                }
            },
            {
                "name": "convert_region_to_code",
                "description": "Mark a region as code",
                "inputSchema": region_schema()
            },
            {
                "name": "convert_region_to_bytes",
                "description": "Mark a region as data bytes",
                "inputSchema": region_schema()
            },
            {
                "name": "convert_region_to_words",
                "description": "Mark a region as data words",
                "inputSchema": region_schema()
            },
            {
                "name": "convert_region_to_petscii",
                "description": "Mark a region as PETSCII text",
                "inputSchema": region_schema()
            },
             {
                "name": "convert_region_to_screencode",
                "description": "Mark a region as screencode text",
                "inputSchema": region_schema()
            }
        ]
    }))
}

fn region_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "start_address": { "type": "integer" },
            "end_address": { "type": "integer" }
        },
        "required": ["start_address", "end_address"]
    })
}

fn list_resources() -> Result<Value, McpError> {
    Ok(json!({
        "resources": [
            {
                "uri": "disasm://main",
                "name": "Full Disassembly",
                "mimeType": "text/plain"
            },
            {
                "uri": "disasm://region/{start_address}/{end_address}",
                "name": "Disassembly Region",
                "mimeType": "text/plain"
            },
             {
                "uri": "hexdump://region/{start_address}/{end_address}",
                "name": "Hexdump Region",
                "mimeType": "text/plain"
            }
        ]
    }))
}

fn handle_tool_call(params: &Value, app_state: &mut AppState) -> Result<Value, McpError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError {
            code: -32602,
            message: "Missing 'name' in tools/call".to_string(),
            data: None,
        })?;

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match name {
        "set_label_name" => {
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
        "set_side_comment" | "set_line_comment" => {
            let address = get_address(&args, "address")?;
            let comment = args
                .get("comment")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let command = if name == "set_side_comment" {
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
        "convert_region_to_code" => convert_region(app_state, &args, BlockType::Code),
        "convert_region_to_bytes" => convert_region(app_state, &args, BlockType::DataByte),
        "convert_region_to_words" => convert_region(app_state, &args, BlockType::DataWord),
        "convert_region_to_petscii" => convert_region(app_state, &args, BlockType::PetsciiText),
        "convert_region_to_screencode" => {
            convert_region(app_state, &args, BlockType::ScreencodeText)
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
    let _range = start_idx..end_idx + 1; // inclusive end for Command logic if needed? 
    // Command::SetBlockType range is usually typical Rust range (start..end means end exclusive)
    // But let's check Command definition.
    // Viewed previously: range: std::ops::Range<usize>
    // And loop: for i in start..end

    // So if user says 1000 to 1000, they mean 1 byte.
    // So range should be start_idx .. end_idx + 1

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
    args.get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| McpError {
            code: -32602,
            message: format!("Missing or invalid '{}'", key),
            data: None,
        })
        .map(|v| v as u16)
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
    } else if uri.starts_with("disasm://region/") {
        let parts: Vec<&str> = uri.split('/').collect();
        // disasm://region/START/END
        if parts.len() < 5 {
            return Err(McpError {
                code: -32602,
                message: "Invalid URI format".to_string(),
                data: None,
            });
        }
        let start_addr = parts[3]
            .parse::<u16>()
            .ok()
            .or_else(|| u16::from_str_radix(parts[3].trim_start_matches("0x"), 16).ok())
            .ok_or(McpError {
                code: -32602,
                message: "Invalid start address".to_string(),
                data: None,
            })?;
        let end_addr = parts[4]
            .parse::<u16>()
            .ok()
            .or_else(|| u16::from_str_radix(parts[4].trim_start_matches("0x"), 16).ok())
            .ok_or(McpError {
                code: -32602,
                message: "Invalid end address".to_string(),
                data: None,
            })?;

        let text = get_disassembly_text(app_state, start_addr, end_addr);
        Ok(json!({
             "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": text
            }]
        }))
    } else if uri.starts_with("hexdump://region/") {
        let parts: Vec<&str> = uri.split('/').collect();
        if parts.len() < 5 {
            return Err(McpError {
                code: -32602,
                message: "Invalid URI format".to_string(),
                data: None,
            });
        }
        let start_addr = parts[3]
            .parse::<u16>()
            .ok()
            .or_else(|| u16::from_str_radix(parts[3].trim_start_matches("0x"), 16).ok())
            .ok_or(McpError {
                code: -32602,
                message: "Invalid start address".to_string(),
                data: None,
            })?;
        let end_addr = parts[4]
            .parse::<u16>()
            .ok()
            .or_else(|| u16::from_str_radix(parts[4].trim_start_matches("0x"), 16).ok())
            .ok_or(McpError {
                code: -32602,
                message: "Invalid end address".to_string(),
                data: None,
            })?;

        // Simple hexdump
        let mut output = String::new();
        let origin = app_state.origin;
        for addr in start_addr..=end_addr {
            if addr < origin || addr >= origin.wrapping_add(app_state.raw_data.len() as u16) {
                continue;
            }
            let idx = (addr - origin) as usize;
            let byte = app_state.raw_data[idx];
            if (addr - start_addr) % 16 == 0 {
                if addr != start_addr {
                    output.push('\n');
                }
                output.push_str(&format!("{:04X}: ", addr));
            }
            output.push_str(&format!("{:02X} ", byte));
        }

        Ok(json!({
             "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": output
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
    for line in &app_state.disassembly {
        if line.address >= start && line.address <= end {
            // Reconstruct line text roughly
            if !line.label.as_ref().is_none_or(|l| l.is_empty()) {
                output.push_str(&format!("{}:\n", line.label.as_ref().unwrap()));
            }
            output.push_str(&format!(
                "  {:04X}  {:20}  {}\n",
                line.address,
                bytes_to_str(&line.bytes),
                line.mnemonic
            ));
        }
    }
    output
}

fn bytes_to_str(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
