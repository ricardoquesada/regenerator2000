---
name: r2000-analyze-program
description: Orchestrates full-program analysis by running block classification, then analyzing all unanalyzed routines and symbols in parallel using subagents.
---

# Analyze Program Workflow

Use this skill when the user asks to "analyze this program", "analyze everything", "full analysis", or wants a complete pass over the loaded binary to classify blocks, document routines, and name symbols.

Execute the following skills in order:

1.  `r2000-analyze-blocks` — Classify all memory regions (code, data, text, tables).
2.  `r2000-analyze-routine` — Analyze and document each subroutine.
3.  `r2000-analyze-symbol` — Analyze and name each data symbol.

---

## Phase 0 — Gather Context and Check Binary Entropy

Before launching subagents, gather binary metadata and check whether the binary is packed.

1.  **Get Binary Info:** Call `r2000_get_binary_info`.
    *   Read the returned `"entropy"` value.
    *   Read the `r2000_get_binary_info` tool description to find the entropy threshold (for example, `7.5`).
    *   If `entropy` is greater than or equal to the threshold:
        *   Ask the user in chat whether to unpack the binary before analysis.
        *   **If the user accepts:**
            *   Call `r2000_unpack_binary`. Note that `r2000_unpack_binary` clears existing comments, labels, and blocks, and can take 10 seconds or more.
            *   **If `r2000_unpack_binary` returns an error:** Log the failure and continue analyzing the current binary.
            *   **If `r2000_unpack_binary` succeeds:** Restart Phase 0 from the beginning and call `r2000_get_binary_info` on the unpacked binary.
        *   **If the user declines:** Continue analyzing the current binary.

---

## Phase 1 — Classify Blocks (`r2000-analyze-blocks`)

Complete block classification before analyzing routines or symbols.

1.  Read `.agent/agents/r2000_cbm_principal_swe.md` to adopt the `r2000-cbm-principal-swe` persona (unless you are already running as `r2000-cbm-principal-swe`).
2.  Read `.agent/skills/r2000-analyze-blocks/SKILL.md`.
3.  Execute the `r2000-analyze-blocks` workflow directly in the current session. Do not delegate Phase 1 to a subagent.

---

## Phase 2 — Analyze Routines (Parallel Subagents)

Launch a `r2000-cbm-principal-swe` subagent for each unanalyzed subroutine.

### 2.1 Identify Unanalyzed Subroutines

Treat a subroutine as already analyzed if its entry point has a line comment.

Build the candidate list as follows:

1.  Call `r2000_get_symbols` to retrieve all user, system, and external labels.
2.  Call `r2000_get_comments` to retrieve all existing comments.
3.  Select labels that represent subroutine entry points:
    *   Labels that start with `s_` (auto-generated subroutine labels).
    *   Labels in a `Code` block that have at least one `JSR` cross-reference.
    *   Pointer-to-code labels (`p_XXXX` inside a `Code` block). These addresses are referenced as pointers (through immediate low/high loads such as `LDX #<addr` / `LDY #>addr` or address tables) and point to executable code, such as:
        *   Chained raster IRQ handlers loaded into `$0314`/`$0315` (or `$0090`/`$0091` on PET) or `$FFFE`/`$FFFF`.
        *   NMI or IRQ handlers loaded into `$FFFA`–`$FFFF`, `$0314`–`$0319`, or `$0090`–`$0095` (PET).
        *   Jump table targets or indirect callback pointers.
    *   The `start` entry-point label.
4.  Skip any candidate that already has a line comment in `r2000_get_comments`.
5.  Place `start` first in the unanalyzed queue so that entry-point context is established first.

### 2.2 Launch Parallel Subagents

*   **Agent Type:** Invoke the **`r2000-cbm-principal-swe`** subagent (defined in `.agent/agents/r2000_cbm_principal_swe.md`) for each routine.
*   **Explicit Target Address:** Pass an explicit address (such as `$XXXX` or decimal `NNNNN`) to each subagent. Never rely on the active editor cursor address.
*   **Concurrency Limit:** Run a rolling window of at most **7 concurrent subagents** to avoid rate-limit errors (`RESOURCE_EXHAUSTED`).
*   **Subagent Prompt:**

    > Read `.agent/skills/r2000-analyze-routine/SKILL.md` and follow its workflow.
    >
    > Analyze the routine at address `$XXXX` (decimal: NNNNN).
    >
    > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
    >
    > Apply all changes automatically: rename the label, add the header comment block, add side comments to key instructions, and format immediate pointer loads with `r2000_set_immediate_format`. Do not ask for user confirmation.
    >
    > Report the new label name, a one-line summary of the routine, and any uncertain areas.

