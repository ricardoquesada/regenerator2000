# MCP Integration

Regenerator 2000 supports the **Model Context Protocol (MCP)**, allowing AI agents like **Antigravity**, **Claude Code**, and **Gemini CLI** to interact directly with your analysis project.

Through the MCP server, an AI assistant can:

- **Read** disassembly, memory dump, and project state.
- **Modify** comments, labels, and block types.
- **Navigate** move cursor.
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

!!! tip

    Ensure the `regenerator2000` application is running with `--mcp-server` **before** you start Claude Code, Gemini or Antigravity as they will attempt to connect on startup.

#### Claude Code

To use Regenerator 2000 with Claude Code:

```shell
claude mcp add -t http regenerator2000 http://127.0.0.1:3000/mcp
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

## Common Workflows

The true power of the MCP server comes from combining the AI's semantic understanding with Regenerator 2000's analysis tools.

### 1. Analyze a Routine

**Goal**: Understand what a specific subroutine does, rename variables, and document it.

!!! example "Prompt"

    > "Analyze the routine at $C000. It seems to be checking sprite collisions. Rename variables accordingly and add comments explaining the logic."

**What happens**: The AI reads the disassembly at `$C000`, follows the code flow, identifies memory accesses (e.g., `$D01E` for sprite collision), and uses `r2000_set_label_name` and `r2000_set_comment` to update your project.

### 2. Identify Code vs. Data

**Goal**: You have a large binary and don't know where the code ends and graphics begin.

!!! example "Prompt"

    > "Scan the region from $2000 to $4000. Identify any valid code sequences and mark the rest as byte data. If you see patterns resembling text, mark them as PETSCII."

**What happens**: The AI uses `r2000_read_region` or `r2000_get_analyzed_blocks` to inspect the memory. It then iteratively applies `r2000_set_data_type` with `"data_type": "code"` or `"byte"` based on its analysis.

### 3. Trace Cross-References

**Goal**: Find everywhere a specific global variable or function is used.

!!! example "Prompt"

    > "Find all `JSR $FFD2` (CHROUT) calls. For each call, document what character or value is being printed before the call."

**What happens**: The AI queries `r2000_get_cross_references` for `$FFD2`, then inspects the instructions immediately preceding each call to deduce the arguments (e.g., `LDA #$05`).

### 4. Semantic Navigation

**Goal**: Move around the project using natural language descriptions instead of addresses.

!!! example "Prompt"

    > "Jump to the high score table."

    > "Go to the interrupt handler."

**What happens**: The AI searches the symbol table or analyzes the code to find the likely candidate, then uses `r2000_jump_to_address` to move your viewport.

## Agent Skills

**Skills** are reusable instruction sets that guide the AI through complex, multi-step tasks. They live in your project's `.agent/skills/` directory and are automatically discovered by compatible AI agents (e.g., Antigravity).

### Installation

To install all available skills at once, run:

```shell
cp -r .agent/skills/r2000* /path/to/your/project/.agent/skills/
```

The resulting layout should look like:

```
your_project/
└── .agent/
    └── skills/
        └── r2000-analyze-blocks/   (or whichever skill you copied)
            └── SKILL.md
```

### Recommended Workflow

For the best results, we recommend applying these skills in the following order:

1. **Macro Analysis**: Use `r2000-analyze-blocks` to break down the binary into code and data regions.
2. **Micro Analysis (Control Flow)**: Use `r2000-analyze-routine` to understand the logic of individual subroutines.
3. **Micro Analysis (Data Flow)**: Use `r2000-analyze-symbol` to identify variables, pointers, and hardware registers.

### Skill: `r2000-analyze-blocks`

Scans a memory range (or the entire binary) and converts each region to the correct block type — separating code from data, text from tables, and pointers from raw bytes. This is the foundational reverse-engineering pass you run on a freshly loaded binary.

**What it does:**

1. Determines the scope (user-specified range or full binary via `r2000_get_binary_info`).
2. Reads existing block classifications with `r2000_get_analyzed_blocks`.
3. Performs multiple passes: traces code from entry points, identifies text strings, detects data tables, and classifies remaining regions.
4. Applies conversions in batches using `r2000_batch_execute` for efficiency.
5. Uses `r2000_toggle_splitter` to correctly separate adjacent tables of the same type.
6. Optionally labels and comments notable regions (entry points, strings, jump tables).
7. Reports a summary of all blocks found and flags ambiguous regions for human review.

