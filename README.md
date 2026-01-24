# Regenerator 2000

![Rust](https://github.com/ricardoquesada/regenerator2000/actions/workflows/rust.yml/badge.svg)
[![Crates.io Version](https://img.shields.io/crates/v/regenerator2000)](https://crates.io/crates/regenerator2000)
[![discord](https://img.shields.io/discord/775177861665521725.svg)](https://discord.gg/r5aMn6Cw5q)

![logo](docs/regenerator2000_logo.png)

A 6502 disassembler with a TUI. A modern take on [Regenerator][regenerator].

[regenerator]: https://csdb.dk/release/?id=247992

## Features

Regenerator 2000 brings modern conveniences to 6502 disassembly:

- **Disassembly**: Full 6502 support including undocumented opcodes.
- **Hex Dump**:
  - Side-by-side view with disassembly.
  - Synchronized or independent navigation.
  - Unshifted or Shifted PETSCII and Screencode charset.
- **Sprites**:
  - Side-by-side view with disassembly.
  - Multicolor or Single Color
- **Bitmap**:
  - Side-by-side view with disassembly.
  - High-Resolution (320x200) and Multicolor (160x200).
  - Uses quadrant-block rendering for full resolution in TUI.
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
  - **Kick Assembler**
  - **ca65**
- **Project Management**: Save and load your work with `.regen2000proj` files.
- **Analysis**: Auto-analysis to identify code and data regions.
- **Editing**:
  - **Labels**: Add, edit, and remove local and global labels.
  - **Comments**: Add side comments and line comments.
  - **Origin**: Change the load address/origin of the binary.
  - **Data Types**: Convert regions to Code, Byte, Word, Address, Lo/Hi Address, Hi/Lo Address, PETSCII Text,
    Screencode Text, External file or Unknown.
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
![bitmap screenshot](docs/regenerator2000_bitmap_screenshot.png)
![blocks screenshot](docs/regenerator2000_blocks_screenshot.png)

## Requirements

### Recommended Terminals

To ensure the best experience, especially regarding keyboard shortcuts and rendering, we recommend using a modern
terminal.

| Platform    | Recommended Terminals                              |
| :---------- | :------------------------------------------------- |
| **Windows** | Windows Terminal, Alacritty, WezTerm               |
| **macOS**   | iTerm2, Ghostty, Alacritty, kitty, WezTerm         |
| **Linux**   | Ghostty, Alacritty, kitty, WezTerm, GNOME Terminal |

## Installation

### From Crates.io

```bash
cargo install regenerator2000
```

### From Source

```bash
git clone https://github.com/ricardoquesada/regenerator2000.git
cd regenerator2000
cargo install --path .
```

## Usage

Start the application with an optional file to load:

```bash
regenerator2000 [path/to/file.prg]
```

Supported file formats: `.prg`, `.crt`, `.t64`, `.vsf`, `.bin`, `.raw`, and `.regen2000proj`.

## Keyboard Shortcuts

Some actions can be triggered with more than one keyboard combination. This is intentional to ensure compatibility
across Windows, macOS, and Linux, and different terminal emulators.

| Context                                         | Action                                                | Shortcut                                |
| :---------------------------------------------- | :---------------------------------------------------- | :-------------------------------------- |
| **Global**                                      | **Activate Menu**                                     | `F10`                                   |
|                                                 | **Exit**                                              | `Ctrl + q`                              |
|                                                 | **Open File**                                         | `Ctrl + o`                              |
|                                                 | **Save Project**                                      | `Ctrl + s`                              |
|                                                 | **Save Project As**                                   | `Alt + s`, `Ctrl + Shift + s`           |
|                                                 | **Export Project (ASM)**                              | `Ctrl + e`                              |
|                                                 | **Export Project As (ASM)**                           | `Alt + e`, `Ctrl + Shift + e`           |
|                                                 | **Document Settings**                                 | `Alt + d`, `Ctrl + Shift + d`           |
|                                                 | **Settings**                                          | `Alt + o`, `Ctrl + ,`                   |
|                                                 | **Undo**                                              | `u`                                     |
|                                                 | **Redo**                                              | `Ctrl + r`                              |
|                                                 | **Switch Pane (betweeen Disasm and right pane)**      | `Tab`                                   |
| **Navigation**                                  | **Move Cursor**                                       | `Up` / `Down` / `j` / `k`               |
|                                                 | **Page Up/Down**                                      | `PageUp` / `PageDown`                   |
|                                                 | **Home/End**                                          | `Home` / `End`                          |
|                                                 | **Jump to Address (Dialog)**                          | `g`                                     |
|                                                 | **Jump to Line (Dialog)**                             | `Alt + g`, `Ctrl + Shift + g`           |
|                                                 | **Jump to Line / End of File**                        | `[Number] G`                            |
|                                                 | **Jump to Operand**                                   | `Enter`                                 |
|                                                 | **Jump to Disassembly (from Panels)**                 | `Enter`                                 |
|                                                 | **Jump Back (History)**                               | `Backspace`                             |
|                                                 | **Previous/Next 10 Lines**                            | `Ctrl + u` / `Ctrl + d`                 |
| **Search**                                      | **Vim Search**                                        | `/`                                     |
|                                                 | **Next / Previous Match**                             | `n` / `Shift + n`                       |
|                                                 | **Search Dialog**                                     | `Ctrl + f`                              |
|                                                 | **Find Next / Previous**                              | `F3` / `Shift + F3`                     |
|                                                 | **Find Cross References**                             | `Ctrl + x`                              |
| **Selection**                                   | **Toggle Visual Mode**                                | `V`                                     |
|                                                 | **Select Text**                                       | `Shift + Up/Down` / Visual Mode + `j/k` |
|                                                 | **Clear Selection**                                   | `Esc`                                   |
| **Editing (Disassembly)**                       | **Set Label**                                         | `l`                                     |
|                                                 | **Add Side Comment**                                  | `;`                                     |
|                                                 | **Add Line Comment**                                  | `:`                                     |
|                                                 | **Convert to Code**                                   | `c`                                     |
|                                                 | **Convert to Byte**                                   | `b`                                     |
|                                                 | **Convert to Word**                                   | `w`                                     |
|                                                 | **Convert to Address**                                | `a`                                     |
|                                                 | **Convert to PETSCII Text**                           | `t`                                     |
|                                                 | **Convert to Screencode Text**                        | `s`                                     |
|                                                 | **Convert to Undefined**                              | `?`                                     |
|                                                 | **Next/Prev Immediate Mode Format**                   | `d` / `D`                               |
|                                                 | **Set Lo/Hi Address**                                 | `<`                                     |
|                                                 | **Set Hi/Lo Address**                                 | `>`                                     |
|                                                 | **Toggle Collapsed Block**                            | `Ctrl + k`                              |
|                                                 | **Toggle Splitter**                                   | `\|`                                    |
|                                                 | **Analyze**                                           | `Ctrl + a`                              |
| **Editing (HexDump, Sprites, Charset, Bitmap)** | **Convert to Byte**                                   | `b`                                     |
| **View**                                        | **Next / Prev Hex Text Mode** (only in Hex Dump View) | `m` / `Shift + m`                       |
|                                                 | **Toggle Multicolor Sprites** (only in Sprites View)  | `m`                                     |
|                                                 | **Toggle Multicolor Charset** (only in Charset View)  | `m`                                     |
|                                                 | **Toggle Bitmap Charset** (only in Bitmap View)       | `m`                                     |
|                                                 | **Toggle Hex Dump View**                              | `Alt + 2`, `Ctrl + 2`                   |
|                                                 | **Toggle Sprites View**                               | `Alt + 3`, `Ctrl + 3`                   |
|                                                 | **Toggle Charset View**                               | `Alt + 4`, `Ctrl + 4`                   |
|                                                 | **Toggle Blocks View**                                | `Alt + 5`, `Ctrl + 5`                   |
| **Menus**                                       | **Navigate Menu**                                     | Arrows                                  |
|                                                 | **Select Item**                                       | `Enter`                                 |
|                                                 | **Close Menu**                                        | `Esc`                                   |

## Support and Documentation

- [User Guide](docs/user_guide.md)
- [Architecture](docs/architecture.md)
- [Support in Discord][discord] (join the #regenerator2000 channel, under "Misc Projects")

[discord]: https://discord.gg/r5aMn6Cw5q

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgements

- [Regenerator][regenerator]: The original 6502 disassembler for Windows.
- [64tass][64tass], [ACME][ACME], [Kick Assembler][Kick Assembler], [ca65][ca65] : Supported assemblers.

[64tass]: https://tass64.sourceforge.net/
[ACME]: https://sourceforge.net/projects/acme-crossass/
[Kick Assembler]: http://www.theweb.dk/KickAssembler/
[ca65]: https://cc65.github.io/

## License

Dual license: MIT and Apache 2
