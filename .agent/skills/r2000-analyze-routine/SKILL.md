---
name: r2000-analyze-routine
description: Analyzes a disassembly subroutine to determine its function by examining code, cross-references, and memory usage, leveraging target system expertise.
---

# Analyze Routine Workflow

Use this skill when the user asks to "analyze this routine" or "what does this function do?"

## 1. Determine System & Context

- Use `r2000_get_binary_info` to get the **system**, **filename**, **description**, and **`may_contain_undocumented_opcodes`** hint.
- **CRITICAL**: The `system` field tells you the target computer (e.g., Commodore 64, VIC-20). You **MUST** become an expert in that specific target computer's memory map, hardware registers, and KERNAL routines.
- **CONTEXT**: The `filename` and `description` tell you the specific software being analyzed. Use this to identify standard libraries (e.g., "Hubbard music driver", "Exomizer decompressor") and to understand the likely purpose of routines based on the game's genre (e.g., "check_collision" in a shooter).
- **UNDOCUMENTED OPCODES**: If `may_contain_undocumented_opcodes` is `true`, expect illegal/undocumented MOS 6502 opcodes (e.g., `LAX`, `SAX`, `SLO`, `DCP`, `ISC`) within routines. These are valid instructions — do not treat them as disassembly errors.

## 2. Identify the Bounds

- If the user provides an address, start there.
- If no address is given, call `r2000_get_disassembly_cursor` first — the routine starts at the cursor address.
- Look for the start (entry point or label) and end (`RTS`, `JMP`, or `RTI`) of the routine.
- **Note**: Some routines end with a `JMP` to a shared epilogue — that still marks the end of _this_ routine's body. Others fall through into the next routine with no explicit return; use cross-references and logic flow to determine the boundary.

## 3. Read the Code

- Use `r2000_read_region` (with `"view": "disasm"`) to get the instructions.
- Analyze the flow:
  - Does it loop?
  - Does it call other known routines or OS entry points?
  - Does it access hardware registers?
- Look for common routine patterns (generic 6502):
  - `SEI` / `CLI` bracketing → IRQ setup/teardown.
  - Loop with `LDA`/`STA` and `DEX`/`DEY`/`BNE` → Memory copy or fill.
  - Bit-shifting, `ADC`/`SBC` chains → Math or decompressor.
  - Reads a memory-mapped I/O address then branches → Hardware polling.
- Use your knowledge of the **target system** (from `r2000_get_binary_info`) to recognize system-specific patterns:
  - Calls to OS/KERNAL entry points → System service calls.
  - Reads/writes to hardware register addresses → I/O, video, sound, or input handling.
  - Writes to interrupt vector locations → IRQ/NMI setup.
  - Writes to sound chip registers → Music/SFX driver tick.
  - Reads input port registers → Joystick, keyboard, or controller polling.

## 4. Check Context

- Use `r2000_get_cross_references` on the entry point to see _who_ calls it.
- This often provides a decisive hint:
  - Called from an initialization block → likely a setup routine.
  - Called from a main loop → likely a per-frame update.
  - Called from an IRQ → must be fast; likely a music tick or raster update.
  - No callers found → may be a dispatch target reached via a pointer/jump table.

## 5. Analyze Data Usage

- For each memory address accessed by the routine (e.g., `LDA $C000`, `STA $02`):
  - Check whether it falls in the **target system's hardware register range**. Use your knowledge of the target system's memory map (hardware registers, OS variables, ROM entry points) based on the `system` value from `r2000_get_binary_info`.
  - Otherwise, call `r2000_get_cross_references` on that address to understand:
    - Is it written only once (init)? → likely a constant or config variable.
    - Is it written _and_ read by multiple routines? → shared state / global variable.
    - Is it in Zero Page (`$00–$FF`) and used with `($addr),Y`? → indirect pointer.
  - **ENUM USAGE DETECTION**: Check if any accessed addresses or immediate values represent logical sets of states, flags, or values.
    - Verify if an existing Project, Global, or System enum (e.g., `vic_registers`, `color_codes`) matches these values. If so, call `r2000_apply_enum_usage` to format the operands at the accessing instruction's address.
    - If no matching enum exists but the values represent a clean logical set (e.g., state machine constants, joystick direction bits), define a new project enum using `r2000_create_project_enum` (with a detailed `description` summarizing its purpose) and then apply it using `r2000_apply_enum_usage` to all relevant instructions.
  - **IMMEDIATE POINTER DETECTION**: Check if the routine loads immediate values representing the low and high bytes of a 16-bit address (e.g., to setup an interrupt vector, store a pointer in Zero Page, or pass an address argument).
    - Typically, this looks like `LDA #<target` (low byte) followed by `STA ptr`, and `LDA #>target` (high byte) followed by `STA ptr+1`.
    - If you find immediate loads corresponding to the low/high byte of a 16-bit target address (which could be a subroutine entry point, a data block, screen RAM, or another label):
      - Call `r2000_set_immediate_format` on the low-byte instruction address with `"format": "low_byte"` and `"target_address": <decimal_target>`.
      - Call `r2000_set_immediate_format` on the high-byte instruction address with `"format": "high_byte"` and `"target_address": <decimal_target>`.

