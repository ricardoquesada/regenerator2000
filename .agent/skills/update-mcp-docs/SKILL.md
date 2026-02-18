---
name: update-mcp-docs
description: Syncs `docs/mcp.md` with the actual tools exposed by `src/mcp/handler.rs`.
---

# Update MCP Docs Skill

This skill introspects the running MCP server and automatically updates the "Available Tools" section in `docs/mcp.md`.

## Usage

Simply run:

```bash
.agent/skills/update-mcp-docs/scripts/run_update.sh
```

This will:

1. Build and start the MCP server (if not already running).
2. Query the `tools/list` endpoint.
3. specific table of tools, arguments, and descriptions.
4. Update `docs/mcp.md`.

## When to Use

Run this skill whenever you add, modify, or remove a tool in `src/mcp/handler.rs` to ensure documentation stays in sync.
