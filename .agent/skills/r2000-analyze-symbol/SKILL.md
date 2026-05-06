---
name: r2000-analyze-symbol
description: Analyzes a specific memory address or label to determine its purpose (variable, flag, pointer, hardware register) by examining its cross-references and usage patterns.
---

# Analyze Symbol Workflow

Use this skill when the user asks to "analyze this label", "what is this variable?", or "trace this address". This skill focuses on **data flow**—understanding what a memory location _represents_ rather than just what code executes.

## 1. Determine Context & System

- **Get the Target**: If the user provides a label or address, use that. If not, use `r2000_get_disassembly_cursor` or `r2000_get_address_details` to identify the address under the cursor.
- **Get the System**: Use `r2000_get_binary_info`.
  - **CRITICAL**: Knowing the system is essential for identifying hardware registers and OS/KERNAL addresses. You **MUST** use your knowledge of the specific target computer's memory map, hardware registers, and OS entry points.
  - **CONTEXT**: Use the `filename` response and `description` (if provided) to identify the specific game or program. This allows you to infer domain-specific labels (e.g., "lap_counter" for a racing game, "lives" for a platformer) and look up known memory maps for popular titles.
  - **UNDOCUMENTED OPCODES**: If `may_contain_undocumented_opcodes` is `true`, the binary may use illegal/undocumented MOS 6502 opcodes. When tracing cross-references, be aware that instructions like `LAX`, `SAX`, `DCP`, etc. are valid and their read/write side effects must be considered in the data flow analysis.

## 2. Gather Usage Data

- Use `r2000_get_cross_references` on the target address.
  - This returns a list of _everywhere_ the address is used (read, write, or modify).
  - **Note**: Pay attention to the instruction type at each reference.
    - **Writes**: `STA`, `STX`, `STY`, `INC`, `DEC`, `ASL`, `LSR`, `ROR`, `ROL`.
    - **Reads**: `LDA`, `LDX`, `LDY`, `BIT`, `CMP`, `CPX`, `CPY`, `ADC`, `SBC`.
    - **Modify**: `INC`, `DEC`, `ASL`, `LSR`, `ROR`, `ROL` (read-modify-write).
- **If `r2000_get_cross_references` returns zero results**:
  - The symbol may be referenced **indirectly** via a pointer — check if the address is in Zero Page (`$00–$FF`) and whether nearby code uses `($addr),Y` or `($addr,X)` patterns.
  - The symbol may be a well-known OS/KERNAL address that the disassembler doesn't generate an explicit cross-reference for — use your knowledge of the target system's memory map based on the `system` value from `r2000_get_binary_info`.
  - It may be **dead code / an unused variable**. Note this in the report.

## 3. Analyze Patterns (Heuristics)

### Is it a Hardware Register?

- Check the address against the **target system's memory map**. Use your knowledge of the target system's hardware registers based on the `system` value from `r2000_get_binary_info`.
- If it matches a known hardware register, rename it to the standard hardware name (e.g., the chip name + register, or the system's conventional name for that register).

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
    Use the following naming conventions consistently:

    | Symbol Kind         | Convention       | Example                              |
    | ------------------- | ---------------- | ------------------------------------ |
    | Zero Page variable  | `zp_` prefix     | `zp_player_lives`, `zp_delay_timer`  |
    | Zero Page pointer   | `zp_ptr_` prefix | `zp_ptr_screen`, `zp_ptr_dest`       |
    | RAM variable        | `snake_case`     | `score_hi`, `current_level`          |
    | Pointer / vector    | `ptr_` prefix    | `ptr_screen`, `vec_irq`              |
    | Hardware register   | `UPPER_SNAKE`    | `VIC_SPR0_X`, `SID_FreqLo1`          |
    | Constant / address  | `UPPER_SNAKE`    | `SCREEN_RAM`, `CHR_ROM_BASE`         |
    | Routine entry point | `snake_case`     | `init_screen`, `draw_sprite`         |

    > **Zero Page rule**: If the symbol's address is ≤ `$FF`, it **must** be prefixed with `zp_`.
    > This applies to all categories above — a Zero Page pointer becomes `zp_ptr_`, a Zero Page flag
    > becomes `zp_is_active`, and so on. Hardware registers and OS/KERNAL constants that live in Zero Page
    > (e.g., C64 Zero Page OS variables) should also use `zp_` to make their addressing mode explicit.

2.  **Document**:
    - Use `r2000_set_comment` with `"type": "line"` at the definition (if it's a variable in memory) to explain its range, purpose, or bitfield layout.
    - Use `r2000_set_comment` with `"type": "side"` at key usages to clarify _why_ it's being read or written (e.g., "Reset life counter", "Check for fire button").

---

## Example Output

If you analyze an IRQ vector address and see:

- References: Written during init, read during IRQ handler.
- Context: Target system's IRQ vector shadow location.
- **Action**: Rename to `IRQ_VECTOR_LO`. Add comment: "Hardware IRQ vector shadow".

If you analyze a Zero Page address (≤ `$FF`) and see:

- References: `STA ($20),Y`.
- Context: Zero Page.
- **Action**: Rename to `zp_ptr_dest`. Add comment: "Destination pointer for memory copy".

## Reporting Results

After completing the analysis, report to the user:

- **Address**: The address analyzed and its current label (if any).
- **Classification**: What kind of symbol it is (flag, counter, pointer, hardware register, state variable, etc.).
- **Evidence**: The key cross-references or usage patterns that led to the conclusion.
- **Actions taken**: What was renamed or commented.
- **Uncertain / no refs**: If `r2000_get_cross_references` returned nothing, explain the possibilities (indirect use, KERNAL address, or dead variable).
