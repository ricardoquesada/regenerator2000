---
name: compare-unpacker
description: Runs unpacker_compare_all to compare the Regenerator 2000 unpacker against unp64 over the corpus manifest (tests/6502/unpack_corpus.toml), records unp64's results with --update, detects unp64 drift with --check, and produces a classified Markdown report with --markdown.
---

# Unpacker Comparison Skill (`compare-unpacker`)

Use this skill to compare the Regenerator 2000 (R2000) unpacker against
`unp64`, to record unp64's results for new corpus cases, or to check that the
recorded results still match the installed unp64.

## Source of truth

* **Case list and recorded unp64 results:**
  [`tests/6502/unpack_corpus.toml`](file:///Users/ricardoq/progs/regenerator2000/tests/6502/unpack_corpus.toml).
  Each `[[case]]` names a fixture in `tests/6502`. Its `[case.unp64]` table
  (range, entry, length, payload hash, overlap window, unp64 version) is
  **written by the tool**, never by hand. `[[group]]` entries name an
  original, unpacked binary that is ground truth for its members.
* **Frozen R2000 outputs:**
  [`tests/6502/unpack_baseline.toml`](file:///Users/ricardoq/progs/regenerator2000/tests/6502/unpack_baseline.toml),
  regenerated with
  `UPDATE_UNPACK_BASELINE=1 cargo test -p regenerator2000-core --test unpack_corpus`.
* **Tool:**
  [`crates/regenerator2000-core/src/bin/unpacker_compare_all/`](file:///Users/ricardoq/progs/regenerator2000/crates/regenerator2000-core/src/bin/unpacker_compare_all/main.rs).
  The shared corpus library lives in its `corpus/` module.
* **CI check:**
  [`crates/regenerator2000-core/tests/unpack_corpus.rs`](file:///Users/ricardoq/progs/regenerator2000/crates/regenerator2000-core/tests/unpack_corpus.rs)
  reads the recorded results, so CI never needs unp64.

Do not hardcode the number of cases in reports or documentation. Count the
`[[case]]` entries in the manifest.

---

## Modes

```bash
# Comparison report only (Markdown on stdout)
cargo run -p regenerator2000-core --bin unpacker_compare_all -- --markdown

# Re-run unp64 and exit 1 if a recorded result no longer matches the binary
cargo run -p regenerator2000-core --bin unpacker_compare_all -- --check --markdown

# Record unp64's results into the manifest and propose exceptions
cargo run -p regenerator2000-core --bin unpacker_compare_all -- --update
```

| Exit code | Meaning |
| :-- | :-- |
| 0 | Success (no drift) |
| 1 | `--check` found drift: re-run `--update` and review the `git diff` |
| 2 | Usage error or tool error (unp64 missing, broken manifest, unrecordable unp64 output) |

The tool runs every case with the case's own `max_instructions` (default
50 M). It runs unp64 as `unp64 <fixture> -o <unique temporary file>`, so it
never writes next to a fixture. unp64's exit code is not trusted: a run
without a `saved $SSSS-$EEEE` line is recorded as `failed`.

> **unp64 location:** `$UNP64` if set (no fallback), else `unp64` on `PATH`,
> else `~/.local/bin/unp64`. Example: `UNP64=/path/to/unp64 cargo run ...`.

---

## Divergence categories

The classifier and the corpus test share one closed set of categories.

| Category | Meaning | Proposed by `--update`? |
| :-- | :-- | :-- |
| `agree` | Same range, entry and payload | No exception needed |
| `padding_byte` | The starts are the BASIC link byte pair (e.g. `$0800`/`$0801`) | Yes, `fields = ["start"]` |
| `unp64_default_end` | unp64's end is above R2000's and on the top page (`>= $FF00`) | Yes, `fields = ["end"]` |
| `unp64_default_start` | unp64's start is `min(load, RAM start)` | Yes, `fields = ["start"]` |
| `unp64_missing_opcode` | unp64 stopped on an unimplemented opcode | Yes, all fields |
| `unexplained` | Differs, and no benign explanation applies | Yes, the differing fields |
| `payload_mismatch` | Bytes differ over the shared range | **Never**: investigate |
| `entry_mismatch` | Both tools resolved different entries | **Never**: investigate |
| `one_side_failed` | R2000 or unp64 failed | Only a failed unp64 run (as `unexplained`) |

Proposals are written **unsigned** (`reviewed_by` missing). The corpus test
fails until a human reviews and signs them. A signed exception survives later
`--update` runs while the recorded results and the differing fields stay the
same. Otherwise it is replaced by a new unsigned proposal. Exceptions that no
longer apply are removed.

`expected_fail` markers are a closed list, and each one masks only its own
failures:

* `padding_convention`: a ground-truth start mismatch on the BASIC link
  byte pair. No case uses it today (Dali did until its start was fixed).
* `payload_mismatch_under_investigation`: payload and overlap hash mismatches
  only (ruling R17).

When a marked case starts passing, the corpus test fails with "unexpected
pass: remove expected_fail".

---

## Workflow

1. **Add a case (optional):** add a `[[case]]` with only `file = "..."` (plus
   `group`, `max_instructions` if needed) to the manifest. Check that the
   fixture is tracked: `git ls-files --error-unmatch tests/6502/<file>`.
2. **Record:** run `--update`. It prints one line per case (record status,
   divergence category, and what happened to the exception). It also lists
   the cases that need a review decision.
3. **Review:** for each proposal, confirm the category and sign it with
   `reviewed_by = "<your name>"`. For `payload_mismatch` / `entry_mismatch`,
   investigate. Never accept them blindly.
4. **Verify:** run
   `cargo test -p regenerator2000-core --test unpack_corpus` (use
   `CORPUS_FILTER=<text>` for a subset). If R2000's outputs changed on
   purpose, regenerate the baseline, then review `git diff tests/6502/unpack_baseline.toml`.
5. **Check fixtures:** `git status --short tests/6502` must show no modified
   fixture.

---

## Create the comparison report artifact

Run `--markdown` (or `--check --markdown`) and save the output as an
artifact, for example `unpacker_comparison_report.md`. The report contains:

* a summary table with the number of cases per divergence category;
* one row per case: file, unp64's packer name, unp64 and R2000 range and
  entry, the divergence category, and how the manifest treats the case
  (ground truth, accepted exception and its review state, or the plain unp64
  reference);
* a **User action items** section: unsigned or pending exceptions,
  `unexplained` cases, and `expected_fail` markers.

Add a short "Key insights" section with new agreements, new divergences, and
any `payload_mismatch` or `entry_mismatch` cases.

---

## Verification

* `cargo test -p regenerator2000-core --test unpack_corpus` passes.
* `cargo run -p regenerator2000-core --bin unpacker_compare_all -- --check` exits 0.
* `git status --short tests/6502` shows no modified fixture.
