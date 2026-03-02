# Regenerator 2000 - Project Instructions

Regenerator 2000 is an interactive 6502 disassembler TUI written in Rust, targeting Commodore 8-bit machines. It uses [ratatui](https://github.com/ratatui-org/ratatui) + crossterm for the terminal UI and exposes an MCP server for programmatic access.

## Commands

```bash
cargo build                      # Debug build
cargo build --release            # Release build
cargo run -- [path/to/file.prg]  # Run with optional file
cargo test                       # Run all tests
cargo test --test disassembler_tests  # Run a single integration test file
cargo fmt -- --check             # Check formatting (CI requirement)
cargo clippy -- -D warnings      # Lint (CI requirement, warnings are errors)
```

CI runs fmt check and clippy only on Linux; tests run on macOS, Linux, and Windows.

## Architecture

The application follows a **unidirectional data flow** (Redux/Elm style):

```
Key Event → Widget::handle_input → WidgetResult::Action(MenuAction)
         → events/input.rs dispatch → Command
         → Command::apply(&mut AppState)
         → analyzer / disassembler update
         → ui() re-renders from AppState + UIState
```

### Core separation

- **`AppState`** (`src/state/`) — single source of truth for all persistent data: raw bytes, block types, labels, comments, cross-refs, bookmarks, undo stack, VICE state. Serialized to `.regen2000proj`.
- **`UIState`** (`src/ui_state.rs`) — transient rendering state only: cursor positions, scroll offsets, active pane, active dialog, navigation history. Never serialized.

**Rule**: UI code must never mutate `AppState` directly for logical changes. All mutations go through `Command::apply()` so undo/redo works.

### Key modules

| Path | Purpose |
|------|---------|
| `src/commands.rs` | `Command` enum + `UndoStack`; every undoable action is a variant |
| `src/events.rs` + `src/events/input.rs` | Main event loop; routes `AppEvent` (crossterm, MCP, VICE, Tick) |
| `src/ui/widget.rs` | `Widget` trait (`render`, `handle_input`, `handle_mouse`); `WidgetResult` |
| `src/ui/menu.rs` | `MenuAction` enum — the bridge between raw key events and semantic actions |
| `src/ui/view_*.rs` | The six main views: Disassembly, HexDump, Sprites, Charset, Bitmap, Blocks |
| `src/ui/dialog_*.rs` | Modal dialogs; each implements `Widget` and is stored in `UIState::active_dialog` |
| `src/analyzer.rs` | Auto-analysis: walks block types to generate labels and cross-refs |
| `src/disassembler/` | 6502 decode + per-assembler formatters (64tass, ACME, KickAssembler, ca65) |
| `src/cpu.rs` | 6502/6510 opcode table and addressing modes |
| `src/parser/` | File parsers: d64, t64, crt, VICE labels (`.lbl`), VICE snapshots (`.vsf`) |
| `src/mcp/` | MCP server: HTTP (port 3000) and stdio transports |
| `src/vice/` | VICE binary monitor protocol client for live debugging |
| `src/state/project.rs` | Project save/load with base64+flate2 compression |

### Adding a new feature

1. Add a `Command` variant in `src/commands.rs` with `apply` (forward) and `undo` (reverse) logic.
2. Add a `MenuAction` variant in `src/ui/menu.rs` and handle it in `src/events/input.rs`.
3. Trigger the action from the relevant `view_*.rs` `handle_input`.

### Git commit style

Present tense, imperative mood, ≤72 chars on first line (e.g., `Add multicolor bitmap export`).