## 6. Synthesize

Combine findings into a summary:

- **Purpose**: What does it do? (e.g., "Clears screen memory").
- **Inputs**: Registers or memory locations used as arguments.
- **Outputs**: Registers or memory locations modified.
- **Side Effects**: Hardware changes, screen updates, etc.

> See **Step 7** for the exact comment block format to use when documenting your findings.

## 7. Document

Add a multi-line comment block on top of the routine, rename the label to a descriptive one, and add **side-comments** to key instructions to explain the logic flow.

The multi-line comment block must be placed **above the first instruction** of the routine using `r2000_set_comment` with `"type": "line"`. It should follow this exact format — the separator line must be used as both the **first** and **last** line of the comment:

```
=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
<Summary of what the routine does>

Inputs:  <registers or memory locations used as arguments, or "None">
Outputs: <registers or memory locations modified, or "None">
Side Effects: <hardware changes, screen updates, etc., or "None">
=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-
```

To document the routine, use:

- `r2000_set_label_name` — to give the routine a descriptive name.
- `r2000_set_comment` with `"type": "line"` — to add the multi-line comment block above the entry point.
- `r2000_set_comment` with `"type": "side"` — to annotate key instructions within the routine body with short inline notes (e.g., explaining what a register holds, why a branch is taken, or what a memory address represents). **Crucial for making the code readable for others.**
- `r2000_create_project_enum` — to define new project enums if needed.
- `r2000_apply_enum_usage` — to apply enums to relevant instruction operands in the routine.
- `r2000_set_immediate_format` — to format immediate low/high byte loads (`low_byte`/`high_byte`) of a target 16-bit address to make pointers and vector setups highly readable.

---

## Common Pitfalls

1. **Fall-through into next routine**: If there is no `RTS`/`JMP`/`RTI` at the apparent end, the routine may intentionally fall through. Check if the next label is also a valid entry point called independently.
2. **Tail calls**: A `JMP sub_routine` at the end is a tail call — the routine _ends_ there. Do not include the target routine's body in this routine's analysis.
3. **Shared epilogue**: Multiple routines may converge to a single `RTS`. The epilogue belongs to none of them specifically; note this in the comment.
4. **Indirect dispatch targets**: A routine with no callers may still be active — it could be referenced by a pointer in a jump table. Check nearby data blocks for address tables.
5. **Undocumented opcodes**: Some programs use illegal/undocumented opcodes (`LAX`, `SAX`, `SLO`, etc.). Check the `may_contain_undocumented_opcodes` hint from `r2000_get_binary_info`. If `true`, these are expected — don't stop disassembly at them. Even if `false`, remain aware of their existence.
6. **IRQ re-entrancy confusion**: If an IRQ handler calls `JSR` routines, those routines may share Zero Page with the main program — context is IRQ-relative.

---

## Reporting Results

After completing the analysis, report to the user:

- **Purpose**: One-sentence description of what the routine does.
- **Inputs / Outputs / Side Effects**: As determined in Step 6.
- **Evidence**: Key instructions or cross-references that led to the conclusion.
- **Actions taken**: What was renamed, what line-comments were added, and which key instructions received **side-comments** for clarity.
- **Uncertain areas**: Any instructions or addresses whose purpose is still unclear.
