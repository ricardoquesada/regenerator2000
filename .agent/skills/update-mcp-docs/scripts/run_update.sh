#!/bin/bash

# Define paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$(dirname "$(dirname "$(dirname "$SCRIPT_DIR")")")")"
LOG_FILE="$SCRIPT_DIR/server.log"
VENV_DIR="$ROOT_DIR/.venv"

# Ensure venv exists
if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment in $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
    "$VENV_DIR/bin/pip" install -r "$ROOT_DIR/tests/requirements.txt"
fi

# Function to check port
check_port() {
    nc -z localhost 3000
    return $?
}

# Start server if needed
if check_port; then
    echo "MCP Server is already running."
    ALREADY_RUNNING=1
else
    echo "Starting MCP Server (logging to $LOG_FILE)..."
    cd "$ROOT_DIR"
    cargo build --quiet
    cargo run -- --headless --mcp-server > "$LOG_FILE" 2>&1 &
    SERVER_PID=$!

    # Wait for startup
    for i in {1..30}; do
        if check_port; then
            echo "Server ready."
            break
        fi
        sleep 1
    done
    ALREADY_RUNNING=0
fi

# Run update script
echo "Updating documentation..."
"$VENV_DIR/bin/python" "$SCRIPT_DIR/update_docs.py"
EXIT_CODE=$?

# Cleanup
if [ "$ALREADY_RUNNING" -eq 0 ]; then
    echo "Stopping server..."
    kill $SERVER_PID 2>/dev/null
    # rm "$LOG_FILE"
fi

exit $EXIT_CODE
