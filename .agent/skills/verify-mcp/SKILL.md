---
name: verify-mcp
description: >-
  Verifies the MCP server with fast Rust test suites and the end-to-end
  HTTP/SSE verification script. Use when testing MCP tool behavior, protocol
  compliance, or regressions in crates/regenerator2000-core/src/mcp/. Don't
  use for updating docs/mcp.md schemas (use update-mcp-docs) or designing a
  new tool (use add-mcp-tool).
---

# Verify MCP Server Skill

Verify MCP server behavior at two layers: fast in-process Rust tests and the
full end-to-end HTTP/SSE suite in `tests/verify_mcp.py`.

## Fast In-Process Rust Tests

Run these checks first for rapid feedback without binding TCP port `3000`:

```bash
cargo test -p regenerator2000-core --lib --features mcp mcp
cargo test --test 'mcp_*'
```

- `--features mcp` is required when testing `regenerator2000-core` standalone
  because `mcp` is an optional feature on that crate.
- `mcp_*` runs `tests/mcp_address_details_test.rs`, `tests/mcp_batch_test.rs`,
  `tests/mcp_resource_test.rs`, and `tests/mcp_selection_test.rs`.

## End-to-End HTTP/SSE Suite

Run the verification script from any working directory:

```bash
bash .agent/skills/verify-mcp/scripts/verify.sh
```

The script executes these steps:

1. Creates `.venv` if missing and installs `tests/requirements.txt` only when
   `requests` cannot be imported.
2. Checks TCP port `3000` and aborts if a server is already listening, because
   `tests/verify_mcp.py` invokes mutating tools (including `r2000_save_project`)
   that could overwrite an open project. Pass `--use-running-server` only when
   targeting a throwaway test server.
3. Copies `tests/6502/rq_intro_01.regen2000proj` into a `/tmp` workspace, builds
   the `regenerator2000` binary, and starts
   `./target/debug/regenerator2000 --headless --mcp-server <temp_copy>` with
   logs written to `/tmp`.
4. Runs `tests/verify_mcp.py` and fails if Python returns a non-zero status or
   prints any `FAIL` line.
5. Stops the server process and deletes all temporary files on exit.

## Troubleshooting

- If the script refuses to run because port `3000` is busy, inspect the active
  listener with `lsof -i :3000` and stop it first.
- To reproduce server startup manually without touching checked-in fixtures, run:

  ```bash
  cp tests/6502/rq_intro_01.regen2000proj /tmp/rq_intro_01.regen2000proj
  cargo run -- --headless --mcp-server /tmp/rq_intro_01.regen2000proj
  ```

- If `tests/verify_mcp.py` fails on tool list checks after an intentional schema
  change, update `tests/verify_mcp.py` and run
  `bash .agent/skills/update-mcp-docs/scripts/run_update.sh` (`update-mcp-docs`
  skill) to sync `docs/mcp.md`.
