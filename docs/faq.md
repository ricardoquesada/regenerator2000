# Frequently Asked Questions (FAQ)

## General

### How do I start analyzing a new file?

You can open a file via **File → Open** (++ctrl+o++) or by passing the filename as an argument when starting the
application from the command line:

```bash
regenerator2000 my_game.prg
```

### What file formats are supported?

Regenerator 2000 supports the following formats:

| Format           | Description                                                      |
| :--------------- | :--------------------------------------------------------------- |
| `.prg`           | Standard Commodore program (first 2 bytes = load address)        |
| `.crt`           | C64 cartridge image with bank selection                          |
| `.d64`           | 35/40-track disk image (pick a PRG from within)                  |
| `.d71`           | 70-track double-sided disk image                                 |
| `.d81`           | 80-track disk image                                              |
| `.t64`           | Tape image container (pick a PRG from within)                    |
| `.vsf`           | VICE snapshot — extracts RAM and uses PC as start address        |
| `.bin` / `.raw`  | Raw binary — requires manually setting the origin                |
| `.dis65`         | 6502bench SourceGen project file                                 |
| `.regen2000proj` | Regenerator 2000 project file (includes all labels and comments) |

### What platforms are supported?

Regenerator 2000 includes platform-specific labels, comments, and memory maps for:

- Commodore 64
- Commodore 128
- Commodore VIC-20
- Commodore Plus/4
- Commodore PET (BASIC 2.0 and 4.0)
- Commodore 1541 disk drive

You can set the platform in **Document Settings** (++alt+d++ or ++ctrl+shift+d++).

### How do I save my work?

Press ++ctrl+s++ to save as a `.regen2000proj` file. This preserves all your labels, comments, block types and bookmarks.
Use ++alt+s++ (or ++ctrl+shift+s++) to **Save As** with a different filename.

### Does it run on Windows, macOS, and Linux?

