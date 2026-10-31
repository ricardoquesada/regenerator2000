//! MCP tool handlers for memory block classification, symbol management, cross-references, scope analysis, searching, and region reading.
//!
//! This module provides tool definitions and execution handlers for analyzing memory ranges,
//! extracting instructions/annotations at specific addresses, querying defined symbols, managing routine scopes,
//! searching memory bytes or disassembly text, and fetching disassembly or hexdump text representation
//! for memory regions or active selections.

use crate::config::SystemConfig;
use crate::mcp::tools::common::{get_address, make_mcp_error};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::Addr;
use crate::view_state::CoreViewState;
use serde_json::{Value, json};

/// Returns JSON schemas for all analysis, memory metadata, searching, and region reading tools.
///
/// # Arguments
///
/// * `system_config` - Reference to system configuration [`SystemConfig`].
///
/// # Returns
///
/// A vector of JSON [`Value`] objects defining the schemas for `r2000_get_binary_info`,
/// `r2000_get_comments`, `r2000_get_blocks`, `r2000_get_address_details`, `r2000_get_symbols`,
/// `r2000_get_cross_references`, `r2000_add_scope`, `r2000_search_memory`, `r2000_search_disassembly`,
/// `r2000_read_region`, and `r2000_read_selected`.
#[must_use]
pub fn tool_schemas(system_config: &SystemConfig) -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_get_binary_info",
            "description": format!(
                "Returns origin address, size, target platform, filename, description, entropy (>{} suggests compression), and illegal opcodes hint.",
                system_config.entropy_threshold
            ),
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_get_comments",
            "description": "Returns user-defined comments and their addresses with optional filters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "addresses": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "Optional list of memory addresses to query comments for (decimal)."
                    },
                    "start_address": {
                        "type": "integer",
                        "description": "Optional lower bound (inclusive) of the address range to filter by (decimal)."
                    },
                    "end_address": {
                        "type": "integer",
                        "description": "Optional upper bound (inclusive) of the address range to filter by (decimal)."
                    },
                    "type": {
                        "type": "string",
                        "enum": ["line", "side"],
                        "description": "Optional filter for comment type ('line' or 'side')."
                    }
                }
            }
        }),
        json!({
            "name": "r2000_get_blocks",
            "description": "Returns all memory blocks with their address range and type (Code, Byte, Word, Address, PETSCII, Screencode, Lo/Hi Address, Hi/Lo Address, Lo/Hi Word, Hi/Lo Word, External File, Undefined). Respects splitters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "block_type": {
                        "type": "string",
                        "description": "Optional filter to return only blocks of a specific type. Case-insensitive."
                    }
                }
            }
        }),
        json!({
            "name": "r2000_get_address_details",
            "description": "Returns detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The memory address to inspect (decimal)." }
                },
                "required": ["address"]
            }
        }),
        json!({
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
                        "enum": ["user", "system", "auto"],
                        "description": "Optional filter to return only labels of a given kind. 'user' = user-defined labels, 'system' = predefined system labels (e.g. KERNAL, hardware registers), 'auto' = auto-generated labels (e.g. s_C000)."
                    }
                }
            }
        }),
        json!({
            "name": "r2000_get_cross_references",
            "description": "Get a list of addresses that reference the given address (e.g. JSRs, JMPs, loads).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address": { "type": "integer", "description": "The target address to find references to (decimal)." }
                },
                "required": ["address"]
            }
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
            "name": "r2000_search_disassembly",
            "description": "Search the disassembly text for a query string or regular expression. Returns a list of matching addresses with context (label, mnemonic, operand, comment). Searches labels, comments, and instructions by default; individual fields can be disabled.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query. Interpreted as a plain case-insensitive substring by default, or as a regex when 'use_regex' is true."
                    },
                    "use_regex": {
                        "type": "boolean",
                        "description": "When true the query is compiled as a case-insensitive regular expression ((?i) is prepended automatically). Defaults to false."
                    },
                    "search_labels": {
                        "type": "boolean",
                        "description": "Include label names in the search. Defaults to true."
                    },
                    "search_comments": {
                        "type": "boolean",
                        "description": "Include side and line comments in the search. Defaults to true."
                    },
                    "search_instructions": {
                        "type": "boolean",
                        "description": "Include mnemonic and operand text in the search. Defaults to true."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matching addresses to return. Defaults to 50."
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
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
        }),
        json!({
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
        }),
    ]
}

