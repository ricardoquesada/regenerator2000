---
name: r2000-analyze-symbol
description: Analyzes a specific memory address or label to determine its purpose (variable, flag, pointer, hardware register) by examining its cross-references and usage patterns using the r2000-cbm-principal-swe agent persona.
---

# Analyze Symbol Workflow

Use this skill when the user asks to "analyze this label", "what is this variable?", or "trace this address".

## Agent Persona (`r2000-cbm-principal-swe`)

This skill uses the **`r2000-cbm-principal-swe`** agent persona for MOS 6502 instructions (including undocumented opcodes and their read/write side effects), Commodore hardware registers, KERNAL/BASIC ROM maps, and data structures.

*   When spawned as the `r2000-cbm-principal-swe` subagent, the persona rules from `.agent/agents/r2000_cbm_principal_swe.md` are already loaded in your system prompt—**do not** read `.agent/agents/r2000_cbm_principal_swe.md` again.
*   Only read `.agent/agents/r2000_cbm_principal_swe.md` if you are executing this skill directly inside the main agent session.

## 1. Determine Context and System (Single Symbol or Clustered Pair)

*   **Identify Target Address(es):** Accept either a **single target address (`$XXXX`)** or a **clustered address pair (`$XXXX` and `$(XXXX+1)`)** provided in the prompt (for example, adjacent zero-page addresses `$FB` and `$FC`, or adjacent RAM variable addresses `$033C` and `$033D`).
    *   If a standalone zero-page pointer label `zpp_XX` is provided without `$(XX + 1)`, also inspect `$(XX + 1)` as its companion high-byte address.
    *   If no address is provided in the prompt, call `r2000_get_disassembly_cursor` or `r2000_get_address_details`.
*   **Read System Info:** If binary metadata is not in your prompt, call `r2000_get_binary_info` to read `system`, `filename`, `description`, and `may_contain_undocumented_opcodes`.

## 2. Gather Cross-References

*   Call `r2000_get_cross_references` on the target address—or on **both** addresses (`$ADDR` and `$(ADDR + 1)`) when given a clustered pair or `zpp_XX` pointer—and inspect every instruction that accesses them:
    *   **Writes:** `STA`, `STX`, `STY`, `SAX`.
    *   **Reads:** `LDA`, `LDX`, `LDY`, `LAX`, `BIT`, `CMP`, `CPX`, `CPY`, `ADC`, `SBC`.
    *   **Read-Modify-Write:** `INC`, `DEC`, `ASL`, `LSR`, `ROL`, `ROR`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`.
*   **Clustered Pair Verification Rule (`$ADDR` and `$(ADDR + 1)`):**
    When analyzing an adjacent pair `($ADDR, $(ADDR + 1))` (or `zpp_XX` and `$(XX + 1)`), compare the cross-references of both addresses to determine whether they are coupled or independent:
    1.  **Coupled 16-Bit Pointer or 16-Bit Word:** They form a 16-bit pair if `$ADDR` is accessed with indirect addressing (`($ADDR),Y`, `($ADDR,X)`, or `JMP ($ADDR)`), or if code loads/stores/adds/increments `$ADDR` (low byte) and `$(ADDR + 1)` (high byte) together as a 16-bit pointer or 16-bit integer.
    2.  **Two Independent 8-Bit Variables:** If cross-references show that `$ADDR` and `$(ADDR + 1)` are used in unrelated contexts as separate 8-bit values, you must still classify, rename, and comment **both** symbols independently in this same turn and return two entries in your summary report.
*   If `r2000_get_cross_references` returns zero results for an address:
    *   Check whether the address is `$addr + 1` of a 16-bit zero-page pointer used with `($addr),Y` or `($addr,X)`.
    *   Check whether the address is a hardware register, OS variable, or KERNAL routine in the active `system`.
    *   Otherwise, note that the address is unreferenced.

## 3. Classify the Symbol(s)

### Hardware Register

*   Compare the address against the hardware register map of the active `system`.
*   Rename matching registers using the standard chip and register name (such as `VIC_BORDER_COLOR` or `CIA1_ICR`).

### Screen RAM, Color RAM, or Global Buffer Offset

*   If the address falls inside a known video, screen, or color RAM buffer, do not skip it.
*   Rename it systematically by base buffer and offset (for example, `SCREEN_RAM_ROW05_COL10`).

### External ROM or KERNAL Routine

*   If the label starts with `e_` or points to ROM or vector space (such as `$E000–$FFFF` or `$0314–$0319`), rename it to its standard OS/KERNAL symbol name (such as `KERNAL_CHROUT` for `$FFD2`, `KERNAL_CLRSCR` for `$E544`, or `KERNAL_IRQ_ENTRY` for `$EA31`).

### 16-Bit Pointer or 16-Bit Word Pair (`$ADDR` and `$(ADDR + 1)`)

*   Check whether the address pair is in Zero Page (`$00–$FF`) and used with `($xx),Y` or `($xx,X)`, or whether it holds a 16-bit vector or 16-bit value (`_lo` / `_hi`) in Zero Page or RAM.
*   Rename both the low byte (`$ADDR`) and high byte (`$(ADDR + 1)`) with matching `_lo` and `_hi` suffixes (such as `zp_ptr_screen_lo` / `zp_ptr_screen_hi` or `score_lo` / `score_hi`).
*   **Format Immediate Pointer Loads:** If code initializes the pointer or word with immediate low/high byte loads (`LDA #<target` and `LDA #>target`):
    *   Call `r2000_set_immediate_format` on the low-byte instruction with `"format": "low_byte"` and `"target_address": <decimal_target>`.
    *   Call `r2000_set_immediate_format` on the high-byte instruction with `"format": "high_byte"` and `"target_address": <decimal_target>`.

