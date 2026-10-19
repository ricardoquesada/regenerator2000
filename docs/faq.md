# Frequently Asked Questions (FAQ)

## General

### How do I start analyzing a new file?

Select **File → Open** (++ctrl+o++) or pass the filename as a command-line argument:

```bash
regenerator2000 my_game.prg
```

### What file formats are supported?

Regenerator 2000 supports the following formats:

| Format           | Description                                                      |
|:-----------------|:-----------------------------------------------------------------|
| `.prg`           | Standard Commodore program (first 2 bytes = load address)        |
| `.crt`           | C64 cartridge image with bank selection                          |
| `.d64`           | 35/40/42-track disk image (select a PRG from the image)          |
| `.d71`           | 70/80-track double-sided disk image                              |
| `.d81`           | 80-track disk image                                              |
| `.t64`           | Tape image container (select a PRG from the container)           |
| `.vsf`           | VICE snapshot — extracts RAM and uses PC as the start address    |
| `.bin` / `.raw`  | Raw binary — set the origin manually                             |
| `.dis65`         | 6502bench SourceGen project file                                 |
| `.regen2000proj` | Regenerator 2000 project file (includes all labels and comments) |

### What systems are supported?

Regenerator 2000 includes system-specific labels, comments, and memory maps for these systems:

- Commodore 64
- Commodore 128
- Commodore VIC-20
- Commodore Plus/4
- Commodore PET (BASIC 2.0 and 4.0)
- Commodore 1541 disk drive

Select the target system in **Document Settings** (++alt+d++ or ++ctrl+shift+d++).

### How do I save my work?

Press ++ctrl+s++ to save the project as a `.regen2000proj` file. This file stores all labels, comments, block types, and bookmarks. Press ++alt+s++ (or ++ctrl+shift+s++) to save the project under a new filename.

### Does it run on Windows, macOS, and Linux?