/// Handles execution of analysis and metadata tool calls.
///
/// # Arguments
///
/// * `name` - The tool identifier string matching one of the analysis tool names.
/// * `args` - The JSON [`Value`] payload holding the tool's arguments.
/// * `app_state` - Mutable reference to the application state [`AppState`].
/// * `view_state` - Reference to view state [`CoreViewState`].
///
/// # Returns
///
/// `Some(Ok(value))` on successful execution, `Some(Err([`McpError`]))` if parameter validation, search, memory reading, or address details retrieval fails, or `None` if `name` is not an analysis tool.
///
/// # Errors
///
/// Returns [`McpError`] if parameter validation, search, memory reading, or address details retrieval fails.
pub fn handle_call(
    name: &str,
    args: &Value,
    app_state: &mut AppState,
    view_state: &CoreViewState,
) -> Option<Result<Value, McpError>> {
    match name {
        "r2000_get_binary_info" => Some(Ok(get_binary_info_impl(app_state))),
        "r2000_get_comments" => Some(get_comments_impl(app_state, args).map(|comments| {
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&comments).unwrap_or_default()
                }]
            })
        })),
        "r2000_get_blocks" => {
            let filter = args.get("block_type").and_then(|v| v.as_str());
            let blocks = get_blocks_impl(app_state, filter);
            Some(Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&blocks).unwrap_or_default()
                }]
            })))
        }
        "r2000_get_address_details" => {
            let address = match get_address(args, "address") {
                Ok(addr) => addr,
                Err(err) => return Some(Err(err)),
            };
            Some(get_address_details_impl(app_state, address).map(|details| {
                json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&details).unwrap_or_default()
                    }]
                })
            }))
        }
        "r2000_get_symbols" => Some(get_symbols_impl(app_state, args).map(|symbols| {
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&symbols).unwrap_or_default()
                }]
            })
        })),
        "r2000_get_cross_references" => {
            let address = match get_address(args, "address") {
                Ok(addr) => addr,
                Err(err) => return Some(Err(err)),
            };
            let refs = get_cross_references_impl(app_state, address);
            Some(Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&refs).unwrap_or_default()
                }]
            })))
        }
        "r2000_add_scope" => Some(add_scope_impl(app_state, args)),
        "r2000_search_memory" => Some(search_memory_impl(app_state, args)),
        "r2000_search_disassembly" => Some(search_disassembly_impl(app_state, args)),
        "r2000_read_region" => Some(read_region_impl(app_state, args)),
        "r2000_read_selected" => Some(read_selected_impl(app_state, view_state, args)),
        _ => None,
    }
}

/// Private helper to format binary metadata information into a JSON response payload.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
///
/// # Returns
///
/// A JSON [`Value`] containing origin, size, system target, filename, description, undocumented opcodes setting, and entropy score.
fn get_binary_info_impl(app_state: &AppState) -> Value {
    let origin = app_state.origin;
    let size = app_state.raw_data.len();
    let system = &app_state.settings.system;
    let filename = app_state
        .file_path
        .as_ref()
        .or(app_state.project_path.as_ref())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(String::from);

    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&json!({
                "origin": origin,
                "size": size,
                "system": system,
                "filename": filename,
                "description": app_state.settings.description,
                "may_contain_undocumented_opcodes": app_state.settings.use_illegal_opcodes,
                "entropy": app_state.entropy()
            })).unwrap_or_default()
        }]
    })
}

/// Helper function to retrieve user line and side comments with optional address and type filters.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing optional `addresses`, `start_address`, `end_address`, and `type`.
///
/// # Returns
///
/// A vector of JSON [`Value`] objects containing comment data (address, type, comment text).
///
/// # Errors
///
/// Returns [`McpError`] if `type` is invalid or if `start_address` / `end_address` are invalid.
pub(crate) fn get_comments_impl(
    app_state: &AppState,
    args: &Value,
) -> Result<Vec<Value>, McpError> {
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
            return Err(make_mcp_error(
                -32602,
                format!("Invalid 'type' value \"{other}\": expected \"line\" or \"side\""),
            ));
        }
        None => None,
    };

    if start_addr.is_some() != end_addr.is_some() {
        return Err(make_mcp_error(
            -32602,
            "Both 'start_address' and 'end_address' must be provided together.".to_string(),
        ));
    }
    if let (Some(s), Some(e)) = (start_addr, end_addr)
        && s > e
    {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address".to_string(),
        ));
    }

    let mut comments = Vec::new();
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
        for (addr, entry) in app_state.annotations.iter() {
            if let Some(ref comment) = entry.user_line_comment
                && addr_passes(&addr)
            {
                comments.push(json!({
                    "address": addr,
                    "type": "line",
                    "comment": comment
                }));
            }
        }
    }

    if type_filter != Some("line") {
        for (addr, entry) in app_state.annotations.iter() {
            if let Some(ref comment) = entry.user_side_comment
                && addr_passes(&addr)
            {
                comments.push(json!({
                    "address": addr,
                    "type": "side",
                    "comment": comment
                }));
            }
        }
    }

    comments.sort_by(|a, b| {
        let addr_a = a["address"].as_u64().unwrap_or(0);
        let addr_b = b["address"].as_u64().unwrap_or(0);
        addr_a.cmp(&addr_b)
    });

    Ok(comments)
}

