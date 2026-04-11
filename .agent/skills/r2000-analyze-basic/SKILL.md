---
name: r2000-analyze-basic
description: Analyzes a sequence of memory containing Commodore BASIC tokens, formats address/word data types, and constructs side comments representing the plain BASIC commands.
---

# Analyze BASIC Tokens Workflow

Use this skill when the user asks to:

- "analyze this BASIC code"
- "decode basic commands from memory"
- "create side comments for BASIC lines"
- "parse basic pointer address and line number"

## 1. Determine Memory Range

Identify the starting and ending addresses of the Commodore BASIC sequence from the user's request or cross-references. If unspecified, prompt the user for clarification.

## 2. Read Memory Buffer

Invoke the `r2000_read_region` tool to read the requested sequence:

- `start_address`: The sequence start address (decimal).
- `end_address`: The sequence end address (decimal).
- `view`: `"hexdump"`

## 3. Process BASIC Lines

Commodore BASIC programs follow a strict structured sequence in memory. Iterate through the sequences byte-by-byte:

### Line Anatomy

1. **Bytes 0–1:** Pointer to the address of the next BASIC line in Little Endian (e.g., `24 04` $\rightarrow$ `$0424`).
2. **Bytes 2–3:** BASIC Line number as a 16-bit integer in Little Endian (e.g., `0A 00` $\rightarrow$ `10`).
3. **Bytes 4–N:** The tokenized BASIC command ending with a `$00` byte.
4. **Termination:** If the pointer bytes are `$00 $00`, the iteration stops.

### Keyword Token Table (V2)

Bytes in step 3 with the high-bit set (`$80` to `$CB`) are keyword tokens. Decode them against this table:

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

_(Note: Bytes between `$20` and `$7F` are literal PETSCII text characters like strings, variables, and numbers)._

## 4. Apply Modifications

Formulate a sequence of commands for `r2000_batch_execute` to apply the modifications for all decoded BASIC lines at once.

For each decoded line:
1. Use `r2000_set_data_type` on Bytes 0–1 as `"address"`.
2. Use `r2000_set_data_type` on Bytes 2–3 as `"word"`.
3. Form a complete BASIC line string (e.g., `10 REM LODE RUNNER`).
4. Use `r2000_set_comment` to set the string as a `"side"` comment at the start of the sequence (Byte 0).

## 5. Save Project

When completed, invoke `r2000_save_project` to persist the comments and types.
