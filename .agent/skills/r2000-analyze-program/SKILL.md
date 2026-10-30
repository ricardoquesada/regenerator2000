---
name: r2000-analyze-program
description: Orchestrates full-program analysis as a 2-tier MapReduce coordinator by delegating BASIC decoding, block classification, topology-ordered routine analysis, and clustered symbol analysis to r2000-cbm-principal-swe worker subagents.
---

# Analyze Program Workflow

Use this skill when the user asks to "analyze this program", "analyze everything", "full analysis", or wants a complete pass over the loaded binary to decode BASIC headers/programs, classify blocks, document routines, and name symbols.

This orchestrator coordinates all four worker skills in strict phase order across `r2000-cbm-principal-swe` subagents:

1.  `r2000-analyze-basic` — Decode tokenized Commodore BASIC stubs or programs at the system BASIC start address and disassemble unmarked `SYS` targets.
2.  `r2000-analyze-blocks` — Classify all `Undefined` memory regions (`Code`, `Byte`, `Word`, `Address`, `PETSCII`, `Screencode`, split tables, `ExternalFile`).
3.  `r2000-analyze-routine` — Analyze and document each subroutine in two-wave call-graph topology order (Wave 2A entry/interrupt anchors first, then Wave 2B leaf-first rolling window).
4.  `r2000-analyze-symbol` — Analyze and name each data variable, 16-bit pointer pair, self-modifying code (SMC) operand, and external symbol.

---

## Orchestrator Role Rule (2-Tier MapReduce Coordinator)

The Main Agent executing `r2000-analyze-program` acts **strictly as a lightweight MapReduce Coordinator**:

*   **Allowed Main-Agent Tools:** Call **only** metadata and persistence MCP tools (`r2000_get_binary_info`, `r2000_get_blocks`, `r2000_get_symbols`, `r2000_get_comments`, `r2000_get_cross_references`, `r2000_save_project`, `r2000_unpack_binary`, `r2000_batch_execute`) and `invoke_subagent` (spawning `r2000-cbm-principal-swe` worker subagents).
*   **No Persona Hijack in Main Session:** The Main Agent must **never** read or load `.agent/agents/r2000_cbm_principal_swe.md` into the main session context.
*   **No Inline Hex Dump or Disassembly Loops:** The Main Agent must **never** call `r2000_read_region` to run raw hex dump or disassembly inspection loops inline. All byte/instruction inspection is delegated to `r2000-cbm-principal-swe` worker subagents.
*   **Subagent Persona Guard Directive:** Every subagent prompt across Phase 1A, Phase 1B, Phase 2 (Wave 2A and Wave 2B), Phase 3, and Phase 4 straggler fix-ups **must** explicitly include:
    > You are already running as the `r2000-cbm-principal-swe` subagent; do NOT read `.agent/agents/r2000_cbm_principal_swe.md`.
*   **MCP Tool Contract Guards:**
    1.  **Range Filter Contract (`r2000_get_symbols` & `r2000_get_comments`):** Never pass `start_address` without `end_address` (or vice versa)—doing so fails with JSON-RPC error `-32602` (`"Both 'start_address' and 'end_address' must be provided together."`). Always omit both parameters (to fetch across the entire binary) or pass both `start_address` and `end_address` together.
    2.  **Non-Fatal Project Save Contract (`r2000_save_project`):** When a raw `.prg`, `.crt`, `.vsf`, or `.bin` file is loaded without an active `.regen2000proj` file, `r2000_save_project` returns JSON-RPC error `-32603` (`"No active project path. Project must be loaded from or saved to a .regen2000proj file before it can be saved."`). Across Phases 1, 2, 3, and 4, treat `"No active project path"` as a **non-fatal warning**: do **not** abort the workflow, record the warning once, and remind the user in the Phase 4 report to save a `.regen2000proj` file in the TUI.
    3.  **Batch Execution Contract (`r2000_batch_execute`):** Use `r2000_batch_execute` only when executing `>= 5` independent calls at once, and always inspect the per-item `"status"` fields in the returned JSON array for any inner `"error"` entries.

