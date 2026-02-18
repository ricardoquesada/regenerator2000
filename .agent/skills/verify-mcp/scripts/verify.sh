#!/bin/bash

LOG_FILE=".agent/skills/verify-mcp/server.log"

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
    echo "Starting MCP Server (logging to $LOG_FILE)..."
    # Build first to verify compilation and avoid timeout during run
    if ! cargo build; then
        echo "Build failed."
        exit 1
    fi

    # Start server in background
    cargo run -- --headless --mcp-server > "$LOG_FILE" 2>&1 &
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
        # Kill process if it's still running (unlikely if it crashed)
        kill $SERVER_PID 2>/dev/null
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

# Cleanup
if [ "$ALREADY_RUNNING" -eq 0 ]; then
    echo "Stopping MCP Server (PID $SERVER_PID)..."
    kill $SERVER_PID
    # Optional: remove log file on success?
    # rm "$LOG_FILE"
fi

exit $EXIT_CODE
