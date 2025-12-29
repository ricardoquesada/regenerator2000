# Regenerator2000

![logo](docs/regenerator2000_logo.png)

A 6502 disassembler with a TUI. A modern take on [Regenerator][regenerator].

[regenerator]: https://csdb.dk/release/?id=247992

## Features

Regenerator2000 brings modern conveniences to 6502 disassembly:

- **Disassembly**: Full 6502 support including undocumented opcodes.
- **Hex View**:
    - Side-by-side view with disassembly.
    - Synchronized or independent navigation.
    - **PETSCII** support (Unshifted and Shifted modes).
- **Platforms**: Supports Commodore 8-bit machines like C64, C128, Plus/4, etc.
- **Import**: Load `.prg`, `.bin`, and `.regen2000proj` files.
- **Export**: Generate compatible assembly source code for:
    - **64tass**
    - **ACME**
- **Project Management**: Save and load your work with `.regen2000proj` files.
- **Analysis**: Auto-analysis to identify code and data regions.
- **Editing**:
    - **Labels**: Add, edit, and remove local and global labels.
    - **Comments**: Add side comments (`;`) and line comments (`:`).
    - **Origin**: Change the load address/origin of the binary.
    - **Data Types**: Convert regions to Code, Byte, Word, Address, Text, or Screencode.
    - **Undo/Redo**: Full history support for all actions.
- **Navigation**:
    - **Jump**: Go to specific addresses, specific line numbers, or follow operands.
    - **X-Ref**: Inspect cross-references for labels/addresses.
    - **History**: Navigate back to previous locations.
- **Customization**:
    - Configure document settings (max x-refs, platform, assembler).
    - Customizable display options (show/hide all labels, etc.).
- **TUI**:
    - Text User interface
    - Everything can be done from the keyboard
    - **Visual Mode**: Vim-like selection for batch operations.
- **Fast**:
    - Extremely fast

![screenshot](docs/regenerator2000_screenshot.png)

## Keyboard Shortcuts

| Context        | Action                       | Shortcut                                |
|:---------------|:-----------------------------|:----------------------------------------|
| **Global**     | **Activate Menu**            | `F10`                                   |
|                | **Exit**                     | `Ctrl + Q`                              |
|                | **Open File**                | `Ctrl + O`                              |
|                | **Save Project**             | `Ctrl + S`                              |
|                | **Save Project As**          | `Ctrl + Shift + S`                      |
|                | **Export Project (ASM)**     | `Ctrl + E`                              |
|                | **Export Project As (ASM)**  | `Ctrl + Shift + E`                      |
|                | **Document Settings**        | `Ctrl + P`                              |
|                | **Undo**                     | `U`                                     |
|                | **Redo**                     | `Ctrl + R`                              |
|                | **Switch Pane (Hex/Disasm)** | `Tab`                                   |
| **Navigation** | **Move Cursor**              | `Up` / `Down` / `j` / `k`               |
|                | **Page Up/Down**             | `PageUp` / `PageDown`                   |
|                | **Home/End**                 | `Home` / `End`                          |
|                | **Jump to Address**          | `g`                                     |
|                | **Jump to Line**             | `Ctrl + Shift + G`                      |
|                | **Jump to Operand**          | `Enter`                                 |
|                | **Jump Back (History)**      | `Backspace`                             |
|                | **Previous/Next 10 Lines**   | `Ctrl + u` / `Ctrl + d`                 |
| **Selection**  | **Toggle Visual Mode**       | `Shift + V`                             |
|                | **Select Text**              | `Shift + Up/Down` / Visual Mode + `j/k` |
|                | **Clear Selection**          | `Esc`                                   |
| **Editing**    | **Set Label**                | `l`                                     |
|                | **Add Side Comment**         | `;`                                     |
|                | **Add Line Comment**         | `:` / `Shift + ;`                       |
|                | **Convert to Code**          | `c`                                     |
|                | **Convert to Byte**          | `b`                                     |
|                | **Convert to Word**          | `w`                                     |
|                | **Convert to Address**       | `a`                                     |
|                | **Convert to Text**          | `t`                                     |
|                | **Convert to Screencode**    | `s`                                     |
| **View**       | **PETSCII Unshifted**        | `Ctrl + Shift + L`                      |
|                | **PETSCII Shifted**          | `Ctrl + L`                              |
|                | **Toggle Hex View**          | `Ctrl + 2`                              |
| **Menus**      | **Navigate Menu**            | Arrows                                  |
|                | **Select Item**              | `Enter`                                 |
|                | **Close Menu**               | `Esc`                                   |

## Build and Run

```bash
cargo run
```
