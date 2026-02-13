# MCP Integration

Regenerator 2000 supports the **Model Context Protocol (MCP)**, allowing AI agents like **Claude Desktop**, **Claude Code**, and **Gemini CLI** to interact directly with your analysis project.

Through the MCP server, an AI assistant can:

-   **Read** disassembly, memory dump, and project state.
-   **Modify** comments, labels, and block types.
-   **Analyze** code structure and control flow.

!!! note "What is MCP?"
    The Model Context Protocol (MCP) is an open standard that enables AI models to interact with local applications and contexts. Learn more at [modelcontextprotocol.io](https://modelcontextprotocol.io).

## Modes of Operation

Regenerator 2000 supports two MCP transport modes:

1.  **HTTP Mode (Streamable/SSE)**:
    -   Runs the MCP server over HTTP using Server-Sent Events (SSE).
    -   Allows concurrent TUI usage (User + AI working together).
    -   **Flag**: `--mcp-server` (starts TUI + Server on port 3000)
    -   **Flag**: `--mcp-server --headless` (starts Headless Server on port 3000)
    -   **Endpoint**: `http://127.0.0.1:3000/mcp`
    -   Recommended option!

2.  **Stdio Mode (Headless)**:
    -   Starts a headless instance of Regenerator 2000.
    -   Ideal for local assistants (e.g., Claude Desktop, Claude Code) that spawn the server as a subprocess.
    -   **Flag**: `--mcp-server-stdio`
    -   Experimental mode, mostly used for testing.


## Configuration

### 1. Start the Server

Before connecting any client, you must start Regenerator 2000 in **Server Mode**. This opens the TUI and starts the HTTP server, allowing you to use the interface while the AI interacts with it.

```bash
# Open your project with the server enabled
regenerator2000 --mcp-server your_project.d64
```

The server will listen on `http://127.0.0.1:3000/mcp` by default.

### 2. Configure Client

#### Claude Code / Claude Desktop

To use Regenerator 2000 with Claude, add the configuration to your `claude_desktop_config.json` or `config.json`.

**Location:**
-   **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
-   **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

**Configuration:**

```json
{
  "mcpServers": {
    "regenerator": {
      "url": "http://127.0.0.1:3000/mcp"
    }
  }
}
```

!!! tip
    Ensure the `regenerator2000` application is running with `--mcp-server` **before** you start Claude Desktop or Claude Code, as they will attempt to connect on startup.

#### Gemini CLI

To use Gemini CLI with the running server, simply provide the URL to the connect command or configuration:

```bash
# Example connection command
gemini-cli connect http://127.0.0.1:3000/mcp
```

## Usage Examples

Once connected, you can prompt the AI to perform complex tasks.

### Analysis

> "Analyze the loaded binary, and add a line comment on top of every function describing what it does."

> "Find all 'JSR $FFD2' calls (CHROUT) and document what is being printed before each call."

> "Analyze the function at $C000. It seems to be checking sprite collisions. Rename variables accordingly."

### Exploration

> "List available tools to see what you can do."

> "Read the disassembly at address $0800 to $0850 and explain what it does."

> "Check the memory dump at $D000. Is this a C64 I/O area?"

### Modification

> "Rename the subroutine at $1000 to 'init_music_player'."

> "Add a comment to address $0810 saying 'Checks for fire button'."

> "Mark the range $2000-$2100 as a byte table."

## Use Cases

### 1. The AI Copilot (HTTP Mode)
Run Regenerator 2000 with `--mcp-server`. You work in the TUI, navigating and making manual edits. Simultaneously, you ask your AI assistant (connected via HTTP) to:
*   "Document this function I'm looking at."
*   "Find where this variable is modified."
*   "Interpret this confusing block of code."

The AI's changes (renaming labels, adding comments) appear instantly in your TUI.

### 2. Automated Deep Dive (Stdio Mode)
Configure Claude Desktop with a specific project file.
*   **Prompt**: "Analyze `adventure.prg`. Find the high score table location and the routine that updates it."
*   **Response**: The AI loads the context, uses search tools, reads memory, and produces a report without you needing to open the interface.
