---
icon: material/frequently-asked-questions
description: Quick answers and troubleshooting fixes for terminals, tmux shortcuts, BASIC tokens, VICE monitor connections, roundtrip verification, and unpacking.
---

# Frequently Asked Questions (FAQ)

Click any question below to expand the answer, or jump straight to [Troubleshooting](#troubleshooting).

## General

/// details | How do I start analyzing a file?
    type: question

Select **File → Open** (++ctrl+o++) or pass a file directly on the command line (`regenerator2000 my_game.prg`). See [Installation & Usage](install.md) for all CLI options.
///

/// details | What file formats and target systems are supported?
    type: question

Regenerator 2000 opens `.prg`, `.crt`, `.d64`, `.d71`, `.d81`, `.t64`, `.vsf` (VICE snapshots), `.dis65` (SourceGen), raw `.bin`/`.raw`, and `.regen2000proj` files.

Built-in definitions cover **Commodore 64**, **128**, **VIC-20**, **Plus/4**, **PET (BASIC 2.0 & 4.0)**, and **1541**, plus experimental configs for NES, Apple II, Atari 8-bit, BBC Micro, and Oric. See [Target Systems](systems.md) and [Installation & Usage](install.md#supported-file-formats).
///

/// details | How do I save or reopen recent work?
    type: question

Press ++ctrl+s++ to save everything (binary, blocks, labels, comments, bookmarks, and undo stack) to a `.regen2000proj` file, or ++alt+s++ / ++ctrl+shift+s++ for **Save As**. Press ++alt+o++ (or ++ctrl+shift+o++) to reopen recent projects.
///

/// details | Does it run on Windows, macOS, and Linux?
    type: question

Yes—grab pre-built binaries from [GitHub Releases](https://github.com/ricardoquesada/regenerator2000/releases/latest) or install with `cargo install regenerator2000`.
///

---

## Troubleshooting

/// details | Some keyboard shortcuts don't work in my terminal or tmux
    type: warning

Terminals intercept certain `Ctrl` chords differently (for example, ++ctrl+2++ inside `tmux`). Every conflicting shortcut provides an `Alt` alternative—such as ++alt+2++ for Hexdump or ++alt+d++ for Document Settings.

For full key reporting and true-color rendering, use one of the [Recommended Terminals](install.md#recommended-terminals).
///

/// details | The display looks garbled or colors look wrong
    type: warning

Verify your terminal supports 256 colors or 24-bit true color. Inside `tmux`, add `set -g default-terminal "tmux-256color"` to `~/.tmux.conf`.
///

/// details | Why is my BASIC program disassembling as illegal opcodes?
    type: warning

Commodore BASIC stores tokenized byte sequences (like `$99` for `PRINT` or `$8F` for `REM`) linked by 2-byte pointers—so forcing **Code** (++c++) decodes tokens as nonsense opcodes.

**Fix:** Select the BASIC range and press ++p++ (**PETSCII**) or ++w++ (**Word**), or let an AI agent run the `r2000-analyze-basic` skill.
///

/// details | Why did the VICE binary monitor fail to connect?
    type: warning

Regenerator 2000 speaks the VICE **Binary Monitor Protocol** (default port `6502`), not the plain-text monitor (port `6510`).

- Start VICE with `x64 -binarymonitor my_program.prg` (or enable **Settings → Monitor → Enable binary monitor**).
- Match `--vice <HOST:PORT>` if you changed VICE's `-binarymonitoraddress`.
- Load the exact same `.prg` in both VICE and Regenerator 2000 so PC addresses and breakpoints align.
///

/// details | Why does roundtrip export verification (--verify) fail?
    type: warning

`--verify` exports to each assembler, assembles the output, and checks byte-for-byte parity against the original binary. Common mismatches come from:

- **Symbol scoping:** A local label resolving under a different global scope in a target assembler.
- **Branch bounds:** A relative branch exceeding `-128/+127` bytes and expanding to a long jump.
- **2-byte `BRK` padding:** Disable **BRK single byte** and enable **Patch BRK** in [Document Settings](settings.md) if your program expects 2-byte `BRK` alignment.
///

/// details | Why does the Binary Unpacker fail or time out?
    type: warning

The unpacker emulates a sandboxed 6502 CPU and aborts after **50,000,000 instructions** (`max_instructions`) so infinite wait loops cannot hang the UI.

If a custom stub waits on unemulated hardware (like raster `$D012`) or lacks a `SYS` header, open **File → Unpack Binary...** and raise **Max Instructions** or specify **Entry Point**, **Depacker Address**, or **Return Address** explicitly. See [Binary Unpacker](unpacker.md).
///

/// details | How do I report a bug or ask a question?
    type: question

Open an issue on [GitHub Issues](https://github.com/ricardoquesada/regenerator2000/issues) or drop into `#regenerator2000` (under *Misc Projects*) on [Discord](https://discord.gg/r5aMn6Cw5q).
///

---

## Disassembly & Block Types

/// details | How do I convert a range of bytes into code, data, or text?
    type: question

Put the cursor on a single line—or press ++shift+v++ first to select a range in **Visual Mode**—then hit the target key (++d++ flow disassemble, ++c++ code, ++b++ byte, ++w++ word, ++a++ address, ++p++ PETSCII, ++s++ screencode, ++shift+e++ external file, or ++question-mark++ reset). See [Block Types — Quick Reference](blocks.md#quick-reference).
///

/// details | What do the auto-generated label prefixes mean?
    type: question

Prefixes encode how the address was referenced (`s_` subroutine, `j_` jump, `b_` branch, `r_` return, `e_` external jump, `a_` absolute, `f_` indexed field, `p_` pointer, and `zpa_`/`zpf_`/`zpp_` zero-page variants). Press ++l++ to rename any label—your names are never overwritten. See [Analysis — Label Prefixes](analysis.md#label-prefixes).
///

/// details | How do I navigate jumps, symbols, and cross-references?
    type: question

Press ++enter++ to follow a jump, call, or address table entry (++backspace++ jumps back), ++ctrl+g++ to jump by hex address, ++ctrl+p++ for **Go to Symbol**, ++ctrl+x++ for **Find Cross References**, and ++slash++ or ++ctrl+f++ to search. See [Keyboard Shortcuts](keyboard_shortcuts.md#disassembly-navigation).
///

/// details | What are the arrows in the left margin, and how does Undo/Redo work?
    type: question

Left-margin arrows visualize jump and branch targets (adjust width via **Arrow Columns** in [Document Settings](settings.md)). Press ++u++ to undo and ++ctrl+r++ to redo—every edit is saved in `.regen2000proj`.
///

/// details | What is "Patch BRK"?
    type: question

6502 hardware treats `BRK` as 2 bytes, though most C64 code treats it as 1 byte (`BRK single byte` enabled by default). Disable **BRK single byte** and enable **Patch BRK** when your binary relies on 2-byte `BRK` signatures so assemblers emit an explicit padding `.byte`. See [Settings — BRK Handling](settings.md#brk-handling).
///

---

## Labels, Comments & Scopes

/// details | What is the difference between global, local, and scope labels?
    type: question

- **Global labels:** Visible anywhere in the program.
- **Local labels** (such as `_loop` or `.skip`): Belong to the nearest preceding global label (toggle inside the ++l++ dialog with ++tab++ then ++space++).
- **Scope labels:** Belong to an explicit scope range and are referenced externally as `scope_name.label_name` (or `::` in ca65).
///

/// details | How do comments and bookmarks work?
    type: question

- **Side comment (++semicolon++):** Inline note after the instruction.
- **Line comment (++colon++):** Multi-line header above the address (++ctrl+j++ / ++shift+enter++ for newlines; ++alt+minus++, ++alt+equal++, ++alt+backslash++ for dividers). Also splits `Address`/`Word` tables.
- **Bookmarks:** Press ++ctrl+b++ to toggle and ++ctrl+shift+b++ (or ++alt+b++) to jump between saved bookmarks.
///

/// details | How do scopes work, and which assemblers support them?
    type: question

Select a range in Visual Mode (++shift+v++) and press ++r++ to isolate local labels inside a routine; press ++delete++ on a scope boundary to remove it. Supported by **64tass** (`.block`/`.bend`), **KickAssembler** (`{`/`}`), and **ca65** (`.proc`/`.endproc`); omitted on **ACME** exports. See [Block Types — Scopes](blocks.md#scopes).
///

---

## Views & Exporting

/// details | How do I switch companion views and keep them synced?
    type: question

Press ++tab++ to move focus between the Disassembly pane and the right pane, and ++alt+1++–++alt+6++ (or ++ctrl+1++–++ctrl+6++) to toggle **Blocks**, **Hexdump**, **Sprites**, **Charset**, **Bitmap**, or **Debugger**. Toggle cursor sync per view in [Settings](settings.md#view-sync) (for Hexdump details, see [Views — Hexdump](views.md#hexdump-view)).
///

/// details | Which assemblers can I export to, and how do I automate exports?
    type: question

Export directly to **64tass**, **ACME**, **KickAssembler**, and **ca65** via ++ctrl+e++ (or ++alt+e++ for *Export As*). For scripts and CI, run headless mode:

```bash
regenerator2000 --headless --export_asm output.asm my_file.regen2000proj
regenerator2000 --headless --assembler acme --export_asm output.asm my_file.regen2000proj
```

You can also export VICE `.lbl` files from **File → Export VICE Labels...**, or isolate large binary assets into separate `.bin` includes with **External File** (++shift+e++). See [Target Assemblers](assemblers.md).
///

---

## VICE Debugger & MCP

/// details | How do I connect and use the VICE live debugger?
    type: question

Start VICE with `x64 -binarymonitor my_program.prg`, launch Regenerator 2000 with `--vice localhost:6502 my_program.prg` (or connect via **Debugger → Connect to VICE...**), and toggle the panel with ++alt+6++. Supports breakpoints (++f2++), watchpoints (++f6++), run-to-cursor (++f4++), stepping (++f7++ / ++f8++ / ++shift+f8++), and live memory dumps. See [Live Debugger](debugger.md).
///

/// details | How do I connect an AI assistant via MCP?
    type: question

Start Regenerator 2000 with `--mcp-server` (HTTP/SSE on port `3000`, live alongside the TUI) or `--mcp-server-stdio` (headless subprocess), then register `http://127.0.0.1:3000/mcp` in Claude Code or Antigravity. See [MCP Integration](mcp.md) for all 29 tools and analysis skills.
///
