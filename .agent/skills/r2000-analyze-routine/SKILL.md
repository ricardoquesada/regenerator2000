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
- Use `r2000_get_disassembly_cursor` if no address is given.
- Look for the start (entry point or label) and end (`RTS`, `JMP`, or `RTI`) of the routine.

## 3. Read the Code

- Use `r2000_read_region` (with `"view": "disasm"`) to get the instructions.
- Analyze the flow:
  - Does it loop?
  - Does it call other known routines (e.g., KERNAL routines like `$FFD2` (CHROUT))?
  - Does it access hardware registers (e.g., `$D000-$D02E` for VIC-II)?

## 4. Check Context

- Use `r2000_get_cross_references` on the entry point to see _who_ calls it.
- This often provides a hint (e.g., called from an initialization block vs. a main loop).

## 5. Analyze Data Usage

- Identify memory addresses accessed (e.g., `LDA $C000`).
- If uncertain about an address's purpose:
  - Use `r2000_search_memory` for values if applicable.
  - Check if it's a known hardware register or a Zero Page variable.

## 6. Synthesize

Combine findings into a summary:

- **Purpose**: What does it do? (e.g., "Clears screen memory").
- **Inputs**: Registers or memory locations used as arguments.
- **Outputs**: Registers or memory locations modified.
- **Side Effects**: Hardware changes, screen updates, etc.

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
