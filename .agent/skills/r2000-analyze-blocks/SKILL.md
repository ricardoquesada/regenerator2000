---
name: r2000-analyze-blocks
description: Analyzes memory regions of a disassembled binary and converts them to the correct block types (code, bytes, words, text, tables, etc.) using MOS 6502 and the target system's expertise.
---

# Analyze Blocks Workflow

Use this skill when the user asks to "analyze blocks", "convert blocks", "identify data regions",
"classify the program", or wants the AI to scan a range (or the entire binary) and mark regions
with their correct block types.

## Overview

When a binary is loaded in Regenerator 2000, the auto-analyzer traces reachable code starting from
the entry point and marks those regions as **Code**. Everything else remains **Undefined** — these
are the blocks that have not yet been explored. The goal of this skill is to walk through the
undefined/unexplored regions, identify what each one _actually_ is, and convert it to the
appropriate block type. This is a fundamental step in reverse engineering — separating code from
data, text from tables, and pointers from raw bytes.

## Block Types Reference

Use `r2000_set_data_type` with the `data_type` enum value from the right column.

| Block Type          | `data_type` value | When to Use                                                                                             |
| ------------------- | ----------------- | ------------------------------------------------------------------------------------------------------- |
| **Code**            | `code`            | Executable MOS 6502 instructions. Valid opcode sequences with coherent control flow.                    |
| **Byte**            | `byte`            | Raw 8-bit data: sprite data, bitmap data, charset data, lookup tables, variables, or unknown data.      |
| **Word**            | `word`            | 16-bit little-endian values: 16-bit variables, math constants, SID frequency values.                    |
| **Address**         | `address`         | 16-bit little-endian pointers to memory locations. Creates cross-references. For jump tables & vectors. |
| **PETSCII Text**    | `petscii`         | PETSCII-encoded strings: game messages, prompts, high score names, print routine data.                  |
| **Screencode Text** | `screencode`      | Screen code text: data written directly to Screen RAM ($0400–$07E7).                                    |
| **Lo/Hi Address**   | `lo_hi_address`   | Split address table: first half = low bytes, second half = high bytes. Even byte count required.        |
| **Hi/Lo Address**   | `hi_lo_address`   | Split address table: first half = high bytes, second half = low bytes. Even byte count required.        |
| **Lo/Hi Word**      | `lo_hi_word`      | Split word table: first half = low bytes, second half = high bytes. E.g., SID frequency tables.         |
| **Hi/Lo Word**      | `hi_lo_word`      | Split word table: first half = high bytes, second half = low bytes.                                     |
| **External File**   | `external_file`   | Large binary blobs: SID music files, raw bitmaps, character sets that should be exported as-is.         |
| **Undefined**       | `undefined`       | Reset a region to unknown state. Use to undo a wrong classification.                                    |

## Step-by-Step Workflow

### 1. Determine Scope

- Ask the user what range to analyze, or default to the **entire binary**.
- Use `r2000_get_binary_info` to get the origin address, size, **system**, **description**, and **`may_contain_undocumented_opcodes`** hint.
  - **CRITICAL**: The `system` field tells you the target computer (e.g., C64, VIC-20). You **MUST** become an expert in that specific target computer's memory map, hardware registers, and KERNAL routines for the duration of the analysis.
  - **CONTEXT**: The `filename` field (e.g., "burnin_rubber.prg", "turrican.d64") and `description` (if provided by the user) give you the specific software context. Use this to search for known memory maps, common drivers (music, compression), and game-specific variables.
  - **UNDOCUMENTED OPCODES**: If `may_contain_undocumented_opcodes` is `true`, the binary may use illegal/undocumented MOS 6502 opcodes (e.g., `LAX`, `SAX`, `SLO`, `DCP`, `ISC`). Do **NOT** misclassify these instructions as data — they are valid code. This is a hint set by the user; it is not guaranteed, but you should be prepared to encounter them.
- Use `r2000_get_blocks` to see what has already been classified. **Focus on the Undefined blocks** — these are the unexplored regions that need classification.
- If the user says "the whole thing" or "entire binary", work in chunks of **~256–512 bytes** to avoid overwhelming context windows.

### 2. Plan the Analysis Order

Process the **Undefined** blocks in **multiple passes**, in this order:

