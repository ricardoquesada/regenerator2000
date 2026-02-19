---
name: r2000-analyze-blocks
description: Analyzes memory regions of a disassembled binary and converts them to the correct block types (code, bytes, words, text, tables, etc.) using MOS 6502 and the target platform's expertise.
---

# Analyze Blocks Workflow

Use this skill when the user asks to "analyze blocks", "convert blocks", "identify data regions",
"classify the program", or wants the AI to scan a range (or the entire binary) and mark regions
with their correct block types.

## Overview

A freshly loaded binary in Regenerator 2000 starts with everything marked as **Code**. The goal
of this skill is to walk through the binary, identify what each region _actually_ is, and convert
it to the appropriate block type. This is a fundamental step in reverse engineering — separating
code from data, text from tables, and pointers from raw bytes.

## Block Types Reference

| Block Type          | Tool                                    | When to Use                                                                                             |
| ------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| **Code**            | `r2000_convert_region_to_code`          | Executable MOS 6502 instructions. Valid opcode sequences with coherent control flow.                    |
| **Byte**            | `r2000_convert_region_to_bytes`         | Raw 8-bit data: sprite data, bitmap data, charset data, lookup tables, variables, or unknown data.      |
| **Word**            | `r2000_convert_region_to_words`         | 16-bit little-endian values: 16-bit variables, math constants, SID frequency values.                    |
| **Address**         | `r2000_convert_region_to_address`       | 16-bit little-endian pointers to memory locations. Creates cross-references. For jump tables & vectors. |
| **PETSCII Text**    | `r2000_convert_region_to_petscii`       | PETSCII-encoded strings: game messages, prompts, high score names, print routine data.                  |
| **Screencode Text** | `r2000_convert_region_to_screencode`    | Screen code text: data written directly to Screen RAM ($0400–$07E7).                                    |
| **Lo/Hi Address**   | `r2000_convert_region_to_lo_hi_address` | Split address table: first half = low bytes, second half = high bytes. Even byte count required.        |
| **Hi/Lo Address**   | `r2000_convert_region_to_hi_lo_address` | Split address table: first half = high bytes, second half = low bytes. Even byte count required.        |
| **Lo/Hi Word**      | `r2000_convert_region_to_lo_hi_word`    | Split word table: first half = low bytes, second half = high bytes. E.g., SID frequency tables.         |
| **Hi/Lo Word**      | `r2000_convert_region_to_hi_lo_word`    | Split word table: first half = high bytes, second half = low bytes.                                     |
| **External File**   | `r2000_convert_region_to_external_file` | Large binary blobs: SID music files, raw bitmaps, character sets that should be exported as-is.         |
| **Undefined**       | `r2000_convert_region_to_undefined`     | Reset a region to unknown state. Use to undo a wrong classification.                                    |

## Step-by-Step Workflow

### 1. Determine Scope

- Ask the user what range to analyze, or default to the **entire binary**.
- Use `r2000_get_binary_info` to get the origin address, size, **platform**, and **description**.
  - **CRITICAL**: The `platform` field tells you the target computer (e.g., C64, VIC-20). You **MUST** become an expert in that specific target computer's memory map, hardware registers, and KERNAL routines for the duration of the analysis.
  - **CONTEXT**: The `filename` field (e.g., "burnin_rubber.prg", "turrican.d64") and `description` (if provided by the user) give you the specific software context. Use this to search for known memory maps, common drivers (music, compression), and game-specific variables.
- Use `r2000_get_analyzed_blocks` to see what has already been classified.
- If the user says "the whole thing" or "entire binary", work in chunks of **~256–512 bytes** to avoid overwhelming context windows.

### 2. Plan the Analysis Order

Process the binary in **multiple passes**, in this order:

1. **Pass 1 — Find entry points and trace code**: Start from known entry points (reset vector, `JSR`/`JMP` targets). Mark reachable code as Code.
2. **Pass 2 — Identify text strings**: Look for PETSCII or screencode strings embedded between code blocks.
3. **Pass 3 — Identify data tables**: Look for byte tables, word tables, address tables, and split (Lo/Hi) tables.
4. **Pass 4 — Classify remaining regions**: Anything not yet classified — decide if it's code, data, or unknown.

### 3. Read and Analyze Each Region

For each chunk of the binary:

- Use `r2000_read_disasm_region` to see how it disassembles.
- Use `r2000_read_hexdump_region` to see raw byte patterns.
- Apply the heuristics below to determine the block type.