**Supported block types:** Code, Byte, Word, Address, PETSCII Text, Screencode Text, Lo/Hi Address, Hi/Lo Address, Lo/Hi Word, Hi/Lo Word, External File, Undefined.

!!! example "Prompt"

    > "Analyze blocks", "Convert blocks", "Identify data regions", "Classify the program"

### Skill: `r2000-analyze-routine`

Analyzes a disassembly subroutine to determine its purpose by examining code, cross-references, and memory usage.

**What it does:**

1. Identifies the bounds of the routine (entry point → `RTS`/`JMP`/`RTI`).
2. Reads and interprets the instructions, detecting loops, KERNAL calls, and hardware register accesses.
3. Checks cross-references to understand the call context.
4. Analyzes data usage (Zero Page variables, hardware registers, etc.).
5. Synthesizes a summary of purpose, inputs, outputs, and side effects.
6. Optionally documents the routine by adding a structured comment block above the entry point and renaming the label.

!!! example "Prompt"

    > "Analyze this routine", "What does this function do?"

### Skill: `r2000-analyze-symbol`

Analyzes a specific memory address or label to determine its purpose (variable, flag, pointer, hardware register) by examining its cross-references and usage patterns.

**What it does:**

1. Determines the target address/label and the platform (C64, VIC-20, etc.).
2. Checks if the address corresponds to a known hardware register.
3. Analyzes all cross-references to see how the symbol is used (read, write, modify).
4. Identifies patterns: Pointers (indirect indexed), Flags (boolean checks), Counters (loops), State variables.
5. Renames the symbol to a descriptive name (`snake_case` for variables, `CapsExpr` for hardware/constants).
6. Adds comments using `r2000_set_comment` (type `"line"` for definitions, type `"side"` for usages) to document the data flow.

!!! example "Prompt"

    > "Analyze this label", "What is this variable?", "Trace this address"

## Available Tools

The server currently exposes **20** tools.

### `r2000_batch_execute`

Executes multiple tool calls sequentially in a single request. Use only when you have 5+ independent operations to perform at once (e.g. marking many regions, renaming many labels). Do not use for operations that depend on each other's results.

**Arguments:**

| Name    | Type    | Description                                 | Required |
| :------ | :------ | :------------------------------------------ | :------: |
| `calls` | `array` | List of tool calls to execute sequentially. |   Yes    |

### `r2000_get_address_details`

Returns detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.

**Arguments:**

| Name      | Type      | Description                              | Required |
| :-------- | :-------- | :--------------------------------------- | :------: |
| `address` | `integer` | The memory address to inspect (decimal). |   Yes    |

### `r2000_get_all_comments`

Returns all user-defined comments (line and side) and their addresses. Each entry has 'address' (integer), 'type' ('line' or 'side'), and 'comment' (string).

_No arguments._

### `r2000_get_analyzed_blocks`

Returns the list of memory blocks as analyzed, including their range and type. Respects splitters.

**Arguments:**

| Name         | Type     | Description                                                                 | Required |
| :----------- | :------- | :-------------------------------------------------------------------------- | :------: |
| `block_type` | `string` | Optional filter to return only blocks of a specific type. Case-insensitive. |    No    |

### `r2000_get_binary_info`

Returns the origin address, size in bytes, target platform (e.g. 'Commodore 64'), filename, and user-provided description of the loaded binary.

_No arguments._

### `r2000_get_cross_references`

Get a list of addresses that reference the given address (e.g. JSRs, JMPs, loads).

**Arguments:**

| Name      | Type      | Description                                         | Required |
| :-------- | :-------- | :-------------------------------------------------- | :------: |
| `address` | `integer` | The target address to find references to (decimal). |   Yes    |

### `r2000_get_disassembly_cursor`

Returns the memory address of the current cursor position in the disassembly view.

_No arguments._

### `r2000_get_symbol_table`

Returns a list of all defined labels (user and system) and their addresses.

_No arguments._

### `r2000_jump_to_address`

Moves the disassembly cursor to a specific memory address and scrolls the view to make it visible. Also keeps the jump history.

**Arguments:**

| Name      | Type      | Description                              | Required |
| :-------- | :-------- | :--------------------------------------- | :------: |
| `address` | `integer` | The target address to jump to (decimal). |   Yes    |

### `r2000_read_region`

Get disassembly or hexdump text for a specific memory range.

**Arguments:**

| Name            | Type      | Description                            | Required |
| :-------------- | :-------- | :------------------------------------- | :------: |
| `end_address`   | `integer` | End address (inclusive), decimal.      |   Yes    |
| `start_address` | `integer` | Start address (inclusive), decimal.    |   Yes    |
| `view`          | `string`  | The view to return. Default: 'disasm'. |    No    |