/// Helper function to retrieve contiguous classified memory blocks.
///
/// # Arguments
///
/// * `app_state` - Reference to the application state [`AppState`].
/// * `filter` - Optional string slice to filter blocks by type name (case-insensitive).
///
/// # Returns
///
/// A vector of JSON objects containing `start_address`, `end_address`, and `type`.
pub(crate) fn get_blocks_impl(app_state: &AppState, filter: Option<&str>) -> Vec<Value> {
    let mut blocks = Vec::new();
    let origin = app_state.origin;
    let max_len = app_state.block_types.len();

    if max_len == 0 {
        return blocks;
    }

    let mut start_idx = 0;
    let mut current_type = app_state.block_types[0];

    for i in 1..max_len {
        let offset = u16::try_from(i).unwrap_or(u16::MAX);
        let addr = origin.wrapping_add(offset);
        let type_ = app_state.block_types[i];
        let is_splitter = app_state.splitters.contains(&addr);

        if type_ != current_type || is_splitter {
            let end_idx = i - 1;
            let start_offset = u16::try_from(start_idx).unwrap_or(u16::MAX);
            let end_offset = u16::try_from(end_idx).unwrap_or(u16::MAX);
            let start_addr = origin.wrapping_add(start_offset);
            let end_addr = origin.wrapping_add(end_offset);

            let should_include = match filter {
                Some(f) => current_type.matches_filter(f),
                None => true,
            };

            if should_include {
                blocks.push(json!({
                    "start_address": start_addr,
                    "end_address": end_addr,
                    "type": current_type.to_string()
                }));
            }

            start_idx = i;
            current_type = type_;
        }
    }

    let end_idx = max_len - 1;
    let start_offset = u16::try_from(start_idx).unwrap_or(u16::MAX);
    let end_offset = u16::try_from(end_idx).unwrap_or(u16::MAX);
    let start_addr = origin.wrapping_add(start_offset);
    let end_addr = origin.wrapping_add(end_offset);
    let should_include = match filter {
        Some(f) => current_type.matches_filter(f),
        None => true,
    };

    if should_include {
        blocks.push(json!({
            "start_address": start_addr,
            "end_address": end_addr,
            "type": current_type.to_string()
        }));
    }

    blocks
}

/// Helper function to retrieve cross-references referencing a specific memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to the application state [`AppState`].
/// * `address` - Target address [`Addr`] to query references for.
///
/// # Returns
///
/// A sorted and deduplicated vector of referencing [`Addr`] locations.
pub(crate) fn get_cross_references_impl(app_state: &AppState, address: Addr) -> Vec<Addr> {
    if let Some(refs) = app_state.cross_refs.get(&address) {
        let mut sorted_refs = refs.clone();
        sorted_refs.sort_unstable();
        sorted_refs.dedup();
        sorted_refs
    } else {
        Vec::new()
    }
}

