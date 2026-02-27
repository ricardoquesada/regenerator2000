---
name: r2000-analyze-routine
description: Analyzes a disassembly subroutine to determine its function by examining code, cross-references, and memory usage, leveraging target platform expertise.
---

# Analyze Routine Workflow

Use this skill when the user asks to "analyze this routine" or "what does this function do?"

## 1. Determine Platform & Context

- Use `r2000_get_binary_info` to get the **platform**, **filename**, and **description**.
- **CRITICAL**: The `platform` field tells you the target computer (e.g., Commodore 64, VIC-20). You **MUST** become an expert in that specific target computer's memory map, hardware registers, and KERNAL routines.
- **CONTEXT**: The `filename` and `description` tell you the specific software being analyzed. Use this to identify standard libraries (e.g., "Hubbard music driver", "Exomizer decompressor") and to understand the likely purpose of routines based on the game's genre (e.g., "check_collision" in a shooter).

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
- **If platform = Commodore 64**, also look for these C64-specific patterns (see **C64 Reference** section below):
  - `JSR $FFD2` (CHROUT) in a loop → Text output to screen.
  - Reads `$DC00`/`$DC01` (CIA 1) → Joystick / keyboard input.
  - Writes to `$D000–$D027` (VIC-II sprites) → Sprite update.
  - Reads `$D019` / writes `$D01A` → IRQ raster handler tick.
  - Writes to `$D400–$D418` (SID) → Sound / music driver tick.
  - `SEI` + stores to `$FFFE/$FFFF` or `$0314/$0315` → IRQ vector setup.

## 4. Check Context

- Use `r2000_get_cross_references` on the entry point to see _who_ calls it.
- This often provides a decisive hint:
  - Called from an initialization block → likely a setup routine.
  - Called from a main loop → likely a per-frame update.
  - Called from an IRQ → must be fast; likely a music tick or raster update.
  - No callers found → may be a dispatch target reached via a pointer/jump table.

## 5. Analyze Data Usage

- For each memory address accessed by the routine (e.g., `LDA $C000`, `STA $02`):
  - Check whether it falls in the **platform's hardware register range**. If **platform = Commodore 64**, see the **C64 Reference** section below. For other platforms, use your knowledge of their memory maps.
  - Otherwise, call `r2000_get_cross_references` on that address to understand:
    - Is it written only once (init)? → likely a constant or config variable.
    - Is it written _and_ read by multiple routines? → shared state / global variable.
    - Is it in Zero Page (`$00–$FF`) and used with `($addr),Y`? → indirect pointer.

## 6. Synthesize

Combine findings into a summary:

- **Purpose**: What does it do? (e.g., "Clears screen memory").
- **Inputs**: Registers or memory locations used as arguments.
- **Outputs**: Registers or memory locations modified.
- **Side Effects**: Hardware changes, screen updates, etc.

> See **Step 7** for the exact comment block format to use when documenting your findings.

## 7. Optional: Document

If the analysis is solid, offer to add a multi-line comment block on top of the routine and/or rename the label.

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
- `r2000_set_comment` with `"type": "side"` — to annotate key instructions within the routine body with short inline notes (e.g., explaining what a register holds, why a branch is taken, or what a memory address represents).

---

## C64 Reference (only when platform = Commodore 64)

> **Use this section only if `r2000_get_binary_info` returns platform = Commodore 64.**
> For other platforms (VIC-20, Apple II, NES, etc.), rely on your own knowledge of that platform's memory map and OS entry points.

### C64 Memory Map

| Address Range | Description                              |
| ------------- | ---------------------------------------- |
| `$0000–$00FF` | Zero Page (fast variables, pointers)     |
| `$0100–$01FF` | CPU Stack                                |
| `$0200–$03FF` | OS work area, BASIC input buffer         |
| `$0314–$0315` | Hardware IRQ vector shadow (CINV)        |
| `$0317–$0318` | BRK / NMI vector shadow                  |
| `$0400–$07FF` | Default Screen RAM (1000 bytes + spare)  |
| `$0800–$9FFF` | BASIC program area / free RAM            |
| `$A000–$BFFF` | BASIC ROM (or RAM underneath)            |
| `$C000–$CFFF` | Free RAM                                 |
| `$D000–$D3FF` | VIC-II registers (when I/O visible)      |
| `$D400–$D7FF` | SID registers (when I/O visible)         |
| `$D800–$DBFF` | Color RAM                                |
| `$DC00–$DCFF` | CIA 1 (keyboard, joystick, IRQ timer)    |
| `$DD00–$DDFF` | CIA 2 (serial bus, NMI, VIC bank select) |
| `$E000–$FFFF` | KERNAL ROM (or RAM underneath)           |
| `$FFFE–$FFFF` | Hardware IRQ vector                      |
| `$FFFA–$FFFB` | NMI vector                               |
| `$FFFC–$FFFD` | RESET vector                             |

### Well-Known KERNAL Entry Points

| Address | Name   | Purpose                       |
| ------- | ------ | ----------------------------- |
| `$FFD2` | CHROUT | Output a character            |
| `$FFE4` | GETIN  | Get a character from keyboard |
| `$FFE1` | STOP   | Check STOP key                |
| `$FFCF` | CHRIN  | Input a character             |
| `$FFD5` | LOAD   | Load from device              |
| `$FFD8` | SAVE   | Save to device                |
| `$FF81` | CINT   | Initialize screen editor      |
| `$FF84` | IOINIT | Initialize I/O                |
| `$FFBA` | SETLFS | Set logical file              |
| `$FFBD` | SETNAM | Set filename                  |

---

## Common Pitfalls

1. **Fall-through into next routine**: If there is no `RTS`/`JMP`/`RTI` at the apparent end, the routine may intentionally fall through. Check if the next label is also a valid entry point called independently.
2. **Tail calls**: A `JMP sub_routine` at the end is a tail call — the routine _ends_ there. Do not include the target routine's body in this routine's analysis.
3. **Shared epilogue**: Multiple routines may converge to a single `RTS`. The epilogue belongs to none of them specifically; note this in the comment.
4. **Indirect dispatch targets**: A routine with no callers may still be active — it could be referenced by a pointer in a jump table. Check nearby data blocks for address tables.
5. **Undocumented opcodes**: Some C64 programs use illegal opcodes (`LAX`, `SAX`, `SLO`, etc.). These are valid code — don't stop disassembly at them.
6. **IRQ re-entrancy confusion**: If an IRQ handler calls `JSR` routines, those routines may share Zero Page with the main program — context is IRQ-relative.

---

## Reporting Results

After completing the analysis, report to the user:

- **Purpose**: One-sentence description of what the routine does.
- **Inputs / Outputs / Side Effects**: As determined in Step 6.
- **Evidence**: Key instructions or cross-references that led to the conclusion.
- **Actions taken**: What was renamed or commented, if Step 7 was applied.
- **Uncertain areas**: Any instructions or addresses whose purpose is still unclear.

Always ask the user's confirmation before applying `r2000_set_label_name` or adding comments, unless they explicitly said "go ahead and document it."
