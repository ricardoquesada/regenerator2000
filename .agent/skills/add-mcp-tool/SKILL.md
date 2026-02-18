---
name: add-mcp-tool
description: Streamlines the process of adding new tools to the MCP server.
---

# Add MCP Tool Workflow

Use this workflow when adding a new tool to the `src/mcp/handler.rs` server.

## 1. Plan the Tool

Before writing code, confirm with the user:

- Tool Name (e.g., `r2000_get_memory`)
- Arguments (e.g., `address: u16`, `length: u16`)
- Return Type (e.g., `Vec<u8>`)
- Description

## 2. Define the Tool in `list_tools`

In `src/mcp/handler.rs`:

- Use `define_tool!` macro or manually add the JSON definition inside the `list_tools` function.
- Ensure arguments follow the JSON schema format.

Example:

```rust
json!({
    "name": "r2000_my_tool",
    "description": "Description of what the tool does.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "arg1": { "type": "string", "description": "..." }
        },
        "required": ["arg1"]
    }
})
```

## 3. Implement the Handler Logic

In `src/mcp/handler.rs`:

- Locate `handle_tool_call_internal`.
- Add a new match arm for your tool name.
- Call a dedicated implementation function (create one if it doesn't exist).

Example:

```rust
"r2000_my_tool" => {
    let arg1 = args["arg1"].as_str().ok_or(McpError::InvalidParams("Missing arg1".to_string()))?;
    let result = my_tool_impl(app_state, arg1)?;
    Ok(json!({ "content": [{ "type": "text", "text": result }] }))
}
```

## 4. Create the Implementation Function

In `src/mcp/handler.rs` (or a sub-module):

- Create a function named `[tool_name]_impl`.
- Accept `&mut AppState` (or `&AppState` if read-only).
- Return `Result<Value, McpError>` or a specific type.

## 5. Add Verification Test

In `tests/verify_mcp.py`:

- Create a new function `test_[tool_name](client)`.
- Use `client.rpc("tools/call", { ... })`.
- Verify the result ("PASS" or "FAIL").
- Add the function call to the `if __name__ == "__main__":` block.

## 6. Verify Correctness

- Run the `verify-mcp` skill:
  ```bash
  .agent/skills/verify-mcp/scripts/verify.sh
  ```
- Fix any compilation errors or test failures.

## 7. Update Documentation (Optional but Recommended)

- If `docs/mcp.md` exists, update the "Tools" section with the new tool definition.
