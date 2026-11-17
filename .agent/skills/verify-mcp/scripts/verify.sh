#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
cd "$ROOT_DIR"

USE_RUNNING_SERVER=0

usage() {
    cat <<'EOF'
Usage: verify.sh [--use-running-server] [-h|--help]

Runs the end-to-end HTTP/SSE MCP test suite (tests/verify_mcp.py) against a
temporary copy of tests/6502/rq_intro_01.regen2000proj.

Options:
  --use-running-server  Connect to an already-running MCP server on port 3000.
                        Only use this against a throwaway server: verify_mcp.py
                        executes mutating tools including r2000_save_project.
  -h, --help            Show this help message and exit.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --use-running-server)
            USE_RUNNING_SERVER=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Error: Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/verify_mcp.XXXXXX")"
LOG_FILE="$TEMP_DIR/server.log"
OUT_LOG="$TEMP_DIR/verify_output.log"
TEST_PROJECT_SRC="$ROOT_DIR/tests/6502/rq_intro_01.regen2000proj"
TEMP_PROJECT="$TEMP_DIR/rq_intro_01.regen2000proj"
VENV_DIR="$ROOT_DIR/.venv"
SERVER_PID=""

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        echo "Stopping MCP Server (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

check_port() {
    nc -z 127.0.0.1 3000 >/dev/null 2>&1 || nc -z localhost 3000 >/dev/null 2>&1
}

# Setup Python venv and dependencies before starting any server process
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment in $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
fi

if ! "$VENV_DIR/bin/python" -c "import requests" >/dev/null 2>&1; then
    echo "Installing Python dependencies from tests/requirements.txt..."
    "$VENV_DIR/bin/pip" install -q -r "$ROOT_DIR/tests/requirements.txt"
fi

if check_port; then
    if [ "$USE_RUNNING_SERVER" -eq 0 ]; then
        echo "Error: Port 3000 is already in use." >&2
        echo "Refusing to run tests/verify_mcp.py against an active MCP server because" >&2
        echo "it executes mutating tools (including r2000_save_project) that could" >&2
        echo "overwrite your open project. Stop that server first, or pass" >&2
        echo "--use-running-server if it is a throwaway instance." >&2
        exit 1
    fi
    echo "Using already-running MCP Server on port 3000 (--use-running-server)."
else
    if [ "$USE_RUNNING_SERVER" -eq 1 ]; then
        echo "Error: --use-running-server specified, but no MCP server is listening on port 3000." >&2
        exit 1
    fi

    if [ ! -f "$TEST_PROJECT_SRC" ]; then
        echo "Error: Test project not found at $TEST_PROJECT_SRC" >&2
        exit 1
    fi

    cp "$TEST_PROJECT_SRC" "$TEMP_PROJECT"

    echo "Building regenerator2000..."
    cargo build --bin regenerator2000

    echo "Starting headless MCP Server with isolated copy of $TEST_PROJECT_SRC..."
    "$ROOT_DIR/target/debug/regenerator2000" --headless --mcp-server "$TEMP_PROJECT" >"$LOG_FILE" 2>&1 &
    SERVER_PID=$!

    SERVER_READY=0
    for _ in {1..60}; do
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            break
        fi
        if check_port; then
            echo "Server is ready (PID $SERVER_PID)."
            SERVER_READY=1
            break
        fi
        sleep 0.25
    done

    if [ "$SERVER_READY" -eq 0 ]; then
        echo "Error: Server failed to start or timed out." >&2
        echo "=== Server Log ===" >&2
        cat "$LOG_FILE" >&2 || true
        echo "==================" >&2
        exit 1
    fi
fi

echo "Running verification tests..."
set +e
"$VENV_DIR/bin/python" -u "$ROOT_DIR/tests/verify_mcp.py" | tee "$OUT_LOG"
PY_EXIT="${PIPESTATUS[0]}"
set -e

if [ "$PY_EXIT" -ne 0 ]; then
    echo "Error: tests/verify_mcp.py exited with status $PY_EXIT." >&2
    exit "$PY_EXIT"
fi

if grep -Eq '^[[:space:]]*FAIL([:[:space:]]|$)' "$OUT_LOG"; then
    echo "Error: One or more checks in tests/verify_mcp.py reported FAIL." >&2
    exit 1
fi

echo "All MCP verification checks passed."