### 4. Apply Conversions

- Use `r2000_batch_execute` to apply multiple conversions at once for efficiency.
- After each batch, use `r2000_get_analyzed_blocks` to verify the result.
- If a conversion was wrong, use `r2000_undo` to revert.
- Use `r2000_toggle_splitter` when you need to separate two adjacent regions of the same type (e.g., two separate byte tables side by side).

### 5. Label and Document

After classifying blocks, optionally:

- Use `r2000_set_label_name` to name entry points, tables, and strings.
- Use `r2000_set_side_comment` or `r2000_set_line_comment` to add context (using conventions from the **r2000-analyze-routine** skill if documenting subroutines).

---

## Identification Heuristics

### Recognizing Code

A region is likely **Code** if:

- It begins with valid MOS 6502 opcodes that form coherent instruction sequences.
- It contains control flow: `JMP`, `JSR`, `BEQ`, `BNE`, `BCC`, `BCS`, `RTS`, `RTI`, `BRK`.
- Branch/jump targets point to valid instruction boundaries.
- It has cross-references: other code calls it via `JSR` or jumps to it.
- It is referenced by a vector table or known entry point.
- Common prologues: `SEI`, `LDA #imm`, `LDX #imm`, `CLD`, `TXS`.
- **Warning signs of NOT code**: consecutive `BRK` ($00), long runs of same byte, decoded instructions that would crash (e.g., `JMP ($0000)`).

### Recognizing Data Bytes

A region is likely **Byte data** if:

- It contains regular patterns that don't form valid instruction sequences.
- It is referenced by `LDA addr,X` / `LDA addr,Y` patterns (table lookups).
- Sprite data: exactly 63 bytes (padded to 64) per sprite, often grouped.
- Bitmap data: regular repeating patterns, often 8 bytes per character cell.
- Color data: values in range $00–$0F (C64 colors).
- Random-looking bytes between two code blocks that produce nonsensical disassembly.

### Recognizing Words (16-bit values)

A region is likely **Word data** if:

- Pairs of bytes that form meaningful 16-bit values (screen addresses, timer values).
- Referenced by code that loads low/high bytes separately (`LDA addr` / `LDA addr+1`).

### Recognizing Address / Pointer Tables

A region is likely an **Address table** if:

- It contains pairs of bytes that, when read as little-endian 16-bit values, point to valid addresses within the binary.
- It is loaded via indirect addressing (`JMP ($addr)`) or indexed reads.
- Common for jump tables, dispatch tables, and vector lists.

### Recognizing Lo/Hi or Hi/Lo Split Tables

A region is likely a **Lo/Hi (or Hi/Lo) Address Table** if:

- There are two equally-sized halves.
- One half contains values that look like low bytes, the other like high bytes.
- Code references them separately: `LDA lo_table,X` / `LDA hi_table,X`, then pushes to stack or stores to a pointer.
- Common pattern: `LDA lo,X / STA ptr / LDA hi,X / STA ptr+1 / JMP (ptr)`.
- The reassembled addresses (combining low and high halves) point to valid locations.
- **Lo/Hi** = low bytes first, high bytes second (more common on 6502).
- **Hi/Lo** = high bytes first, low bytes second.

**Important**: When two split halves are in adjacent memory, use `r2000_toggle_splitter` at the boundary between the lo and hi halves to prevent the auto-merger from combining them into one block.

### Recognizing PETSCII Text

A region is likely **PETSCII text** if:

- Bytes are in the printable PETSCII range ($20–$7E for unshifted, $C0–$DF for shifted).
- Contains recognizable ASCII-like text (PETSCII shares $20–$5F with ASCII).
- Referenced by KERNAL print routines like `$FFD2` (CHROUT) or `$AB1E` (BASIC STROUT).
- Terminated by a null byte ($00), a return ($0D), or high-bit-set sentinel ($80+).
- Common in: game messages ("GAME OVER", "PRESS FIRE"), menus, credits.

### Recognizing Screencode Text

A region is likely **Screencode text** if:

- Bytes are screen codes ($00–$3F = uppercase, $00 = '@', $01 = 'A', etc.).
- Referenced by code that copies directly to $0400–$07E7 (Screen RAM).
- Patterns like `LDA data,X / STA $0400,X` strongly suggest screencode.
- Full screen dumps are exactly 1000 bytes ($03E8).

### Recognizing External Files (Binary Blobs)

A region is likely an **External File** if:

