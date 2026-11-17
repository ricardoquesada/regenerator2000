---
name: add-mcp-tool
description: >-
  Adds a new Model Context Protocol (MCP) tool to regenerator2000-core across
  the mcp/tools submodules, Rust unit tests, verify_mcp.py, and docs/mcp.md.
  Use when adding or extending an r2000_* MCP tool. Don't use for modifying
  TUI-only dialogs or running standalone MCP verification (use verify-mcp).
---

# Add MCP Tool Workflow

Follow these steps to add a new `r2000_*` tool to the MCP server in
`crates/regenerator2000-core/src/mcp/`.

## 1. Put Domain Logic on `AppState` First

Keep all validation, business rules, and undoable state mutations (`Command` +
`UndoStack`) in `regenerator2000-core` (typically as methods on `AppState`),
never inline inside `mcp/tools/`. Both the TUI and MCP must call the same core
methods.

## 2. Pick the Target Submodule (`crates/regenerator2000-core/src/mcp/tools/`)

`handler.rs` dispatches domain tools to four submodules:

- `analysis_tools.rs`: Read-only queries, blocks, symbols, cross-references,
  search, region reads, and `r2000_add_scope` (`tool_schemas(&SystemConfig)` +
  `handle_call(name, args, app_state, view_state)`).
- `navigation_tools.rs`: Disassembly cursor queries and jumps (`tool_schemas()`
  + `handle_call(name, args, app_state, view_state)`).
- `modification_tools.rs`: State mutations such as labels, symbolic offsets,
  comments, block types, immediate formats, splitters, and enums
  (`tool_schemas()` + `handle_call(name, args, app_state)`). Note: `handler.rs`
  automatically wraps `modification_tools::handle_call` with
  `DisassemblyViewAnchor` to preserve the TUI cursor.
- `session_tools.rs`: Binary unpacking, project saving, and undo/redo
  (`tool_schemas()` + `handle_call(name, args, app_state, view_state)`).

*(If you add a disassembly-mutating tool outside `modification_tools.rs`, wrap
its branch in `handler.rs::handle_tool_call_internal` with
`DisassemblyViewAnchor::capture` and `restore`.)*

## 3. Add the Schema and Handler

In the chosen submodule:

1. Append a `serde_json::json!({ ... })` schema entry to `tool_schemas`:

   ```rust
   json!({
       "name": "r2000_my_tool",
       "description": "Concise description of what the tool does.",
       "inputSchema": {
           "type": "object",
           "properties": {
               "address": {
                   "type": "integer",
                   "description": "Target 6502 address (decimal, e.g. 4096 for $1000)."
               }
           },
           "required": ["address"]
       }
   })
   ```

2. Match `"r2000_my_tool"` in `handle_call` and implement `my_tool_impl` with
   the shared parsers and error helper in `mcp/tools/common.rs` (`get_address`,
   `get_optional_address`, `get_optional_address_range`, `make_mcp_error`):

   ```rust
   fn my_tool_impl(app_state: &mut AppState, args: &Value) -> Result<Value, McpError> {
       let addr = get_address(args, "address")?;
       let label = args
           .get("label")
           .and_then(Value::as_str)
           .ok_or_else(|| make_mcp_error(-32602, "Missing required parameter: 'label'"))?;

       let summary = app_state
           .apply_my_operation(addr, label)
           .map_err(|msg| make_mcp_error(-32602, msg))?;

       Ok(json!({
           "content": [{ "type": "text", "text": summary }]
       }))
   }
   ```

## 4. Add Unit and Integration Tests

1. **Rust unit test**: Add a `#[test]` in the submodule's `#[cfg(test)] mod tests`
   with `make_app_state` (and `make_view_state` when needed) from
   `crate::mcp::tools::common`, covering valid calls and `-32602` parameter
   errors.
2. **End-to-end HTTP test**: Add `test_my_tool(client)` in `tests/verify_mcp.py`
   and call it from the `if __name__ == "__main__":` block.

## 5. Sync `docs/mcp.md` and Verify

Run all checks and regenerate the tool table in `docs/mcp.md`:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash .agent/skills/verify-mcp/scripts/verify.sh
bash .agent/skills/update-mcp-docs/scripts/run_update.sh
```
