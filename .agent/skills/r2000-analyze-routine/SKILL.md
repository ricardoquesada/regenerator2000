---
name: r2000-analyze-routine
description: Analyzes a disassembly subroutine to determine its function by examining code, cross-references, and memory usage using the r2000-cbm-principal-swe agent persona.
---

# Analyze Routine Workflow

Use this skill when the user asks to "analyze this routine" or "what does this function do?".

## Agent Persona (`r2000-cbm-principal-swe`)

This skill uses the **`r2000-cbm-principal-swe`** agent persona for MOS 6502 instructions (including undocumented opcodes), Commodore hardware registers, KERNAL/BASIC ROM maps, and demoscene algorithms.

*   If you are not already running as the `r2000-cbm-principal-swe` agent, read `.agent/agents/r2000_cbm_principal_swe.md` first and follow its technical rules.

## 1. Determine System and Context

*   If binary metadata is not in your prompt, call `r2000_get_binary_info` to read `system`, `filename`, `description`, and `may_contain_undocumented_opcodes`.

## 2. Identify the Routine Bounds

*   Start at the address provided in the prompt. If no address is provided, call `r2000_get_disassembly_cursor` to get the cursor address.
*   Locate the entry point and the terminating instruction (`RTS`, `JMP`, or `RTI`).
*   Note that a routine can end with a tail-call `JMP` to another routine or fall through into the next labeled routine. Check cross-references to confirm the boundary.

## 3. Read the Instructions

*   Call `r2000_read_region` with `"view": "disasm"` to read the instructions.
*   Identify standard 6502 patterns:
    *   `SEI` and `CLI` around vector/register writes -> Interrupt configuration.
    *   `LDA` / `STA` loop with `DEX` / `DEY` / `BNE` -> Memory copy or fill.
    *   Bit shifts (`ASL`, `LSR`, `ROL`, `ROR`) and `ADC` / `SBC` chains -> Arithmetic or decompression.
    *   Reads from an I/O register followed by a conditional branch -> Hardware status polling.
    *   Writes to `label + 1` or `label + 2` -> Self-modifying code (SMC) patching an instruction operand.
*   Identify system-specific operations for the active `system`:
    *   Calls to KERNAL or BASIC ROM addresses -> OS service calls.
    *   Accesses to video, sound, timer/input, or banking registers (VIC-II/SID/CIA/`$01` on C64/C128; MMU/VDC on C128; VIC-I/VIA on VIC-20; PIA/VIA/CRTC on PET; TED/`$FF3E–$FF3F` on Plus/4; VIA1/VIA2 on 1541) -> Hardware control.
    *   Writes to `$0314`/`$0315` (`$0090`/`$0091` on PET) or `$FFFE`/`$FFFF` -> Interrupt handler installation or raster IRQ chaining.

## 4. Check Callers and Cross-References

*   Call `r2000_get_cross_references` on the routine entry point:
    *   Called once from `start` -> Initialization routine.
    *   Called from a main loop -> Per-frame update routine.
    *   Called from an ISR or referenced by `$0314`/`$0315` (`$0090`/`$0091` on PET) or `$FFFE`/`$FFFF` -> Interrupt routine.
    *   Referenced in an address or word table -> Jump-table or `RTS`-dispatch target.

## 5. Analyze Data and Operand Usage

For each memory address accessed by the routine:

*   Check whether the address is a hardware register, OS variable, or ROM entry point in the active `system`.
*   Call `r2000_get_cross_references` on RAM addresses to see whether the address is a constant, global variable, or zero-page pointer (`($addr),Y`).
*   **Apply Enums:** Check whether immediate values or memory accesses represent discrete states or bitflags:
    *   If an existing enum matches, call `r2000_apply_enum_usage` on the instruction address.
    *   If the values form a new state set or bitmask, create a project enum with `r2000_create_project_enum` and apply it with `r2000_apply_enum_usage`.
*   **Format Immediate Pointers:** Check whether the routine loads the low and high bytes of a 16-bit target address (such as `LDA #<target : STA ptr` and `LDA #>target : STA ptr+1`):
    *   Call `r2000_set_immediate_format` on the low-byte instruction with `"format": "low_byte"` and `"target_address": <decimal_target>`.
    *   Call `r2000_set_immediate_format` on the high-byte instruction with `"format": "high_byte"` and `"target_address": <decimal_target>`.

## 6. Summarize Routine Behavior

Determine the following four attributes:

*   **Purpose:** What the routine does in one concise sentence.
*   **Inputs:** Registers or memory addresses read as parameters (or `None`).
*   **Outputs:** Registers or memory addresses written as results (or `None`).
*   **Side Effects:** Hardware register changes, screen updates, or stack changes (or `None`).

## 7. Apply Documentation

1.  Call `r2000_set_label_name` to give the routine a descriptive `snake_case` name.
2.  Call `r2000_set_comment` with `"type": "line"` at the entry point using this exact format:

```
=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
<Summary of what the routine does>

Inputs:  <registers or memory locations used as arguments, or "None">
Outputs: <registers or memory locations modified, or "None">
Side Effects: <hardware changes, screen updates, etc., or "None">
=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
```

3.  Call `r2000_set_comment` with `"type": "side"` on key instructions inside the routine to explain register state, branch conditions, and hardware operations.

---

## Common Pitfalls

1.  **Fall-Through Execution:** A routine without a trailing `RTS`, `JMP`, or `RTI` falls through into the next routine. Document the fall-through in the header comment.
2.  **Tail Calls:** A `JMP other_routine` ends the current routine. Do not merge `other_routine` into the current routine's summary.
3.  **Hardware vs. Software ISR Tails:** An ISR that jumps to the KERNAL interrupt tail (such as `JMP $EA31` or `JMP $EA81` on C64) runs via the software IRQ vector (`$0314`/`$0315`, or `$0090`/`$0091` on PET). An ISR that saves `A`/`X`/`Y` itself and ends with `RTI` runs via the `$FFFE`/`$FFFF` RAM hardware vector (with KERNAL ROM banked out).
4.  **Undocumented Opcodes:** Treat valid undocumented opcodes (`LAX`, `SAX`, `DCP`, `ISC`, `SLO`, `RLA`, `SRE`, `RRA`, `ANC`, `ALR`, `AXS`) as intentional instructions.

---

## Reporting Results

Report the following to the user or parent agent:

*   **New Label Name** and **Purpose** (one-sentence summary).
*   **Inputs, Outputs, and Side Effects**.
*   **Actions Taken** (label rename, header comment, side comments, enums, and immediate formats applied).
*   **Uncertain Areas** (any unresolved addresses or branches).
