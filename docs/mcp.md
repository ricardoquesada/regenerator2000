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

**What happens**: The AI reads the disassembly at `$C000`, follows the code flow, identifies memory accesses (e.g., `$D01E` for sprite collision), and uses `r2000_set_label_name` and `r2000_set_line_comment` to update your project.

### 2. Identify Code vs. Data

**Goal**: You have a large binary and don't know where the code ends and graphics begin.

!!! example "Prompt"

    > "Scan the region from $2000 to $4000. Identify any valid code sequences and mark the rest as byte data. If you see patterns resembling text, mark them as PETSCII."

**What happens**: The AI uses `r2000_read_disasm_region` or `r2000_get_analyzed_blocks` to inspect the memory. It then iteratively applies `r2000_convert_region_to_code` or `r2000_convert_region_to_bytes` based on its analysis.

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
6. Adds line comments (definitions) and side comments (usages) to document the data flow.

!!! example "Prompt"

    > "Analyze this label", "What is this variable?", "Trace this address"

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

Returns the origin address, size of the analyzed binary in bytes, the target platform (e.g. 'Commodore 64', 'Commodore 128'), the filename if available, and a user-provided description.

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