Yes. Download pre-compiled binaries for all three platforms from the [GitHub Releases](https://github.com/ricardoquesada/regenerator2000/releases/latest) page. You can also install the package from Crates.io with `cargo install regenerator2000`.

### How do I reopen a recent project?

Select **File → Open Recent** (++alt+o++ or ++ctrl+shift+o++) to open your recent files and projects.

---

## Disassembly

### How do I change the type of a block?

Move the cursor to the target line (or press ++shift+v++ to select a range in Visual Mode) and press the block key:

| Key               | Block Type          |
|:------------------|:--------------------|
| ++c++             | Code                |
| ++b++             | Byte                |
| ++w++             | Word                |
| ++a++             | Address             |
| ++p++             | PETSCII Text        |
| ++s++             | Screencode Text     |
| ++less-than++     | Lo/Hi Address Table |
| ++greater-than++  | Hi/Lo Address Table |
| ++comma++         | Lo/Hi Word Table    |
| ++period++        | Hi/Lo Word Table    |
| ++e++             | External File       |
| ++question-mark++ | Undefined (reset)   |

See [Block Types](blocks.md) for descriptions and assembler-specific examples.

### How do I select multiple lines?

Press ++shift+v++ to enter **Visual Mode**. Press ++arrow-up++ or ++arrow-down++ (or ++j++ / ++k++) to extend the selection. Press a block-type key to convert the entire selection, or press ++escape++ to exit Visual Mode.

### Can I rename labels?

Yes. Move the cursor to a label and press ++l++. Type the new name and press ++enter++. Regenerator 2000 updates every instruction that references that address with the new name.

### What do the label prefixes mean?

Each auto-generated label starts with a prefix that indicates how the code references that address. For example, `s_C000` marks a subroutine at `$C000`, and `zpa_A0` marks a zero-page absolute address at `$A0`.

See [Analysis — Label Prefixes](analysis.md#label-prefixes) for the complete prefix table and assignment rules.

### What is the difference between a local and a global label?

- **Global Labels**: Top-level symbols that any instruction in the program can reference.
- **Local Labels** (such as `_loop` or `.skip`): Belong to the nearest preceding **Global Label** and remain visible only until the next global label. Use local labels for loop targets to avoid name collisions across subroutines.
- **Scope Labels**: Belong to a custom scope block (such as a procedure or block). Code outside the scope must reference them as `scope_name.label_name`.

Press ++l++ to open the label dialog, press ++tab++ to focus the local scope checkbox, and press ++space++ to toggle it.

### How do I navigate the disassembly?

| Action                | Shortcut                |
|:----------------------|:------------------------|
| Jump to address       | ++ctrl+g++ or ++alt+g++ |
| Jump to line number   | ++ctrl+shift+g++        |
| Follow a jump/branch  | ++enter++               |
| Go back in history    | ++backspace++           |
| Go to symbol by name  | ++ctrl+p++              |
| Find cross-references | ++ctrl+x++              |
| Search (Vim-style)    | ++slash++               |
| Search dialog         | ++ctrl+f++              |
| Next / previous match | ++n++ / ++shift+n++     |

### What are arrows in the disassembly view?

The left columns of the disassembly view draw arrows for jump (`JMP`) and branch (`BNE`, `BEQ`, `BCC`) targets. These arrows show loops and conditional paths. Change the "Arrow Columns" value in **Document Settings** to adjust the number of columns.

### How does undo/redo work?

Press ++u++ to undo the last action, and press ++ctrl+r++ to redo it. Regenerator 2000 records every block change, label edit, and comment edit on the undo stack and saves it in the project file.

### What is "Patch BRK"?

The 6502 `BRK` instruction occupies 2 bytes, but most C64 programs use it as a 1-byte instruction. By default, Regenerator 2000 enables **BRK single byte**. If your program uses 2-byte `BRK` instructions, clear **BRK single byte** in Document Settings and enable **Patch BRK** to preserve the padding byte. See [Settings](settings.md) for details.

---

## Labels & Comments

### How do I add a comment?

- **Side comment** (inline after the instruction): Press ++semicolon++ and enter your comment.
- **Line comment** (on a separate line above the instruction): Press ++colon++ and enter your comment.

Line comments also act as **splitters** that prevent adjacent blocks of the same type from merging.

### Can I write multi-line comments?

Yes. In the line comment dialog, press ++ctrl+j++ (or ++shift+enter++) to insert a line break. Use these shortcuts to insert separator lines:

- ++alt+minus++: Insert a dash separator (`---`).
- ++alt+equal++: Insert an equals separator (`===`).
- ++alt+backslash++: Insert a mixed separator (`-=-`).

### What are bookmarks and how do I use them?

Bookmarks mark specific addresses so you can return to them quickly:

- ++ctrl+b++: Toggle a bookmark at the current address.
- ++ctrl+shift+b++ (or ++alt+b++): Open the Bookmarks dialog to jump to a saved bookmark.

---

## Scopes

### What are scopes?

Scopes (also called namespaces or procedures) group code into logical blocks such as subroutines. Labels inside a scope remain local to that scope, so two routines can both use a label named `loop` without conflict.

### How do I create a scope?

1. Press ++shift+v++ to select the code range in Visual Mode.
2. Press ++r++ to create a scope over the selection.

If the start address has no label, Regenerator 2000 creates a default label. Press ++l++ to rename it.

### How do I remove a scope?

Move the cursor to the first or last line of the scope and press ++delete++.

If you place the cursor on an external label, ++delete++ excludes that external address from analysis instead.

### Which assemblers support scopes?

| Assembler     | Scope Syntax            |
|:--------------|:------------------------|
| 64tass        | `.block` / `.bend`      |
| KickAssembler | `{` / `}`               |
| ca65          | `.proc` / `.endproc`    |
| ACME          | Not supported (ignored) |

See [Block Types — Scopes](blocks.md#scopes) for syntax examples in each assembler.

---

## Views

### How do I switch between views?

Press ++tab++ to switch focus between the Disassembly View (left pane) and the right pane. Use these shortcuts to select the active view in the right pane:

| Shortcut                | View     |
|:------------------------|:---------|
| ++alt+1++ or ++ctrl+1++ | Blocks   |
| ++alt+2++ or ++ctrl+2++ | Hex Dump |
| ++alt+3++ or ++ctrl+3++ | Sprites  |
| ++alt+4++ or ++ctrl+4++ | Charset  |
| ++alt+5++ or ++ctrl+5++ | Bitmap   |
| ++alt+6++ or ++ctrl+6++ | Debugger |

### How do I sync the right pane with the disassembly?

By default, the [Hex Dump](views.md#hexdump-view) and Blocks views follow the Disassembly cursor. Open **File → Settings** (++alt+p++ or ++ctrl+comma++) to toggle cursor synchronization for each view.

---

## Exporting

### Which assemblers are supported for export?

Regenerator 2000 exports assembly code for four assemblers:

1. **64tass**
2. **ACME**
3. **KickAssembler**
4. **ca65**

See [Assemblers](assemblers.md) for command-line flags and linker configurations.

### How do I export my project?

- ++ctrl+e++: Export to ASM with the previous filename, or prompt if none exists.
- ++alt+e++ (or ++ctrl+shift+e++): Prompt for a filename and export.

Run exports from the command line in headless mode:

```bash
# Export to ASM:
regenerator2000 --headless --export_asm output.asm my_file.regen2000proj

# Override assembler format:
regenerator2000 --headless --assembler acme --export_asm output.asm my_file.regen2000proj
```

### Can I export VICE labels?

Yes. Select **File → Export → Export VICE Labels...** to generate a `.lbl` file for the VICE monitor.

### What is the "External File" block type for?

When you mark a memory region as "External File" (++e++), the exporter writes those bytes to a separate `.bin` file. It then emits a `.binary "filename.bin"` directive in the `.asm` output to keep large binary assets (such as music or graphics) separate.

---

## VICE Debugger

### How do I connect to VICE?

1. Start VICE with the binary monitor active: `x64 -binarymonitor my_program.prg`
2. Load the **same** binary in Regenerator 2000: `regenerator2000 my_program.prg`
3. Select **Debugger → Connect to VICE...** and press ++enter++ (default: `localhost:6502`).
4. Press ++alt+6++ to open the Debugger panel.

!!! warning

    Run the **same binary** in both VICE and Regenerator 2000. If the binaries differ, breakpoints and the PC display will not align.

### What debugging operations are supported?

| Action               | Shortcut     |
|:---------------------|:-------------|
| Toggle Breakpoint    | ++f2++       |
| Toggle Breakpoint... | ++shift+f2++ |
| Run to Cursor        | ++f4++       |
| Watchpoint           | ++f6++       |
| Step Instruction     | ++f7++       |
| Step Over            | ++f8++       |
| Step Out             | ++shift+f8++ |
| Run / Continue       | ++f9++       |

See [Debugger (VICE Integration)](debugger.md) for full details.

---

## MCP Integration

### What is MCP?

The **Model Context Protocol (MCP)** is an open standard that lets AI assistants control tools programmatically. Regenerator 2000 includes an MCP server so AI agents can read disassembly, set labels, add comments, and change block types.

### How do I start the MCP server?

- **HTTP mode** (port 3000): `regenerator2000 --mcp-server my_project.regen2000proj`
- **stdio mode** (headless): `regenerator2000 --mcp-server-stdio my_project.regen2000proj`

See [MCP Integration](mcp.md) for the complete list of tools and resources.

---

## Troubleshooting

### Some keyboard shortcuts don't work in my terminal

Terminals process key combinations differently. If a ++ctrl++ shortcut fails, use the ++alt++ shortcut instead. Use one of these modern terminals for full shortcut support:

- **macOS**: iTerm2, Ghostty, Alacritty, kitty, WezTerm
- **Windows**: Windows Terminal, Alacritty, WezTerm
- **Linux**: Ghostty, Alacritty, kitty, WezTerm, GNOME Terminal

### The display looks garbled or colors are wrong

Verify that your terminal supports 256 colors or 24-bit true color. If you use `tmux`, add `set -g default-terminal "tmux-256color"` to your `tmux` configuration file.

### Why is my BASIC program disassembling as illegal opcodes?

Commodore BASIC programs store tokenized byte sequences linked by 2-byte pointers. If you convert a BASIC line to 6502 code (++c++), the decoder interprets BASIC tokens (such as `$99` for `PRINT` or `$8F` for `REM`) as opcodes.

To fix this, select the BASIC memory range and press ++p++ (**PETSCII Text**) or ++w++ (**Word**). Alternatively, run the `r2000-analyze-basic` skill to format BASIC pointers and decode tokens into side comments automatically.

### Why did VICE binary monitor fail to connect?

Regenerator 2000 connects to the VICE **Binary Monitor Protocol** over TCP (default port `6502`), not the plain-text monitor:

- **Enable Binary Monitor mode**: Start VICE with `x64 -binarymonitor` or enable **Settings → Monitor → Enable binary monitor**.
- **Check the port type**: The binary monitor sends 4-byte binary headers (`0x02`, `0x00`, little-endian length). Connecting to a plain-text monitor port (such as `6510`) fails the handshake.
- **Load identical binaries**: Load the exact same `.prg` file in both VICE and Regenerator 2000 so PC addresses and breakpoints match.

### Why does roundtrip assembler export verification fail?

Roundtrip verification (`cargo test --test roundtrip_export_test` or `--verify`) compares the reassembled output byte-for-byte against the original binary. Common failure causes include:

- **Symbol Scoping**: A local label (such as `.skip`) resolves to a different global label scope in the target assembler.
- **Branch Bounds**: An assembler expands a relative branch (`BNE`) into a long jump (`JMP`) when the offset exceeds -128/+127 bytes.
- **BRK Padding**: The 6502 `BRK` opcode takes 2 bytes (`$00 $00`), whereas C64 code often treats it as 1 byte. Enable **Patch BRK** in Document Settings if your binary requires 2-byte `BRK` alignment.

### Why does the Binary Unpacker fail or timeout?

The unpacker runs a cycle-accurate 6502 emulation sandbox (`src/unpacker/`) to decompress binaries:

- **Timeout Safety**: The unpacker stops after **50,000,000 instructions** (`max_instructions`). If a depacker stub waits on an unemulated hardware condition (such as raster line `$D012`), emulation reaches the instruction limit and stops.
- **Vector Traps**: The unpacker traps KERNAL ROM vectors (`$FFE4` GETIN and `$FFD2` CHROUT) to supply keystrokes and execute `RTS`. For non-standard depackers, enter explicit **Entry Point** or **Depacker Address** values in the Unpack Options dialog.

### How do I report a bug or request a feature?

Open an issue on GitHub: [https://github.com/ricardoquesada/regenerator2000/issues](https://github.com/ricardoquesada/regenerator2000/issues).

You can also join the `#regenerator2000` channel under "Misc Projects" on the [Discord](https://discord.gg/r5aMn6Cw5q) server.
