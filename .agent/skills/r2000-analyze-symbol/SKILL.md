---
name: r2000-analyze-symbol
description: Analyzes a specific memory address or label to determine its purpose (variable, flag, pointer, hardware register) by examining its cross-references and usage patterns using the r2000-cbm-principal-swe agent persona.
---

# Analyze Symbol Workflow

Use this skill when the user asks to "analyze this label", "what is this variable?", or "trace this address".

## Agent Persona (`r2000-cbm-principal-swe`)

This skill uses the **`r2000-cbm-principal-swe`** agent persona for MOS 6502 instructions (including undocumented opcodes and their read/write side effects), Commodore hardware registers, KERNAL/BASIC ROM maps, and data structures.

*   If you are not already running as the `r2000-cbm-principal-swe` agent, read `.agent/agents/r2000_cbm_principal_swe.md` first and follow its technical rules.

## 1. Determine Context and System

*   **Identify Target Address:** Use the address or label provided in the prompt. If none is provided, call `r2000_get_disassembly_cursor` or `r2000_get_address_details`.
*   **Read System Info:** If binary metadata is not in your prompt, call `r2000_get_binary_info` to read `system`, `filename`, `description`, and `may_contain_undocumented_opcodes`.

## 2. Gather Cross-References

*   Call `r2000_get_cross_references` on the target address and inspect every instruction that accesses it:
    *   **Writes:** `STA`, `STX`, `STY`, `SAX`.
    *   **Reads:** `LDA`, `LDX`, `LDY`, `LAX`, `BIT`, `CMP`, `CPX`, `CPY`, `ADC`, `SBC`.
    *   **Read-Modify-Write:** `INC`, `DEC`, `ASL`, `LSR`, `ROL`, `ROR`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`.
*   If `r2000_get_cross_references` returns zero results:
    *   Check whether the address is `$addr + 1` of a 16-bit zero-page pointer used with `($addr),Y` or `($addr,X)`.
    *   Check whether the address is a hardware register, OS variable, or KERNAL routine in the active `system`.
    *   Otherwise, note that the address is unreferenced.

## 3. Classify the Symbol

### Hardware Register

*   Compare the address against the hardware register map of the active `system`.
*   Rename matching registers using the standard chip and register name (such as `VIC_BORDER_COLOR` or `CIA1_ICR`).

### Screen RAM, Color RAM, or Global Buffer Offset

*   If the address falls inside a known video, screen, or color RAM buffer, do not skip it.
*   Rename it systematically by base buffer and offset (for example, `SCREEN_RAM_ROW05_COL10`).

### External ROM or KERNAL Routine

*   If the label starts with `e_` or points to ROM or vector space (such as `$E000–$FFFF` or `$0314–$0319`), rename it to its standard OS/KERNAL symbol name (such as `KERNAL_CHROUT` for `$FFD2`, `KERNAL_CLRSCR` for `$E544`, or `KERNAL_IRQ_ENTRY` for `$EA31`).

### 16-Bit Pointer

*   Check whether the address is in Zero Page (`$00–$FF`) and used with `($xx),Y` or `($xx,X)`, or whether it holds a 16-bit vector.
*   Rename low and high bytes consistently (such as `zp_ptr_screen_lo` / `zp_ptr_screen_hi` or `zp_ptr_screen`).
*   **Format Immediate Pointer Loads:** If code initializes the pointer with immediate low/high byte loads (`LDA #<target` and `LDA #>target`):
    *   Call `r2000_set_immediate_format` on the low-byte instruction with `"format": "low_byte"` and `"target_address": <decimal_target>`.
    *   Call `r2000_set_immediate_format` on the high-byte instruction with `"format": "high_byte"` and `"target_address": <decimal_target>`.

### Self-Modifying Code (SMC) Operand

*   If the symbol lies at `instruction_addr + 1` or `instruction_addr + 2` inside a `Code` block, it is a self-modified operand byte.
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

1.  **Rename the Symbol:** Call `r2000_set_label_name` using these naming conventions:

    | Symbol Kind         | Convention        | Example                             |
    | ------------------- | ----------------- | ----------------------------------- |
    | Zero-page variable  | `zp_` prefix      | `zp_player_lives`, `zp_delay_timer` |
    | Zero-page pointer   | `zp_ptr_` prefix  | `zp_ptr_screen`, `zp_ptr_dest`      |
    | RAM variable        | `snake_case`      | `score_hi`, `current_level`         |
    | Pointer / vector    | `ptr_` / `vec_`   | `ptr_level_data`, `vec_irq_next`    |
    | Hardware register   | `UPPER_SNAKE`     | `VIC_SPR0_X`, `SID_V1_FREQ_LO`      |
    | Constant / buffer   | `UPPER_SNAKE`     | `SCREEN_RAM`, `COLOR_RAM`           |
    | Routine entry point | `snake_case`      | `init_screen`, `draw_sprite`        |
    | External ROM call   | `KERNAL_` / `OS_` | `KERNAL_CHROUT`, `KERNAL_CLRCHN`    |

    > **Zero-Page Rule:** Prefix every symbol at address `<= $FF` with `zp_`.

2.  **Add Comments:**
    *   Call `r2000_set_comment` with `"type": "line"` at the symbol's address (if it is inside the loaded binary) to document its purpose and valid values.
    *   Call `r2000_set_comment` with `"type": "side"` at key read/write instructions to explain how the value is used.

---

## Reporting Results

Report the following to the user or parent agent:

*   **Address**, **Old Label**, and **New Label Name**.
*   **Classification** (flag, counter, pointer, SMC operand, hardware register, state variable).
*   **Evidence** (cross-references and instruction patterns).
*   **Uncertain Areas** (if any).