---

## Phase 0 — Gather Context and Check Binary Entropy (Inline in Main Agent)

Before launching worker subagents, gather binary metadata and check whether the binary is packed or compressed:

1.  **Get Binary Info & Initial Blocks:** Call `r2000_get_binary_info` and `r2000_get_blocks`.
    *   Record `origin`, `size`, `system`, `filename`, `description`, `may_contain_undocumented_opcodes`, and `entropy`.
    *   Read the `r2000_get_binary_info` tool description to find the packing entropy threshold (for example, `7.5`).
2.  **Handle High Entropy (Packed Binary):**
    *   If `entropy` is greater than or equal to the threshold (`entropy >= threshold`):
        *   Ask the user directly in the main chat stream whether to unpack the binary via `r2000_unpack_binary` before analysis.
        *   **If the user accepts:**
            *   Call `r2000_unpack_binary`. Note that `r2000_unpack_binary` clears existing comments, labels, and blocks, and can take 10 seconds or more.
            *   **If `r2000_unpack_binary` succeeds:** Restart Phase 0 from Step 1 and re-call `r2000_get_binary_info` and `r2000_get_blocks` on the unpacked binary.
            *   **If `r2000_unpack_binary` returns an error:** Log the failure and continue analyzing the current binary.
        *   **If the user declines:** Continue analyzing the current binary.

---

## Phase 1 — BASIC Decoding & Memory Block Classification (Worker Subagents)

Complete BASIC decoding (Phase 1A) and memory block classification (Phase 1B) via `r2000-cbm-principal-swe` subagents before analyzing routines or symbols.

### Phase 1A — System-Agnostic BASIC Start Detection & Analysis (`r2000-analyze-basic`)

1.  **Check System BASIC Start Address:**
    Compare `origin` and `system` from `r2000_get_binary_info` against the valid Commodore BASIC start addresses (never hardcode `$0801` alone):
    *   **C64:** `$0801` (`2049`)
    *   **C128:** `$1C01` (`7169`)
    *   **VIC-20:** `$1001` (`4097` unexpanded), `$0401` (`1025` with +3K RAM), or `$1201` (`4609` with +8K/+16K/+24K RAM)
    *   **PET:** `$0401` (`1025`)
    *   **Plus/4 & C16:** `$1001` (`4097`)
    *   **1541 / 1571 / 1581 (Disk Drives):** None (disk drive ROM/RAM binaries have no BASIC start region; skip Phase 1A).
2.  **Detect Unanalyzed BASIC Region at `origin`:**
    *   Check the block covering `origin` from `r2000_get_blocks` (and side comments from `r2000_get_comments` if `origin` is already formatted as data).
    *   An unanalyzed BASIC region exists if `origin` matches a BASIC start address for `system` (or lies below `start` at a valid BASIC start address) and the bytes at `origin` are in an `Undefined` block (or lack decoded BASIC line side comments at `origin`).
3.  **Spawn Phase 1A Subagent & Wait (Sequential Barrier):**
    *   If an unanalyzed BASIC region exists at `origin`, spawn **1** `r2000-cbm-principal-swe` subagent with the prompt below.
    *   **CRITICAL SEQUENTIAL BARRIER:** You **must wait for the Phase 1A subagent to complete** before calling `r2000_get_blocks` for Phase 1B. If Phase 1B calls `r2000_get_blocks` before Phase 1A finishes, Phase 1B will see the BASIC region as `Undefined` and overwrite Phase 1A's `address`/`word`/`byte` formatting.
    *   **Phase 1A Subagent Prompt** (explicitly instructing *"You are already running as the `r2000-cbm-principal-swe` subagent; do NOT read `.agent/agents/r2000_cbm_principal_swe.md`."*):

        > You are already running as the r2000-cbm-principal-swe subagent; do NOT read .agent/agents/r2000_cbm_principal_swe.md.
        >
        > Read `.agent/skills/r2000-analyze-basic/SKILL.md` and follow its workflow autonomously without asking the user for addresses.
        >
        > Analyze the Commodore BASIC region starting at `start_address` = `$XXXX` (decimal: NNNNN). Walk the 16-bit Next Line Pointer chain across 256-byte windows until the `$00 $00` end-of-program terminator to determine `end_address` automatically.
        >
        > Binary info: system = {system}, origin = {origin}, size = {size}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
        >
        > Format Next Line Pointers (`address`), line numbers (`word`), tokenized statements (`byte`), and the `$00 $00` terminator (`word`), attach decoded BASIC side comments (respecting the quoted-string PETSCII guard for bytes `>= $80` inside `"..."`), and call `r2000_disassemble` on any `SYS <addr>` target that falls inside an `Undefined` block. Skip `r2000_save_project` (the orchestrator saves at the end of Phase 1).
        >
        > Report the decoded BASIC lines, the `[start_address, end_address]` span, and any disassembled `SYS` entry points.

