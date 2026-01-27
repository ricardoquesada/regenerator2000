# Regenerator 2000 - Project Overview

## Identity
Regenerator 2000 is a modern, interactive 6502 disassembler and regenerator written in Rust. It is designed to be a successor to the original Windows-based "Regenerator", bringing the experience to the terminal (TUI) with cross-platform support (macOS, Linux, Windows).

## Tech Stack
- **Language**: Rust (Edition 2024)
- **TUI Framework**: [Ratatui](https://github.com/ratatui-org/ratatui) (formerly tui-rs)
- **Terminal Backend**: Crossterm
- **Error Handling**: `anyhow`
- **Serialization**: `serde`, `serde_json`
- **Compression**: `flate2` (used for project file compression)
- **Images**: `image`, `ratatui-image` (for potential graphics interoperability)

## Architecture
The application follows a **Unidirectional Data Flow** architecture, similar to Redux or The Elm Architecture.

1. **Input**: Key events are captured in `events.rs` and dispatched to the active `Widget`.
2. **Action**: Widgets return `WidgetResult::Action` (e.g., `MenuAction::SetBlockType`).
3. **Command**: Actions are converted into **Commands** (`src/commands.rs`).
   - **Crucial**: All state mutations MUST be encapsulated in a Command to support **Undo/Redo**.
4. **State**: Commands modify `AppState` (`src/state.rs`).
5. **Analysis**: Changes trigger the **Analyzer** (`src/analyzer.rs`) and **Disassembler** (`src/disassembler.rs`) to update the model.
6. **Render**: The UI (`src/ui.rs` and submodules) renders based on the new state.

### Key Directories
- **`src/`**: specific logic.
  - **`disassembler/`**: Core disassembly logic (decoding opcodes, formatting).
  - **`ui/`**: TUI implementation. All views (HexDump, Disassembly, etc.) implement the `Widget` trait.
  - **`cpu.rs`**: 6502/6510 CPU model (opcodes, addressing modes).
  - **`state.rs`**: Single source of truth for application data.
  - **`commands.rs`**: Command pattern implementation.
- **`tests/`**: Integration tests.
- **`docs/`**: Documentation (Architecture, User Guide).

## Development Guidelines

### 1. State Management
- **Never modify `AppState` directly from the UI** for logical changes. dispatch a `Command`.
- `UIState` (`src/ui_state.rs`) maps transient state (scroll position, active cursor, active pane).

### 2. UI Components
- All UI components (dialogs, views) should implement the `Widget` trait (`src/ui/widget.rs`).
- `Widget::handle_input` processes events.
- `Widget::render` draws to the frame.

### 3. Testing
- Run tests with `cargo test`.
- Integration tests in `tests/` check the assembler/disassembler round-trip accuracy.

## Build & Run
- **Build**: `cargo build`
- **Run**: `cargo run -- [path/to/rom.prg]`
- **Release**: `cargo build --release`

## Common Tasks
- **Adding a new feature**:
  1. Define the logical change in `src/commands.rs`.
  2. Add the UI trigger in the appropriate `src/ui/view_*.rs` file.
  3. Ensure `AppState` handles the command.
- **Fixing a bug**:
  - Check `events.rs` for input issues.
  - Check `disassembler.rs` for output generation issues.

## Project Goals
- **Interactive**: Immediate feedback.
- **Authentic**: Accurate representation of C64 binaries (PRG, CRT, etc.).
- **User Friendly**: Vim-like navigation, intuitive shortcuts.
