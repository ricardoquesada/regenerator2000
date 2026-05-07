---
name: r2000-analyze-program
description: Orchestrates full-program analysis by running block classification, then analyzing all unanalyzed routines and symbols in parallel using subagents.
---

# Analyze Program Workflow

Use this skill when the user asks to "analyze this program", "analyze everything", "full analysis",
or wants a complete end-to-end pass over the loaded binary — classifying blocks, documenting
routines, and naming symbols.

This skill **orchestrates** the following existing skills in order:

1. `r2000-analyze-blocks` — Classify all memory regions (code vs data vs text, etc.).
2. `r2000-analyze-routine` — Analyze and document each subroutine.
3. `r2000-analyze-symbol` — Analyze and name each data symbol.

---

## Phase 0 — Gather Context

Before starting, collect the information needed by all subsequent phases:

1. **Binary info**: Call `r2000_get_binary_info` → get `system`, `filename`, `description`, `may_contain_undocumented_opcodes`.
2. **Existing blocks**: Call `r2000_get_analyzed_blocks` → get the current block classification map.
3. **All symbols**: Call `r2000_get_symbols` → get all labels (user + system), including external labels.
4. **Existing line comments**: Call `r2000_get_comments` with `type = "line"` → used to detect routines that already have documentation header blocks.

Store all of this data — you will reference it in every subsequent phase to skip already-analyzed items.

---

## Phase 1 — Classify Blocks (`r2000-analyze-blocks`)

> **This phase always runs first.** Block classification is a prerequisite for all subsequent analysis.

1. Read the skill file at `.agent/skills/r2000-analyze-blocks/SKILL.md`.
2. Execute the `r2000-analyze-blocks` workflow **directly** (not via a subagent — this is a single long-running pass over the entire binary that you perform yourself).
3. After completion, **refresh the state** for later phases:
   - Call `r2000_get_analyzed_blocks` again.
   - Call `r2000_get_symbols` again (new labels may have been created during block analysis).
   - Call `r2000_get_comments` with `type = "line"` again.

---

## Phase 2 — Analyze Routines (parallel subagents)

**Goal**: For every subroutine that hasn't been analyzed yet, launch a subagent to analyze it.

### 2.1 Identify Unanalyzed Subroutines

A subroutine is considered **already analyzed** if its entry point has **any line comment** (the presence of a line comment indicates it has already been documented).

To build the candidate list:

1. From the refreshed `r2000_get_symbols` data, filter labels that represent subroutine entry points:
   - Labels whose name starts with `s_` (auto-generated subroutine labels), OR
   - Labels in a `Code` block that are the target of at least one `JSR` cross-reference, OR
   - **NMI/IRQ handler labels** — `p_XXXX` labels that are targets of interrupt vector addresses. These are pointers set up by the program to handle hardware interrupts and should be analyzed as routines. Identify them by checking whether the label's address is referenced by any of the following vector locations:
     - **Hardware vectors (all 6502 systems)**:
       - `$FFFA`/`$FFFB` — NMI vector
       - `$FFFE`/`$FFFF` — IRQ/BRK vector
     - **Shadow vectors (Commodore 64 / C128 only)**:
       - `$0314`/`$0315` — IRQ shadow vector
       - `$0318`/`$0319` — NMI shadow vector
     - To detect these: check whether the `p_XXXX` label's address appears as the target of a cross-reference originating from any of the vector addresses listed above, or whether the label's address is stored as a 16-bit word at those vector locations.
     - **RTI heuristic**: Additionally, any `p_XXXX` label in a `Code` block whose routine ends with an `RTI` instruction should be treated as a candidate NMI/IRQ handler, even if no direct vector cross-reference was found. An `RTI` (Return from Interrupt) is a strong indicator that the code is an interrupt service routine.
   - **Entry point label** — a label named exactly `main_init`. This is the program's entry point and is critical for understanding the overall program flow.
2. For each candidate, check if it already has a line comment (from the refreshed `r2000_get_comments` data). If yes → **skip it** (already documented).
3. The remaining list = **unanalyzed routines**.
4. **Ordering**: If `main_init` is in the unanalyzed list, it must be placed **first** — it is the program entry point and should be analyzed before all other routines. This ensures that the entry-point context is available when analyzing subsequent routines.

### 2.2 Launch Parallel Subagents

- Launch up to **10 subagents in parallel**, each analyzing one routine.
- For each subagent, provide this prompt:

  > Read the skill file at `.agent/skills/r2000-analyze-routine/SKILL.md` and follow its workflow.
  >
  > Analyze the routine at address `$XXXX` (decimal: NNNNN).
  >
  > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
  >
  > **Apply all changes automatically** — rename the label, add the header comment block, and add side comments to key instructions. Do NOT ask for user confirmation.
  >
  > When done, report: the new label name, a one-line summary of what the routine does, and any uncertain areas.

