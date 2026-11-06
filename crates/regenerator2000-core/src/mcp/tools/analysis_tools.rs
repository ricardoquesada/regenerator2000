//! MCP tool handlers for memory block classification, symbol queries, cross-references, scopes, search, and region reads.
//!
//! This module defines schemas and execution handlers that inspect memory blocks,
//! query instruction details, filter symbols and comments, create lexical scopes,
//! search memory bytes or disassembly text, and read disassembly or hexdump ranges.

use crate::commands::Command;
use crate::config::SystemConfig;
use crate::mcp::tools::common::{
    get_address, get_optional_address_range, make_mcp_error, parse_address_value,
};
use crate::mcp::types::McpError;
use crate::state::AppState;
use crate::state::types::{Addr, BlockType};
use crate::view_state::CoreViewState;
use serde_json::{Value, json};
use std::fmt::Write as _;

/// Returns JSON schemas for all analysis, metadata, search, and region-read tools.
#[must_use]
pub fn tool_schemas(system_config: &SystemConfig) -> Vec<Value> {
    vec![
        json!({
            "name": "r2000_get_binary_info",
            "description": format!(
                "Return origin address, size, target platform, filename, description, entropy (>{} suggests compression), and undocumented opcodes flag.",
                system_config.entropy_threshold
            ),
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "r2000_get_comments",
            "description": "Return user-defined comments and their addresses with optional filters.",
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
            "description": "Return all memory blocks with their address range and type (Code, Byte, Word, Address, PETSCII, Screencode, Lo/Hi Address, Hi/Lo Address, Lo/Hi Word, Hi/Lo Word, External File, Undefined). Respects splitters.",
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
            "description": "Return detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.",
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
            "description": "Return defined labels (user, system, and auto) and their addresses. With no arguments, this tool returns all symbols. Provide optional filters to narrow results: 'names' matches specific label names, 'start_address' and 'end_address' limit to an address range, and 'kind' filters by label kind. All active filters combine with logical AND.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "names": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of label names to look up. Case-sensitive."
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
            "description": "Return a list of addresses that reference the given address (for example, JSR, JMP, or load/store instructions).",
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
            "description": "Add a lexical scope that covers the specified memory range. Use this tool to isolate local labels inside a subroutine. Nested scopes are not supported.",
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
            "description": "Search memory for a byte sequence or text string and return matching addresses.",
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
                        "description": "Encoding for the query. 'text' searches both PETSCII and Screencode. 'hex' searches raw byte patterns. Defaults to 'hex' when the query matches hex bytes, otherwise 'text'."
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "r2000_search_disassembly",
            "description": "Search disassembly text for a substring or regular expression. Returns matching addresses with label, mnemonic, operand, and comment fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query. Interpreted as a case-insensitive substring by default, or as a regex when 'use_regex' is true."
                    },
                    "use_regex": {
                        "type": "boolean",
                        "description": "When true, compiles the query as a case-insensitive regular expression. Defaults to false."
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
            "description": "Return disassembly or hexdump text for a specific memory range.",
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
            "description": "Return disassembly or hexdump text for the active UI selection, or the row under the cursor when no selection is active.",
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

/// Dispatches analysis and metadata tool calls by `name`.
///
/// Returns `Some(Ok(value))` on success, `Some(Err(McpError))` on failure, or `None`
/// if `name` does not match an analysis tool.
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

/// Formats binary metadata (origin, size, target system, filename, description, and entropy) as JSON.
#[must_use]
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

/// Queries user line and side comments with optional address, range, and type filters.
///
/// # Errors
///
/// Returns [`McpError`] if `"addresses"`, `"start_address"`, `"end_address"`, or `"type"` is invalid.
pub(crate) fn get_comments_impl(
    app_state: &AppState,
    args: &Value,
) -> Result<Vec<Value>, McpError> {
    let addr_filter: Option<Vec<Addr>> = match args.get("addresses") {
        None | Some(Value::Null) => None,
        Some(Value::Array(arr)) => Some(
            arr.iter()
                .map(|v| parse_address_value(v, "addresses"))
                .collect::<Result<Vec<Addr>, McpError>>()?,
        ),
        Some(_) => {
            return Err(make_mcp_error(
                -32602,
                "Invalid 'addresses' (expected an array of addresses)",
            ));
        }
    };

    let range_filter = get_optional_address_range(args)?;

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

    let mut comments = Vec::new();
    for (addr, entry) in app_state.annotations.iter() {
        if let Some((s, e)) = range_filter
            && (addr < s || addr > e)
        {
            continue;
        }
        if let Some(ref addrs) = addr_filter
            && !addrs.contains(&addr)
        {
            continue;
        }

        if type_filter != Some("side")
            && let Some(ref comment) = entry.user_line_comment
        {
            comments.push(json!({
                "address": addr,
                "type": "line",
                "comment": comment
            }));
        }
        if type_filter != Some("line")
            && let Some(ref comment) = entry.user_side_comment
        {
            comments.push(json!({
                "address": addr,
                "type": "side",
                "comment": comment
            }));
        }
    }

    Ok(comments)
}

/// Returns contiguous classified memory blocks, respecting user splitters and optional type `filter`.
#[must_use]
pub(crate) fn get_blocks_impl(app_state: &AppState, filter: Option<&str>) -> Vec<Value> {
    let mut blocks = Vec::new();
    let origin = app_state.origin;
    let max_len = app_state.block_types.len();

    if max_len == 0 {
        return blocks;
    }

    let mut push_block = |start_idx: usize, end_idx: usize, block_type: BlockType| {
        if filter.is_none_or(|f| block_type.matches_filter(f)) {
            let start_offset = u16::try_from(start_idx).unwrap_or(u16::MAX);
            let end_offset = u16::try_from(end_idx).unwrap_or(u16::MAX);
            blocks.push(json!({
                "start_address": origin.wrapping_add(start_offset),
                "end_address": origin.wrapping_add(end_offset),
                "type": block_type.to_string()
            }));
        }
    };

    let mut start_idx = 0;
    let mut current_type = app_state.block_types[0];

    for i in 1..max_len {
        let offset = u16::try_from(i).unwrap_or(u16::MAX);
        let addr = origin.wrapping_add(offset);
        let type_ = app_state.block_types[i];

        if type_ != current_type || app_state.splitters.contains(&addr) {
            push_block(start_idx, i - 1, current_type);
            start_idx = i;
            current_type = type_;
        }
    }

    push_block(start_idx, max_len - 1, current_type);
    blocks
}

/// Returns all typed cross-references ([`XRef`](crate::state::XRef)) that target `address`.
///
/// The returned list is sorted and deduplicated by `(from, kind)`. Each entry contains:
/// - `"address"`: Numeric source address (`u16`).
/// - `"from_address"`: Numeric source address (`u16`).
/// - `"from_hex"`: Uppercase 4-digit hex string (for example, `"$C010"`).
/// - `"kind"`: Snake-case [`XRefKind::as_str`](crate::state::XRefKind::as_str) classification.
/// - `"badge"`: Bracketed [`XRefKind::badge`](crate::state::XRefKind::badge) label.
#[must_use]
pub(crate) fn get_cross_references_impl(app_state: &AppState, address: Addr) -> Vec<Value> {
    let Some(refs) = app_state.cross_refs.get(&address) else {
        return Vec::new();
    };

    let mut sorted_refs = refs.clone();
    sorted_refs.sort_unstable();
    sorted_refs.dedup();
    sorted_refs
        .into_iter()
        .map(|xref| {
            let ref_addr = xref.from;
            json!({
                "address": ref_addr,
                "from_address": ref_addr.0,
                "from_hex": format!("${:04X}", ref_addr.0),
                "kind": xref.kind.as_str(),
                "badge": xref.kind.badge(),
            })
        })
        .collect()
}

/// Queries symbols with optional filters for `names`, `start_address`/`end_address`, and `kind`.
///
/// # Errors
///
/// Returns [`McpError`] if `"kind"`, `"start_address"`, or `"end_address"` is invalid.
pub(crate) fn get_symbols_impl(app_state: &AppState, args: &Value) -> Result<Vec<Value>, McpError> {
    let name_filter: Option<Vec<&str>> = args
        .get("names")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());

    let range_filter = get_optional_address_range(args)?;

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

    let mut symbols = Vec::new();
    for (&addr, labels) in &app_state.labels {
        if let Some((s, e)) = range_filter
            && (addr < s || addr > e)
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

    Ok(symbols)
}

/// Returns detailed metadata for a single memory `address`.
///
/// When incoming cross-references ([`XRef`](crate::state::XRef)) exist for `address`, the returned
/// `metadata` object populates both:
/// - `"cross_refs_in"`: Sorted, deduplicated array of numeric source addresses (`Vec<Addr>`).
/// - `"typed_cross_refs_in"`: Array of typed cross-reference objects from [`get_cross_references_impl`].
///
/// # Errors
///
/// Returns [`McpError`] if `address` is outside the loaded binary bounds.
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
        .map_or_else(|| "Unknown".to_string(), ToString::to_string);

    let mut details = json!({
        "address": address,
        "type": block_type,
        "block_type": block_type,
        "metadata": {}
    });

    // Prefer a line that carries actual machine bytes at `address` so preamble equates
    // (at `Addr::ZERO`) or 0-byte splitters do not shadow the instruction line.
    let line_opt = app_state
        .disassembly
        .iter()
        .find(|l| l.external_label_address.is_none() && l.address == address && !l.bytes.is_empty())
        .or_else(|| {
            app_state
                .disassembly
                .iter()
                .find(|l| l.external_label_address.is_none() && l.address == address)
        });

    if let Some(line) = line_opt {
        details["instruction"] = json!({
            "bytes": line.bytes,
            "mnemonic": line.mnemonic.to_uppercase(),
            "op_code": line.bytes.first().map(|b| format!("${b:02X}")),
            "operands": line.operand,
        });

        if let Some(target) = line.target_address {
            details["metadata"]["target_address"] = json!(target.0);
            details["metadata"]["referenced_address"] = json!(target.0);
        } else if line.opcode.is_some() && line.bytes.len() == 3 {
            let addr = u16::from_le_bytes([line.bytes[1], line.bytes[2]]);
            details["metadata"]["referenced_address"] = json!(addr);
        }
    }

    if let Some(refs) = app_state.cross_refs.get(&address) {
        // Keep `cross_refs_in` as a sorted, deduplicated integer array for backward compatibility.
        let mut addrs: Vec<Addr> = refs.iter().map(|x| x.from).collect();
        addrs.sort_unstable();
        addrs.dedup();
        details["metadata"]["cross_refs_in"] = json!(addrs);
        details["metadata"]["typed_cross_refs_in"] =
            json!(get_cross_references_impl(app_state, address));
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
        if let Some(fmt) = entry.immediate_format {
            details["metadata"]["operand_format"] = json!(format!("{fmt:?}"));
        }
    }
    if !comments.is_empty() {
        details["metadata"]["comments"] = json!(comments);
    }

    Ok(details)
}

/// Adds a lexical scope over `start_address..=end_address` and re-analyzes the binary.
///
/// # Errors
///
/// Returns [`McpError`] if `start_address > end_address`, if the range is outside the
/// loaded binary bounds, or if the range overlaps an existing scope.
fn add_scope_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
    let start_addr = get_address(args, "start_address")?;
    let end_addr = get_address(args, "end_address")?;

    if start_addr > end_addr {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address",
        ));
    }

    if app_state.is_external(start_addr) || app_state.is_external(end_addr) {
        return Err(make_mcp_error(
            -32602,
            format!("Address range {start_addr}..={end_addr} is outside loaded binary range"),
        ));
    }

    let overlaps = app_state
        .annotations
        .scopes()
        .iter()
        .any(|(&s, &e)| start_addr <= e && end_addr >= s);

    if overlaps {
        return Err(make_mcp_error(
            -32602,
            "Cannot create scope: overlaps with an existing scope",
        ));
    }

    let mut commands = Vec::new();
    let has_label = app_state
        .labels
        .get(&start_addr)
        .is_some_and(|l| !l.is_empty());
    if !has_label {
        let default_name = format!("scope_{:04X}", start_addr.0);
        let label_cmd = app_state
            .create_set_user_label_command(start_addr, &default_name, false)
            .map_err(|msg| make_mcp_error(-32602, msg))?;
        commands.push(label_cmd);
    }

    let old_end = app_state.annotations.get(start_addr).and_then(|e| e.scope);
    commands.push(Command::AddScope {
        start: start_addr,
        end: end_addr,
        old_end,
    });

    let command = if commands.len() == 1 {
        commands.remove(0)
    } else {
        Command::Batch(commands)
    };

    command.apply(app_state);
    let (analysis_cmd, msg) = app_state.perform_analysis();
    app_state.push_command(Command::Batch(vec![analysis_cmd, command]));

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("Added Scope from {start_addr} to {end_addr}. {msg}")
        }]
    }))
}

