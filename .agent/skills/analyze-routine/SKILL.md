---
name: analyze-routine
description: Analyzes a disassembly subroutine to determine its function by examining code, cross-references, and memory usage.
---

# Analyze Routine Workflow

Use this skill when the user asks to "analyze this routine" or "what does this function do?"

## 1. Identify the Bounds

- If the user provides an address, start there.
- Use `r2000_get_disassembly_cursor` if no address is given.
- Look for the start (entry point or label) and end (`RTS`, `JMP`, or `RTI`) of the routine.

## 2. Read the Code

- Use `r2000_read_disasm_region` to get the instructions.
- Analyze the flow:
  - Does it loop?
  - Does it call other known routines (e.g., KERNAL routines like `$FFD2` (CHROUT))?
  - Does it access hardware registers (e.g., `$D000-$D02E` for VIC-II)?

## 3. Check Context

- Use `r2000_get_cross_references` on the entry point to see _who_ calls it.
- This often provides a hint (e.g., called from an initialization block vs. a main loop).

## 4. Analyze Data Usage

- Identify memory addresses accessed (e.g., `LDA $C000`).
- If uncertain about an address's purpose:
  - Use `r2000_search_memory` for values if applicable.
  - Check if it's a known hardware register or a Zero Page variable.

## 5. Synthesize

Combine findings into a summary:

- **Purpose**: What does it do? (e.g., "Clears screen memory").
- **Inputs**: Registers or memory locations used as arguments.
- **Outputs**: Registers or memory locations modified.
- **Side Effects**: Hardware changes, screen updates, etc.

## 6. Optional: Document

If the analysis is solid, offer to add a multi-line comment block on top of the routine and/or rename the label.

The multi-line comment block must be placed **above the first instruction** of the routine using `r2000_set_line_comment`. It should follow this exact format — the separator line must be used as both the **first** and **last** line of the comment:

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
- `r2000_set_line_comment` — to add the multi-line comment block above the entry point.
- `r2000_set_side_comment` — to annotate key instructions within the routine body with short inline notes (e.g., explaining what a register holds, why a branch is taken, or what a memory address represents).
