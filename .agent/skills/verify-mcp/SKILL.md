---
name: verify-mcp
description: Verifies the MCP server functionality by running the integration test suite.
---

# Verify MCP Server Skill

This skill runs the `tests/verify_mcp.py` integration test suite to ensure the MCP server is functioning correctly.

## Usage

Run the verification script provided in the `scripts` directory from the project root. This script will automatically:

1. Check for `.venv` directory and create it if missing.
2. Install dependencies from `tests/requirements.txt`.
3. Check if the MCP server is running on port 3000.
4. If not, build and start it in headless server mode.
5. Run the Python verification tests.
6. Shut down the server if it was started by the script.

### Command

```bash
bash .agent/skills/verify-mcp/scripts/verify.sh
```

## Troubleshooting

- If the server fails to start, try running `cargo run -- --headless --mcp-server` manually to see errors.
- Ensure `python3` and `requests` are installed.
