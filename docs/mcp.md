---
icon: material/robot-outline
description: Connect AI assistants like Claude Code and Antigravity via HTTP/SSE or stdio MCP with 29 structured tools and 5 reverse-engineering skills.
---

# MCP Integration

Regenerator 2000 includes a built-in **Model Context Protocol (MCP)** server so AI coding assistants—such as **Antigravity** and **Claude Code**—can inspect memory, trace subroutines, rename symbols, and classify blocks directly inside your active project.

Through MCP, an assistant can:

**Read**
:   Disassembly listings, raw memory hexdumps, block maps, labels, comments, and typed cross-references.

**Modify**
:   Labels, line/side comments, symbolic offsets, immediate formats, lexical scopes, enums, splitters, and block types—all tracked in the undo stack.

**Navigate & Analyze**
:   Jump the TUI cursor to symbols or addresses, search memory or disassembly, run recursive flow analysis, and unpack compressed binaries.

/// details | What is MCP?
    type: info

The Model Context Protocol (MCP) is an open standard that lets AI models call structured tools and read resources exposed by local applications. See [modelcontextprotocol.io](https://modelcontextprotocol.io) for the protocol specification.
///

---

## Transport Modes

| Mode | CLI Flag | Endpoint / Transport | Best For |
| :--- | :--- | :--- | :--- |
| **HTTP (Streamable / SSE)** *(Recommended)* | `--mcp-server` *(add `--headless` to skip TUI)* | `http://127.0.0.1:3000/mcp` | Live pair reverse-engineering where the AI updates the TUI in real time |
| **Stdio (Headless)** | `--mcp-server-stdio` *(implies `--headless`)* | Standard I/O subprocess | Automated pipelines or clients (like Claude Desktop) that launch the server on demand |

---

## Setup & Configuration

!!! tip "Video Walkthrough"

    Prefer to watch the workflow in action?

    [:material-youtube: Watch the MCP Setup & Demo Video](https://www.youtube.com/watch?v=_HW2d7kNCQw){ .md-button }

### 1. Start the Server

Start Regenerator 2000 with `--mcp-server` **before** launching your AI client so the client connects cleanly at startup:

```bash title="Terminal"
regenerator2000 --mcp-server your_project.regen2000proj
```

### 2. Connect Your Client

=== "Claude Code"

    Register the HTTP server from your terminal:

    ```bash
    claude mcp add -t http regenerator2000 http://127.0.0.1:3000/mcp
    ```

    /// details | Manual JSON config (`claude.json`)
        type: example

    ```json title="claude.json"
    {
      "mcpServers": {
        "regenerator2000": {
          "type": "http",
          "url": "http://127.0.0.1:3000/mcp"
        }
      }
    }
    ```

    ///

=== "Antigravity CLI"

    Register the HTTP server globally with `agy`:

    ```bash
    agy mcp add regenerator2000 http://127.0.0.1:3000/mcp --scope user -t http
    ```

    /// details | Manual JSON config (`~/.gemini/antigravity/mcp_config.json`)
        type: example

    ```json title="~/.gemini/antigravity/mcp_config.json"
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

    ///

---

## Common Workflows

Click through four typical prompts to see which MCP tools the assistant orchestrates behind the scenes:

=== "Analyze a Routine"

    > *"Analyze the routine at $C000. It checks sprite collisions. Rename variables and add comments that explain the logic."*

    The assistant calls `r2000_read_region` and `r2000_get_address_details` around `$C000`, traces control flow and hardware register reads (such as `$D01E` for sprite collisions), then renames symbols and writes header/side comments via `r2000_set_label_name` and `r2000_set_comment`.

=== "Separate Code vs. Data"

    > *"Scan $2000–$4000. Disassemble valid code sequences, mark tables as bytes, and mark strings as PETSCII."*

    The assistant inspects raw bytes and existing regions via `r2000_read_region` and `r2000_get_blocks`, then applies classifications in bulk using `r2000_disassemble` and `r2000_set_data_type` (`"code"`, `"byte"`, or `"petscii"`).

=== "Trace Cross-References"

    > *"Find every `JSR $FFD2` (CHROUT) call and comment the character value loaded before each call."*

    The assistant queries `r2000_get_cross_references` for `$FFD2`, reads the preceding `LDA #$XX` instructions at each call site, and annotates each caller with `r2000_set_comment`.

=== "Semantic Navigation"

    > *"Jump to the high score table."* / *"Go to the interrupt handler."*

    The assistant searches labels and comments via `r2000_get_symbols` or `r2000_search_disassembly`, resolves the target address, and moves your live TUI cursor there with `r2000_jump_to_address`.

---

## Agent Skills

**Skills** are reusable workflow playbooks stored in `.agent/skills/` that guide AI agents through multi-step reverse engineering.

```bash title="Copy skills into your analysis workspace"
cp -r .agent/skills/r2000* /path/to/your/project/.agent/skills/
```

```text title="Expected directory layout"
your_project/
└── .agent/
    └── skills/
        └── r2000-analyze-blocks/
            └── SKILL.md
```

!!! note "Claude Code users"

    Claude Code loads skills from `.claude/` instead of `.agent/`. Create a symlink so both clients share the same skill folders:

    ```bash
    ln -s .agent .claude
    ```

### Recommended Workflow

Run the four individual skills in order—or launch **`r2000-analyze-program`** to orchestrate all four automatically using parallel subagents:

| Skill | Scope | Example Prompts |
| :--- | :--- | :--- |
| `r2000-analyze-program` | Full program (orchestrates all 4 skills in parallel) | *"Analyze this program"*, *"Full analysis"* |
| `r2000-analyze-basic` | Tokenized Commodore BASIC header & `SYS` entry | *"Analyze BASIC"*, *"Decode BASIC header"* |
| `r2000-analyze-blocks` | Macro partitioning into code, text, and tables | *"Analyze blocks"*, *"Classify the program"* |
| `r2000-analyze-routine` | Single subroutine control flow and header comments | *"Analyze this routine"*, *"What does this function do?"* |
| `r2000-analyze-symbol` | Single address/variable cross-references & naming | *"Analyze this label"*, *"What is this variable?"* |

/// details | Skill details: `r2000-analyze-program`
    type: note

Coordinates BASIC decoding, block classification, and parallel subagent analysis of routines and symbols:

1. **Phase 0 — Gather Context:** Reads binary metadata, entropy, existing blocks, symbols, and comments.
2. **Phase 1 — Decode BASIC & Classify Blocks:** Runs `r2000-analyze-basic` on BASIC headers and `r2000-analyze-blocks` across undefined regions.
3. **Phase 2 — Analyze Routines:** Collects unanalyzed subroutines (`s_XXXX`, uncommented `JSR` targets, `p_XXXX` pointers inside `Code` blocks, and `start`) and spawns parallel subagents running `r2000-analyze-routine`.
4. **Phase 3 — Analyze Symbols:** Collects unanalyzed data symbols (`zpp_`, `zpf_`, `zpa_`, `p_`, `f_`, `a_`) and spawns parallel subagents running `r2000-analyze-symbol`.
5. **Phase 4 — Save & Report:** Saves the project and summarizes analyzed routines, renamed symbols, and ambiguous regions.
///

/// details | Skill details: `r2000-analyze-basic`
    type: note

Decodes tokenized Commodore BASIC lines at the system BASIC start address:

1. Resolves the BASIC start address from `r2000_get_binary_info` (`$0801` C64, `$1C01` C128, `$1001`/`$0401`/`$1201` VIC-20, `$0401` PET, `$1001` Plus/4 & C16).
2. Walks the 16-bit next-line pointer chain until the `$00 $00` end-of-program marker.
3. Formats next-line pointers as `address`, line numbers as `word`, and token payloads as `byte` blocks.
4. Adds decoded BASIC statements as side comments and disassembles unmarked `SYS` targets.
///

/// details | Skill details: `r2000-analyze-blocks`
    type: note

Scans a memory range (or the full binary) and converts regions into block types:

1. Reads the target address range via `r2000_get_binary_info` and existing blocks via `r2000_get_blocks`.
2. Traces code from entry points, locates text strings, detects data tables, and classifies remaining regions.
3. Applies block conversions in batches via `r2000_batch_execute`.
4. Inserts splitters via `r2000_toggle_splitter` to separate adjacent tables of the same type.
5. Labels and comments notable regions (entry points, strings, jump tables), then summarizes classified blocks and flags ambiguous ranges.
///

/// details | Skill details: `r2000-analyze-routine`
    type: note

Documents a single subroutine by inspecting instructions, cross-references, and memory accesses:

1. Identifies start and end addresses (entry point to `RTS`, `JMP`, or `RTI`).
2. Decodes instruction flow, loops, KERNAL calls, and hardware register accesses.
3. Inspects cross-references to determine call context.
4. Analyzes zero-page variables and memory operands.
5. Summarizes purpose, inputs, outputs, and side effects, then adds a header line comment and renames the routine label.
///

/// details | Skill details: `r2000-analyze-symbol`
    type: note

Determines the role of a memory address or label from its cross-references and usage patterns:

1. Identifies target address, current label, and target platform.
2. Checks whether the address maps to a hardware register or ROM entry point.
3. Inspects cross-references to classify read, write, and read-modify-write patterns.
4. Classifies the symbol as a pointer, flag, loop counter, or state variable.
5. Renames variables (`snake_case`) or constants (`UPPER_CASE`) and documents data flow via `r2000_set_comment`.
///

---

## Available Resources

| Resource URI | MIME Type | Contents |
| :--- | :--- | :--- |
| `binary://main` | `application/octet-stream` | Raw PRG payload (including 2-byte load address header) as base64 |
| `disassembly://current` | `text/x-asm` | Complete disassembled text of the active binary |
| `hexdump://current` | `text/plain` | Formatted memory hexdump of the active binary |
| `symbols://current` | `application/json` | All user, system, and auto-generated labels |
| `comments://current` | `application/json` | All user line and side comments |
| `blocks://current` | `application/json` | All memory block classifications and byte ranges |

---

## Available Tools

The server exposes **29** tools across **5** categories. Expand any tool below to inspect its full description and argument schema.

### Inspection & Reading

| Tool | Summary |
| :--- | :--- |
| [`r2000_get_binary_info`](#r2000_get_binary_info) | Return origin address, size, target platform, filename, description, entropy (>7.2000003 suggests compression), and undocumented opcodes flag. |
| [`r2000_get_blocks`](#r2000_get_blocks) | Return all memory blocks with their address ranges and types (Code, Byte, Word, Address, text, split tables, External File, Undefined). |
| [`r2000_read_region`](#r2000_read_region) | Return disassembly or hexdump text for a specific memory range. |
| [`r2000_read_selected`](#r2000_read_selected) | Return disassembly or hexdump text for the active UI selection, or the row under the cursor when no selection is active. |
| [`r2000_get_address_details`](#r2000_get_address_details) | Return detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type. |
| [`r2000_get_symbols`](#r2000_get_symbols) | Return defined labels (user, system, auto, and derived symbolic offsets) and their addresses. |
| [`r2000_get_comments`](#r2000_get_comments) | Return user-defined comments and their addresses with optional filters. |
| [`r2000_get_cross_references`](#r2000_get_cross_references) | Return a list of addresses that reference the given address (for example, JSR, JMP, or load/store instructions). |

/// details | `r2000_get_binary_info` — Return origin address, size, target platform, filename, description, entropy (>7.2000003 suggests compression), and undocumented opcodes flag.
    type: abstract
    attrs: {id: r2000_get_binary_info}

Return origin address, size, target platform, filename, description, entropy (>7.2000003 suggests compression), and undocumented opcodes flag.

_No arguments._
///

/// details | `r2000_get_blocks` — Return all memory blocks with their address ranges and types (Code, Byte, Word, Address, text, split tables, External File, Undefined).
    type: abstract
    attrs: {id: r2000_get_blocks}

Return all memory blocks with their address ranges and types (Code, Byte, Word, Address, text, split tables, External File, Undefined). Respects splitters.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `block_type` | `string` | No | Optional filter to return only blocks of a specific type. Case-insensitive. |
///

/// details | `r2000_read_region` — Return disassembly or hexdump text for a specific memory range.
    type: abstract
    attrs: {id: r2000_read_region}

Return disassembly or hexdump text for a specific memory range.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `end_address` | `integer` | Yes | End address (inclusive), decimal. |
| `start_address` | `integer` | Yes | Start address (inclusive), decimal. |
| `view` | `string` | No | The view to return. Default: 'disasm'. |
///

/// details | `r2000_read_selected` — Return disassembly or hexdump text for the active UI selection, or the row under the cursor when no selection is active.
    type: abstract
    attrs: {id: r2000_read_selected}

Return disassembly or hexdump text for the active UI selection, or the row under the cursor when no selection is active.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `view` | `string` | No | The view to return. Default: 'disasm'. |
///

/// details | `r2000_get_address_details` — Return detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.
    type: abstract
    attrs: {id: r2000_get_address_details}

Return detailed information about a specific memory address: instruction semantics, cross-references, labels, comments, and block type.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The memory address to inspect (decimal). |
///

/// details | `r2000_get_symbols` — Return defined labels (user, system, auto, and derived symbolic offsets) and their addresses.
    type: abstract
    attrs: {id: r2000_get_symbols}

Return defined labels (user, system, auto, and derived symbolic offsets) and their addresses. Without arguments, this tool returns all symbols. Optional filters narrow results by 'names', 'name', 'start_address', 'end_address', and 'kind'. Active filters combine with logical AND.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `end_address` | `integer` | No | Optional upper bound (inclusive) of the address range to filter by (decimal). |
| `kind` | `string` | No | Optional filter to return only symbols of a given kind. 'user' = user-defined labels, 'system' = predefined system labels (e.g. KERNAL, hardware registers), 'auto' = auto-generated labels (e.g. s_C000), 'derived' = global address symbolic offset expressions (e.g. zp_ptr + 1). |
| `name` | `string` | No | Optional substring filter matching label names (or expression text and base label name for derived symbolic offsets). Case-sensitive. |
| `names` | `array` | No | Optional list of label names to look up. Case-sensitive. |
| `start_address` | `integer` | No | Optional lower bound (inclusive) of the address range to filter by (decimal). |
///

/// details | `r2000_get_comments` — Return user-defined comments and their addresses with optional filters.
    type: abstract
    attrs: {id: r2000_get_comments}

Return user-defined comments and their addresses with optional filters.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `addresses` | `array` | No | Optional list of memory addresses to query comments for (decimal). |
| `end_address` | `integer` | No | Optional upper bound (inclusive) of the address range to filter by (decimal). |
| `start_address` | `integer` | No | Optional lower bound (inclusive) of the address range to filter by (decimal). |
| `type` | `string` | No | Optional filter for comment type ('line' or 'side'). |
///

/// details | `r2000_get_cross_references` — Return a list of addresses that reference the given address (for example, JSR, JMP, or load/store instructions).
    type: abstract
    attrs: {id: r2000_get_cross_references}

Return a list of addresses that reference the given address (for example, JSR, JMP, or load/store instructions).

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The target address to find references to (decimal). |
///

### Disassembly & Editing

| Tool | Summary |
| :--- | :--- |
| [`r2000_disassemble`](#r2000_disassemble) | Run recursive flow disassembly starting at a specified address. |
| [`r2000_set_data_type`](#r2000_set_data_type) | Set the block data type for a memory region (bytes, words, addresses, text, split tables, or external file; for executable code, prefer r2000_disassemble). |
| [`r2000_set_label_name`](#r2000_set_label_name) | Set or delete a user-defined label (or global symbolic offset expression such as 'zp_ptr + 1') at a specific MOS 6502 memory address. |
| [`r2000_set_comment`](#r2000_set_comment) | Set or delete a comment at a specific address. |
| [`r2000_set_immediate_format`](#r2000_set_immediate_format) | Set the operand format for an immediate instruction at a specific address. |
| [`r2000_set_symbolic_offset`](#r2000_set_symbolic_offset) | Format an operand, address table (including Lo/Hi, Hi/Lo, or contiguous RTS target-1 tables), or target address as a symbolic offset (e.g. |
| [`r2000_add_scope`](#r2000_add_scope) | Add a lexical scope that covers the specified memory range. |
| [`r2000_toggle_splitter`](#r2000_toggle_splitter) | Toggle a block splitter at a specific address. |

/// details | `r2000_disassemble` — Run recursive flow disassembly starting at a specified address.
    type: abstract
    attrs: {id: r2000_disassemble}

Run recursive flow disassembly starting at a specified address.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | Start address to disassemble (decimal). |
///

/// details | `r2000_set_data_type` — Set the block data type for a memory region (bytes, words, addresses, text, split tables, or external file; for executable code, prefer r2000_disassemble).
    type: abstract
    attrs: {id: r2000_set_data_type}

Set the block data type for a memory region (bytes, words, addresses, text, split tables, or external file; for executable code, prefer r2000_disassemble). Use 'address', 'lo_hi_address', or 'hi_lo_address' for pointer and jump tables—including RTS (target - 1) dispatch tables paired with r2000_set_symbolic_offset (offset: -1, scope: 'operand'). Reserve 'word', 'lo_hi_word', and 'hi_lo_word' for non-pointer numeric 16-bit values.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `data_type` | `string` | Yes | Data type classification: 'address', 'lo_hi_address', or 'hi_lo_address' for 16-bit pointer or RTS (target - 1) tables (enables cross-references and r2000_set_symbolic_offset); 'word', 'lo_hi_word', or 'hi_lo_word' for numeric 16-bit constants; 'byte', 'petscii', 'screencode', 'external_file', 'undefined', or 'code'. |
| `end_address` | `integer` | Yes | End of the memory region (inclusive), decimal. |
| `start_address` | `integer` | Yes | Start of the memory region (inclusive), decimal. |
///

/// details | `r2000_set_label_name` — Set or delete a user-defined label (or global symbolic offset expression such as 'zp_ptr + 1') at a specific MOS 6502 memory address.
    type: abstract
    attrs: {id: r2000_set_label_name}

Set or delete a user-defined label (or global symbolic offset expression such as 'zp_ptr + 1') at a specific MOS 6502 memory address. Use this tool to name subroutines, variables, jump targets, or pointer/struct fields.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The memory address where the label should be set (decimal, e.g. 4096 for $1000). |
| `name` | `string` | Yes | Label name (e.g. 'init_screen'), symbolic offset expression (e.g. 'zp_ptr + 1'), or '' to delete an existing user label or symbolic offset. |
///

/// details | `r2000_set_comment` — Set or delete a comment at a specific address.
    type: abstract
    attrs: {id: r2000_set_comment}

Set or delete a comment at a specific address. 'line' comments appear on their own line before the instruction (multi-line supported). 'side' comments appear inline on the instruction line. Pass an empty string (or omit 'comment') to delete an existing comment.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The memory address for the comment (decimal, e.g. 4096 for $1000). |
| `comment` | `string` | No | Comment text (omit the leading comment prefix character), or an empty string to delete the existing comment. |
| `type` | `string` | Yes | 'line' = comment on its own line before the instruction. 'side' = inline comment on the same line. |
///

/// details | `r2000_set_immediate_format` — Set the operand format for an immediate instruction at a specific address.
    type: abstract
    attrs: {id: r2000_set_immediate_format}

Set the operand format for an immediate instruction at a specific address. Supports hexadecimal, decimal, binary, and low/high byte references to a 16-bit address (with optional signed symbolic byte displacement).

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The address of the instruction (decimal). |
| `format` | `string` | Yes | hex=$00, decimal=0, binary=%00000000, low_byte=#<p_ADDRESS, high_byte=#>p_ADDRESS. |
| `offset` | `integer` | No | Optional signed byte displacement (such as -1 for #<(sub_C100 - 1)). Accepts a base or effective target_address and checks the immediate operand byte. |
| `radix` | `string` | No | Offset radix when offset is non-zero ('decimal' -> '- 1', 'hex' -> '- $1'). Default: 'decimal'. |
| `target_address` | `integer` | No | The 16-bit target address decimal (e.g. 60000 for $EA31). Required when format is 'low_byte' or 'high_byte'. |
///

/// details | `r2000_set_symbolic_offset` — Format an operand, address table (including Lo/Hi, Hi/Lo, or contiguous RTS target-1 tables), or target address as a symbolic offset (e.g.
    type: abstract
    attrs: {id: r2000_set_symbolic_offset}

Format an operand, address table (including Lo/Hi, Hi/Lo, or contiguous RTS target-1 tables), or target address as a symbolic offset (e.g. 'zp_ptr + 1', 'sub_C100 - 1'). For RTS dispatch tables, first set data_type to 'address', 'lo_hi_address', or 'hi_lo_address' (never 'word', 'lo_hi_word', or 'hi_lo_word'), then apply offset: -1 with scope: 'operand'. Pass 'address' for one site or address, or pass 'start_address' and 'end_address' for a table or struct range. Set the offset with 'expression' ('zp_ptr + 1' or '' to clear), 'base' (name or address), or 'offset' (signed integer, 0 to clear).

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | No | Single instruction, table entry, or target memory address (decimal). |
| `base` | `string \| integer` | No | Base symbol name (e.g. 'zp_ptr', 'SCREEN_RAM') or base address (e.g. 251 or '$00FB'). Automatically calculates offset = effective_address - base. |
| `end_address` | `integer` | No | End address (inclusive, decimal) when applying to an address table or struct range. |
| `expression` | `string` | No | Symbolic expression such as 'zp_ptr + 1', 'sub_C100 - 1', 'SCREEN_RAM + $28', or '' to clear. |
| `offset` | `integer` | No | Signed byte displacement from base (e.g. -1 for RTS dispatch tables or 1-based indexed loops, +1 for pointer high byte, 0 to clear). Automatically calculates base = effective_address - offset. |
| `radix` | `string` | No | Offset radix ('decimal' -> '+ 40', 'hex' -> '+ $28'). Default: 'decimal'. |
| `scope` | `string` | No | 'operand' sets a per-site offset on the instruction or address table ('address', 'lo_hi_address', 'hi_lo_address') at address/range. 'address' sets a global offset on the target address across all references (or across a byte/word struct range). Default 'auto' uses 'operand' on instructions or address tables, and 'address' when no operand or address table exists (such as 'byte', 'word', or 'lo_hi_word' blocks). Pass explicit 'operand' for address/RTS tables. |
| `start_address` | `integer` | No | Start address (inclusive, decimal) when applying to an address table or struct range. |
///

/// details | `r2000_add_scope` — Add a lexical scope that covers the specified memory range.
    type: abstract
    attrs: {id: r2000_add_scope}

Add a lexical scope that covers the specified memory range. Use this tool to isolate local labels inside a subroutine. Nested scopes are not supported.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `end_address` | `integer` | Yes | End address of the scope (inclusive), decimal. |
| `start_address` | `integer` | Yes | Start address of the scope (inclusive), decimal. |
///

/// details | `r2000_toggle_splitter` — Toggle a block splitter at a specific address.
    type: abstract
    attrs: {id: r2000_toggle_splitter}

Toggle a block splitter at a specific address. Splitters prevent the disassembler from merging adjacent blocks of the same type.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The memory address where the splitter should be toggled (decimal). |
///

### Enums

| Tool | Summary |
| :--- | :--- |
| [`r2000_create_project_enum`](#r2000_create_project_enum) | Create a new project-specific enum definition in the project state. |
| [`r2000_update_project_enum`](#r2000_update_project_enum) | Update or rename an existing project-specific enum. |
| [`r2000_delete_project_enum`](#r2000_delete_project_enum) | Delete a project-specific enum definition. |
| [`r2000_apply_enum_usage`](#r2000_apply_enum_usage) | Apply or clear an enum usage annotation for an immediate operand at a specific address. |

/// details | `r2000_create_project_enum` — Create a new project-specific enum definition in the project state.
    type: abstract
    attrs: {id: r2000_create_project_enum}

Create a new project-specific enum definition in the project state.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `description` | `string` | No | Optional summary of the enum purpose. |
| `name` | `string` | Yes | Unique alphanumeric identifier. |
| `variants` | `object` | Yes | Variant map where keys are numeric strings (decimal, hex 0x/$, bin 0b/%) and values are variant names. |
///

/// details | `r2000_update_project_enum` — Update or rename an existing project-specific enum.
    type: abstract
    attrs: {id: r2000_update_project_enum}

Update or rename an existing project-specific enum.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `description` | `string` | No | Optional updated summary of the enum purpose. |
| `name` | `string` | Yes | Existing name of the enum to update. |
| `new_name` | `string` | No | Optional new name when renaming the enum. |
| `variants` | `object` | No | Variant map. |
///

/// details | `r2000_delete_project_enum` — Delete a project-specific enum definition.
    type: abstract
    attrs: {id: r2000_delete_project_enum}

Delete a project-specific enum definition.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `force` | `boolean` | No | When true, forces deletion even if the enum is in use. Defaults to false. |
| `name` | `string` | Yes | Name of the enum to delete. |
///

/// details | `r2000_apply_enum_usage` — Apply or clear an enum usage annotation for an immediate operand at a specific address.
    type: abstract
    attrs: {id: r2000_apply_enum_usage}

Apply or clear an enum usage annotation for an immediate operand at a specific address.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The address of the instruction (decimal). |
| `name` | `string` | No | Name of the enum to apply, or empty string to clear. |
///

### Navigation & Search

| Tool | Summary |
| :--- | :--- |
| [`r2000_get_disassembly_cursor`](#r2000_get_disassembly_cursor) | Return the memory address of the current cursor position in the disassembly view. |
| [`r2000_jump_to_address`](#r2000_jump_to_address) | Move the disassembly cursor to a specific memory address, scroll the view to make it visible, and record the previous position in the navigation history. |
| [`r2000_search_disassembly`](#r2000_search_disassembly) | Search disassembly text for a substring or regular expression. |
| [`r2000_search_memory`](#r2000_search_memory) | Search memory for a byte sequence or text string and return matching addresses. |

/// details | `r2000_get_disassembly_cursor` — Return the memory address of the current cursor position in the disassembly view.
    type: abstract
    attrs: {id: r2000_get_disassembly_cursor}

Return the memory address of the current cursor position in the disassembly view.

_No arguments._
///

/// details | `r2000_jump_to_address` — Move the disassembly cursor to a specific memory address, scroll the view to make it visible, and record the previous position in the navigation history.
    type: abstract
    attrs: {id: r2000_jump_to_address}

Move the disassembly cursor to a specific memory address, scroll the view to make it visible, and record the previous position in the navigation history.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `address` | `integer` | Yes | The target address to jump to (decimal). |
///

/// details | `r2000_search_disassembly` — Search disassembly text for a substring or regular expression.
    type: abstract
    attrs: {id: r2000_search_disassembly}

Search disassembly text for a substring or regular expression. Returns matching addresses with label, mnemonic, operand, and comment fields.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `max_results` | `integer` | No | Maximum number of matching addresses to return. Defaults to 50. |
| `query` | `string` | Yes | The search query. Interpreted as a case-insensitive substring by default, or as a regex when 'use_regex' is true. |
| `search_comments` | `boolean` | No | Include side and line comments in the search. Defaults to true. |
| `search_instructions` | `boolean` | No | Include mnemonic and operand text in the search. Defaults to true. |
| `search_labels` | `boolean` | No | Include label names in the search. Defaults to true. |
| `use_regex` | `boolean` | No | When true, compiles the query as a case-insensitive regular expression. Defaults to false. |
///

/// details | `r2000_search_memory` — Search memory for a byte sequence or text string and return matching addresses.
    type: abstract
    attrs: {id: r2000_search_memory}

Search memory for a byte sequence or text string and return matching addresses.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `encoding` | `string` | No | Encoding for the query. 'text' searches both PETSCII and Screencode. 'hex' searches raw byte patterns. Defaults to 'hex' when the query matches hex bytes, otherwise 'text'. |
| `query` | `string` | Yes | The search query. For hex: space-separated bytes, e.g. 'A9 00'. For text: plain string. |
///

### Session, Batching & Unpacking

| Tool | Summary |
| :--- | :--- |
| [`r2000_batch_execute`](#r2000_batch_execute) | Execute multiple tool calls sequentially in a single request. |
| [`r2000_unpack_binary`](#r2000_unpack_binary) | Run dynamic 6502 unpacking analysis on the loaded binary payload. |
| [`r2000_save_project`](#r2000_save_project) | Save the current project state to the active .regen2000proj file. |
| [`r2000_undo`](#r2000_undo) | Undo the latest operation. |
| [`r2000_redo`](#r2000_redo) | Redo the latest undone operation. |

/// details | `r2000_batch_execute` — Execute multiple tool calls sequentially in a single request.
    type: abstract
    attrs: {id: r2000_batch_execute}

Execute multiple tool calls sequentially in a single request. Use this tool only when you have 5 or more independent operations to run at once.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `calls` | `array` | Yes | List of tool calls to execute sequentially. |
///

/// details | `r2000_unpack_binary` — Run dynamic 6502 unpacking analysis on the loaded binary payload.
    type: abstract
    attrs: {id: r2000_unpack_binary}

Run dynamic 6502 unpacking analysis on the loaded binary payload.

| Argument | Type | Required | Description |
| :--- | :--- | :---: | :--- |
| `depacker_address` | `string` | No | Optional forced depacker address. |
| `entry_point` | `string` | No | Optional forced entry point address (hex string or integer). |
| `max_instructions` | `integer` | No | Maximum instructions to emulate. |
| `return_address` | `string` | No | Optional forced return address. |
///

/// details | `r2000_save_project` — Save the current project state to the active .regen2000proj file.
    type: abstract
    attrs: {id: r2000_save_project}

Save the current project state to the active .regen2000proj file.

_No arguments._
///

/// details | `r2000_undo` — Undo the latest operation.
    type: abstract
    attrs: {id: r2000_undo}

Undo the latest operation.

_No arguments._
///

/// details | `r2000_redo` — Redo the latest undone operation.
    type: abstract
    attrs: {id: r2000_redo}

Redo the latest undone operation.

_No arguments._
///