1. **Pass 1 — Identify provably-reachable code**: Mark Undefined regions as Code **only** when there is concrete proof they are executed. See the strict criteria in [Recognizing Code](#recognizing-code) below. **Do NOT** convert a region to Code just because it "looks like" valid 6502 instructions — random data often disassembles into plausible-looking instruction sequences.
2. **Pass 2 — Identify text strings**: Look for PETSCII or screencode strings within Undefined regions.
3. **Pass 3 — Identify data tables**: Look for byte tables, word tables, address tables, and split (Lo/Hi) tables.
4. **Pass 4 — Classify remaining Undefined regions**: Anything still Undefined — decide if it's data or leave it as Undefined for human review. **Never** speculatively convert Undefined to Code in this pass.

### 3. Read and Analyze Each Region

For each chunk of the binary:

- Use `r2000_read_region` (with `"view": "disasm"`) to see how it disassembles.
- Use `r2000_read_region` (with `"view": "hexdump"`) to see raw byte patterns.
- Apply the heuristics below to determine the block type.

### 4. Apply Conversions

- Use `r2000_batch_execute` to apply multiple `r2000_set_data_type` calls at once for efficiency.
- After each batch, use `r2000_get_blocks` to verify the result.
- If a conversion was wrong, use `r2000_undo` to revert.
- Use `r2000_toggle_splitter` when you need to separate two adjacent regions of the same type (e.g., two separate byte tables side by side).

### 5. Label and Document

After classifying blocks, optionally:

- Use `r2000_set_label_name` to name entry points, tables, and strings.
- Use `r2000_set_comment` (type `"line"` or `"side"`) to add context (using conventions from the **r2000-analyze-routine** skill if documenting subroutines).

---

## Identification Heuristics

### Recognizing Code

> **CRITICAL**: Do NOT mark an Undefined region as Code just because it disassembles into valid-looking 6502 instructions. Random data frequently produces plausible instruction sequences. You **MUST** have at least one of the following concrete proofs before converting to Code:

A region should be marked as **Code** only when **at least one** of these conditions is met:

- **It is a `JSR`/`JMP` target**: Existing analyzed code contains a `JSR $addr` or `JMP $addr` that lands in this region. Check cross-references with `r2000_get_cross_references`.
- **It is a branch target**: An already-analyzed branch instruction (`BNE`, `BEQ`, `BCC`, `BCS`, `BPL`, `BMI`, `BVC`, `BVS`) targets this region.
- **It is a vector/handler**: The region's address appears in a known vector table (e.g., NMI, IRQ/BRK vectors at `$FFFA`–`$FFFF`), an Address or Lo/Hi Address block, or a jump table referenced by `JMP ($addr)`.
- **It is explicitly identified by the user**: The user tells you this region is code.

If none of these conditions are met, leave the region as **Undefined** or classify it as data — even if the bytes happen to disassemble into valid instructions.

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

## Batch Conversion Strategy

When converting large ranges, use `r2000_batch_execute` to group `r2000_set_data_type` calls. Example:

```
r2000_batch_execute with calls:
  - r2000_set_data_type: start=2049, end=2303, data_type="code"
  - r2000_set_data_type: start=2304, end=2367, data_type="byte"
  - r2000_set_data_type: start=2368, end=2431, data_type="petscii"
  - r2000_set_data_type: start=2432, end=2560, data_type="code"
```

This avoids making dozens of individual round-trip tool calls.

---

## Common Pitfalls

1. **Speculative code conversion**: This is the **most dangerous mistake**. Never mark a region as Code unless you have concrete proof it is executed (JSR/JMP/branch target, vector table entry, or user confirmation). Random data routinely disassembles into plausible-looking instruction sequences — this does NOT make it code.
2. **Data misidentified as code**: Look for disassembly with nonsensical instruction sequences, impossible branches, or `BRK` ($00) floods. These are data, not code.
3. **Code misidentified as data**: If raw bytes form valid instruction sequences _and_ have incoming cross-references (JSR/JMP targets), they are probably code even if they look odd.
4. **Forgetting splitters**: Two adjacent byte tables will auto-merge into one. Use `r2000_toggle_splitter` at the boundary.
5. **Lo/Hi table half-size errors**: Lo/Hi and Hi/Lo tables _must_ have an even total byte count. Verify the halves are equal-sized.
6. **Text encoding confusion**: PETSCII ≠ Screencode. If copied to $0400, it's screencode. If passed to CHROUT ($FFD2), it's PETSCII.
7. **Undocumented opcodes**: Some programs use illegal/undocumented opcodes (e.g., `LAX`, `SAX`, `SLO`). Check the `may_contain_undocumented_opcodes` hint from `r2000_get_binary_info`. If `true`, be extra cautious about classifying unfamiliar instruction sequences as data — they may be valid code using undocumented opcodes. Even if `false`, some programs still use them, so remain vigilant.

---

## Reporting Results

After completing the analysis, provide a summary:

- Total number of blocks identified, grouped by type.
- Notable findings (e.g., "Found 3 text strings", "Found a Lo/Hi jump table at $1200").
- Any uncertain/ambiguous regions that need human review.
- Offer to save the project using `r2000_save_project`.
