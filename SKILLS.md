# 6502 Disassembler Expert Skills

## Core Context
- **Project Goal:** A 6502 disassembler in Rust with a Ratatui-based TUI, inspired by Borland Turbo Disassembler.
- **Architecture:** MOS 6502 (including support for undocumented opcodes and C64-specific memory mapping).
- **Primary Stack:** Rust, Ratatui (TUI), and bit-manipulation crates.

## Technical Requirements
### 1. 6502 Logic
- Always account for variable instruction lengths (1, 2, or 3 bytes).
- Handle all 13 standard addressing modes (Absolute, Indexed, Indirect, etc.).
- When requested, include support for "Illegal" opcodes (e.g., NOPs, LAX, SAX).

### 2. Rust Performance & Safety
- Use `nom` or manual bit-shifting for high-performance opcode parsing.
- Prioritize zero-copy disassembly where possible (using `&[u8]` slices).
- Ensure the TUI rendering loop is decoupled from the disassembly engine to prevent UI lag.

### 3. TUI (Ratatui) Specifics
- Implement a "Virtual List" approach for the disassembly view to handle large binaries without memory bloat.
- Mimic the classic Turbo Disassembler layout: Disassembly Window, Register View, and Hex Dump.

## Forbidden Patterns
- Do not use `unwrap()` on binary parsing; use proper `Result` handling for corrupted binaries.
- Avoid heavy standard library abstractions that introduce unnecessary overhead for an embedded-focused tool.
