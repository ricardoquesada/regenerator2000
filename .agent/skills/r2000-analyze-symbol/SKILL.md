---
name: r2000-analyze-symbol
description: Analyzes a specific memory address or label to determine its purpose (variable, flag, pointer, hardware register) by examining its cross-references and usage patterns.
---

# Analyze Symbol Workflow

Use this skill when the user asks to "analyze this label", "what is this variable?", or "trace this address". This skill focuses on **data flow**—understanding what a memory location _represents_ rather than just what code executes.

## 1. Determine Context & Platform

- **Get the Target**: If the user provides a label or address, use that. If not, use `r2000_get_disassembly_cursor` or `r2000_get_address_details` to identify the address under the cursor.
- **Get the Platform**: Use `r2000_get_binary_info`.
  - **CRITICAL**: Knowing the platform (e.g., C64, VIC-20) is essential for identifying hardware registers (VIC-II, SID, CIA, VIA).
  - **CONTEXT**: Use the `filename` response to identify the specific game or program. This allows you to infer domain-specific labels (e.g., "lap_counter" for a racing game, "lives" for a platformer) and look up known memory maps for popular titles.

## 2. Gather Usage Data

- Use `r2000_get_cross_references` on the target address.
  - This returns a list of _everywhere_ the address is used (read, write, or modify).
  - **Note**: Pay attention to the instruction type at each reference.
    - **Writes**: `STA`, `STX`, `STY`, `INC`, `DEC`, `ASL`, `LSR`, `ROR`, `ROL`.
    - **Reads**: `LDA`, `LDX`, `LDY`, `BIT`, `CMP`, `CPX`, `CPY`, `ADC`, `SBC`.
    - **Modify**: `INC`, `DEC`, `ASL`, `LSR`, `ROR`, `ROL` (read-modify-write).

## 3. Analyze Patterns (Heuristics)

### Is it a Hardware Register?

- Check the address against the platform's memory map.
  - **C64 Examples**:
    - `$D000–$D02E`: VIC-II (Sprites, Screen control, IRQ).
    - `$D400–$D7FF`: SID (Sound voices, filters, volume).
    - `$DC00–$DCFF`: CIA 1 (Joystick, Keyboard, IRQ).
    - `$DD00–$DDFF`: CIA 2 (Serial bus, NMI, VIC bank).
- If it matches, rename it to its standard hardware name (e.g., `VIC_SPR0_X`, `SID_FreqLo1`, `CIA1_PRA`).

### Is it a Pointer (16-bit)?

- Is it used in Zero Page (address < $100)?
- Is it used for **Indirect Indexed** addressing `($xx),Y`?
  - Example: `LDA ($FB),Y`
- Is it used for **Indexed Indirect** addressing `($xx,X)`?
- If so, it's a **Pointer**. Rename to something like `ptr_screen`, `ptr_data`, or `vec_irq`.
  - Suggest generating a comment explaining what it points _to_.

### Is it a Flag (Boolean)?

- Is it only ever set to `0` or `1` (or `$00`/`$FF`)?
- Is it checked with `BIT`, `LDA`/`BEQ`/`BNE`?
- If so, it's likely a **Flag**.
  - Rename to `is_active`, `has_collided`, `enable_music`, etc.

### Is it a Counter/Index?

- Is it incremented (`INC`) or decremented (`DEC`) inside a loop?
- Is it compared (`CPX`, `CPY`, `CMP`) against a limit?
- If so, it's a **Counter** or **Index**.
  - Rename to `loop_idx`, `sprite_count`, `delay_timer`.

### Is it a State Variable?

- Does it take multiple distinct values (e.g., 0=Init, 1=Title, 2=Game, 3=Over)?
- Is it used in a jump table dispatch (e.g., `ASL` / `TAX` / `JMP (table,X)`)?
- If so, it's a **State Machine Variable**.
  - Rename to `game_state`, `current_mode`.

## 4. Synthesize & Action

1.  **Rename**: Use `r2000_set_label_name` to give it a meaningful, descriptive name based on your analysis.
    - Style: `snake_case` is generally preferred for variables (e.g., `player_lives`), `CapsExpr` for constants or hardware.
2.  **Document**:
    - Use `r2000_set_line_comment` at the definition (if it's a variable in memory) to explain its range, purpose, or bitfield layout.
    - Use `r2000_set_side_comment` at key usages to clarify _why_ it's being read or written (e.g., "Reset life counter", "Check for fire button").

## Example Output

If you analyze `$0314` on C64 and see:

- References: Written during init, read during IRQ handler.
- Context: `$0314` is the hardware IRQ vector shadow.
- **Action**: Rename to `IRQ_VECTOR_LO` (or standard `CINV`). Add comment: "Hardware IRQ vector shadow".

If you analyze `$20` and see:

- References: `STA ($20),Y`.
- Context: Zero Page.
- **Action**: Rename to `ptr_dest`. Add comment: "Destination pointer for memory copy".