Yes. Pre-compiled binaries are available for all three platforms on the
[GitHub Releases](https://github.com/ricardoquesada/regenerator2000/releases/latest) page.
You can also install from Crates.io with `cargo install regenerator2000`.

### How do I reopen a recent project?

Use **File → Open Recent** (++alt+o++ or ++ctrl+shift+o++) to quickly access your most recently opened files and
projects.

---

## Disassembly

### How do I change the type of a block?

Place the cursor on the desired line (or select a range with ++shift+v++ for Visual Mode) and press the corresponding
key:

| Key               | Block Type          |
| :---------------- | :------------------ |
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

See [Block Types](blocks.md) for detailed explanations and assembler-specific examples.

### How do I select multiple lines?

Press ++shift+v++ to enter **Visual Mode** (similar to Vim). Use ++arrow-up++ / ++arrow-down++ (or ++j++ / ++k++) to
extend the selection. Then apply a block type conversion or other operation to the entire selection.
Press ++escape++ to exit Visual Mode.

### Can I rename labels?

Yes. Place the cursor on a label and press ++l++. Type the new name and press ++enter++. The label will be renamed
globally — every instruction that references that address will use the new name.

### What do the label prefixes mean?

Auto-generated labels use a short prefix that indicates how the address is referenced in the code. For example,
`s_C000` means a subroutine entry point at `$C000`, while `zpa_A0` means a zero-page absolute address at `$A0`.

See [Analysis — Label Prefixes](analysis.md#label-prefixes) for the complete prefix reference and how the
analyzer decides which prefix to assign.

### What is the difference between a local and a global label?

- **Global Labels**: Top-level symbols accessible from anywhere in the program.
- **Local Labels** (e.g., `_loop`, `.skip` depending on the assembler): Belong to the nearest **Global Label** above them
  and are only visible between that global label and the next one. They are useful for loop targets or temporary variables
  to avoid name collisions between different subroutines.
- **Scope Labels**: When a global label is used _inside_ a custom scope block (such as an explicit procedure or block),
  it becomes a scope label and must be accessed via `scope_name.label_name` from the outside, rather than being a truly global one.

In the TUI, you can toggle between local and global scope when creating a label or editing an existing one.
Press ++l++ to open the label dialog, and then use ++tab++ to focus the checkbox and ++space++ to toggle it.

### How do I navigate the disassembly?

| Action                | Shortcut                |
| :-------------------- | :---------------------- |
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

The columns on the left side of the disassembly draw visual arrows showing the flow of jump (`JMP`) and branch
(`BNE`, `BEQ`, `BCC`, etc.) instructions. This helps you quickly see loops, conditional paths, and where control flow
goes. You can configure the number of arrow columns in **Document Settings** (the "Arrow Columns" option).

### How does undo/redo work?

Press ++u++ to undo the last action and ++ctrl+r++ to redo. Every block type change, label rename, comment edit,
and similar action is recorded on the undo stack. The history is saved with the project file.

### What is "Patch BRK"?

The 6502 `BRK` instruction is technically 2 bytes, but most C64 programs treat it as 1 byte. By default,
**BRK single byte** is enabled. If your program uses the 2-byte form, disable it in Document Settings and optionally
enable **Patch BRK** to ensure the padding byte is correctly exported.
See [Settings](settings.md) for details.

---

## Labels & Comments

### How do I add a comment?

- **Side comment** (inline, after the instruction): Press ++semicolon++ and type your comment.
- **Line comment** (on its own line, above the instruction): Press ++colon++ and type your comment.

Line comments also act as **splitters** — they prevent adjacent blocks of the same type from being auto-merged.

### Can I write multi-line comments?

Yes. In the line comment dialog, press ++ctrl+j++ (or ++shift+enter++) to insert a new line. You can also use shortcut
keys for quick separators:

- ++ctrl+minus++: Insert a dash separator (`---`)
- ++ctrl+plus++: Insert an equals separator (`===`)
- ++ctrl+backslash++: Insert a mixed separator (`-=-`)

### What are bookmarks and how do I use them?

Bookmarks let you mark addresses you want to return to quickly:

- ++ctrl+b++: Toggle a bookmark at the current address.
- ++ctrl+shift+b++ (or ++alt+b++): Open the Bookmarks dialog to navigate between bookmarks.

---

## Scopes

### What are scopes?

Scopes (also called namespaces or procedures) allow you to group code into logical blocks, typically representing
routines or functions. Labels defined inside a scope are **local** to that scope, preventing naming conflicts. For
example, two different routines can both have a local label called `loop` without collision.

### How do I create a scope?

1. Select the range of code using ++shift+v++ (Visual Mode).
2. Press ++r++ to create a scope over the selection.

A default label is created at the scope's start address if one doesn't already exist. You can rename it with ++l++.

### How do I remove a scope?

Place the cursor on the first or last line of the scope and press ++delete++.

### Which assemblers support scopes?

| Assembler     | Scope Syntax            |
| :------------ | :---------------------- |
| 64tass        | `.block` / `.bend`      |
| KickAssembler | `{` / `}`               |
| ca65          | `.proc` / `.endproc`    |
| ACME          | Not supported (ignored) |

See [Block Types — Scopes](blocks.md#scopes) for detailed examples in each assembler's syntax.

---

## Views

### How do I switch between views?

Use ++tab++ to switch focus between the Disassembly View (left) and the right pane. Toggle which view appears in the
right pane using:

| Shortcut                | View     |
| :---------------------- | :------- |
| ++alt+1++ or ++ctrl+1++ | Blocks   |
| ++alt+2++ or ++ctrl+2++ | Hex Dump |
| ++alt+3++ or ++ctrl+3++ | Sprites  |
| ++alt+4++ or ++ctrl+4++ | Charset  |
| ++alt+5++ or ++ctrl+5++ | Bitmap   |
| ++alt+6++ or ++ctrl+6++ | Debugger |

### How do I sync the right pane with the disassembly?

By default, the Hex Dump and Blocks views sync with the Disassembly cursor. You can enable or disable syncing for each
view independently in **File → Settings** (++alt+p++ or ++ctrl+comma++).

### What do the entropy values in the Hex Dump mean?

Entropy measures how "random" a block of data is (Shannon entropy, 0.0–8.0):

| Symbol  | Entropy Level     | Typical Content                |
| :------ | :---------------- | :----------------------------- |
| (empty) | Low (< 2.0)       | Repetitive data, zeroed memory |
| `░`     | Moderate (< 4.0)  | Text, simple data tables       |
| `▒`     | Mixed (< 6.0)     | Code, structured data          |
| `▓`     | High (< 7.5)      | Graphics, music data           |
| `█`     | Very High (≥ 7.5) | Compressed or encrypted data   |

If a file has overall high entropy, Regenerator 2000 displays a warning suggesting it may be packed or compressed.
You can adjust the threshold in Settings.

---

## Exporting

### Which assemblers are supported for export?

Regenerator 2000 supports exporting to:

1. **64tass**
2. **ACME**
3. **KickAssembler**
4. **ca65**

See [Assemblers](assemblers.md) for detailed command lines and configuration.

### How do I export my project?

- ++ctrl+e++: Export to ASM (uses the last saved filename, or prompts).
- ++alt+e++ (or ++ctrl+shift+e++): Export As (always prompts for a filename).

You can also export from the command line:

```bash
# Export to ASM:
regenerator2000 --headless --export_asm output.asm my_file.regen2000proj

# Override assembler format:
regenerator2000 --headless --assembler acme --export_asm output.asm my_file.regen2000proj
```

### Can I export VICE labels?

Yes. Use **File → Export → Export VICE Labels...** to generate a `.lbl` file that can be loaded in VICE's monitor for
debugging.

### What is the "External File" block type for?

When you mark a memory region as "External File" (++e++), the exporter writes those bytes to a separate `.bin` file
and emits a `.binary "filename.bin"` (or equivalent) directive in the ASM output. This keeps large binary blobs
(music, graphics, charset data) out of the main source file.

---

## VICE Debugger

### How do I connect to VICE?

1. Start VICE with the binary monitor enabled: `x64 -binarymonitor my_program.prg`
2. Load the **same** binary in Regenerator 2000: `regenerator2000 my_program.prg`
3. In Regenerator 2000, open **Debugger → Connect to VICE...** and press ++enter++ (default: `localhost:6502`).
4. Open the Debugger panel with ++alt+6++.

!!! warning

    Both VICE and Regenerator 2000 must be running the **same binary**. If the binaries differ, breakpoints
    and the PC display will be misaligned.

### What debugging operations are supported?

| Action               | Shortcut     |
| :------------------- | :----------- |
| Toggle Breakpoint    | ++f2++       |
| Toggle Breakpoint... | ++shift+f2++ |
| Run to Cursor        | ++f4++       |
| Watchpoint           | ++f6++       |
| Step Instruction     | ++f7++       |
| Step Over            | ++f8++       |
| Step Out             | ++shift+f8++ |
| Run / Continue       | ++f9++       |

For comprehensive details, see [Debugger (VICE Integration)](debugger.md).

---

## MCP Integration

### What is MCP?

The **Model Context Protocol (MCP)** is an open standard that allows AI assistants to interact with tools
programmatically. Regenerator 2000 implements an MCP server, enabling AI agents to read disassembly, set labels,
add comments, and manipulate blocks.

### How do I start the MCP server?

- **HTTP mode** (port 3000): `regenerator2000 --mcp-server my_file.prg`
- **stdio mode** (headless): `regenerator2000 --mcp-server-stdio my_file.prg`

See [MCP Integration](mcp.md) for the full list of available tools and resources.

---

## Troubleshooting

### Some keyboard shortcuts don't work in my terminal

Different terminals handle key combinations differently. If a ++ctrl++ shortcut doesn't work, try the ++alt++
alternative (most shortcuts have both). For the best experience, we recommend using a modern terminal:

- **macOS**: iTerm2, Ghostty, Alacritty, kitty, WezTerm
- **Windows**: Windows Terminal, Alacritty, WezTerm
- **Linux**: Ghostty, Alacritty, kitty, WezTerm, GNOME Terminal

### The display looks garbled or colors are wrong

Ensure your terminal supports 256 colors or true color (24-bit). Most modern terminals do.
If running inside `tmux` or `screen`, add `set -g default-terminal "tmux-256color"` to your tmux config.

### How do I report a bug or request a feature?

Open an issue on GitHub:
[https://github.com/ricardoquesada/regenerator2000/issues](https://github.com/ricardoquesada/regenerator2000/issues)

You can also join the [Discord](https://discord.gg/r5aMn6Cw5q) server (look for the `#regenerator2000` channel under
"Misc Projects").