/// Helper function to query symbols with optional filtering by name, address range, or kind.
///
/// # Arguments
///
/// * `app_state` - Reference to the application state [`AppState`].
/// * `args` - JSON [`Value`] containing optional `names`, `start_address`, `end_address`, and `kind` parameters.
///
/// # Returns
///
/// A vector of JSON [`Value`] objects containing symbol definitions (name, address, kind).
///
/// # Errors
///
/// Returns [`McpError`] if `kind` is unrecognized or if `start_address` / `end_address` are invalid.
pub(crate) fn get_symbols_impl(app_state: &AppState, args: &Value) -> Result<Vec<Value>, McpError> {
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
        Some("system") | Some("platform") => Some("System"),
        Some("auto") => Some("auto"),
        Some(other) => {
            return Err(make_mcp_error(
                -32602,
                format!(
                    "Invalid 'kind' value \"{other}\": expected \"user\", \"system\", or \"auto\""
                ),
            ));
        }
        None => None,
    };

    if start_addr.is_some() != end_addr.is_some() {
        return Err(make_mcp_error(
            -32602,
            "Both 'start_address' and 'end_address' must be provided together.".to_string(),
        ));
    }
    if let (Some(s), Some(e)) = (start_addr, end_addr)
        && s > e
    {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address".to_string(),
        ));
    }

    let mut symbols = Vec::new();
    for (addr, labels) in &app_state.labels {
        if let (Some(s), Some(e)) = (start_addr, end_addr)
            && (*addr < s || *addr > e)
        {
            continue;
        }

        for label in labels {
            if let Some(k) = kind_filter {
                let label_kind = format!("{:?}", label.kind);
                if !label_kind.eq_ignore_ascii_case(k) {
                    continue;
                }
            }

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

    symbols.sort_by(|a, b| {
        let addr_a = a["address"].as_u64().unwrap_or(0);
        let addr_b = b["address"].as_u64().unwrap_or(0);
        addr_a.cmp(&addr_b)
    });

    Ok(symbols)
}

/// Helper function to retrieve detailed metadata for a single memory address [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to the application state [`AppState`].
/// * `address` - Target memory address [`Addr`] to inspect.
///
/// # Returns
///
/// A JSON [`Value`] containing detailed memory address metadata (semantics, labels, comments, cross-references, block type).
///
/// # Errors
///
/// Returns [`McpError`] if `address` is outside loaded binary boundaries.
pub(crate) fn get_address_details_impl(
    app_state: &AppState,
    address: Addr,
) -> Result<Value, McpError> {
    if app_state.is_external(address) {
        return Err(make_mcp_error(
            -32602,
            format!("Address ${address:04X} is outside loaded binary range"),
        ));
    }

    let idx = address.offset_from(app_state.origin);
    let block_type = app_state
        .block_types
        .get(idx)
        .map_or_else(|| "Unknown".to_string(), |b| b.to_string());

    let mut details = json!({
        "address": address,
        "type": block_type,
        "block_type": block_type,
        "metadata": {}
    });

    if let Some(line) = app_state.disassembly.iter().find(|l| l.address == address) {
        details["instruction"] = json!({
            "bytes": line.bytes,
            "mnemonic": line.mnemonic.to_uppercase(),
            "op_code": line.bytes.first().map(|b| format!("${b:02X}")),
            "operands": line.operand,
        });

        if let Some(target) = line.target_address {
            details["metadata"]["target_address"] = json!(target.0);
            details["metadata"]["referenced_address"] = json!(target.0);
        } else if line.bytes.len() == 3 {
            let addr = u16::from_le_bytes([line.bytes[1], line.bytes[2]]);
            details["metadata"]["referenced_address"] = json!(addr);
        }
    }

    if let Some(refs) = app_state.cross_refs.get(&address) {
        details["metadata"]["cross_refs_in"] = json!(refs);
    }

    if let Some(labels) = app_state.labels.get(&address) {
        let label_names: Vec<String> = labels.iter().map(|l| l.name.clone()).collect();
        details["metadata"]["labels"] = json!(label_names);
    }

    let mut comments = Vec::new();
    if let Some(entry) = app_state.annotations.get(address) {
        if let Some(c) = &entry.user_line_comment {
            comments.push(format!("[User Line] {c}"));
        }
        if let Some(c) = &entry.user_side_comment {
            comments.push(format!("[User Side] {c}"));
        }
        if let Some(c) = &entry.system_comment {
            comments.push(format!("[System] {c}"));
        }
    }
    if !comments.is_empty() {
        details["metadata"]["comments"] = json!(comments);
    }

    if let Some(fmt) = app_state
        .annotations
        .get(address)
        .and_then(|e| e.immediate_format)
    {
        details["metadata"]["operand_format"] = json!(format!("{fmt:?}"));
    }

    Ok(details)
}

/// Private helper to add a scope covering a memory range and trigger re-analysis.
///
/// # Arguments
///
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `start_address` and `end_address`.
///
/// # Returns
///
/// A JSON [`Value`] notification confirming scope creation and affected address bounds.
///
/// # Errors
///
/// Returns [`McpError`] if start address > end address or scope overlaps an existing scope.
fn add_scope_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let start_addr = get_address(args, "start_address")?;
    let end_addr = get_address(args, "end_address")?;

    if start_addr > end_addr {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address".to_string(),
        ));
    }

    let mut overlaps = false;
    for (s, e) in app_state
        .annotations
        .iter()
        .filter_map(|(s, e)| e.scope.map(|end| (s, end)))
    {
        if start_addr <= e && end_addr >= s {
            overlaps = true;
            break;
        }
    }

    if overlaps {
        return Err(make_mcp_error(
            -32602,
            "Cannot create scope: overlaps with an existing scope".to_string(),
        ));
    }

    let mut commands = Vec::new();
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

    let old_end = app_state.annotations.get(start_addr).and_then(|e| e.scope);
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

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Added Scope from {start_addr} to {end_addr}. {msg}")
        }]
    }))
}