### Phase 1B — Memory Block Classification & 4 KB Partitioning (`r2000-analyze-blocks`)

1.  **Query Remaining `Undefined` Blocks After Phase 1A:**
    *   After Phase 1A finishes (or immediately if Phase 1A was not needed), call `r2000_get_blocks` with `"block_type": "Undefined"`.
    *   Compute the total `Undefined` byte count across all returned `Undefined` blocks:
        `total_undefined_bytes = sum(block.end_address - block.start_address + 1)`.
2.  **Partition by 4 KB (`4096` Bytes) Threshold:**
    *   **If `total_undefined_bytes == 0`:** All memory regions are already classified; proceed directly to Phase 1C.
    *   **If `total_undefined_bytes <= 4096` (4 KB):** Spawn **1** `r2000-cbm-principal-swe` subagent covering `[first_undefined_start, last_undefined_end]`.
    *   **If `total_undefined_bytes > 4096` (4 KB):** Partition the `Undefined` blocks into **up to 4 non-overlapping `[range_start, range_end]` address ranges** (distributing disjoint `Undefined` blocks or splitting contiguous large `Undefined` regions into non-overlapping slices of roughly equal byte count) and spawn up to **4 parallel `r2000-cbm-principal-swe` subagents**, passing each subagent its strict `[range_start, range_end]` bounds.
    *   **Phase 1B Subagent Prompt** (explicitly instructing *"You are already running as the `r2000-cbm-principal-swe` subagent; do NOT read `.agent/agents/r2000_cbm_principal_swe.md`."*):

        > You are already running as the r2000-cbm-principal-swe subagent; do NOT read .agent/agents/r2000_cbm_principal_swe.md.
        >
        > Read `.agent/skills/r2000-analyze-blocks/SKILL.md` and follow its workflow.
        >
        > Strict Assigned Address Range: `start_address` = `$XXXX` (decimal: NNNNN) to `end_address` = `$YYYY` (decimal: MMMMM). Only inspect, disassemble, and classify `Undefined` regions within `[$XXXX, $YYYY]`; never modify blocks outside this assigned range.
        >
        > Binary info: system = {system}, origin = {origin}, size = {size}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
        >
        > Apply all block conversions, splitters, table/string labels, and comments automatically without asking for user confirmation. When using `r2000_batch_execute` (for `>= 5` operations), inspect per-item `"status"` fields for any inner `"error"` entries.
        >
        > Report all classified blocks within `[$XXXX, $YYYY]` grouped by type, newly discovered code entry points or tables, and any regions left Undefined.

3.  **Wait for Phase 1B Completion:** Wait for all Phase 1B subagents to complete before starting Phase 2.

### Phase 1C — Save Phase 1 Progress

*   Call `r2000_save_project`.
*   If `r2000_save_project` returns JSON-RPC error `-32603` (`"No active project path"`), record it as a **non-fatal warning** and continue to Phase 2.

---

## Phase 2 — Topology-Ordered Routine Analysis (Two-Wave Subagents)

Analyze all unanalyzed subroutines using `r2000-cbm-principal-swe` subagents in a **Two-Wave Call-Graph Topology** so that entry-point setup, memory banking, and leaf helper names are established before high-level callers are documented.

