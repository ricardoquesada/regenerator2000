#!/bin/bash

LOG_FILE=".agent/skills/verify-mcp/server.log"
TEST_PROJECT_SRC="tests/6502/rq_intro_01.regen2000proj"
TEMP_DIR=""
SERVER_PID=""
ALREADY_RUNNING=0

cleanup() {
    if [ "$ALREADY_RUNNING" -eq 0 ] && [ -n "$SERVER_PID" ]; then
        echo "Stopping MCP Server (PID $SERVER_PID)..."
        kill "$SERVER_PID" 2>/dev/null
    fi
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
    rm -f "$LOG_FILE"
}
trap cleanup EXIT

# Function to check if port 3000 is open
check_port() {
    nc -z localhost 3000
    return $?
}

# Start server if not running
if check_port; then
    echo "MCP Server is already running."
    ALREADY_RUNNING=1
else
    if [ ! -f "$TEST_PROJECT_SRC" ]; then
        echo "Error: Test project not found at $TEST_PROJECT_SRC"
        exit 1
    fi

    TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/verify_mcp.XXXXXX")"
    TEMP_PROJECT="$TEMP_DIR/rq_intro_01.regen2000proj"
    cp "$TEST_PROJECT_SRC" "$TEMP_PROJECT"

    echo "Starting MCP Server with test project $TEST_PROJECT_SRC (logging to $LOG_FILE)..."
    # Build first to verify compilation and avoid timeout during run
    if ! cargo build; then
        echo "Build failed."
        exit 1
    fi

    # Start server in background using the isolated test project copy
    cargo run -- --headless --mcp-server "$TEMP_PROJECT" > "$LOG_FILE" 2>&1 &
    SERVER_PID=$!

    # Wait for server to be ready (up to 30 seconds)
    SERVER_READY=0
    for i in {1..30}; do
        if check_port; then
            echo "Server is ready (PID $SERVER_PID)."
            SERVER_READY=1
            break
        fi
        sleep 1
    done

    if [ "$SERVER_READY" -eq 0 ]; then
        echo "Error: Server failed to start or timed out."
        echo "=== Server Log ==="
        cat "$LOG_FILE"
        echo "=================="
        exit 1
    fi
    ALREADY_RUNNING=0
fi

# Setup Python venv
VENV_DIR=".venv"
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment in $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
fi

# Install requirements
echo "Installing requirements..."
"$VENV_DIR/bin/pip" install -r tests/requirements.txt > /dev/null

# Run tests using venv python
echo "Running verification tests..."
"$VENV_DIR/bin/python" tests/verify_mcp.py
EXIT_CODE=$?

exit $EXIT_CODE