### Self-Modifying Code (SMC) Operand

*   If the symbol lies at `instruction_addr + 1` or `instruction_addr + 2` inside a `Code` block, it is a self-modified operand byte (or a paired 16-bit self-modified operand at `instruction_addr + 1` and `instruction_addr + 2`).
*   Name it after the operation it controls (such as `smc_blit_src_lo` / `smc_blit_src_hi`).

### Flag or Bitmask

*   If code sets the address to `0`/`1` or `$00`/`$FF` and tests it with `BEQ`, `BNE`, `BPL`, `BMI`, or `BIT`, classify it as a flag or bitmask (such as `is_paused` or `zp_irq_ready`).
*   If individual bits represent distinct flags, define an enum with `r2000_create_project_enum` and apply it with `r2000_apply_enum_usage`.

### Counter or Index

*   If code increments (`INC`) or decrements (`DEC`) the address in a loop and compares it (`CMP`, `CPX`, `CPY`), classify it as a counter or index (such as `frame_counter` or `zp_sprite_idx`).

### State Variable

*   If the address holds discrete mode values or indexes a jump table, classify it as a state variable (such as `game_state`).
*   Define a project enum with `r2000_create_project_enum` for the state values and apply it with `r2000_apply_enum_usage`.

## 4. Rename and Document

1.  **Rename the Symbol(s):** Call `r2000_set_label_name` on the target address (or on **both** `$ADDR` and `$(ADDR + 1)` when analyzing a pair, whether coupled as `_lo`/`_hi` or classified as two independent 8-bit variables) using these naming conventions:

    | Symbol Kind         | Convention        | Example                                                |
    | ------------------- | ----------------- | ------------------------------------------------------ |
    | Zero-page variable  | `zp_` prefix      | `zp_player_lives`, `zp_delay_timer`                    |
    | Zero-page pointer   | `zp_ptr_` prefix  | `zp_ptr_screen_lo`, `zp_ptr_screen_hi`, `zp_ptr_dest`  |
    | RAM variable        | `snake_case`      | `score_lo`, `score_hi`, `current_level`                |
    | Pointer / vector    | `ptr_` / `vec_`   | `ptr_level_data_lo`, `ptr_level_data_hi`, `vec_irq_lo` |
    | Hardware register   | `UPPER_SNAKE`     | `VIC_SPR0_X`, `SID_V1_FREQ_LO`                         |
    | Constant / buffer   | `UPPER_SNAKE`     | `SCREEN_RAM`, `COLOR_RAM`                              |
    | Routine entry point | `snake_case`      | `init_screen`, `draw_sprite`                           |
    | External ROM call   | `KERNAL_` / `OS_` | `KERNAL_CHROUT`, `KERNAL_CLRCHN`                       |

    > **Zero-Page Rule:** Prefix every symbol at address `<= $FF` with `zp_` (including 4-digit auto-labels at `<= $00FF` such as `a_00FB`, `f_00FB`, or `p_00FB`).

    *   **Duplicate Label Collision & System Label Protection:**
        *   If an address already has a valid `System` label (such as `VIC_BORDER_COLOR`, `CIA1_ICR`, `KERNAL_CHROUT`), do **not** overwrite it, and never attempt to assign an existing `System` label name to a different address.
        *   If `r2000_set_label_name` returns `"Label '<name>' already exists"` (because a concurrent subagent, user label, or system symbol already claimed `<name>`), disambiguate by appending the symbol's hex address or a specific context suffix (for example, `zp_ptr_copy_fc`, `zp_temp_fd`, or `frame_counter_033c`) and retry `r2000_set_label_name`.

2.  **Add Comments:**
    *   Call `r2000_set_comment` with `"type": "line"` at each target symbol's address (if it lies inside the loaded binary) to document its purpose and valid values.
    *   Call `r2000_set_comment` with `"type": "side"` at key read/write instructions to explain how the value or pointer is used.

---

## Reporting Results

Report the following to the user or parent agent (returning **one summary row/entry per symbol**, or **two summary rows/entries** when given a clustered pair `$XXXX` and `$(XXXX+1)`—both when paired as `_lo`/`_hi` and when classified as two independent 8-bit variables):

*   **Address**, **Old Label**, and **New Label Name** (for each analyzed address).
*   **Classification** (16-bit pointer `_lo`/`_hi`, 16-bit word `_lo`/`_hi`, independent 8-bit flag/counter/state variable, SMC operand, hardware register, or external ROM symbol).
*   **Evidence** (cross-references and instruction patterns).
*   **Uncertain Areas** (if any).
