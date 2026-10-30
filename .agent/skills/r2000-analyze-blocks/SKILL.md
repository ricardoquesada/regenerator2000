---
name: r2000-analyze-blocks
description: Analyzes memory regions of a disassembled binary and converts them to the correct block types (code, bytes, words, text, tables, etc.) using the r2000-cbm-principal-swe agent persona.
---

# Analyze Blocks Workflow

Use this skill when the user asks to "analyze blocks", "convert blocks", "identify data regions", or "classify the program".

## Agent Persona (`r2000-cbm-principal-swe`)

This skill uses the **`r2000-cbm-principal-swe`** agent persona for MOS 6502 instructions (including undocumented opcodes), Commodore hardware registers, KERNAL/BASIC ROM maps, and data structures.

*   When spawned as the `r2000-cbm-principal-swe` subagent, the persona rules from `.agent/agents/r2000_cbm_principal_swe.md` are already loaded in your system prompt—**do not** read `.agent/agents/r2000_cbm_principal_swe.md` again.
*   Only read `.agent/agents/r2000_cbm_principal_swe.md` if you are executing this skill directly inside the main agent session.

## Overview

When Regenerator 2000 loads a binary, the auto-analyzer traces reachable code from the entry point and marks those regions as **Code**. All other regions remain **Undefined**. In this workflow, you inspect each Undefined region, identify its content, and set its block type.

## Block Types Reference

Use `r2000_set_data_type` with the `data_type` value in the table below (except for **Code**, where you must call `r2000_disassemble`):

| Block Type          | `data_type` Value / Tool | When to Use                                                                                         |
| ------------------- | ------------------------ | --------------------------------------------------------------------------------------------------- |
| **Code**            | Call `r2000_disassemble` | **Do not use `r2000_set_data_type`.** Call `r2000_disassemble` at the code entry point address.     |
| **Byte**            | `byte`                   | Raw 8-bit data: sprites, bitmaps, charsets, lookup tables, variables, or unknown data.              |
| **Word**            | `word`                   | 16-bit little-endian values: 16-bit variables, constants, SID frequency values, or `RTS` (`addr-1`) entries. |
| **Address**         | `address`                | 16-bit little-endian pointers to memory addresses. Creates cross-references for jump tables/vectors.|
| **PETSCII Text**    | `petscii`                | PETSCII strings: messages, prompts, high-score names, or print routine data.                        |
| **Screencode Text** | `screencode`             | Screen code text written directly to the active system's Screen RAM (`$0400` C64/C128, `$1E00`/`$1000` VIC-20, `$8000` PET, `$0C00` Plus/4). |
| **Lo/Hi Address**   | `lo_hi_address`          | Split address table: low bytes in the first half, high bytes in the second half (even byte count).  |
| **Hi/Lo Address**   | `hi_lo_address`          | Split address table: high bytes in the first half, low bytes in the second half (even byte count).  |
| **Lo/Hi Word**      | `lo_hi_word`             | Split 16-bit word table: low bytes first, high bytes second (such as `PHA/PHA/RTS` `addr-1` tables).|
| **Hi/Lo Word**      | `hi_lo_word`             | Split 16-bit word table: high bytes first, low bytes second.                                        |
| **External File**   | `external_file`          | Large binary blocks: SID music, raw bitmaps, or character sets.                                     |
| **Undefined**       | `undefined`              | Resets a region to the unknown state.                                                               |

## Step-by-Step Workflow

### 1. Determine Scope (Strict Address Range Isolation)

*   **Strict Address Range Isolation:** When an explicit address range `[start_address, end_address]` is passed in the prompt by the user or orchestrator (`r2000-analyze-program`), you must **only** inspect and classify `Undefined` blocks (or slices of `Undefined` blocks) within `[start_address, end_address]`. **Never** inspect, disassemble, or modify any blocks outside your assigned `[start_address, end_address]` range, as sibling subagents may be classifying adjacent ranges concurrently.
*   Only default to the entire binary when no address range is specified in the prompt.
*   If binary metadata is not already provided in your prompt, call `r2000_get_binary_info` to read the origin address, size, `system`, `filename`, `description`, and `may_contain_undocumented_opcodes` hint.
*   Call `r2000_get_blocks` to list blocks and locate the **Undefined** regions within your assigned `[start_address, end_address]` scope.
*   Process large Undefined regions in chunks of 256 to 512 bytes.