/// Private helper to execute memory byte/text search queries.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing query string and optional encoding.
///
/// # Errors
///
/// Returns [`McpError`] if `query` is missing or search query parsing fails.
fn search_memory_impl(app_state: &AppState, args: &Value) -> Result<Value, McpError> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'query'"))?;
    let encoding = args.get("encoding").and_then(|v| v.as_str());
    let matches = crate::state::search::search_memory_raw(app_state, query, encoding, 100)
        .map_err(|msg| make_mcp_error(-32602, msg))?;
    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&matches).unwrap_or_default()
        }]
    }))
}

/// Private helper to execute disassembly text search queries.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing query string, regex options, and search field filters.
///
/// # Errors
///
/// Returns [`McpError`] if `query` is missing or regex compilation fails.
fn search_disassembly_impl(app_state: &AppState, args: &Value) -> Result<Value, McpError> {
    use crate::state::search::{self, SearchFilters};

    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_mcp_error(-32602, "Missing 'query'"))?;
    let use_regex = args
        .get("use_regex")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let filters = SearchFilters {
        labels: args
            .get("search_labels")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        comments: args
            .get("search_comments")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        instructions: args
            .get("search_instructions")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        hex_bytes: false,
        text: false,
        use_regex,
    };
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .map_or(50, |n| n as usize);

    let results = search::search_disassembly(app_state, query, &filters, max_results)
        .map_err(|msg| make_mcp_error(-32602, msg))?;

    let values: Vec<_> = results
        .into_iter()
        .map(|r| {
            json!({
                "address": format!("${:04X}", r.address.0),
                "address_decimal": r.address.0,
                "label": r.label.as_deref().unwrap_or(""),
                "mnemonic": r.mnemonic,
                "operand": r.operand,
                "comment": r.comment,
            })
        })
        .collect();

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&values).unwrap_or_default()
        }]
    }))
}