*   **Rolling Window Execution:**
    1.  Launch up to 7 subagents to fill the active slots.
    2.  When any subagent finishes, immediately launch the next routine from the queue into the free slot. Do not wait for the entire batch to finish.
    3.  Continue until all queued routines complete.
    4.  If a subagent fails with a quota error (`RESOURCE_EXHAUSTED` / Code 429), log the failure and process that routine sequentially after a short delay.

### 2.3 Save Progress

After all routine subagents complete, call `r2000_save_project`.

---

## Phase 3 — Analyze Symbols (Parallel Subagents)

Launch a `r2000-cbm-principal-swe` subagent for each unanalyzed data symbol and external jump symbol.

### 3.1 Identify Unanalyzed Symbols

Treat a symbol as already analyzed if it has a user-defined name (not an auto-generated prefix `a_XXXX`, `f_XXXX`, `p_XXXX`, `zpa_XX`, `zpf_XX`, `zpp_XX`, `e_XXXX`) or if it is a known system register, KERNAL entry point, or OS variable.

Build the candidate list as follows:

1.  Call `r2000_get_symbols` to fetch the updated symbol list.
2.  Collect labels that match auto-generated patterns:
    *   `zpp_XX`, `zpf_XX`, `zpa_XX` (zero-page pointers, fields, and absolute addresses).
    *   `p_XXXX`, `f_XXXX`, `a_XXXX`, `e_XXXX` (non-zero-page pointers, fields, absolute addresses, and external ROM jump targets).
3.  Exclude the following labels:
    *   `s_XXXX` subroutine labels (analyzed in Phase 2).
    *   `b_XXXX` branch labels.
    *   `p_XXXX` labels inside a `Code` block (analyzed in Phase 2).

### 3.2 Launch Parallel Subagents

*   **Agent Type:** Invoke the **`r2000-cbm-principal-swe`** subagent (defined in `.agent/agents/r2000_cbm_principal_swe.md`) for each symbol.
*   **Explicit Target Address:** Pass an explicit address (`$XXXX` or decimal `NNNNN`) to each subagent. Never use the editor cursor position.
*   **Concurrency Limit:** Run at most **7 concurrent subagents**.
*   **Complete Queue Processing:** Process the entire unanalyzed symbol queue using the rolling window. Do not truncate the queue, skip custom symbols, or stop early.
*   **Subagent Prompt:**

    > Read `.agent/skills/r2000-analyze-symbol/SKILL.md` and follow its workflow.
    >
    > Analyze the symbol at address `$XXXX` (decimal: NNNNN). Current label: `{current_label}`.
    >
    > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
    >
    > Apply all changes automatically: rename the label, add line or side comments, and format immediate pointer loads with `r2000_set_immediate_format`. Do not ask for user confirmation.
    >
    > Report the old label, the new label name, the symbol classification (flag, counter, pointer, state variable), and any uncertain areas.

*   **Rolling Window Execution:**
    1.  Launch up to 7 subagents to fill the active slots.
    2.  When any subagent finishes, immediately launch the next symbol from the queue.
    3.  Continue until all queued symbols complete.
    4.  If a subagent fails with a quota error (`RESOURCE_EXHAUSTED` / Code 429), log the failure and process that symbol sequentially after a short delay.

### 3.3 Save Progress

After all symbol subagents complete, call `r2000_save_project`.

---

## Phase 4 — Save and Report

1.  Call `r2000_save_project` to persist all changes.
2.  Present a summary report with the following sections:

### Blocks Summary

*   Total blocks classified, grouped by type (Code, Byte, Word, PETSCII, Screencode, Address).
*   Key findings from block classification (such as text tables or jump tables).

### Routines Summary

*   Total routines analyzed and a summary table:

    | Address | Old Label | New Label       | Summary                                 |
    | ------- | --------- | --------------- | --------------------------------------- |
    | `$C000` | `s_C000`  | `init_screen`   | Clears screen RAM and sets border color |
    | `$C050` | `s_C050`  | `read_joystick` | Reads CIA1 port A for joystick 2 input  |

### Symbols Summary

*   Total symbols analyzed and a summary table:

    | Address | Old Label | New Label       | Classification        |
    | ------- | --------- | --------------- | --------------------- |
    | `$02`   | `zpp_02`  | `zp_ptr_screen` | Pointer (ZP indirect) |
    | `$0400` | `a_0400`  | `score_display` | Screencode buffer     |

### Uncertain, Skipped, or Unanalyzed Items

*   List every symbol and routine that remains unanalyzed, skipped, or uncertain.
*   Never report "0 unanalyzed items" if any auto-generated labels (`f_XXXX`, `a_XXXX`, `s_XXXX`) remain unanalyzed.

### Errors, Resource Quotas, and Fallbacks

*   Document all subagent errors, timeouts, or rate-limit events (`RESOURCE_EXHAUSTED` / Code 429).
*   For each event, list the target address, label name, subagent role, error message, and fallback resolution.
*   If `r2000_save_project` fails, notify the user immediately.