### `r2000_read_selected`

Get disassembly or hexdump for the range currently selected in the UI. If nothing is selected, returns the instruction/row under the cursor.

**Arguments:**

| Name   | Type     | Description                            | Required |
| :----- | :------- | :------------------------------------- | :------: |
| `view` | `string` | The view to return. Default: 'disasm'. |    No    |

### `r2000_redo`

Redoes the latest undone operation.

_No arguments._

### `r2000_save_project`

Saves the current project state to the existing .regen2000proj file. Only works if the project was previously loaded from or saved to a project file.

_No arguments._

### `r2000_search_memory`

Search for a sequence of bytes or a text string in the memory. Returns a list of addresses where the sequence is found.

**Arguments:**

| Name       | Type     | Description                                                                                 | Required |
| :--------- | :------- | :------------------------------------------------------------------------------------------ | :------: |
| `encoding` | `string` | Encoding for the query. Defaults to 'hex' if query looks like hex bytes, otherwise 'ascii'. |    No    |
| `query`    | `string` | The search query. For hex: space-separated bytes, e.g. 'A9 00'. For text: plain string.     |   Yes    |

### `r2000_set_comment`

Adds a comment at a specific address. 'line' comments appear on their own line before the instruction (supports multi-line). 'side' comments appear inline on the same line as the instruction.

**Arguments:**

| Name      | Type      | Description                                                                                        | Required |
| :-------- | :-------- | :------------------------------------------------------------------------------------------------- | :------: |
| `address` | `integer` | The memory address for the comment (decimal, e.g. 4096 for $1000).                                 |   Yes    |
| `comment` | `string`  | The comment text. Do not include the ';' prefix.                                                   |   Yes    |
| `type`    | `string`  | 'line' = comment on its own line before the instruction. 'side' = inline comment on the same line. |   Yes    |

### `r2000_set_data_type`

Sets the data type for a memory region. Use this to mark regions as code, bytes, addresses, text, split tables, etc.

**Arguments:**

| Name            | Type      | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Required |
| :-------------- | :-------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------: |
| `data_type`     | `string`  | code=MOS 6502 instructions; byte=raw 8-bit data (sprites, charset, tables, unknowns); word=16-bit LE values; address=16-bit LE pointers (creates X-Refs, use for jump tables/vectors); petscii=PETSCII text; screencode=Screen code text (data written to $0400); lo_hi_address=split address table, low bytes first then high bytes (even count required); hi_lo_address=split address table, high bytes first (even count required); lo_hi_word=split word table, low bytes first (e.g. SID freq tables); hi_lo_word=split word table, high bytes first; external_file=large binary blob (SID, bitmap, charset) to export as-is; undefined=reset region to unknown state. |   Yes    |
| `end_address`   | `integer` | End of the memory region (inclusive), decimal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |   Yes    |
| `start_address` | `integer` | Start of the memory region (inclusive), decimal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |   Yes    |

### `r2000_set_label_name`

Sets a user-defined label at a specific MOS 6502 memory address. Use this to name functions, variables, or jump targets to make the disassembly more readable.

**Arguments:**

| Name      | Type      | Description                                                                      | Required |
| :-------- | :-------- | :------------------------------------------------------------------------------- | :------: |
| `address` | `integer` | The memory address where the label should be set (decimal, e.g. 4096 for $1000). |   Yes    |
| `name`    | `string`  | The label name (e.g. 'init_screen', 'loop_start').                               |   Yes    |

### `r2000_set_operand_format`

Sets the display format for immediate values (operands) at a specific address. Useful for visualizing bitmasks.

**Arguments:**

| Name      | Type      | Description                               | Required |
| :-------- | :-------- | :---------------------------------------- | :------: |
| `address` | `integer` | The address of the instruction (decimal). |   Yes    |
| `format`  | `string`  | hex=$00, decimal=0, binary=%00000000.     |   Yes    |

### `r2000_toggle_splitter`

Toggles a Splitter at a specific address. Splitters prevent the auto-merger from combining adjacent blocks of the same type. Crucial for separating adjacent Lo/Hi table halves.

**Arguments:**

| Name      | Type      | Description                                                        | Required |
| :-------- | :-------- | :----------------------------------------------------------------- | :------: |
| `address` | `integer` | The memory address where the splitter should be toggled (decimal). |   Yes    |

### `r2000_undo`

Undoes the latest operation.

_No arguments._