Treat a routine as already analyzed if its entry-point address already has a line comment (`"type": "line"`) in `r2000_get_comments`.

### Wave 2A — Entry Point & Interrupt Vector Anchors (Sequential Barrier)

1.  **Identify Wave 2A Anchor Routines Using Only Metadata Tools:**
    Call `r2000_get_symbols`, `r2000_get_blocks` (`"block_type": "Code"`), and `r2000_get_comments` (`"type": "line"` — omitting both `start_address` and `end_address` or passing both together). Select unanalyzed routine entry points in `Code` blocks that match any of the following anchor criteria:
    *   The `start` entry-point routine.
    *   Any unanalyzed entry point in a `Code` block that is a **valid instruction start address** (not a self-modified mid-instruction operand byte at `instr + 1` or `instr + 2`) and is either:
        *   Labeled `p_XXXX` (`"type": "Pointer"`) in a `Code` block or named as an interrupt/vector handler (`irq_*`, `nmi_*`, `isr_*`) during Phase 1B, **OR**
        *   Referenced by code that writes to software or hardware interrupt vectors (`$0314`–`$0319` on C64/C128/VIC-20/Plus/4, `$0090`–`$0095` on PET, or `$FFFA`–`$FFFF` NMI/RESET/IRQ vectors, verified via `r2000_get_cross_references`).
2.  **Launch Wave 2A Subagents & Wait:**
    *   Spawn `r2000-cbm-principal-swe` subagents (up to 7 concurrent) for the Wave 2A anchor routines using the Phase 2 Subagent Prompt below.
    *   **CRITICAL WAVE 2A BARRIER:** **Wait for all Wave 2A subagents to complete before starting Wave 2B** so that global initialization context, IRQ vector chains, and top-level labels are persisted first.

### Wave 2B — Metadata-Only Leaf-First Call-Graph Ordering (Rolling Window, Max 7)

1.  **Re-Fetch Updated Metadata After Wave 2A:**
    *   Call `r2000_get_symbols` (to pick up any newly discovered routines or labels from Wave 2A).
    *   Call `r2000_get_blocks` with `"block_type": "Code"`.
    *   Call `r2000_get_comments` with `"type": "line"` (never passing `start_address` without `end_address`).
2.  **Filter Remaining Unanalyzed Routine Entry Points:**
    *   **Include** unanalyzed routine entry points inside `Code` blocks that lack a line comment in `r2000_get_comments`:
        *   Auto-generated subroutine labels (`s_XXXX`).
        *   Labels in a `Code` block that have at least one `JSR` cross-reference.
        *   Valid instruction-start pointer-to-code labels (`p_XXXX`) in `Code` blocks (jump table targets, state-machine callbacks, or chained interrupt handlers not already analyzed in Wave 2A).
        *   Unanalyzed indirect-jump (`JMP ($XXXX)`) or inter-routine tail-call (`JMP $XXXX`) entry-point labels (`j_XXXX`) that head a separate routine in a `Code` block.
    *   **Exclude:**
        *   Any routine whose entry address already has a line comment in `r2000_get_comments`.
        *   `b_XXXX` local branch labels.
        *   `r_XXXX` (`LabelType::Return`) single-instruction `RTS`/`RTI` return target labels (promoted by `analyzer.rs` when the target byte is `$60` or `$40`).
        *   Internal `j_XXXX` (`LabelType::Jump`) branch/loop labels inside a routine body.
