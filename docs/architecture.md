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
            | Dispatch to Widget
            v
    +-------+--------+
    | Active Widget  |
    | (View/Dialog)  |
    +-------+--------+
            | Results in Action/Command
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

### 7. UI Architecture

The UI is built on `crossterm` and `ratatui` with a custom `Widget` trait abstraction.

- **`Widget` Trait** (`ui/widget.rs`):
  Defines the interface for all UI components (Views, Dialogs, Menu, StatusBar).

  ```rust
  pub trait Widget {
      fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState);
      fn handle_input(&mut self, key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult;
  }
  ```

- **Core UI Components**:
  - **`main.rs`**: Initializes the terminal and event loop.
  - **`events.rs`**: The primary input router. It determines the active pane and dispatches input events to the corresponding `Widget`.
  - **`ui.rs`**: The top-level layout engine. It renders the Menu, StatusBar, and the active Main View.

- **UI State Management (`ui_state.rs`)**:
  Tracks transient interface state:
  - **Active Pane**: Enum (`Disassembly`, `HexDump`, `Blocks`) identifying the focused tool.
  - **Active Dialog**: `Option<Box<dyn Widget>>` allowing modal dialogs to take over input and rendering.
  - **View State**: Cursor positions, scroll offsets, and view-specific modes (e.g., Hexdump PETSCII/Screencode modes).

## Data Flow

1. **Input**: User presses a key (e.g., `C`).
2. **Dispatch**: `events.rs` routes the key to the active `Widget` (e.g., `DisassemblyView`).
3. **Action**: The Widget processes the input and returns a `WidgetResult::Action` (e.g., `MenuAction::Code`).
4. **Execution**: The action is converted into a `Command` (e.g., `SetBlockType`), pushed to the `UndoStack`, and applied to `AppState`.
5. **Update**: `AppState` modifies the data (e.g., updates `BlockType` array).
6. **Analysis**: The change triggers `analyzer.rs` to re-scan the code.
7. **Disassembly**: `AppState` calls `Disassembler::disassemble()` to regenerate the cached `DisassemblyLine`s.
8. **Render**: The main loop calls `ui::draw()`, which asks every visible `Widget` to render itself based on the new `AppState`.

## Persistence

Projects are saved as compressed JSON files (`.regen2000proj`).

- **Structure**: Serializes the `ProjectState` struct.
- **Efficiency**: Uses Run-Length Encoding for block types to save space.
- **Portability**: Designed to be portable across different machines, storing relative paths where possible.
