---
name: r2000-analyze-basic
description: Analyzes a sequence of memory containing Commodore BASIC tokens, formats address/word data types, and constructs side comments representing the plain BASIC commands using the r2000-cbm-principal-swe agent persona.
---

# Analyze BASIC Tokens Workflow

Use this skill when the user asks to:

*   "analyze this BASIC code"
*   "decode BASIC commands from memory"
*   "create side comments for BASIC lines"
*   "parse BASIC pointer address and line number"

## Agent Persona (`r2000-cbm-principal-swe`)

This skill uses the **`r2000-cbm-principal-swe`** agent persona for Commodore BASIC header conventions (`SYS` entry points, memory layout) and 6502 context.

*   When spawned as the `r2000-cbm-principal-swe` subagent, the persona rules from `.agent/agents/r2000_cbm_principal_swe.md` are already loaded in your system prompt—**do not** read `.agent/agents/r2000_cbm_principal_swe.md` again.
*   Only read `.agent/agents/r2000_cbm_principal_swe.md` if you are executing this skill directly inside the main agent session.

## 1. Determine Memory Range Autonomously

Identify the start and end addresses of the Commodore BASIC sequence from the prompt or binary metadata without pausing to ask the user:

1.  **Resolve `start_address`:**
    *   If `start_address` is explicitly provided in the prompt, use it.
    *   If `start_address` is not specified, call `r2000_get_binary_info` to read `origin`, `size`, and `system`, and default `start_address` to `origin` when it matches a valid Commodore BASIC start address for the active system:
        *   **C64:** `$0801` (`2049`)
        *   **C128:** `$1C01` (`7169`)
        *   **VIC-20:** `$1001` (`4097` unexpanded), `$0401` (`1025` with +3K RAM), or `$1201` (`4609` with +8K/+16K/+24K RAM)
        *   **PET:** `$0401` (`1025`)
        *   **Plus/4 & C16:** `$1001` (`4097`)
        *   *(Disk drives 1541/1571/1581 have no BASIC).*
2.  **Resolve `end_address` Automatically (Walk Next Line Pointer Chain):**
    *   If `end_address` is not specified, **do not ask the user**.
    *   Read an initial 256-byte window from `start_address` to `min(start_address + 255, origin + size - 1)` using `r2000_read_region` with `"view": "hexdump"`.
    *   Starting at `start_address`, walk the 16-bit little-endian **Next Line Pointer** (`bytes 0..1` of each BASIC line).
    *   If a Next Line Pointer points beyond the currently fetched window (and stays within `[origin, origin + size - 1]`), call `r2000_read_region` for subsequent 256-byte windows until you reach the `$00 $00` end-of-program terminator.
    *   Set `end_address` to the address of the second byte of the `$00 $00` end-of-program marker (inclusive of both `$00 $00` bytes).

## 2. Read Memory Buffer

Call `r2000_read_region` to read the byte sequence (in 256-byte windows if discovering `end_address` dynamically, or across `[start_address, end_address]` once known):

*   `start_address`: Start address (decimal).
*   `end_address`: End address (decimal).
*   `view`: `"hexdump"`

## 3. Decode BASIC Lines

Iterate through the BASIC lines starting at `start_address`. Follow each 16-bit "Next Line Pointer" until you reach the `$00 $00` end-of-program marker.

### Line Structure

1.  **Bytes 0–1 (Next Line Pointer):** 16-bit little-endian address of the next BASIC line (for example, `0B 08` -> `$080B`, or `24 04` -> `$0424`).
2.  **Bytes 2–3 (Line Number):** 16-bit little-endian integer line number (for example, `0A 00` -> `10`).
3.  **Bytes 4–N (Tokens):** Tokenized BASIC statement bytes terminated by `$00`.
4.  **End of Program:** A Next Line Pointer equal to `$00 $00` marks the end of the BASIC program.

### Quoted String PETSCII Guard (`"..."`)

*   Track whether the decoder is currently inside a double-quoted literal string (`$22` = `"`).
*   **Outside quotes:** Bytes in the range `$80` to `$CB` are BASIC keyword tokens (decoded via the table below).
*   **Inside quotes (`"..."`, between opening `$22` and closing `$22` or end-of-line `$00`):** Bytes `>= $80` are **PETSCII control or graphics codes** (for example, `$93` = `[CLR]`, `$91` = `[UP]`, `$9D` = `[LEFT]`), **not** BASIC keyword tokens. Render them as readable PETSCII control tags (such as `{CLR}`) rather than BASIC keywords.

### BASIC V2 Keyword Token Table

When **outside** double quotes, decode bytes in the range `$80` to `$CB` using this table:

| Hex   | Keyword   | Hex   | Keyword  | Hex   | Keyword | Hex   | Keyword  |
| ----- | --------- | ----- | -------- | ----- | ------- | ----- | -------- |
| `$80` | `END`     | `$93` | `LOAD`   | `$A6` | `SPC(`  | `$B9` | `POS`    |
| `$81` | `FOR`     | `$94` | `SAVE`   | `$A7` | `THEN`  | `$BA` | `SQR`    |
| `$82` | `NEXT`    | `$95` | `VERIFY` | `$A8` | `NOT`   | `$BB` | `RND`    |
| `$83` | `DATA`    | `$96` | `DEF`    | `$A9` | `STEP`  | `$BC` | `LOG`    |
| `$84` | `INPUT#`  | `$97` | `POKE`   | `$AA` | `+`     | `$BD` | `EXP`    |
| `$85` | `INPUT`   | `$98` | `PRINT#` | `$AB` | `-`     | `$BE` | `COS`    |
| `$86` | `DIM`     | `$99` | `PRINT`  | `$AC` | `*`     | `$BF` | `SIN`    |
| `$87` | `READ`    | `$9A` | `CONT`   | `$AD` | `/`     | `$C0` | `TAN`    |
| `$88` | `LET`     | `$9B` | `LIST`   | `$AE` | `^`     | `$C1` | `ATN`    |
| `$89` | `GOTO`    | `$9C` | `CLR`    | `$AF` | `AND`   | `$C2` | `PEEK`   |
| `$8A` | `RUN`     | `$9D` | `CMD`    | `$B0` | `OR`    | `$C3` | `LEN`    |
| `$8B` | `IF`      | `$9E` | `SYS`    | `$B1` | `>`     | `$C4` | `STR$`   |
| `$8C` | `RESTORE` | `$9F` | `OPEN`   | `$B2` | `=`     | `$C5` | `VAL`    |
| `$8D` | `GOSUB`   | `$A0` | `CLOSE`  | `$B3` | `<`     | `$C6` | `ASC`    |
| `$8E` | `RETURN`  | `$A1` | `GET`    | `$B4` | `SGN`   | `$C7` | `CHR$`   |
| `$8F` | `REM`     | `$A2` | `NEW`    | `$B5` | `INT`   | `$C8` | `LEFT$`  |
| `$90` | `STOP`    | `$A3` | `TAB(`   | `$B6` | `ABS`   | `$C9` | `RIGHT$` |
| `$91` | `ON`      | `$A4` | `TO`     | `$B7` | `USR`   | `$CA` | `MID$`   |
| `$92` | `WAIT`    | `$A5` | `FN`     | `$B8` | `FRE`   | `$CB` | `GO`     |

Bytes between `$20` and `$7F` are literal PETSCII characters (strings, variable names, and numbers).

## 4. Apply Modifications & Disassemble Unmarked `SYS` Targets

1.  **Format BASIC Lines and Attach Side Comments:**
    For each decoded line:
    *   Call `r2000_set_data_type` on Bytes 0–1 with `"data_type": "address"`.
    *   Call `r2000_set_data_type` on Bytes 2–3 with `"data_type": "word"`.
    *   Call `r2000_set_data_type` from Byte 4 through the `$00` line terminator with `"data_type": "byte"`.
    *   Format the decoded BASIC statement string (for example, `10 SYS 2061`).
    *   Call `r2000_set_comment` with `"type": "side"` at Byte 0 to attach the decoded BASIC statement.
    *   Repeat for each line until the Next Line Pointer equals `$00 $00`. Format the final 2-byte `$00 $00` end-of-program marker as `"word"`.

2.  **Batch Execution Hygiene (`r2000_batch_execute`):**
    *   Use `r2000_batch_execute` **only** when you have `>= 5` independent operations to perform at once (if fewer than 5 operations, call the tools directly).
    *   Because `r2000_batch_execute` returns top-level `Ok` even if individual calls fail, always inspect the per-item `"status"` fields in the returned JSON array and handle or retry any inner `"error"` entries.

3.  **Disassemble Unmarked `SYS` Targets:**
    *   If any decoded BASIC line contains `SYS <addr>` (token `$9E` followed by a resolved target address `<addr>` inside the loaded binary), check `r2000_get_blocks` to see whether `<addr>` currently lies inside an `Undefined` block.
    *   If `<addr>` falls inside an `Undefined` block, call `r2000_disassemble` at `<addr>` so the machine-code entry point is marked and traced as `Code`.

## 5. Save Project (Standalone Execution Only)

*   If you were invoked as a Phase 1A worker subagent by `r2000-analyze-program`, **skip** calling `r2000_save_project` here (the orchestrator saves at the end of Phase 1).
*   If you are running this skill standalone, call `r2000_save_project` to persist the updated data types and comments. If `r2000_save_project` returns `"No active project path"` (because the binary was loaded from a raw `.prg`/`.bin`/`.crt`/`.vsf` without an active `.regen2000proj` file), treat it as a **non-fatal warning** and remind the user to save the project in the TUI.