/// Private helper to read disassembly or hexdump text for a specified memory range.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `args` - JSON [`Value`] containing `start_address`, `end_address`, and optional `view`.
///
/// # Errors
///
/// Returns [`McpError`] if addresses are invalid or `view` is unrecognized.
fn read_region_impl(app_state: &AppState, args: &Value) -> Result<Value, McpError> {
    let start_addr = get_address(args, "start_address")?;
    let end_addr = get_address(args, "end_address")?;

    if start_addr > end_addr {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address".to_string(),
        ));
    }

    let view = args
        .get("view")
        .and_then(|v| v.as_str())
        .unwrap_or("disasm");

    let text = match view {
        "disasm" => get_disassembly_text(app_state, start_addr, end_addr),
        "hexdump" => get_hexdump_text(app_state, start_addr, end_addr),
        other => {
            return Err(make_mcp_error(
                -32602,
                format!("Invalid 'view' value \"{other}\": expected \"disasm\" or \"hexdump\""),
            ));
        }
    };
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Private helper to read disassembly or hexdump for the active selection or cursor row.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `view_state` - Reference to view state [`CoreViewState`].
/// * `args` - JSON [`Value`] containing optional `view` parameter.
///
/// # Errors
///
/// Returns [`McpError`] if selection indices are out of range or `view` is unrecognized.
fn read_selected_impl(
    app_state: &AppState,
    view_state: &CoreViewState,
    args: &Value,
) -> Result<Value, McpError> {
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
            return Err(make_mcp_error(
                -32602,
                format!("Invalid 'view' value \"{other}\": expected \"disasm\" or \"hexdump\""),
            ));
        }
    };
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Helper function to generate assembly source text for a memory address range [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `start` - Start memory address [`Addr`] (inclusive).
/// * `end` - End memory address [`Addr`] (inclusive).
///
/// # Returns
///
/// A [`String`] containing assembly directives, labels, instructions, and comments.
pub(crate) fn get_disassembly_text(app_state: &AppState, start: Addr, end: Addr) -> String {
    let matching_lines = app_state
        .disassembly
        .iter()
        .filter(|l| l.address >= start && l.address <= end)
        .count();
    let capacity = matching_lines.saturating_mul(40).max(32);
    let mut output = String::with_capacity(capacity);
    output.push_str(&format!("* = ${start:04X}\n"));

    for line in &app_state.disassembly {
        if line.address >= start && line.address <= end {
            if let Some(comment) = &line.line_comment {
                for comment_line in comment.lines() {
                    output.push_str(&format!("; {comment_line}\n"));
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

/// Private helper to calculate the disassembly selection address range [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `view_state` - Reference to view state [`CoreViewState`].
///
/// # Errors
///
/// Returns [`McpError`] if start or end line indices are invalid.
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

    let start_line = app_state
        .disassembly
        .get(start_idx)
        .ok_or_else(|| make_mcp_error(-32602, "Invalid start index"))?;

    let end_line = app_state
        .disassembly
        .get(end_idx)
        .ok_or_else(|| make_mcp_error(-32602, "Invalid end index"))?;

    Ok((start_line.address, end_line.address))
}

/// Private helper to calculate the hexdump selection address range [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `view_state` - Reference to view state [`CoreViewState`].
///
/// # Errors
///
/// Returns [`McpError`] if bounds computation fails.
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

    let start_val = aligned_origin.saturating_add(start_row.saturating_mul(bytes_per_row));
    let end_val = aligned_origin
        .saturating_add((end_row.saturating_add(1)).saturating_mul(bytes_per_row))
        .saturating_sub(1);

    let start_addr = Addr(start_val as u16);
    let end_addr = Addr(end_val as u16);

    let end_limit = app_state
        .raw_data
        .len()
        .checked_sub(1)
        .map_or(origin, |len_minus_1| {
            origin.wrapping_add(len_minus_1 as u16)
        });

    let max_end_val = aligned_origin + app_state.raw_data.len();

    let final_start = if start_val < (origin.0 as usize) || app_state.is_external(start_addr) {
        origin
    } else {
        start_addr
    };

    let final_end = if end_val >= max_end_val || app_state.is_external(end_addr) {
        end_limit
    } else {
        end_addr
    };

    Ok((final_start, final_end))
}

/// Helper function to generate 16-byte formatted hexdump text for an address range [`Addr`].
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `start_addr` - Start memory address [`Addr`] (inclusive).
/// * `end_addr` - End memory address [`Addr`] (inclusive).
///
/// # Returns
///
/// Formatted hex string representation of memory bytes.
pub(crate) fn get_hexdump_text(app_state: &AppState, start_addr: Addr, end_addr: Addr) -> String {
    let num_bytes = if start_addr.0 <= end_addr.0 {
        (end_addr.0 as usize) - (start_addr.0 as usize) + 1
    } else {
        0
    };
    let mut output = String::with_capacity(num_bytes.saturating_mul(4));
    let origin = app_state.origin;
    for addr_val in start_addr.0..=end_addr.0 {
        let addr = Addr(addr_val);
        if app_state.is_external(addr) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::common::{make_app_state, make_view_state};

    #[test]
    fn test_r2000_get_blocks_valid_range() {
        let app_state = make_app_state(0x1000, 256);
        let blocks = get_blocks_impl(&app_state, None);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["start_address"], json!(Addr(0x1000)));
        assert_eq!(blocks[0]["end_address"], json!(Addr(0x10FF)));
        assert_eq!(blocks[0]["type"], "Code");
    }

    #[test]
    fn test_r2000_get_symbols_filter() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state.labels.insert(
            Addr(0x1000),
            vec![crate::state::Label {
                name: "entry_point".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        let args = json!({ "kind": "user" });
        let symbols = get_symbols_impl(&app_state, &args).expect("Should return symbols");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0]["name"], "entry_point");
    }

    #[test]
    fn test_r2000_get_cross_references_existing_target() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state
            .cross_refs
            .insert(Addr(0x1000), vec![Addr(0x1020), Addr(0x1050)]);
        let refs = get_cross_references_impl(&app_state, Addr(0x1000));
        assert_eq!(refs, vec![Addr(0x1020), Addr(0x1050)]);
    }

    #[test]
    fn test_r2000_get_address_details_out_of_bounds() {
        let app_state = make_app_state(0x1000, 256);
        let err = get_address_details_impl(&app_state, Addr(0x2000))
            .expect_err("Should fail for out of bounds address");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("outside loaded binary range"));
    }

    #[test]
    fn test_r2000_get_binary_info_in_analysis_tools() {
        let app_state = make_app_state(0x1000, 256);
        let view_state = make_view_state();
        let info = get_binary_info_impl(&app_state);
        let text = info["content"][0]["text"]
            .as_str()
            .expect("Text field present");
        assert!(text.contains("\"origin\": 4096"));
        assert!(text.contains("\"size\": 256"));

        let mut app_state_mut = app_state;
        let res = handle_call(
            "r2000_get_binary_info",
            &json!({}),
            &mut app_state_mut,
            &view_state,
        )
        .expect("Tool call should match")
        .expect("Call should succeed");
        let res_text = res["content"][0]["text"].as_str().unwrap();
        assert!(res_text.contains("\"origin\": 4096"));
    }

    #[test]
    fn test_r2000_get_comments_in_analysis_tools() {
        let mut app_state = make_app_state(0x1000, 256);
        let view_state = make_view_state();
        app_state.annotations.update(Addr(0x1000), |e| {
            e.user_line_comment = Some("Header line comment".to_string());
            e.user_side_comment = Some("Inline side comment".to_string());
        });
        app_state.annotations.update(Addr(0x1010), |e| {
            e.user_line_comment = Some("Another comment".to_string());
        });

        let parse_comments = |res: Value| -> Vec<Value> {
            let text = res["content"][0]["text"]
                .as_str()
                .expect("Expected CallToolResult content text");
            serde_json::from_str(text).expect("Expected JSON array of comments")
        };

        // Test all comments via handle_call
        let res = handle_call(
            "r2000_get_comments",
            &json!({}),
            &mut app_state,
            &view_state,
        )
        .expect("Tool call should match")
        .expect("Call should succeed");
        let comments = parse_comments(res);
        assert_eq!(comments.len(), 3);

        // Test line filter
        let line_res = handle_call(
            "r2000_get_comments",
            &json!({ "type": "line" }),
            &mut app_state,
            &view_state,
        )
        .unwrap()
        .unwrap();
        let line_comments = parse_comments(line_res);
        assert_eq!(line_comments.len(), 2);
        assert_eq!(line_comments[0]["comment"], "Header line comment");

        // Test side filter
        let side_res = handle_call(
            "r2000_get_comments",
            &json!({ "type": "side" }),
            &mut app_state,
            &view_state,
        )
        .unwrap()
        .unwrap();
        let side_comments = parse_comments(side_res);
        assert_eq!(side_comments.len(), 1);
        assert_eq!(side_comments[0]["comment"], "Inline side comment");

        // Test address range filter
        let range_res = handle_call(
            "r2000_get_comments",
            &json!({ "start_address": 0x1000, "end_address": 0x1005 }),
            &mut app_state,
            &view_state,
        )
        .unwrap()
        .unwrap();
        let range_comments = parse_comments(range_res);
        assert_eq!(range_comments.len(), 2);

        // Test addresses array filter
        let addrs_res = handle_call(
            "r2000_get_comments",
            &json!({ "addresses": [0x1010] }),
            &mut app_state,
            &view_state,
        )
        .unwrap()
        .unwrap();
        let addrs_comments = parse_comments(addrs_res);
        assert_eq!(addrs_comments.len(), 1);
        assert_eq!(addrs_comments[0]["comment"], "Another comment");
    }

    #[test]
    fn test_analysis_tools_return_valid_call_tool_result() {
        let mut app_state = make_app_state(0x1000, 256);
        let view_state = make_view_state();

        for (tool_name, args) in [
            ("r2000_get_binary_info", json!({})),
            ("r2000_get_comments", json!({})),
            ("r2000_get_blocks", json!({})),
            ("r2000_get_address_details", json!({ "address": 0x1000 })),
            ("r2000_get_symbols", json!({})),
            ("r2000_get_cross_references", json!({ "address": 0x1000 })),
            (
                "r2000_add_scope",
                json!({ "start_address": 0x1000, "end_address": 0x1010 }),
            ),
        ] {
            let res = handle_call(tool_name, &args, &mut app_state, &view_state)
                .expect("Tool call should match")
                .expect("Tool call should succeed");
            serde_json::from_value::<rmcp::model::CallToolResult>(res)
                .unwrap_or_else(|e| panic!("{tool_name} did not return CallToolResult: {e}"));
        }
    }

    #[test]
    fn test_r2000_search_memory_in_analysis_tools() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state.raw_data[0] = 0xEA;
        app_state.raw_data[1] = 0xEA;
        let view_state = make_view_state();
        let res = handle_call(
            "r2000_search_memory",
            &json!({ "query": "EA EA", "encoding": "hex" }),
            &mut app_state,
            &view_state,
        )
        .expect("Tool call should match")
        .expect("Call should succeed");

        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("4096"));
    }

    #[test]
    fn test_r2000_search_disassembly_string_match() {
        let app_state = make_app_state(0x1000, 256);
        let args = json!({ "query": "NOP", "search_instructions": true });
        let res = search_disassembly_impl(&app_state, &args).expect("Search should succeed");
        assert!(res["content"][0]["text"].as_str().is_some());
    }

    #[test]
    fn test_r2000_read_region_slice() {
        let app_state = make_app_state(0x1000, 256);
        let args = json!({
            "start_address": 0x1000,
            "end_address": 0x1005,
            "view": "disasm"
        });
        let res = read_region_impl(&app_state, &args).expect("Read region should succeed");
        let text = res["content"][0]["text"]
            .as_str()
            .expect("Text field present");
        assert!(text.contains("* = $1000"));
    }

    #[test]
    fn test_r2000_read_selected_in_analysis_tools() {
        let mut app_state = make_app_state(0x1000, 256);
        let mut view_state = make_view_state();
        view_state.cursor_index = 0;

        let res = handle_call(
            "r2000_read_selected",
            &json!({ "view": "disasm" }),
            &mut app_state,
            &view_state,
        )
        .expect("Tool call should match")
        .expect("Call should succeed");

        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("* = $1000"));
    }

    #[test]
    fn test_r2000_read_region_invalid_range() {
        let app_state = make_app_state(0x1000, 256);
        let args = json!({
            "start_address": 0x1010,
            "end_address": 0x1000
        });
        let err = read_region_impl(&app_state, &args).expect_err("Should fail when start > end");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("start_address must be <= end_address"));
    }

    #[test]
    fn test_r2000_read_region_invalid_view() {
        let app_state = make_app_state(0x1000, 256);
        let args = json!({
            "start_address": 0x1000,
            "end_address": 0x1005,
            "view": "invalid_view"
        });
        let err = read_region_impl(&app_state, &args).expect_err("Should fail for invalid view");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Invalid 'view' value"));
    }

    #[test]
    fn test_r2000_read_selected_invalid_view() {
        let app_state = make_app_state(0x1000, 256);
        let view_state = make_view_state();
        let err = read_selected_impl(&app_state, &view_state, &json!({ "view": "unknown" }))
            .expect_err("Should fail for invalid view");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Invalid 'view' value"));
    }

    #[test]
    fn test_r2000_search_memory_missing_query() {
        let app_state = make_app_state(0x1000, 256);
        let err =
            search_memory_impl(&app_state, &json!({})).expect_err("Should fail missing query");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'query'"));
    }

    #[test]
    fn test_r2000_search_memory_invalid_encoding() {
        let app_state = make_app_state(0x1000, 256);
        let err = search_memory_impl(
            &app_state,
            &json!({ "query": "EA EA", "encoding": "invalid_encoding" }),
        )
        .expect_err("Should fail for invalid encoding");
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_r2000_search_disassembly_missing_query() {
        let app_state = make_app_state(0x1000, 256);
        let err = search_disassembly_impl(&app_state, &json!({}))
            .expect_err("Should fail for missing query");
        assert_eq!(err.code, -32602);
        assert!(err.message.contains("Missing 'query'"));
    }

    #[test]
    fn test_r2000_get_blocks_filter_aliases() {
        let mut app_state = make_app_state(0x1000, 32);
        app_state.block_types[0..8].fill(crate::state::BlockType::PetsciiText);
        app_state.block_types[8..16].fill(crate::state::BlockType::ScreencodeText);
        app_state.block_types[16..24].fill(crate::state::BlockType::LoHiAddress);
        app_state.block_types[24..32].fill(crate::state::BlockType::Undefined);

        for alias in ["PETSCII Text", "PETSCII", "petscii"] {
            let blocks = get_blocks_impl(&app_state, Some(alias));
            assert_eq!(blocks.len(), 1, "failed for alias {alias}");
            assert_eq!(blocks[0]["type"], "PETSCII Text");
        }

        for alias in ["Screencode Text", "Screencode", "screencode"] {
            let blocks = get_blocks_impl(&app_state, Some(alias));
            assert_eq!(blocks.len(), 1, "failed for alias {alias}");
            assert_eq!(blocks[0]["type"], "Screencode Text");
        }

        for alias in ["Lo/Hi Address", "lo_hi_address"] {
            let blocks = get_blocks_impl(&app_state, Some(alias));
            assert_eq!(blocks.len(), 1, "failed for alias {alias}");
            assert_eq!(blocks[0]["type"], "Lo/Hi Address");
        }
    }
}