3.  **Compute Leaf-First Call-Graph Order Using Only Metadata Tools (`r2000_get_cross_references`):**
    *   Sort all routine entry points in `Code` blocks by ascending address `R_1 < R_2 < ... < R_k`. Define each routine `R_i`'s address span as `[addr(R_i), addr(R_{i+1}) - 1]` (bounded by the end of its `Code` block).
    *   For all remaining unanalyzed routines `S`, query `r2000_get_cross_references` (using `r2000_batch_execute` when querying `>= 5` routines and checking per-item `"status"` fields).
    *   For each caller address `caller_pc` returned in `cross_references(S)`, map `caller_pc` to the routine `R_i` whose address span `[addr(R_i), addr(R_{i+1}) - 1]` contains `caller_pc` (ignoring self-calls `R_i == S`). This establishes a directed call edge `R_i -> S` (`R_i` calls unanalyzed routine `S`).
    *   Compute each unanalyzed routine `R_i`'s outgoing unanalyzed call count: `out_degree(R_i) = count(distinct unanalyzed S where R_i -> S)`.
    *   Sort the Wave 2B queue in **ascending order of `out_degree(R_i)`** (tie-breaking by ascending address):
        *   `out_degree == 0` (**pure leaf routines** that make 0 calls to unanalyzed subroutines) are placed **first**.
        *   Intermediate helper routines (`out_degree == 1, 2, ...`) follow next.
        *   High-level dispatchers with the largest `out_degree` are placed **last**, so their callees already have descriptive names and header comments when the dispatcher is analyzed.
4.  **Launch Wave 2B Subagents (Rolling Window of Max 7 Concurrent):**
    *   **Agent Type:** Invoke **`r2000-cbm-principal-swe`** for each routine.
    *   **Explicit Target Address:** Always pass an explicit address (`$XXXX` and decimal `NNNNN`); never rely on the editor cursor position.
    *   **Concurrency Limit:** Maintain a rolling window of at most **7 concurrent `r2000-cbm-principal-swe` subagents**:
        1.  Launch up to 7 subagents from the leaf-first queue.
        2.  As soon as any subagent finishes, immediately launch the next routine in the sorted queue into the freed slot without waiting for the entire batch to drain.
        3.  Continue until all queued routines complete.
        4.  If a subagent fails with a quota error (`RESOURCE_EXHAUSTED` / Code 429), log the event and retry that routine sequentially after a short delay.
    *   **Phase 2 (Wave 2A & Wave 2B) Subagent Prompt** (explicitly instructing *"You are already running as the `r2000-cbm-principal-swe` subagent; do NOT read `.agent/agents/r2000_cbm_principal_swe.md`."*):

        > You are already running as the r2000-cbm-principal-swe subagent; do NOT read .agent/agents/r2000_cbm_principal_swe.md.
        >
        > Read `.agent/skills/r2000-analyze-routine/SKILL.md` and follow its workflow.
        >
        > Analyze the routine at address `$XXXX` (decimal: NNNNN). Current label: `{current_label}`.
        >
        > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
        >
        > Apply all changes automatically without asking for user confirmation:
        > 1. Rename ONLY this routine's own entry label (and its own internal branch labels if needed). NEVER rename other subroutines (`s_XXXX`) or global/zero-page data symbols (`a_XXXX`, `f_XXXX`, `p_XXXX`, `e_XXXX`, `zpa_XX`, `zpf_XX`, `zpp_XX`, `a_00XX`, `f_00XX`, `p_00XX`).
        > 2. Never overwrite valid `System` labels; if `r2000_set_label_name` returns `"Label '<name>' already exists"`, disambiguate by appending the hex address or a context suffix (e.g., `delay_loop_c140`) and retry.
        > 3. Attach the structured header line comment block at `$XXXX`, add side comments to key instructions, and format immediate pointer loads with `r2000_set_immediate_format`.
        >
        > Report the old label, the new label name, a one-line summary of the routine, and any uncertain areas.

### Phase 2C — Save Phase 2 Progress

*   After all Wave 2B routine subagents complete, call `r2000_save_project`.
*   If `r2000_save_project` returns `"No active project path"` (`-32603`), treat it as a **non-fatal warning** and continue to Phase 3.

---

## Phase 3 — Clustered Symbol Analysis (Rolling Window, Max 7)

Launch `r2000-cbm-principal-swe` subagents to analyze all remaining unanalyzed data symbols, zero-page pointers, self-modifying code (SMC) operands, and external ROM/KERNAL symbols, clustering adjacent 16-bit candidate pairs to eliminate concurrent edit collisions.

### 3.1 Identify Unanalyzed Symbols & Apply the SMC Rule

