# MCP Integration

Regenerator 2000 supports the **Model Context Protocol (MCP)**. AI agents such as **Antigravity** and **Claude Code** connect to the MCP server to inspect and modify your disassembly project directly.

Through the MCP server, an AI assistant performs four core operations:

-   **Read** disassembly text, memory dumps, and project state.
-   **Modify** comments, labels, and block types.
-   **Navigate** to memory addresses and move the cursor.
-   **Analyze** code structure and control flow.

!!! note "What is MCP?"

    The Model Context Protocol (MCP) is an open standard that connects AI models to local applications and data sources. See [modelcontextprotocol.io](https://modelcontextprotocol.io) for specification details.

## Modes of Operation

Regenerator 2000 supports two MCP transport modes:

1.  **HTTP Mode (Streamable/SSE)** (Recommended):

    -   Runs the MCP server over HTTP with Server-Sent Events (SSE).
    -   Supports concurrent TUI and AI usage in the same session.
    -   **Flag**: `--mcp-server` (starts the TUI and HTTP server on port `3000`).
    -   **Flag**: `--mcp-server --headless` (starts the headless HTTP server on port `3000`).
    -   **Endpoint**: `http://127.0.0.1:3000/mcp`

2.  **Stdio Mode (Headless)**:

    -   Starts a headless instance of Regenerator 2000 over standard input/output.
    -   Works with local AI clients (such as Claude Desktop or Claude Code) that spawn the server as a subprocess.
    -   **Flag**: `--mcp-server-stdio` (implies `--headless`).

## Configuration

<iframe width="560" height="315" src="https://www.youtube.com/embed/_HW2d7kNCQw" title="YouTube video player" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share" referrerpolicy="strict-origin-when-cross-origin" allowfullscreen></iframe>

### 1. Start the Server

Start Regenerator 2000 with the `--mcp-server` flag before you connect an MCP client. This command opens the TUI and starts the HTTP server on `http://127.0.0.1:3000/mcp`:

```shell
# Open your project with the server enabled
regenerator2000 --mcp-server your_project.regen2000proj
```

### 2. Configure the Client

!!! tip

    Start `regenerator2000 --mcp-server` **before** you start Claude Code or Antigravity so the client connects at startup.

#### Claude Code

Run this command to register Regenerator 2000 with Claude Code:

```shell
claude mcp add -t http regenerator2000 http://127.0.0.1:3000/mcp
```

Alternatively, add this block to `claude.json`:

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

#### Antigravity CLI

Run this command to register the running server with Antigravity CLI:

```bash
# Example connection command
agy mcp add regenerator2000 http://127.0.0.1:3000/mcp --scope user -t http
```

Alternatively, add this block to `~/.gemini/antigravity/mcp_config.json`:

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

Combine AI semantic analysis with Regenerator 2000 tools to accelerate reverse engineering.

### 1. Analyze a Routine

**Goal**: Identify what a subroutine does, rename its variables, and add documentation comments.

!!! example "Prompt"

    > "Analyze the routine at $C000. It checks sprite collisions. Rename variables and add comments that explain the logic."

**Behavior**: The AI reads the disassembly at `$C000`, traces control flow, identifies hardware register accesses (such as `$D01E` for sprite collisions), and calls `r2000_set_label_name` and `r2000_set_comment` to update the project.

### 2. Identify Code vs. Data

**Goal**: Separate code from graphics and data tables in an unanalyzed memory range.

!!! example "Prompt"

    > "Scan the region from $2000 to $4000. Identify valid code sequences and mark the rest as byte data. Mark text strings as PETSCII."

**Behavior**: The AI calls `r2000_read_region` and `r2000_get_blocks` to inspect the memory range, then calls `r2000_set_data_type` with `"data_type": "code"`, `"byte"`, or `"petscii"`.

### 3. Trace Cross-References

**Goal**: Locate every reference to a global variable or subroutine.

!!! example "Prompt"

    > "Find all `JSR $FFD2` (CHROUT) calls. Document the character or value loaded before each call."

**Behavior**: The AI calls `r2000_get_cross_references` for `$FFD2`, inspects the instructions before each call site (such as `LDA #$05`), and adds side comments.

### 4. Semantic Navigation

**Goal**: Navigate the project using natural-language queries instead of hexadecimal addresses.

!!! example "Prompt"

    > "Jump to the high score table."

    > "Go to the interrupt handler."

**Behavior**: The AI searches the symbol table or disassembly text, locates the target address, and calls `r2000_jump_to_address` to move the TUI cursor.

## Agent Skills

**Skills** are reusable instruction files in `.agent/skills/` that guide AI agents through multi-step reverse-engineering workflows.

### Installation

Copy the skill folders into your project directory:

```shell
cp -r .agent/skills/r2000* /path/to/your/project/.agent/skills/
```

The target directory structure appears as follows:

```text
your_project/
└── .agent/
    └── skills/
        └── r2000-analyze-blocks/   (or whichever skill you copied)
            └── SKILL.md
```

!!! note "Claude Code users"

    Claude Code loads skills from `.claude/` instead of `.agent/`. Create a symbolic link so both Antigravity and Claude Code share the same skill files:

    ```shell
    ln -s .agent .claude
    ```

### Recommended Workflow

Run the analysis skills in this sequence:

1.  **Macro Analysis**: Run `r2000-analyze-blocks` to partition the binary into code and data blocks.
2.  **Micro Analysis (Control Flow)**: Run `r2000-analyze-routine` to document individual subroutines.
3.  **Micro Analysis (Data Flow)**: Run `r2000-analyze-symbol` to classify variables, pointers, and hardware registers.

Alternatively, run **`r2000-analyze-program`** to execute all three phases automatically with parallel subagents.

### Skill: `r2000-analyze-program`

Executes a complete analysis of the loaded binary. This skill runs block classification first and then spawns parallel subagents to analyze subroutines and data symbols:

1.  **Phase 0 — Gather Context**: Reads binary metadata, existing blocks, symbols, and line comments.
2.  **Phase 1 — Classify Blocks**: Runs `r2000-analyze-blocks` to classify memory regions (code, data, text, and tables) and refreshes state.
3.  **Phase 2 — Analyze Routines**: Finds unanalyzed subroutines (`s_XXXX` labels, uncommented `JSR` targets, `p_XXXX` pointers inside `Code` blocks, and the `start` entry point). Spawns a rolling window of up to 10 parallel subagents that run `r2000-analyze-routine`.
4.  **Phase 3 — Analyze Symbols**: Finds unanalyzed data symbols with auto-generated prefixes (`zpp_`, `zpf_`, `zpa_`, `p_`, `f_`, `a_`). Spawns up to 10 parallel subagents that run `r2000-analyze-symbol` to classify and rename each symbol.
5.  **Phase 4 — Save & Report**: Saves the project and generates a summary report of analyzed routines, renamed symbols, and ambiguous items.

!!! example "Prompt"

    > "Analyze this program", "Full analysis", "Analyze everything"

### Skill: `r2000-analyze-blocks`

Scans a memory range (or the entire binary) and converts each region to the appropriate block type:

1.  Reads the target address range via `r2000_get_binary_info`.
2.  Reads existing block classifications via `r2000_get_blocks`.
3.  Traces code from entry points, locates text strings, detects data tables, and classifies remaining regions.
4.  Applies block conversions in batches via `r2000_batch_execute`.
5.  Inserts splitters via `r2000_toggle_splitter` to separate adjacent tables of the same type.
6.  Labels and comments notable regions (entry points, strings, and jump tables).
7.  Outputs a summary of all classified blocks and flags ambiguous regions for manual review.

**Supported block types:** Code, Byte, Word, Address, PETSCII Text, Screencode Text, Lo/Hi Address, Hi/Lo Address, Lo/Hi Word, Hi/Lo Word, External File, Undefined.

!!! example "Prompt"

    > "Analyze blocks", "Convert blocks", "Identify data regions", "Classify the program"

### Skill: `r2000-analyze-routine`

Analyzes a subroutine by inspecting its instructions, cross-references, and memory accesses:

1.  Identifies the start and end addresses of the subroutine (entry point to `RTS`, `JMP`, or `RTI`).
2.  Decodes the instruction flow, loops, KERNAL calls, and hardware register accesses.
3.  Inspects cross-references to determine the call context.
4.  Analyzes zero-page variables and memory operands.
5.  Summarizes the purpose, inputs, outputs, and side effects of the routine.
6.  Adds a header comment block above the entry point and renames the subroutine label.

!!! example "Prompt"

    > "Analyze this routine", "What does this function do?"

### Skill: `r2000-analyze-symbol`

Analyzes a memory address or label by inspecting its cross-references and access patterns:

1.  Identifies the target address, current label, and target platform.
2.  Checks whether the address maps to a hardware register or ROM entry point.
3.  Inspects all cross-references to classify read, write, and read-modify-write operations.
4.  Classifies the symbol as a pointer, flag, loop counter, or state variable.
5.  Renames the symbol using `snake_case` for variables or `UPPER_CASE` for constants.
6.  Adds line and side comments via `r2000_set_comment` to document data flow.

!!! example "Prompt"

    > "Analyze this label", "What is this variable?", "Trace this address"

## Available Resources

The server exposes **6** resources:

-   `binary://main`: Raw PRG binary payload (including the 2-byte load address header) encoded as a base64 blob (`application/octet-stream`).
-   `disassembly://current`: Complete disassembled text of the active binary file (`text/x-asm`).
-   `hexdump://current`: Formatted memory hexdump of the active binary file (`text/plain`).
-   `symbols://current`: All labels, routines, and symbols defined in the active disassembly session (`application/json`).
-   `comments://current`: All user line and side comments in the active disassembly session (`application/json`).
-   `blocks://current`: All memory block classifications and byte ranges (`application/json`).

## Available Tools

The server exposes **28** tools.

### `r2000_add_scope`

Adds a lexical scope over the specified memory range to group a subroutine. New symbols inside this range default to local scope and require explicit scope qualification from outside. Regenerator 2000 does not support nested scopes.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `end_address` | `integer` | Inclusive end address of the scope in decimal. | Yes |
| `start_address` | `integer` | Inclusive start address of the scope in decimal. | Yes |

### `r2000_apply_enum_usage`

Applies or clears an enum usage for an immediate operand at a specific address.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Instruction address in decimal. | Yes |
| `name` | `string` | Name of the enum to apply, or an empty string to clear. | No |

### `r2000_batch_execute`

Executes multiple tool calls sequentially in a single request. Use this tool to batch 5 or more independent operations.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `calls` | `array` | List of tool calls to execute sequentially. | Yes |

### `r2000_create_project_enum`

Creates a project-specific enum definition inside the project file.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `description` | `string` | Optional summary of the enum purpose. | No |
| `name` | `string` | Unique alphanumeric identifier. | Yes |
| `variants` | `object` | Map of numeric strings (decimal, hex `0x`/`$`, or binary `0b`/`%`) to variant names. | Yes |

### `r2000_delete_project_enum`

Deletes a project-specific enum definition.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `force` | `boolean` | Deletes the enum even when instructions reference it (default: `false`). | No |
| `name` | `string` | Name of the enum to delete. | Yes |

### `r2000_disassemble`

Runs recursive flow disassembly from the specified start address.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Start address to disassemble in decimal. | Yes |

### `r2000_get_address_details`

Returns instruction semantics, cross-references, labels, comments, and block type for a specific memory address.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Memory address to inspect in decimal. | Yes |

### `r2000_get_binary_info`

Returns the origin address, size, target platform, filename, description, Shannon entropy (values above `7.2` indicate compression), and illegal-opcode hint.

_No arguments._

### `r2000_get_blocks`

Returns all memory blocks with their address ranges and block types (`Code`, `Byte`, `Word`, `Address`, `PETSCII`, `Screencode`, `Lo/Hi Address`, `Hi/Lo Address`, `Lo/Hi Word`, `Hi/Lo Word`, `External File`, or `Undefined`). This tool respects user splitters.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `block_type` | `string` | Case-insensitive filter that returns only blocks of a specific type. | No |

### `r2000_get_comments`

Returns user-defined comments and their addresses with optional filters.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `addresses` | `array` | Optional list of memory addresses in decimal. | No |
| `end_address` | `integer` | Optional inclusive upper address bound in decimal. | No |
| `start_address` | `integer` | Optional inclusive lower address bound in decimal. | No |
| `type` | `string` | Optional comment type filter (`'line'` or `'side'`). | No |

### `r2000_get_cross_references`

Returns all addresses that reference the target address (such as `JSR`, `JMP`, and memory load/store instructions).

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Target address to query in decimal. | Yes |

### `r2000_get_disassembly_cursor`

Returns the memory address of the current cursor position in the Disassembly View.

_No arguments._

### `r2000_get_symbols`

Returns defined labels and their addresses. Without arguments, this tool returns all symbols. Optional filters narrow the results using logical `AND`: `names` resolves specific label names, `start_address` and `end_address` restrict the address range, and `kind` filters by label source.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `end_address` | `integer` | Optional inclusive upper address bound in decimal. | No |
| `kind` | `string` | Optional label source filter: `'user'` (user-defined), `'system'` (KERNAL and hardware registers), or `'auto'` (analyzer labels such as `s_C000`). | No |
| `names` | `array` | Optional case-sensitive list of label names to match. | No |
| `start_address` | `integer` | Optional inclusive lower address bound in decimal. | No |

### `r2000_jump_to_address`

Moves the disassembly cursor to a specific memory address, scrolls the viewport, and records the jump in navigation history.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Target address to jump to in decimal. | Yes |

### `r2000_read_region`

Returns disassembly or hexdump text for a specific memory range.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `end_address` | `integer` | Inclusive end address in decimal. | Yes |
| `start_address` | `integer` | Inclusive start address in decimal. | Yes |
| `view` | `string` | View format to return (default: `'disasm'`). | No |

### `r2000_read_selected`

Returns disassembly or hexdump text for the active UI selection, or returns the row under the cursor when no selection is active.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `view` | `string` | View format to return (default: `'disasm'`). | No |

### `r2000_redo`

Redoes the most recently undone command.

_No arguments._

### `r2000_save_project`

Saves the current project state to the active `.regen2000proj` file.

_No arguments._

### `r2000_search_disassembly`

Searches disassembly text for a substring or regular expression and returns matching addresses with context (label, mnemonic, operand, and comment). By default, this tool searches labels, comments, and instructions.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `max_results` | `integer` | Maximum number of matching addresses to return (default: `50`). | No |
| `query` | `string` | Case-insensitive search substring, or regular expression when `use_regex` is `true`. | Yes |
| `search_comments` | `boolean` | Includes side and line comments in the search (default: `true`). | No |
| `search_instructions` | `boolean` | Includes mnemonics and operands in the search (default: `true`). | No |
| `search_labels` | `boolean` | Includes label names in the search (default: `true`). | No |
| `use_regex` | `boolean` | Compiles `query` as a case-insensitive regular expression with `(?i)` (default: `false`). | No |

### `r2000_search_memory`

Searches binary memory for a byte sequence or text string and returns all matching addresses.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `encoding` | `string` | Query encoding: `'text'` searches PETSCII and Screencode, and `'hex'` searches raw byte patterns. | No |
| `query` | `string` | Search query (space-separated hex bytes such as `'A9 00'`, or a plain text string). | Yes |

### `r2000_set_comment`

Sets or clears a comment at a specific address. A `'line'` comment appears on a dedicated line before the instruction and supports multiple lines. A `'side'` comment appears inline next to the instruction.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Target memory address in decimal (for example, `4096` for `$1000`). | Yes |
| `comment` | `string` | Comment text without a leading `;` character. | Yes |
| `type` | `string` | Comment placement: `'line'` (above instruction) or `'side'` (inline). | Yes |

### `r2000_set_data_type`

Sets the block classification (`code`, `byte`, `word`, `address`, `petscii`, `screencode`, or split tables) for a memory range.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `data_type` | `string` | Block type classification string. | Yes |
| `end_address` | `integer` | Inclusive end address of the memory region in decimal. | Yes |
| `start_address` | `integer` | Inclusive start address of the memory region in decimal. | Yes |

### `r2000_set_immediate_format`

Sets the display format for an immediate operand at a specific address (hexadecimal, decimal, binary, or low/high byte of a 16-bit address).

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Instruction address in decimal. | Yes |
| `format` | `string` | Display format: `hex` (`$00`), `decimal` (`0`), `binary` (`%00000000`), `low_byte` (`#<p_ADDRESS`), or `high_byte` (`#>p_ADDRESS`). | Yes |
| `target_address` | `integer` | 16-bit target address in decimal (required when `format` is `'low_byte'` or `'high_byte'`). | No |

### `r2000_set_label_name`

Assigns or removes a user-defined label at a specific 6502 memory address.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Target memory address in decimal (for example, `4096` for `$1000`). | Yes |
| `name` | `string` | Label identifier (for example, `'init_screen'` or `'loop_start'`). | Yes |

### `r2000_toggle_splitter`

Toggles a block splitter at a specific address so the auto-merger does not combine adjacent blocks of the same type.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `address` | `integer` | Memory address in decimal where the splitter toggles. | Yes |

### `r2000_undo`

Undoes the most recent command.

_No arguments._

### `r2000_unpack_binary`

Runs dynamic 6502 unpacking emulation on the loaded binary payload.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `depacker_address` | `string` | Optional forced depacker routine address. | No |
| `entry_point` | `string` | Optional forced entry point address (hexadecimal string or integer). | No |
| `max_instructions` | `integer` | Maximum number of instructions to emulate. | No |
| `return_address` | `string` | Optional forced unpacked target address. | No |

### `r2000_update_project_enum`

Updates or renames an existing project-specific enum definition.

**Arguments:**

| Name | Type | Description | Required |
| :--- | :--- | :--- | :---: |
| `description` | `string` | Optional updated summary of the enum purpose. | No |
| `name` | `string` | Current name of the enum to update. | Yes |
| `new_name` | `string` | Optional new identifier when renaming the enum. | No |
| `variants` | `object` | Updated variant map. | No |
