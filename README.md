# Regenerator 2000 ![Rust](https://github.com/ricardoquesada/regenerator2000/actions/workflows/rust.yml/badge.svg) [![Crates.io Version](https://img.shields.io/crates/v/regenerator2000)](https://crates.io/crates/regenerator2000)

![logo](docs/regenerator2000_logo.png)

A 6502 disassembler with a TUI. A modern take on [Regenerator][regenerator].

[regenerator]: https://csdb.dk/release/?id=247992

## Features

Regenerator 2000 brings modern conveniences to 6502 disassembly:

- **Disassembly**: Full 6502 support including undocumented opcodes.
- **Hex Dump**:
    - Side-by-side view with disassembly.
    - Synchronized or independent navigation.
    - Unshifted or Shifted PETSCII charset.
- **Sprites**:
    - Side-by-side view with disassembly.
    - Multicolor or Single Color
- **Charset**:
    - Side-by-side view with disassembly.
    - Multicolor or Single Color
- **Blocks**:
    - Side-by-side view with disassembly.
- **Platforms**: Supports Commodore 8-bit machines like C64, C128, Plus/4, etc.
- **Import**: Load `.prg`, `.crt`, `.t64`, `.vsf`, `.bin`, `.raw`, and
  `.regen2000proj` files.
- **Export**: Generate compatible assembly source code for:
    - **64tass**
    - **ACME**
- **Project Management**: Save and load your work with `.regen2000proj` files.
- **Analysis**: Auto-analysis to identify code and data regions.
- **Editing**:
    - **Labels**: Add, edit, and remove local and global labels.
    - **Comments**: Add side comments and line comments.
    - **Origin**: Change the load address/origin of the binary.
    - **Data Types**: Convert regions to Code, Byte, Word, Address, Lo/Hi Address, Hi/Lo Address, Text,
      Screencode, External file or Unknown.
    - **Undo/Redo**: Full history support for all actions.
- **Navigation**:
    - **Jump**: Go to specific addresses, specific line numbers, or follow
      operands.
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

![hexdump screenshot](docs/regenerator2000_hexdump_screenshot.png)
![charset screenshot](docs/regenerator2000_charset_screenshot.png)
![sprites screenshot](docs/regenerator2000_sprites_screenshot.png)
![blocks screenshot](docs/regenerator2000_blocks_screenshot.png)

## Keyboard Shortcuts

| Context        | Action                                                       | Shortcut                                |
|:---------------|:-------------------------------------------------------------|:----------------------------------------|
| **Global**     | **Activate Menu**                                            | `F10`                                   |
|                | **Exit**                                                     | `Ctrl + q`                              |
|                | **Open File**                                                | `Ctrl + o`                              |
|                | **Save Project**                                             | `Ctrl + s`                              |
|                | **Save Project As**                                          | `Ctrl + Shift + s`                      |
|                | **Export Project (ASM)**                                     | `Ctrl + e`                              |
|                | **Export Project As (ASM)**                                  | `Ctrl + Shift + e`                      |
|                | **Document Settings**                                        | `Ctrl + Shift + d`                      |
|                | **Settings**                                                 | `Ctrl + ,`                              |
|                | **Undo**                                                     | `u`                                     |
|                | **Redo**                                                     | `Ctrl + r`                              |
|                | **Switch Pane (Hex Dump/Disasm)**                            | `Tab`                                   |
| **Navigation** | **Move Cursor**                                              | `Up` / `Down` / `j` / `k`               |
|                | **Page Up/Down**                                             | `PageUp` / `PageDown`                   |
|                | **Home/End**                                                 | `Home` / `End`                          |
|                | **Jump to Address (Dialog)**                                 | `g`                                     |
|                | **Jump to Line (Dialog)**                                    | `Ctrl + Shift + g`                      |
|                | **Jump to Line / End of File**                               | `[Number] G`                            |
|                | **Jump to Operand**                                          | `Enter`                                 |
|                | **Jump Back (History)**                                      | `Backspace`                             |
|                | **Previous/Next 10 Lines**                                   | `Ctrl + u` / `Ctrl + d`                 |
| **Search**     | **Vim Search**                                               | `/`                                     |
|                | **Next / Previous Match**                                    | `n` / `Shift + n`                       |
|                | **Search Dialog**                                            | `Ctrl + f`                              |
|                | **Find Next / Previous**                                     | `F3` / `Shift + F3`                     |
| **Selection**  | **Toggle Visual Mode**                                       | `V`                                     |
|                | **Select Text**                                              | `Shift + Up/Down` / Visual Mode + `j/k` |
|                | **Clear Selection**                                          | `Esc`                                   |
| **Editing**    | **Set Label**                                                | `l`                                     |
|                | **Add Side Comment**                                         | `;`                                     |
|                | **Add Line Comment**                                         | `:`                                     |
|                | **Convert to Code**                                          | `c`                                     |
|                | **Convert to Byte**                                          | `b`                                     |
|                | **Convert to Word**                                          | `w`                                     |
|                | **Convert to Address**                                       | `a`                                     |
|                | **Convert to Text**                                          | `t`                                     |
|                | **Convert to Screencode**                                    | `s`                                     |
|                | **Convert to Undefined**                                     | `?`                                     |
|                | **Next/Prev Immediate Mode Format**                          | `d` / `D`                               |
|                | **Set Lo/Hi Address**                                        | `<`                                     |
|                | **Set Hi/Lo Address**                                        | `>`                                     |
|                | **Collapse Block**                                           | `Ctrl + k`                              |
|                | **Uncollapse Block**                                         | `Ctrl + Shift + k`                      |
|                | **Toggle Splitter**                                          | `\|`                                    |
|                | **Analyze**                                                  | `Ctrl + a`                              |
| **View**       | **Toggle PETSCII Shifted/Unshifted** (only in Hex Dump View) | `m`                                     |
|                | **Toggle Multicolor Sprites** (only in Sprites View)         | `m`                                     |
|                | **Toggle Multicolor Charset** (only in Charset View)         | `m`                                     |
|                | **Toggle Hex Dump View**                                     | `Ctrl + 2`                              |
|                | **Toggle Sprites View**                                      | `Ctrl + 3`                              |
|                | **Toggle Charset View**                                      | `Ctrl + 4`                              |
| **Menus**      | **Navigate Menu**                                            | Arrows                                  |
|                | **Select Item**                                              | `Enter`                                 |
|                | **Close Menu**                                               | `Esc`                                   |

## Build and Run

```bash
cargo run
```

## User Guide

* [User Guide](docs/user_guide.md)