1.  **Re-Fetch Symbols & Comments:**
    *   Call `r2000_get_symbols` (and `r2000_get_comments` with `"type": "line"` plus `r2000_get_blocks` if needed, never passing `start_address` without `end_address`).
2.  **Collect Auto-Generated Symbol Candidates:**
    Select all unanalyzed symbols that still have auto-generated label names (`"kind": "Auto"`):
    *   **2-Digit Zero-Page Auto-Labels (`<= $00FF`):** `zpp_XX`, `zpf_XX`, `zpa_XX` (zero-page pointers, fields, and absolute addresses; note that `analyzer.rs` locks in `first_type` by lowest referencing instruction address, so a 16-bit ZP pointer is frequently labeled `zpa_XX` + `zpa_YY` or `zpp_XX` + `zpa_YY`).
    *   **4-Digit Zero-Page Auto-Labels (`<= $00FF`):** `a_00XX`, `f_00XX`, `p_00XX` (created by `LabelType::format_label` when a zero-page address is accessed via a 16-bit absolute instruction such as `LDA $00FB` or `STA $00FB,X`).
    *   **RAM / I/O / External Auto-Labels (`>= $0100`):** `p_XXXX`, `f_XXXX`, `a_XXXX`, `e_XXXX` (pointers, fields, absolute addresses, and external jump targets).
3.  **Exclusions:**
    *   **Exclude** `s_XXXX` subroutine labels (handled in Phase 2).
    *   **Exclude** `b_XXXX` local branch labels.
    *   **Exclude** `r_XXXX` (`LabelType::Return`) single-instruction `RTS`/`RTI` return labels.
    *   **Exclude** internal `j_XXXX` (`LabelType::Jump`) branch/jump labels.
    *   **Exclude** `System` labels and already-renamed `User` labels.
4.  **Self-Modifying Code (SMC) / Mid-Instruction `p_XXXX` Rule Inside `Code` Blocks:**
    *   Do **not** blindly skip all `p_XXXX` labels inside `Code` blocks!
    *   **Only skip** a `p_XXXX` label inside a `Code` block if it was **already analyzed as a routine entry point in Phase 2** (i.e., its address already has a routine header line comment in `r2000_get_comments`).
    *   **Retain** every unanalyzed `p_XXXX`, `a_XXXX`, and `f_XXXX` inside a `Code` block that lacks a routine line comment—these represent **Self-Modifying Code (SMC) operand addresses** (`instr + 1` or `instr + 2`) or inline data symbols and **must** be analyzed in Phase 3.

### 3.2 Pair Clustering (`($ADDR, $(ADDR + 1))`)

Sort all candidate unanalyzed symbols from Step 3.1 by ascending numeric `address` and group adjacent addresses into tasks as follows:

1.  **Consecutive Zero-Page Pair (`<= $00FF`):** Whenever two unanalyzed zero-page symbols (`zpp_XX`, `zpf_XX`, `zpa_XX`, `a_00XX`, `f_00XX`, `p_00XX`) sit at consecutive addresses `$ADDR` and `$(ADDR + 1)` (for example, `$FB` and `$FC`), **group `($ADDR, $(ADDR + 1))` into a single subagent task** and advance past both symbols.
2.  **Standalone `zpp_XX` / `p_00XX` Companion Rule:** If a zero-page pointer label `zpp_XX` (or `p_00XX`) has no auto-label at `$(XX + 1)` (because the high byte was only accessed indirectly via `($XX),Y`), queue `$XX` as a single subagent task and explicitly instruct the subagent to also inspect and name `$(XX + 1)`.
3.  **Consecutive RAM / SMC Pair (`>= $0100`):** Whenever two unanalyzed internal/RAM/SMC symbols (`f_XXXX`, `a_XXXX`, `p_XXXX`) sit at consecutive addresses `$ADDR` and `$(ADDR + 1)` (such as a 16-bit variable `score_lo`/`score_hi` or a 16-bit self-modified operand at `instr + 1` and `instr + 2`), **group `($ADDR, $(ADDR + 1))` into a single subagent task** and advance past both symbols.
4.  **Single Symbol Task:** Any remaining symbol `$ADDR` without an immediately adjacent unanalyzed partner forms a single-address subagent task.