/// Searches raw binary memory for a byte pattern or text string.
///
/// # Errors
///
/// Returns [`McpError`] if `"query"` is missing or if pattern parsing fails.
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

/// Searches disassembly rows for a substring or regular expression.
///
/// # Errors
///
/// Returns [`McpError`] if `"query"` is missing or if regular expression compilation fails.
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
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(50);

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

/// Reads disassembly or hexdump text for `start_address..=end_address`.
///
/// # Errors
///
/// Returns [`McpError`] if the address range or `"view"` parameter is invalid.
fn read_region_impl(app_state: &AppState, args: &Value) -> Result<Value, McpError> {
    let start_addr = get_address(args, "start_address")?;
    let end_addr = get_address(args, "end_address")?;

    if start_addr > end_addr {
        return Err(make_mcp_error(
            -32602,
            "start_address must be <= end_address",
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

/// Reads disassembly or hexdump text for the active UI selection or cursor row.
///
/// # Errors
///
/// Returns [`McpError`] if the selection indices are out of bounds or `"view"` is invalid.
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

/// Generates assembly text for `start..=end`.
#[must_use]
pub(crate) fn get_disassembly_text(app_state: &AppState, start: Addr, end: Addr) -> String {
    let matching_lines = app_state
        .disassembly
        .iter()
        .filter(|l| l.address >= start && l.address <= end)
        .count();
    let capacity = matching_lines.saturating_mul(40).max(32);
    let mut output = String::with_capacity(capacity);
    let _ = writeln!(output, "* = ${start:04X}");

    for line in &app_state.disassembly {
        if line.address >= start && line.address <= end {
            if let Some(comment) = &line.line_comment {
                for comment_line in comment.lines() {
                    let _ = writeln!(output, "; {comment_line}");
                }
            }

            if let Some(label) = &line.label
                && !label.is_empty()
            {
                let _ = writeln!(output, "{label}:");
            }

            if line.comment.is_empty() {
                if line.operand.is_empty() {
                    let _ = writeln!(output, "${:04X} {}", line.address, line.mnemonic);
                } else {
                    let _ = writeln!(
                        output,
                        "${:04X} {} {}",
                        line.address, line.mnemonic, line.operand
                    );
                }
            } else {
                let instruction = if line.operand.is_empty() {
                    line.mnemonic.clone()
                } else {
                    format!("{} {}", line.mnemonic, line.operand)
                };
                let _ = writeln!(
                    output,
                    "${:04X} {:<20} ; {}",
                    line.address, instruction, line.comment
                );
            }
        }
    }
    output
}

/// Computes the inclusive `(start, end)` address range for the disassembly selection in `view_state`.
///
/// # Errors
///
/// Returns [`McpError`] if the start or end line index is out of bounds.
fn get_selection_range_disasm(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Result<(Addr, Addr), McpError> {
    let cursor_idx = view_state.cursor_index;
    let (start_idx, end_idx) = match view_state.selection_start {
        Some(sel_start) => (sel_start.min(cursor_idx), sel_start.max(cursor_idx)),
        None => (cursor_idx, cursor_idx),
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

/// Computes the inclusive `(start, end)` address range for the hexdump selection in `view_state`.
///
/// # Errors
///
/// Returns [`McpError`] if the selection bounds cannot be resolved.
fn get_selection_range_hexdump(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> Result<(Addr, Addr), McpError> {
    let cursor_row = view_state.hex_cursor_index;
    let (start_row, end_row) = match view_state.hex_selection_start {
        Some(sel_start) => (sel_start.min(cursor_row), sel_start.max(cursor_row)),
        None => (cursor_row, cursor_row),
    };

    let origin = app_state.origin;
    let bytes_per_row: usize = if matches!(
        view_state.right_pane,
        crate::view_state::RightPane::HexDump8
    ) {
        8
    } else {
        16
    };
    let alignment_padding = (origin.0 as usize) % bytes_per_row;
    let aligned_origin = (origin.0 as usize) - alignment_padding;

    let start_val = aligned_origin.saturating_add(start_row.saturating_mul(bytes_per_row));
    let end_val = aligned_origin
        .saturating_add((end_row.saturating_add(1)).saturating_mul(bytes_per_row))
        .saturating_sub(1);

    let end_limit = app_state
        .raw_data
        .len()
        .checked_sub(1)
        .map_or(origin, |len_minus_1| {
            #[allow(clippy::cast_possible_truncation)]
            origin.wrapping_add(len_minus_1 as u16)
        });

    let origin_usize = origin.0 as usize;
    let max_end_val = origin_usize.saturating_add(app_state.raw_data.len());

    // Check `usize` bounds before converting to `u16` so large row indices clamp to
    // the binary endpoints instead of wrapping modulo 65536.
    let final_start = match u16::try_from(start_val) {
        Ok(addr) if start_val >= origin_usize && !app_state.is_external(Addr(addr)) => Addr(addr),
        _ if start_val >= max_end_val => end_limit,
        _ => origin,
    };

    let final_end = match u16::try_from(end_val) {
        Ok(addr) if end_val < max_end_val && !app_state.is_external(Addr(addr)) => Addr(addr),
        _ => end_limit,
    };

    Ok((final_start, final_end))
}

/// Formats `start_addr..=end_addr` as 16-byte hexdump rows.
#[must_use]
pub(crate) fn get_hexdump_text(app_state: &AppState, start_addr: Addr, end_addr: Addr) -> String {
    let num_bytes = if start_addr.0 <= end_addr.0 {
        (end_addr.0 as usize) - (start_addr.0 as usize) + 1
    } else {
        0
    };
    let mut output = String::with_capacity(num_bytes.saturating_mul(4));
    let origin = app_state.origin;
    let mut emitted_bytes: usize = 0;

    for addr_val in start_addr.0..=end_addr.0 {
        let addr = Addr(addr_val);
        if app_state.is_external(addr) {
            continue;
        }
        let idx = addr.offset_from(origin);
        let byte = app_state.raw_data[idx];
        if emitted_bytes.is_multiple_of(16) {
            if emitted_bytes > 0 {
                output.push('\n');
            }
            let _ = write!(output, "${addr:04X}: ");
        }
        let _ = write!(output, "{byte:02X} ");
        emitted_bytes += 1;
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
        app_state.labels.insert(
            Addr(0x1020),
            vec![crate::state::Label {
                name: "later_label".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        let args = json!({ "kind": "user", "start_address": "$1000", "end_address": "$1010" });
        let symbols = get_symbols_impl(&app_state, &args).expect("Should return symbols");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0]["name"], "entry_point");
    }

    #[test]
    fn test_r2000_get_cross_references_existing_target() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0x1020), crate::state::XRefKind::Call),
                crate::state::XRef::new(Addr(0x1050), crate::state::XRefKind::Read),
            ],
        );
        let refs = get_cross_references_impl(&app_state, Addr(0x1000));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["address"], 0x1020);
        assert_eq!(refs[0]["kind"], "call");
        assert_eq!(refs[0]["badge"], "[C]");
        assert_eq!(refs[1]["address"], 0x1050);
        assert_eq!(refs[1]["kind"], "read");
        assert_eq!(refs[1]["badge"], "[R]");
    }

    #[test]
    fn test_r2000_get_cross_references_includes_kind_and_badge() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0x1050), crate::state::XRefKind::Write),
                crate::state::XRef::new(Addr(0x1020), crate::state::XRefKind::Read),
                crate::state::XRef::new(Addr(0x1050), crate::state::XRefKind::Write),
            ],
        );
        let refs = get_cross_references_impl(&app_state, Addr(0x1000));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0]["address"], 0x1020);
        assert_eq!(refs[0]["from_address"], 0x1020);
        assert_eq!(refs[0]["from_hex"], "$1020");
        assert_eq!(refs[0]["kind"], "read");
        assert_eq!(refs[0]["badge"], "[R]");
        assert_eq!(refs[1]["address"], 0x1050);
        assert_eq!(refs[1]["from_address"], 0x1050);
        assert_eq!(refs[1]["from_hex"], "$1050");
        assert_eq!(refs[1]["kind"], "write");
        assert_eq!(refs[1]["badge"], "[W]");
    }

    #[test]
    fn test_r2000_get_address_details_typed_cross_refs() {
        let mut app_state = make_app_state(0x1000, 256);
        app_state.cross_refs.insert(
            Addr(0x1000),
            vec![
                crate::state::XRef::new(Addr(0x1010), crate::state::XRefKind::ReadModifyWrite),
                crate::state::XRef::new(Addr(0x1005), crate::state::XRefKind::Call),
                crate::state::XRef::new(Addr(0x1005), crate::state::XRefKind::Offset),
            ],
        );
        let details = get_address_details_impl(&app_state, Addr(0x1000))
            .expect("Should return address details");
        assert_eq!(
            details["metadata"]["cross_refs_in"],
            json!([0x1005, 0x1010])
        );
        let typed_refs = details["metadata"]["typed_cross_refs_in"]
            .as_array()
            .expect("Should have typed_cross_refs_in array");
        assert_eq!(typed_refs.len(), 3);
        assert_eq!(typed_refs[0]["from_address"], 0x1005);
        assert_eq!(typed_refs[0]["from_hex"], "$1005");
        assert_eq!(typed_refs[0]["kind"], "call");
        assert_eq!(typed_refs[0]["badge"], "[C]");
        assert_eq!(typed_refs[1]["from_address"], 0x1005);
        assert_eq!(typed_refs[1]["from_hex"], "$1005");
        assert_eq!(typed_refs[1]["kind"], "offset");
        assert_eq!(typed_refs[1]["badge"], "[O]");
        assert_eq!(typed_refs[2]["from_address"], 0x1010);
        assert_eq!(typed_refs[2]["from_hex"], "$1010");
        assert_eq!(typed_refs[2]["kind"], "read_modify_write");
        assert_eq!(typed_refs[2]["badge"], "[R/W]");
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
    fn test_r2000_get_address_details_3_byte_data_does_not_fabricate_referenced_address() {
        let mut app_state = make_app_state(0x1000, 3);
        app_state.raw_data = vec![0x11, 0x22, 0x33];
        app_state.block_types = vec![BlockType::DataByte; 3];
        app_state.disassemble();

        let details = get_address_details_impl(&app_state, Addr(0x1000))
            .expect("address details should succeed");
        assert!(details["metadata"].get("referenced_address").is_none());
    }

    #[test]
    fn test_r2000_add_scope_rejects_out_of_bounds_and_duplicate_label() {
        let mut app_state = make_app_state(0x1000, 32);
        let oob_err = add_scope_impl(
            &mut app_state,
            &json!({ "start_address": 0x2000, "end_address": 0x2010 }),
        )
        .expect_err("out of bounds scope must fail");
        assert!(oob_err.message.contains("outside loaded binary range"));
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

        // Test address range filter (including hex string addresses)
        let range_res = handle_call(
            "r2000_get_comments",
            &json!({ "start_address": "$1000", "end_address": "$1005" }),
            &mut app_state,
            &view_state,
        )
        .unwrap()
        .unwrap();
        let range_comments = parse_comments(range_res);
        assert_eq!(range_comments.len(), 2);

        // Test addresses array filter (including hex string addresses)
        let addrs_res = handle_call(
            "r2000_get_comments",
            &json!({ "addresses": ["$1010"] }),
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
    fn test_r2000_read_selected_hexdump8_and_overflow_clamping() {
        let app_state = make_app_state(0x1000, 32);
        let mut view_state = make_view_state();
        view_state.right_pane = crate::view_state::RightPane::HexDump8;
        view_state.hex_cursor_index = 1;

        let (start, end) = get_selection_range_hexdump(&app_state, &view_state)
            .expect("hexdump8 selection range should succeed");
        assert_eq!((start, end), (Addr(0x1008), Addr(0x100F)));

        // Row index 4096 would wrap modulo 65536 to $1000 if truncated before bounds checking.
        view_state.right_pane = crate::view_state::RightPane::HexDump16;
        view_state.hex_cursor_index = 4096;
        let (clamped_start, clamped_end) = get_selection_range_hexdump(&app_state, &view_state)
            .expect("overflowing row index should clamp to binary end");
        assert_eq!((clamped_start, clamped_end), (Addr(0x101F), Addr(0x101F)));
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