- **Wait for all subagents in the current batch to complete** before launching the next batch.
- If there are more than 10 unanalyzed routines, process them in batches of 10.

### 2.3 Post-Phase Refresh

After all routine subagents complete:

- Call `r2000_get_symbols` again (labels were renamed by subagents).
- Call `r2000_save_project` to persist changes so far.

---

## Phase 3 — Analyze Symbols (parallel subagents)

**Goal**: For every data symbol (internal + external) that hasn't been analyzed yet, launch a subagent.

### 3.1 Identify Unanalyzed Symbols

A symbol is considered **already analyzed** if:

- It has a **user-defined** name (i.e., NOT an auto-generated prefix name like `a_XXXX`, `f_XXXX`, `p_XXXX`, `zpa_XX`, `zpf_XX`, `zpp_XX`), OR
- It is a well-known system address (hardware register, KERNAL entry point, OS variable).

To build the candidate list:

1. From the refreshed `r2000_get_symbols` data, collect all labels whose name matches auto-generated patterns:
   - `zpp_XX`, `zpf_XX` and `zpa_XX` — auto-generated pointers, fields and absolute addresses in the zero page.
   - `p_XXXX`, `f_XXXX` and `a_XXXX` — auto-generated pointers, fields and absolute addresses outside the zero page.
2. Do **NOT** include:
   - `s_XXXX` labels — those were handled in Phase 2.
   - `b_XXXX` labels — those are branch labels, not data symbols.
   - `e_XXXX` labels — those are external JMP/JSR targets, not data symbols.
   - `p_XXXX` labels that were already identified as NMI/IRQ handlers in Phase 2 — those are routines, not data symbols.
3. The remaining list = **unanalyzed symbols**.

### 3.2 Launch Parallel Subagents

- Same parallelism strategy as Phase 2: up to **10 subagents in parallel**, batched.
- For each subagent, provide this prompt:

  > Read the skill file at `.agent/skills/r2000-analyze-symbol/SKILL.md` and follow its workflow.
  >
  > Analyze the symbol at address `$XXXX` (decimal: NNNNN). Current label: `{current_label}`.
  >
  > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
  >
  > **Apply all changes automatically** — rename the label and add comments (line and/or side). Do NOT ask for user confirmation.
  >
  > When done, report: the old label, the new label name, the classification (flag, counter, pointer, state variable, etc.), and any uncertain areas.

- **Wait for all subagents in the current batch to complete** before launching the next batch.

### 3.3 Post-Phase Refresh

After all symbol subagents complete:

- Call `r2000_save_project` to persist all changes.

---

## Phase 4 — Save & Report

1. Call `r2000_save_project` one final time to ensure everything is persisted.
2. Create a **summary report** for the user, including:

### Blocks Summary

- Total number of blocks classified, grouped by type (Code, Byte, Word, PETSCII, etc.).
- Notable findings from block analysis (e.g., "Found 3 text strings at $1200", "Jump table at $0A00").

### Routines Summary

- Total number of routines analyzed.
- Table of results:

  | Address | Old Label  | New Label       | Summary                                 |
  | ------- | ---------- | --------------- | --------------------------------------- |
  | `$C000` | `s_C000`   | `init_screen`   | Clears screen RAM and sets border color |
  | `$C050` | `s_C050`   | `read_joystick` | Reads CIA1 port A for joystick 2 input  |
  | ...     | ...        | ...             | ...                                     |

### Symbols Summary

- Total number of symbols analyzed.
- Table of results:

  | Address | Old Label  | New Label       | Classification        |
  | ------- | ---------- | --------------- | --------------------- |
  | `$02`   | `zpp_02`   | `ptr_screen`    | Pointer (ZP indirect) |
  | `$0400` | `a_0400`   | `score_display` | Screencode buffer     |
  | ...     | ...        | ...             | ...                   |

### Uncertain Items

- List any routines or symbols that subagents flagged as uncertain or could not fully determine.
- These are candidates for manual review by the user.

---

## Error Handling

- If a subagent fails or times out, **log the failure** but continue with the remaining subagents. Do not abort the entire analysis.
- After all phases complete, include any failures in the summary report under a "Errors" section.
- If `r2000_save_project` fails, warn the user immediately.

---

## Example Invocation

The user says:

> "Analyze this program"

The agent:

1. Reads this skill file.
2. Gathers context (Phase 0).
3. Classifies blocks (Phase 1) — this may take several minutes for large binaries.
4. Identifies 15 unanalyzed subroutines → launches 10 subagents, waits, launches 5 more (Phase 2).
5. Identifies 8 unanalyzed symbols → launches 8 subagents (Phase 3).
6. Saves and produces the summary report (Phase 4).