### 3.3 Launch Parallel Symbol Subagents (Rolling Window, Max 7)

*   **Agent Type:** Invoke **`r2000-cbm-principal-swe`** for each symbol task (single address or clustered pair).
*   **Explicit Target Address(es):** Pass explicit hex and decimal addresses (`$XXXX` / `NNNNN`, or `$XXXX` and `$(XXXX+1)`) to each subagent. Never rely on the editor cursor position.
*   **Concurrency Limit & Full Queue Processing:** Maintain a rolling window of at most **7 concurrent `r2000-cbm-principal-swe` subagents**. Process the entire symbol task queue without truncating or stopping early. If a subagent hits `RESOURCE_EXHAUSTED` (Code 429), log the event and retry sequentially after a short delay.
*   **Phase 3 Subagent Prompt** (explicitly instructing *"You are already running as the `r2000-cbm-principal-swe` subagent; do NOT read `.agent/agents/r2000_cbm_principal_swe.md`."*):

    > You are already running as the r2000-cbm-principal-swe subagent; do NOT read .agent/agents/r2000_cbm_principal_swe.md.
    >
    > Read `.agent/skills/r2000-analyze-symbol/SKILL.md` and follow its workflow.
    >
    > Target Symbol(s): {`$XXXX` (decimal: NNNNN, current label: `{label}`) OR clustered pair `$XXXX` (decimal: NNNNN, current label: `{label_1}`) and `$(XXXX+1)` (decimal: NNNNN+1, current label: `{label_2}`), or `zpp_XX` + companion high byte `$(XX+1)`}.
    >
    > Binary info: system = {system}, filename = {filename}, description = {description}, may_contain_undocumented_opcodes = {hint}.
    >
    > Apply all changes automatically without asking for user confirmation:
    > 1. If given a clustered pair `($ADDR, $(ADDR + 1))` (or `zpp_XX` and `$(XX + 1)`), call `r2000_get_cross_references` on BOTH addresses:
    >    - If they form a 16-bit pointer or 16-bit word, rename them with matching `_lo` and `_hi` suffixes (prefixing zero-page addresses `<= $00FF` with `zp_` / `zp_ptr_`) and format immediate pointer loads with `r2000_set_immediate_format` (`low_byte` / `high_byte`).
    >    - If cross-references show they are two independent 8-bit variables at adjacent addresses, classify, rename, and comment BOTH symbols independently in this same turn and return two rows in your summary report.
    > 2. Never overwrite valid `System` labels; if `r2000_set_label_name` returns `"Label '<name>' already exists"`, disambiguate with a hex address or context suffix (e.g., `zp_ptr_copy_fc`) and retry.
    >
    > Report the address, old label, new label name, classification, and any uncertain areas for each symbol analyzed.

### 3.4 Save Phase 3 Progress

*   After all Phase 3 symbol subagents complete, call `r2000_save_project`.
*   If `r2000_save_project` returns `"No active project path"` (`-32603`), treat it as a **non-fatal warning** and continue to Phase 4.

---

## Phase 4 — Final Verification Audit & Structured Summary Report

### 4.1 Verification Audit & Straggler Fix-Up Wave

1.  **Re-Fetch Final State:**
    *   Call `r2000_get_symbols` and `r2000_get_comments` (always omitting both `start_address` and `end_address`, or providing both together—never pass `start_address` without `end_address`).
