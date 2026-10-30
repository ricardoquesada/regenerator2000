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

*   If you are not already running as the `r2000-cbm-principal-swe` agent, read `.agent/agents/r2000_cbm_principal_swe.md` first and follow its technical rules.

## 1. Determine Memory Range

Identify the start and end addresses of the Commodore BASIC sequence from the user's request or cross-references. If the range is not specified, ask the user for the start and end addresses.

## 2. Read Memory Buffer

Call `r2000_read_region` to read the byte sequence:

*   `start_address`: Start address (decimal).
*   `end_address`: End address (decimal).
*   `view`: `"hexdump"`

## 3. Decode BASIC Lines

Iterate through the BASIC lines starting at the initial address. Follow each "Next Line Pointer" until you reach the `$00 $00` end-of-program marker.

### Line Structure

1.  **Bytes 0–1 (Next Line Pointer):** 16-bit little-endian address of the next BASIC line (for example, `24 04` -> `$0424`).
2.  **Bytes 2–3 (Line Number):** 16-bit little-endian integer line number (for example, `0A 00` -> `10`).
3.  **Bytes 4–N (Tokens):** Tokenized BASIC statement bytes terminated by `$00`.
4.  **End of Program:** A Next Line Pointer equal to `$00 $00` marks the end of the BASIC program.

### BASIC V2 Keyword Token Table

Decode bytes in the range `$80` to `$CB` using this table:

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

## 4. Apply Modifications

Call `r2000_batch_execute` to apply the formatting and comments for all decoded BASIC lines in one batch.

For each decoded line:

1.  Call `r2000_set_data_type` on Bytes 0–1 with `"data_type": "address"`.
2.  Call `r2000_set_data_type` on Bytes 2–3 with `"data_type": "word"`.
3.  Call `r2000_set_data_type` from Byte 4 through the `$00` terminator with `"data_type": "byte"`.
4.  Format the decoded BASIC statement string (for example, `10 SYS 2061`).
5.  Call `r2000_set_comment` with `"type": "side"` at Byte 0 to attach the decoded BASIC statement.

Repeat for each line until the Next Line Pointer equals `$00 $00`. Format the final `$00 $00` end-of-program marker as `"word"`.

## 5. Save Project

Call `r2000_save_project` to save the updated data types and comments.