- It is a large contiguous block of non-code data (hundreds or thousands of bytes).
- It matches a known format: SID header (`PSID`/`RSID`), bitmap, charset.
- Charset data: 2048 bytes ($0800), 256 chars × 8 bytes each.
- Sprite data blocks: multiples of 64 bytes.
- The binary loads it to a hardware-mapped region (e.g., $D000 for charset, bitmap areas).

---

## Commodore 64 Memory Map Reference

When analyzing, keep these known address ranges in mind:

| Address Range | Description                             |
| ------------- | --------------------------------------- |
| `$0000–$00FF` | Zero Page (fast variables, pointers)    |
| `$0100–$01FF` | CPU Stack                               |
| `$0200–$03FF` | OS work area, BASIC input buffer        |
| `$0400–$07FF` | Default Screen RAM (1000 bytes + spare) |
| `$0800–$9FFF` | BASIC program area / free RAM           |
| `$A000–$BFFF` | BASIC ROM (or RAM underneath)           |
| `$C000–$CFFF` | Free RAM                                |
| `$D000–$D3FF` | VIC-II registers (when I/O visible)     |
| `$D400–$D7FF` | SID registers (when I/O visible)        |
| `$D800–$DBFF` | Color RAM                               |
| `$DC00–$DCFF` | CIA 1 (keyboard, joystick, IRQ)         |
| `$DD00–$DDFF` | CIA 2 (serial, NMI, VIC bank)           |
| `$E000–$FFFF` | KERNAL ROM (or RAM underneath)          |

### Well-Known KERNAL Entry Points

| Address | Name    | Purpose                       |
| ------- | ------- | ----------------------------- |
| `$FFD2` | CHROUT  | Output a character            |
| `$FFE4` | GETIN   | Get a character from keyboard |
| `$FFE1` | STOP    | Check STOP key                |
| `$FFCF` | CHRIN   | Input a character             |
| `$FFE7` | CLALL   | Close all files               |
| `$FFC0` | OPEN    | Open file                     |
| `$FFC3` | CLOSE   | Close file                    |
| `$FFBA` | SETLFS  | Set logical file              |
| `$FFBD` | SETNAM  | Set filename                  |
| `$FFD5` | LOAD    | Load from device              |
| `$FFD8` | SAVE    | Save to device                |
| `$FF81` | CINT    | Initialize screen editor      |
| `$FF84` | IOINIT  | Initialize I/O                |
| `$FF87` | RAMTAS  | Initialize RAM, tape, vectors |
| `$FFFE` | IRQ Vec | Hardware IRQ vector           |
| `$FFFA` | NMI Vec | Non-maskable interrupt vector |
| `$FFFC` | RESET   | Reset vector                  |

---

## Batch Conversion Strategy

When converting large ranges, use `r2000_batch_execute` to group conversions. Example:

```
r2000_batch_execute with calls:
  - r2000_convert_region_to_code:      $0801–$08FF
  - r2000_convert_region_to_bytes:     $0900–$093F
  - r2000_convert_region_to_petscii:   $0940–$097F
  - r2000_convert_region_to_code:      $0980–$0A00
```

This avoids making dozens of individual round-trip tool calls.

---

## Common Pitfalls

1. **Data misidentified as code**: Look for disassembly with nonsensical instruction sequences, impossible branches, or `BRK` ($00) floods. These are data, not code.
2. **Code misidentified as data**: If raw bytes form valid instruction sequences _and_ have incoming cross-references (JSR/JMP targets), they are probably code even if they look odd.
3. **Forgetting splitters**: Two adjacent byte tables will auto-merge into one. Use `r2000_toggle_splitter` at the boundary.
4. **Lo/Hi table half-size errors**: Lo/Hi and Hi/Lo tables _must_ have an even total byte count. Verify the halves are equal-sized.
5. **Text encoding confusion**: PETSCII ≠ Screencode. If copied to $0400, it's screencode. If passed to CHROUT ($FFD2), it's PETSCII.
6. **Undocumented opcodes**: Some C64 programs use illegal opcodes (e.g., `LAX`, `SAX`, `SLO`). These are valid code — don't misclassify them as data.

---

## Reporting Results

After completing the analysis, provide a summary:

- Total number of blocks identified, grouped by type.
- Notable findings (e.g., "Found 3 text strings", "Found a Lo/Hi jump table at $1200").
- Any uncertain/ambiguous regions that need human review.
- Offer to save the project using `r2000_save_project`.
