# Regenerator 2000 - Agent Instructions

Regenerator 2000 is an interactive 6502 disassembler TUI written in Rust, targeting Commodore 8-bit machines. It uses [ratatui](https://github.com/ratatui-org/ratatui) + crossterm for the terminal UI and exposes an MCP (Model Context Protocol) server for programmatic access.

## Architecture & Crate Structure

The project is split into two main crates to separate logic from presentation:

- **`regenerator2000-core`**: Contains all UI-agnostic logic.
  - `analyzer.rs`: Auto-analysis (walking code, generating labels).
  - `commands.rs`: The `Command` enum and `UndoStack`. Every undoable action is a command.
  - `cpu.rs`: 6502 opcode table and addressing mode logic.
  - `disassembler/`: 6502 decoding and assembler formatters (64tass, ACME, KickAssembler, ca65).
  - `mcp/`: MCP server implementation (stdio and HTTP transports).
  - `parser/`: File format parsers (.prg, .d64, .t64, .crt, .vsf snapshots).
  - `state/`: Persistent application state and project serialization.
  - `vice/`: VICE binary monitor protocol client for live debugging.
  - `exporter/`: Code for exporting disassembly to ASM or HTML.

- **`regenerator2000-tui`**: Contains all terminal UI-related code.
  - `ui/`: TUI widgets, views (Disassembly, HexDump, etc.), and dialogs.
  - `ui_state.rs`: Transient UI state (cursor, scroll, active dialog).
  - `events/`: Input handling and event loop.
  - `theme.rs`: Color scheme and styling logic.

## Core Domain Model

### Key Types (`crates/regenerator2000-core/src/state/types.rs`)

- **`Addr`**: A wrapper around `u16` representing a 6502 address.
- **`Platform`**: Target machine (C64, C128, VIC20, PET, etc.).
- **`Assembler`**: Target assembler syntax (Tass64, Acme, Ca65, Kick).
- **`BlockType`**: How a range of bytes is interpreted (Code, DataByte, PetsciiText, LoHiAddress, etc.).
- **`LabelType`**: Semantic meaning of a label (Subroutine, Jump, Branch, Pointer, etc.).

### State Management

- **`AppState`** (`crates/regenerator2000-core/src/state/app_state.rs`): The single source of truth for persistent data. Includes `raw_data`, `block_types`, `labels`, `user_comments`, `undo_stack`, and `settings`. Serialized to `.regen2000proj`.
- **`UIState`** (`crates/regenerator2000-tui/src/ui_state.rs`): Transient state like cursor positions, scroll offsets, and active dialogs. Never serialized.

## Application Logic Flow

Regenerator 2000 follows a unidirectional data flow (Redux/Elm style):

1. **Input**: `crates/regenerator2000-tui/src/events/input.rs` maps key events to `MenuAction`.
2. **Dispatch**: `MenuAction` is converted into one or more `Command` variants.
3. **Execution**: `Command::apply(&mut AppState)` modifies the state and pushes to `UndoStack`.
4. **Analysis**: `AppState::analyze()` (via `analyzer.rs`) updates labels and cross-references.
5. **Render**: `ui()` in `crates/regenerator2000-tui/src/ui.rs` re-renders the TUI from `AppState` + `UIState`.

## AI Agent Skills (`.agent/skills/`)

Specialized tools are available for common tasks. Invoke them via `activate_skill`:

- `r2000-analyze-basic`: Analyze BASIC programs and mark pointers.
- `r2000-analyze-blocks`: Auto-detect and set block types (text, data).
- `r2000-analyze-routine`: Trace a subroutine and mark code paths.
- `r2000-analyze-symbol`: Find and label all references to a specific address.
- `add-mcp-tool`: Templates for adding new programmatic tools to the MCP server.
- `coding`: General Rust coding assistance with project context.
- `code-review`: Review changes for idiomatic Rust and project conventions.

## Development Workflow

### Commands

```bash
cargo build                      # Debug build
cargo run -- [file.prg]          # Run with optional file
cargo test                       # Run all tests
cargo fmt -- --check             # Check formatting
cargo clippy -- -D warnings      # Lint (warnings are errors)
```

### Adding Features

- **New Block Type**: Add to `BlockType` enum in `types.rs`, update `analyzer.rs`, and add a shortcut in `input.rs`.
- **New Assembler**: Add to `Assembler` enum, implement `Formatter` trait in `disassembler/formatter_*.rs`.
- **New Command**: Add variant to `Command` in `commands.rs`, implement `apply` and `undo`.

### Git Commit Style

Use present tense, imperative mood (e.g., `Add support for D71 disk images`). Limit first line to 72 chars.

### Testing

- **Unit Tests**: Located in `src` files (e.g., `cpu.rs`).
- **Integration Tests**: Located in `tests/` directory. Use these for testing disassembler output, project serialization, and MCP functionality.
- **Verification**: Always run `cargo test` and `cargo clippy` before finalizing changes.