### 2. Plan the Analysis Order

Process the **Undefined** blocks within your assigned `[start_address, end_address]` scope in four passes:

1.  **Pass 1 — Identify Reachable Code:** Call `r2000_disassemble` at a code entry point **only** when you have concrete proof that the CPU executes that address. Follow the rules in [Recognizing Code](#recognizing-code). Never disassemble a region only because the bytes look like valid 6502 instructions.
2.  **Pass 2 — Identify Text Strings:** Locate PETSCII and screencode strings in Undefined regions.
3.  **Pass 3 — Identify Data Tables:** Locate byte tables, word tables, address tables, and split (`Lo/Hi` or `Hi/Lo`) tables.
4.  **Pass 4 — Classify Remaining Undefined Regions:** Convert remaining regions to data types (`byte`, `word`, `external_file`) or leave them as Undefined for manual review. Do not run speculative disassembly in Pass 4.

### 3. Read and Analyze Each Region

For each chunk within your assigned `[start_address, end_address]` scope:

*   Call `r2000_read_region` with `"view": "disasm"` to inspect instruction decoding.
*   Call `r2000_read_region` with `"view": "hexdump"` to inspect raw byte values and ASCII/PETSCII text.
*   Apply the identification rules below to select the block type.

### 4. Apply Conversions

*   **For Code:** Call `r2000_disassemble` at the entry point address. Then call `r2000_get_blocks` immediately to refresh the block list.
*   **For Data:** When you have `>= 5` independent `r2000_set_data_type` operations, call `r2000_batch_execute` to group them into one call (if fewer than 5 operations, call `r2000_set_data_type` directly).
*   **Per-Item Error Checking (`r2000_batch_execute`):** Because `r2000_batch_execute` returns top-level `Ok` even if individual calls inside `calls` fail, always inspect the returned per-item `"status"` fields in the JSON response array and handle or retry any inner `"error"` entries.
*   Call `r2000_get_blocks` after each batch to verify the updated boundaries.
*   Call `r2000_undo` if a conversion is incorrect.
*   Call `r2000_toggle_splitter` at the boundary between two adjacent blocks of the same type so they do not merge.

### 5. Label and Document

After classifying blocks within your assigned `[start_address, end_address]` scope:

*   Call `r2000_set_label_name` to name newly discovered entry points, tables, and strings (never overwrite valid `System` labels; if `r2000_set_label_name` fails with `"Label '<name>' already exists"`, disambiguate by appending the hex address suffix, such as `str_title_1400`, and retry).
*   Call `r2000_set_comment` (`"line"` or `"side"`) to document table layouts and purpose.

---

## Identification Rules

### Recognizing Code

> **IMPORTANT:** Do not mark an Undefined region as Code solely because it decodes into valid 6502 instructions. Require at least one of the following proofs before calling `r2000_disassemble`:

*   **`JSR` or `JMP` Target:** Analyzed code contains a `JSR $addr` or `JMP $addr` instruction targeting this address (verify with `r2000_get_cross_references`).
*   **Branch Target:** An analyzed branch instruction (`BNE`, `BEQ`, `BCC`, `BCS`, `BPL`, `BMI`, `BVC`, `BVS`) targets this address.
*   **Vector or Jump Table Target:** The address is stored in a hardware vector (`$FFFA–$FFFF`), software vector (`$0314–$0319` on C64/C128/VIC-20/Plus/4, or `$0090–$0095` on PET), pointer load (`LDA #<addr` / `LDA #>addr`), or jump table (`JMP ($addr)` or `PHA/PHA/RTS` table entry + 1).
*   **Explicit User Instruction:** The user identifies the address as code.

### Recognizing Data Bytes

Classify a region as `byte` when:

*   Code reads it with indexed addressing (`LDA addr,X` or `LDA addr,Y`).
*   It contains sprite data (63 bytes padded to 64 bytes per sprite).
*   It contains bitmap or character data (8 bytes per character cell).
*   It contains color values (such as `$00–$0F` nybbles on C64/C128/VIC-20, or `$00–$7F` luminance/color bytes on Plus/4).
*   It lies between two code blocks, has no incoming code flow, and contains non-instruction data.

### Recognizing Words (16-Bit Values)

Classify a region as `word` when:

*   Byte pairs form 16-bit little-endian numbers (such as timer reload values, screen offsets, or scores).
*   Code reads the low byte at `addr` and the high byte at `addr + 1`.

### Recognizing Address and Pointer Tables

Classify a region as `address` when:

*   Byte pairs form 16-bit little-endian addresses that point to valid targets in the binary or ROM.
*   Code reads the table through `JMP ($addr)` or indexed pointer loads.

### Recognizing Lo/Hi or Hi/Lo Split Tables

Classify a region as `lo_hi_address`, `hi_lo_address`, `lo_hi_word`, or `hi_lo_word` when:

*   Two equal-length byte arrays sit adjacent in memory (total byte count is even).
*   One half holds low bytes and the other half holds high bytes.
*   Code loads from both halves with the same index (`LDA lo_table,X` and `LDA hi_table,X`) and stores them to a pointer or pushes them to the stack.
*   Use `lo_hi_word` or `hi_lo_word` when the table stores `target_address - 1` for `PHA / PHA / RTS` dispatch.
*   Call `r2000_toggle_splitter` if needed to separate adjacent split tables.

### Recognizing PETSCII Text

Classify a region as `petscii` when:

*   Bytes fall in printable PETSCII ranges (`$20–$7E` or `$C0–$DF`) with terminators such as `$00`, `$0D`, or `$80+`.
*   Code passes the data to `$FFD2` (`CHROUT`) or a ROM string-output routine.

### Recognizing Screencode Text

Classify a region as `screencode` when:

*   Bytes encode screen codes (`$00` = `@`, `$01–$1A` = `A–Z`, `$20` = space, `$30–$39` = `0–9`).
*   Code copies the bytes directly to the active system's Screen RAM (`$0400–$07E7` on C64/C128, `$1E00–$1FFF` or `$1000–$11FF` on VIC-20, `$8000–$87CF` on PET, or `$0C00–$0FE7` on Plus/4).

### Recognizing External Files

Classify a region as `external_file` when:

*   It is a large binary asset such as a 2048-byte (`$0800`) character set, a bitmap, or an embedded SID module.

---

## Batch Conversion Example

When you have `>= 5` independent data-type changes, group them in `r2000_batch_execute`, and run `r2000_disassemble` separately for code entry points:

1.  Call `r2000_batch_execute` with `r2000_set_data_type` calls for `5+` data blocks (`byte`, `word`, `petscii`, `screencode`, `address`), and inspect the returned per-item `"status"` fields for any inner `"error"` entries.
2.  Call `r2000_disassemble` for each verified code entry point.
3.  Call `r2000_get_blocks` to refresh the block list.

---

## Common Pitfalls

1.  **Speculative Disassembly:** Never call `r2000_disassemble` without proof that execution reaches that address.
2.  **Using `r2000_set_data_type` for Code:** Never pass `"code"` to `r2000_set_data_type`. Always call `r2000_disassemble`.
3.  **Violating Assigned Range:** When given an address range `[start_address, end_address]`, never modify or disassemble blocks outside `[start_address, end_address]`.
4.  **Missing Splitters:** Use `r2000_toggle_splitter` between adjacent tables of the same type.
5.  **Odd Byte Count in Split Tables:** Ensure `lo_hi_*` and `hi_lo_*` blocks have an even number of bytes.
6.  **PETSCII vs. Screencode:** Bytes sent to `$FFD2` are `petscii`; bytes written to the active system's Screen RAM (`$0400` on C64/C128, `$1E00`/`$1000` on VIC-20, `$8000` on PET, `$0C00` on Plus/4) are `screencode`.
7.  **Undocumented Opcodes:** Check `may_contain_undocumented_opcodes` from `r2000_get_binary_info`. Do not misclassify valid undocumented opcodes (`LAX`, `SAX`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`) as raw data.

---

## Reporting Results

After completing block analysis, report:

*   Assigned address range `[start_address, end_address]` (or full binary) and total blocks classified, grouped by type.
*   Key discoveries (code entry points, text strings, jump tables, sprite/charset buffers).
*   Any ambiguous regions left for manual review.
