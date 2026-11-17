---
name: update-mcp-docs
description: >-
  Syncs the Available Tools section in docs/mcp.md by querying tools/list from
  the headless stdio MCP server. Use when adding, renaming, or changing tool
  schemas in crates/regenerator2000-core/src/mcp/tools/ or handler.rs. Don't
  use for MCP behavior tests (use verify-mcp) or designing a new tool (use
  add-mcp-tool).
---

# Update MCP Docs Skill

Regenerates the `## Available Tools` section in `docs/mcp.md` directly from the
live `tools/list` response over `--mcp-server-stdio`.

## Source-of-Truth Rule

Edit tool and argument descriptions in the Rust `tool_schemas()` functions in
`crates/regenerator2000-core/src/mcp/tools/*.rs` (and `handler.rs` for
`r2000_batch_execute`), not by hand inside `## Available Tools` in
`docs/mcp.md`. Running this skill overwrites `## Available Tools` from the Rust
schemas.

## Usage

Run the update script from any directory:

```bash
bash .agent/skills/update-mcp-docs/scripts/run_update.sh
```

The script executes three steps without opening TCP port `3000` or creating a
Python `.venv`:

1. Builds `regenerator2000` with `cargo build --quiet`.
2. Queries `tools/list` from `cargo run --quiet -- --mcp-server-stdio`.
3. Replaces `## Available Tools` in `docs/mcp.md` (preserving all sections
   above and below it) with sorted tool headings and argument tables.

To preview output into a temporary copy without modifying `docs/mcp.md`, pass
`--output /tmp/mcp_preview.md`.