2.  **Audit Remaining Auto-Labels:**
    *   Inspect the symbol list for any remaining unanalyzed auto-generated routine or data/pointer/external labels: `s_XXXX`, `zpp_XX`, `zpf_XX`, `zpa_XX`, `a_00XX`, `f_00XX`, `p_00XX`, `p_XXXX`, `f_XXXX`, `a_XXXX`, `e_XXXX`.
    *   **Exclude from Unanalyzed Failures:** Ignore `b_XXXX` local branch labels, `r_XXXX` (`LabelType::Return`) return labels, and internal `j_XXXX` (`LabelType::Jump`) branch labels.
    *   **Straggler Fix-Up Wave:** If any analyzable routine (`s_XXXX` lacking a line comment) or symbol (`zpp_XX`, `zpf_XX`, `zpa_XX`, `a_00XX`, `f_00XX`, `p_00XX`, `p_XXXX`, `f_XXXX`, `a_XXXX`, `e_XXXX`) remains unanalyzed (for example, newly created during Phase 2/3 or the second byte of a pair), spawn a fix-up wave of `r2000-cbm-principal-swe` subagents (max 7 concurrent, including the `"You are already running as the r2000-cbm-principal-swe subagent; do NOT read .agent/agents/r2000_cbm_principal_swe.md."` directive) using the Phase 2 or Phase 3 prompt template to complete them before generating the final report.
3.  **Final Project Save:**
    *   Call `r2000_save_project` to persist all changes.
    *   If `r2000_save_project` returns JSON-RPC error `-32603` (`"No active project path"`), treat it as a **non-fatal warning** and include an explicit reminder in the report below instructing the user to save a `.regen2000proj` file in the Regenerator 2000 TUI.

### 4.2 Present Structured Markdown Summary Report

Present a comprehensive Markdown summary report containing the following five sections:

#### BASIC & Blocks Summary

*   Decoded BASIC stub/program summary (if present at `origin`, including line range and `SYS` target address).
*   Total blocks classified, grouped by type (`Code`, `Byte`, `Word`, `PETSCII`, `Screencode`, `Address`, `Lo/Hi`, `Hi/Lo`, `ExternalFile`).
*   Key findings from block classification (such as text tables, split jump tables, sprite/charset buffers, or newly disassembled entry points).

#### Routines Summary

*   Total routines analyzed (across Wave 2A and Wave 2B) and a summary table:

    | Address | Old Label | New Label       | Summary                                 |
    | ------- | --------- | --------------- | --------------------------------------- |
    | `$080D` | `start`   | `start`         | Initializes IRQ vectors and main loop   |
    | `$C000` | `s_C000`  | `init_screen`   | Clears screen RAM and sets border color |
    | `$C050` | `s_C050`  | `read_joystick` | Reads CIA1 port A for joystick 2 input  |

#### Symbols Summary

*   Total symbols analyzed (including clustered 16-bit `_lo`/`_hi` pairs, SMC operands, and independent 8-bit variables) and a summary table:

    | Address | Old Label | New Label          | Classification               |
    | ------- | --------- | ------------------ | ---------------------------- |
    | `$FB`   | `zpp_FB`  | `zp_ptr_screen_lo` | Pointer low byte (ZP indir)  |
    | `$FC`   | `zpa_FC`  | `zp_ptr_screen_hi` | Pointer high byte (ZP indir) |
    | `$0400` | `a_0400`  | `score_display`    | Screencode buffer            |

#### Uncertain, Skipped, or Unanalyzed Items

*   List every symbol and routine that remains unanalyzed, skipped, or uncertain (excluding `b_XXXX`, `r_XXXX`, and internal `j_XXXX` branch labels).
*   Never report `"0 unanalyzed items"` if any analyzable auto-generated labels (`s_XXXX`, `f_XXXX`, `a_XXXX`, `p_XXXX`, `e_XXXX`, `zpa_XX`, `zpf_XX`, `zpp_XX`, `a_00XX`, `f_00XX`, `p_00XX`) remain unanalyzed.

#### Errors, Resource Quotas, and Fallbacks

*   Document all subagent errors, timeouts, label collision retries, or rate-limit events (`RESOURCE_EXHAUSTED` / Code 429).
*   For each event, list the target address, label name, subagent role, error message, and fallback resolution.
*   **Project Persistence Status:** If `r2000_save_project` succeeded, confirm that the `.regen2000proj` file was saved. If `r2000_save_project` returned `"No active project path"` (`-32603`), display a clear reminder: *"Note: Analysis changes are active in memory, but no `.regen2000proj` path is currently set. Please save the project (`.regen2000proj`) in the Regenerator 2000 TUI to persist your changes to disk."*
