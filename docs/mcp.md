# MCP Integration

Regenerator 2000 supports the **Model Context Protocol (MCP)**, allowing AI agents like **Claude Desktop**, **Claude Code**, and **Gemini CLI** to interact directly with your analysis project.

Through the MCP server, an AI assistant can:

- **Read** disassembly, memory dump, and project state.
- **Modify** comments, labels, and block types.
- **Navigate** the UI (move cursor, switch views).
- **Analyze** code structure and control flow.

!!! note "What is MCP?"

    The Model Context Protocol (MCP) is an open standard that enables AI models to interact with local applications and contexts. Learn more at [modelcontextprotocol.io](https://modelcontextprotocol.io).

## Modes of Operation

Regenerator 2000 supports two MCP transport modes:

1.  **HTTP Mode (Streamable/SSE)**:
    - Runs the MCP server over HTTP using Server-Sent Events (SSE).
    - Allows concurrent TUI usage (User + AI working together).
    - **Flag**: `--mcp-server` (starts TUI + Server on port 3000)
    - **Flag**: `--mcp-server --headless` (starts Headless Server on port 3000)
    - **Endpoint**: `http://127.0.0.1:3000/mcp`
    - Recommended option!

2.  **Stdio Mode (Headless)**:
    - Starts a headless instance of Regenerator 2000.
    - Ideal for local assistants (e.g., Claude Desktop, Claude Code) that spawn the server as a subprocess.
    - **Flag**: `--mcp-server-stdio`, implies `--headless`.
    - Experimental mode, mostly used for testing.

## Configuration

  <iframe width="560" height="315" src="https://www.youtube.com/embed/_HW2d7kNCQw" title="YouTube video player" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share" referrerpolicy="strict-origin-when-cross-origin" allowfullscreen></iframe>

### 1. Start the Server

Before connecting any client, you must start Regenerator 2000 with **MCP Server** enabled. This opens the TUI and starts the HTTP server, allowing you to use the interface while the AI interacts with it.

```shell
# Open your project with the server enabled
regenerator2000 --mcp-server your_project.regen2000proj
```

The server will listen on `http://127.0.0.1:3000/mcp` by default.

### 2. Configure Client

#### Claude Code

To use Regenerator 2000 with Claude Code:

```shell
claude mcp add regenerator2000 http://127.0.0.1:3000/mcp
```

Or, alternatively, add the following to your `claude.json`:

```json
{
  "mcpServers": {
    "regenerator2000": {
      "type": "http",
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
gemini mcp add regenerator2000 http://127.0.0.1:3000/mcp --scope user -t http
```

Or, alternatively, add the following to `~/.gemini/settings.json`:

```json
{
  "mcpServers": {
    "regenerator2000": {
      "url": "http://localhost:3000/mcp",
      "type": "http"
    }
  }
}
```

#### Antigravity

To use Antigravity with the running server, add the following to `~/.gemini/antigravity/mcp_config.json`:

```json
{
  "mcpServers": {
    "regenerator2000": {
      "serverUrl": "http://127.0.0.1:3000/mcp",
      "headers": {
        "Accept": "application/json, text/event-stream",
        "Content-Type": "application/json"
      }
    }
  }
}
```

## Usage Examples

!!! note

    If it fails to use the "Regenerator2000 MCP", you can prefix the first prompt with:

    > "Using regenerator2000 mcp server, ..."

Once connected, you can prompt the AI to perform complex tasks.

### Analysis

> "Analyze this routine"

> "Find all 'JSR $FFD2' calls (CHROUT) and document what is being printed before each call."

> "Analyze the routine at $C000. It seems to be checking sprite collisions. Rename variables accordingly."

> "Analyze in detail the routine that I'm looking at. Add comments and labels as needed. Convert regions to code, byte, word, petscii, screencode, etc as needed."

### Exploration

> "List available tools to see what you can do."

### Navigation

> "Jump to address $1000."

> "Go to the 'init_screen' label."

## Agent Skills

**Skills** are reusable instruction sets that guide the AI through complex, multi-step tasks. They live in your project's `.agent/skills/` directory and are automatically discovered by compatible AI agents (e.g., Antigravity).

To install a skill, copy its folder from the Regenerator 2000 source tree into your own project:

```shell
# From the Regenerator 2000 repo, copy the desired skill into your project
cp -r .agent/skills/analyze-routine /path/to/your/project/.agent/skills/
```

The resulting layout should look like:

```
your_project/
└── .agent/
    └── skills/
        └── analyze-routine/
            └── SKILL.md
```

### `analyze-routine`

Analyzes a disassembly subroutine to determine its purpose by examining code, cross-references, and memory usage.

**What it does:**

1. Identifies the bounds of the routine (entry point → `RTS`/`JMP`/`RTI`).
2. Reads and interprets the instructions, detecting loops, KERNAL calls, and hardware register accesses.
3. Checks cross-references to understand the call context.
4. Analyzes data usage (Zero Page variables, hardware registers, etc.).
5. Synthesizes a summary of purpose, inputs, outputs, and side effects.
6. Optionally documents the routine by adding a structured comment block above the entry point and renaming the label.

**Example prompts:**

> "Analyze this routine"

> "Analyze the routine at $C000. It seems to be checking sprite collisions. Rename variables accordingly."

> "Analyze in detail the routine that I'm looking at. Add comments and labels as needed."

## Use Cases

### 1. The AI Copilot (HTTP Mode)

Run Regenerator 2000 with `--mcp-server`. You work in the TUI, navigating and making manual edits. Simultaneously, you ask your AI assistant (connected via HTTP) to:

- "Document this function I'm looking at."
- "Interpret this confusing block of code."

The AI's changes (renaming labels, adding comments) appear instantly in your TUI.

### 2. Automated Deep Dive (Stdio Mode)

Configure Claude Desktop with a specific project file.

- **Prompt**: "Analyze the loaded program. Find the high score table location and the routine that updates it."
- **Response**: The AI loads the context, uses search tools, reads memory, and produces a report without you needing to open the interface.

## Available Tools

The server currently exposes **34** tools.

### `r2000_batch_execute`

Executes multiple tools in a single request. Use this for bulk operations like renaming multiple labels to avoid round-trip latency.

**Arguments:**

| Name    | Type    | Description                                 | Required |
| :------ | :------ | :------------------------------------------ | :------: |
| `calls` | `array` | List of tool calls to execute sequentially. |   Yes    |

### `r2000_convert_region_to_address`

Marks a memory region as Address (16-bit Little-Endian pointers). This type explicitly tells the analyzer that the value points to a location in memory, creating Cross-References (X-Refs). Essential for Jump Tables, vectors, and pointers.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_bytes`

Marks a memory region as raw Data Byte (8-bit values). Use this for sprite data, bitmpa data, charset data, distinct variables, 8-bit tables, or memory regions where the data format is unknown.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_code`

Marks a memory region as executable code. This forces the disassembler to interpret bytes as MOS 6502 instructions. Use this for all executable machine code.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_external_file`

Marks a memory region as External File (binary data). Use for large chunks of included binary data (like music SID files, raw bitmaps, or character sets) that should be exported as included binary files.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_hi_lo_address`

Marks a memory region as a Hi/Lo Address Table. Must have an even number of bytes. The first half determines the high bytes, the second half the low bytes of the addresses.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_hi_lo_word`

Marks a memory region as a Hi/Lo Word Table. Must have an even number of bytes. The first half contains the high bytes, the second half the low bytes of the words. Use case: SID frequency tables.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_lo_hi_address`

Marks a memory region as a Lo/Hi Address Table. Must have an even number of bytes. The first half determines the low bytes, the second half the high bytes of the addresses

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_lo_hi_word`

Marks a memory region as a Lo/Hi Word Table. Must have an even number of bytes. The first half contains the low bytes, the second half the high bytes of the words. Use case: SID frequency tables.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_petscii`

Marks a memory region as PETSCII encoded text. Use for game messages, high score names, or print routines.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_screencode`

Marks a memory region as Commodore Screen Code encoded text (Matrix codes). Use for data directly copied to Screen RAM ($0400).

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_undefined`

Resets the block to an 'Unknown' / 'Undefined' state.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_convert_region_to_words`

Marks a memory region as Data Word (16-bit Little-Endian values). Use this for 16-bit variables or math constants.

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_get_address_details`

Returns detailed information about a specific memory address, including instruction semantics, cross-references, and state metadata. Use this to dive deep into a specific instruction or data point.

**Arguments:**

| Name      | Type                      | Description                    | Required |
| :-------- | :------------------------ | :----------------------------- | :------: |
| `address` | `integer` &#124; `string` | The memory address to inspect. |   Yes    |

### `r2000_get_all_comments`

Returns a list of all user-defined comments (both line and side comments) and their addresses. The returned JSON is a list of objects, each containing 'address' (integer), 'type' (string: 'line' or 'side'), and 'comment' (string).

_No arguments._

### `r2000_get_analyzed_blocks`

Returns the list of memory blocks as analyzed, including their range and type. Respects splitters. Supported types: Code, Byte, Word, Address, PETSCII Text, Screencode Text, Lo/Hi Address, Hi/Lo Address, Lo/Hi Word, Hi/Lo Word, External File, Undefined.

**Arguments:**

| Name         | Type     | Description                                                                 | Required |
| :----------- | :------- | :-------------------------------------------------------------------------- | :------: |
| `block_type` | `string` | Optional filter to return only blocks of a specific type. Case-insensitive. |    No    |

### `r2000_get_binary_info`

Returns the origin address and size of the analyzed binary in bytes.

_No arguments._

### `r2000_get_cross_references`

Get a list of addresses that reference the given address (e.g. JSRs, JMPs, loads).

**Arguments:**

| Name      | Type                      | Description                               | Required |
| :-------- | :------------------------ | :---------------------------------------- | :------: |
| `address` | `integer` &#124; `string` | The target address to find references to. |   Yes    |

### `r2000_get_disassembly_cursor`

Returns the memory address of the current cursor position in the disassembly view.

_No arguments._

### `r2000_get_symbol_table`

Returns a list of all defined labels (user and system) and their addresses.

_No arguments._

### `r2000_jump_to_address`

Moves the disassembly cursor to a specific memory address and scrolls the view to make it visible. Also keeps the history of jumps.

**Arguments:**

| Name      | Type                      | Description                    | Required |
| :-------- | :------------------------ | :----------------------------- | :------: |
| `address` | `integer` &#124; `string` | The target address to jump to. |   Yes    |

### `r2000_read_disasm_region`

Get MOS 6502 disassembly text for a specific memory range. Supports decimal (4096), hex (0x1000) and 6502 hex ($1000).

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_read_hexdump_region`

Get raw hexdump view for a specific C64 memory range. Supports decimal (4096), hex (0x1000) and 6502 hex ($1000).

**Arguments:**

| Name            | Type                      | Description | Required |
| :-------------- | :------------------------ | :---------- | :------: |
| `end_address`   | `integer` &#124; `string` | -           |   Yes    |
| `start_address` | `integer` &#124; `string` | -           |   Yes    |

### `r2000_read_selected_disasm`

Get disassembly text for the range currently selected in the UI. If no range is selected, it returns the instruction under the cursor.

_No arguments._

### `r2000_read_selected_hexdump`

Get hexdump view for the range currently selected in the UI. If no range is selected, it returns the byte row under the cursor.

_No arguments._

### `r2000_redo`

Redoes the latest undone operation. Use this command to re-apply changes that were previously undone.

_No arguments._

### `r2000_save_project`

Saves the current project state to the existing .regen2000proj file. This tool only works if the project was previously loaded from or saved to a project file. It does not accept a filename for security reasons.

_No arguments._

### `r2000_search_memory`

Search for a sequence of bytes or a text string in the memory. Returns a list of addresses where the sequence is found.

**Arguments:**

| Name       | Type     | Description                                                                                                                                                                       | Required |
| :--------- | :------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------: |
| `encoding` | `string` | The encoding to use for text search. Options: 'ascii', 'petscii', 'screencode', 'hex'. Default is 'hex' if query looks like hex, otherwise tries to guess or defaults to 'ascii'. |    No    |
| `query`    | `string` | The search query. can be a hex string (e.g. 'A9 00'), or text.                                                                                                                    |   Yes    |

### `r2000_set_label_name`

Sets a user-defined label at a specific MOS 6502 memory address. Use this to name functions, variables, or jump targets to make the C64 disassembly more readable.

**Arguments:**

| Name      | Type                      | Description                                                                    | Required |
| :-------- | :------------------------ | :----------------------------------------------------------------------------- | :------: |
| `address` | `integer` &#124; `string` | The memory address where the label should be set (e.g., 4096, 0x1000 or $1000) |   Yes    |
| `name`    | `string`                  | The name of the label (e.g., 'init_screen', 'loop_start')                      |   Yes    |

### `r2000_set_line_comment`

Adds a line comment at a specific address. Line comments appear on their own line before the instruction and can act as visual separators. It supports multi-line comments.

**Arguments:**

| Name      | Type                      | Description                                                      | Required |
| :-------- | :------------------------ | :--------------------------------------------------------------- | :------: |
| `address` | `integer` &#124; `string` | The memory address for the comment (e.g., 4096, 0x1000 or $1000) |   Yes    |
| `comment` | `string`                  | The comment text                                                 |   Yes    |

### `r2000_set_operand_format`

Sets the display format for immediate values (operands) at a specific address. Useful for visualizing bitmasks or characters.

**Arguments:**

| Name      | Type                      | Description                                                                    | Required |
| :-------- | :------------------------ | :----------------------------------------------------------------------------- | :------: |
| `address` | `integer` &#124; `string` | The address of the instruction.                                                |   Yes    |
| `format`  | `string`                  | The desired format. Options: 'hex' ($00), 'decimal' (0), 'binary' (%00000000). |   Yes    |

### `r2000_set_side_comment`

Adds a side comment to a specific address. Side comments appear on the same line as the instruction.

**Arguments:**

| Name      | Type                      | Description                                                      | Required |
| :-------- | :------------------------ | :--------------------------------------------------------------- | :------: |
| `address` | `integer` &#124; `string` | The memory address for the comment (e.g., 4096, 0x1000 or $1000) |   Yes    |
| `comment` | `string`                  | The comment text                                                 |   Yes    |

### `r2000_toggle_splitter`

Toggles a Splitter at a specific address. Splitters prevent the auto-merger from combining adjacent blocks of the same type. Crucial for separating adjacent Lo/Hi tables.

**Arguments:**

| Name      | Type                      | Description                                                                           | Required |
| :-------- | :------------------------ | :------------------------------------------------------------------------------------ | :------: |
| `address` | `integer` &#124; `string` | The memory address where the splitter should be toggled (e.g., 4096, 0x1000 or $1000) |   Yes    |

### `r2000_undo`

Undoes the latest operation. Use this command to revert changes if you made a mistake or want to go back to a previous state.

_No arguments._
