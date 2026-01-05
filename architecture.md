# Architecture

Regenerator 2000 is an interactive disassembler for the Commodore 64, written in Rust. It follows a unidirectional data flow architecture where user events modify the application state through commands, triggering re-analysis and re-rendering of the view.

## High-Level Overview

```text
      +------------+
      | User Input |
      +-----+------+
            | Keys
            v
      +-----+------+
      | Event Loop |
      +-----+------+
            | Dispatch
            v
    +-------+--------+
    | Command System |
    +-------+--------+
            | Apply/Undo
            v
    +-------+--------+ <..................................
    |   Application  |                                   :
    |      State     | -------------------+              :
    +-------+--------+       Triggers     |              :
            |                             |              :
            | Requests                    v              :
            |                     +-------+-------+      :
            v                     | Code Analyzer |      :
    +-------+--------+            +-------+-------+      :
    |  Disassembly   |                    | Updates      :
    |     Engine     |                    v              :
    +-------+--------+            +-------+-------+      :
            | Generates           |  Auto Labels  | .....:
            v                     +---------------+
    +-------+--------+
    |  Disassembly   |
    |     Lines      |
    +-------+--------+
            | Render
            v
    +-------+--------+      +-------+--------+
    |  TUI Renderer  | <--- |    UI State    |
    +----------------+      +----------------+
```

## Core Components

### 1. Application State (`state.rs`)

The central hub of the application. It holds:

- **`ProjectState`**: The persistent part of the state, containing:
    - **Raw Data**: The binary being disassembled.
    - **Block Types**: A parallel array to the raw data, defining how each byte should be interpreted (Code, Data, Text, etc.).
    - **Labels & Comments**: User-defined and system-defined metadata.
    - **Document Settings**: Configurable options like `.text` line length, BRK handling, etc.
- **Undo Stack**: History of commands for Undo/Redo functionality.
- **Disassembly Cache**: Use to avoid re-disassembling the entire file on every frame.

### 2. Disassembly Engine (`disassembler/`)

Responsible for converting raw bytes into human-readable assembly code based on the state.

- **`disassembler.rs`**: The main driver. It iterates through the raw data, respecting `BlockType` definitions, and produces a list of `DisassemblyLine`s.
- **`formatter.rs`**: A trait abstracting the differences between assembler syntaxes.
- **`acme.rs` / `tass.rs`**: Implementations for specific assemblers (ACME, 64tass).

### 3. CPU Model (`cpu.rs`)

Provides the domain model for the MOS 6502/6510 CPU.

- **`Opcode`**: Definitions of all supported opcodes, including cycle counts, addressing modes, and descriptions.
- **`AddressingMode`**: Enum defining the different addressing modes (Absolute, ZeroPage, Immediate, etc.).
Used by both the **Disassembler** (to decode instructions) and the **Analyzer** (to understand control flow).

### 4. Command System (`commands.rs`)

Implements the **Command Pattern**. Granular actions (e.g., `SetBlockType`, `SetLabel`) are encapsulated as Structs that know how to:

- **Apply**: Execute the change on `AppState`.
- **Undo**: Revert the change.
This enables robust Undo/Redo functionality and ensures state consistency.

### 5. Analyzer (`analyzer.rs`)

A heuristic engine that runs after state changes. It:

- Traces code paths (following JMPs and branches).
- Identifies referenced addresses.
- Auto-generates labels (e.g., `j_loop_0400`) based on usage context (subroutine, branch target, pointer).

### 6. Exporter (`exporter.rs`)

Handles the generation of complete, compilable source code files.

- Supports multiple assembler formats (ACME, 64tass) via the `Formatter` trait.
- Ensures output validity by checking for label collisions and handling syntax-specific requirements.

### 7. UI & Event Loop

- **`main.rs`**: Sets up the terminal (using `crossterm`) and initializes the main loop.
- **`events.rs`**: Listens for input and maps key combinations to `Command`s. It also handles main application logic that intersects with UI actions (like navigation or search).
- **`ui.rs`**: Renders the application to the terminal using `ratatui`. It is stateless regarding business logic, only displaying what is in `AppState` and `UiState`.
- **`ui_state.rs`**: Manages the transient state of the user interface, such as:
    - Active Pane (Disassembly vs Hex Dump)
    - Dialog States (Search, About, Confirmation, etc.)
    - Scroll Offsets and Cursor Positions (if not directly in `AppState`)

## Data Flow

1. **Input**: User presses `C` (Code).
2. **Event**: `events.rs` captures the key and creates a `Command::SetBlockType` for the selected range.
3. **Execution**: The command is pushed to the `UndoStack` and applied to `AppState`.
4. **Update**: `AppState` updates the `BlockType` array.
5. **Analysis**: The change triggers `analyzer.rs` to re-scan the code connectivity, potentially adding or removing auto-labels.
6. **Disassembly**: `AppState` calls `Disassembler::disassemble()` to regenerate the cached `DisassemblyLine`s.
7. **Render**: The main loop calls `ui::draw()`, passing `AppState` and `UiState`, to display the new state.

## Persistence

Projects are saved as compressed JSON files (`.regen2000proj`).

- **Structure**: Serializes the `ProjectState` struct.
- **Efficiency**: Uses Run-Length Encoding for block types to save space.
- **Portability**: Designed to be portable across different machines, storing relative paths where possible.
