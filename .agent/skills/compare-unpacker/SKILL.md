---
name: compare-unpacker
description: Runs unpacker_compare_all to compare Regenerator 2000 unpacker against unp64, comparing start/end addresses and entry points across C64 PRG binaries, and generates a comparison report.
---

# Unpacker Comparison Skill (`compare-unpacker`)

Use this skill when you need to benchmark or compare the Regenerator 2000 unpacker against `unp64` across all test PRG binaries registered in `test_unpack_known_prg_files`.

## Single Source of Truth

The single source of truth for test PRG files is `test_unpack_known_prg_files` in [`crates/regenerator2000-core/src/unpacker.rs`](file:///Users/ricardoq/progs/regenerator2000/crates/regenerator2000-core/src/unpacker.rs).

The comparison tool ([`crates/regenerator2000-core/src/bin/unpacker_compare_all.rs`](file:///Users/ricardoq/progs/regenerator2000/crates/regenerator2000-core/src/bin/unpacker_compare_all.rs)) dynamically parses `unpacker.rs` to extract the exact list of active test PRGs. Do not hardcode the total count of test files in reports or documentation.

---

## Workflow Overview

1. **Execute Comparison Binary**: Run `unpacker_compare_all` to execute both `unp64` and Regenerator 2000's `unpack()` engine on all test files dynamically extracted from `test_unpack_known_prg_files`.
2. **Analyze Output**: Extract and compare start address, end address, and entry point for every binary.
3. **Generate Report Artifact**: Create a comprehensive Markdown comparison report summarizing results, exact matches, and categorized discrepancies.

---

## Step 1: Run the Comparison Tool

Execute the comparison binary:

```bash
cargo run -p regenerator2000-core --bin unpacker_compare_all
```

> **Note**: The binary automatically looks for `unp64` in `PATH`, `~/.local/bin/unp64`, `~/bin/unp64`, or via the `UNP64` / `UNP64_PATH` environment variable. If `unp64` is in a custom location, specify `UNP64=/path/to/unp64 cargo run ...`.

---

## Step 2: Analyze & Categorize Discrepancies

For each file listed by the tool, examine the three primary fields:

- **Start Address** (`start_addr`): Range start address (`unp64` vs `R2000`).
- **End Address** (`end_addr`): Range end address (`unp64` vs `R2000`).
- **Entry Point** (`entry_point`): Program entry point address (`unp64` vs `R2000`).

### Common Discrepancy Patterns

When classifying mismatches, categorize them into standard root causes:
1. **Exact Match (PASS)**: Start, end, and entry point match 100%.
2. **Header Byte Offset (1-byte shift)**: `$0800` vs `$0801` start address difference due to PRG 2-byte load address header padding.
3. **Payload vs. Container**: `unp64` dumps the compressed input file range (e.g., Screen RAM `$0400`), while R2000 isolates the actual decompressed payload range (e.g., `$EFB0`).
4. **Tail Padding**: Minor byte differences at end of RAM allocation.
5. **Phase 2 Timeout**: Emulation exceeded instruction limit (e.g., stuck in BASIC keyboard polling loop).

---

## Step 3: Create the Comparison Report Artifact

Write the formatted comparison report as an artifact (e.g., `unpacker_comparison_report.md` in the artifact directory).

### Report Structure Template

```markdown
# Regenerator 2000 Unpacker vs unp64 Comparison Report

**Date**: <Current Date>
**Test Suite Source**: `test_unpack_known_prg_files` in `unpacker.rs`
**Tools**: `regenerator2000-core` unpacker vs `unp64`

## Executive Summary

| Metric | Count | Percentage |
| :--- | :---: | :---: |
| **Total Test PRGs Analyzed** | **<Total Count>** | 100% |
| **Pass (100% Exact Match)** | **<Pass Count>** | <Pass %> |
| **Fail / Discrepancy** | **<Fail Count>** | <Fail %> |

## Comparison Breakdown Table

| Status | PRG File | unp64 (Range / Entry) | R2000 (Range / Entry) | Discrepancy Note |
| :--- | :--- | :--- | :--- | :--- |
| **PASS** | `c64_mule.exo3.prg` | `$0800-$9D19 ($1100)` | `$0800-$9D19 ($1100)` | Exact match |
| **FAIL** | `c64_boilerplate.exo3.prg` | `$0801-$FEA4 ($1000)` | `$0800-$FEA4 ($1000)` | 1-byte load address offset |
| ... | ... | ... | ... | ... |

## Key Insights & Next Steps

1. Highlight any newly fixed binaries or improvements.
2. Summarize remaining failure categories (header offsets, timeout cases, etc.).
```

---

## Verification

After generating the report, verify:
- All test PRGs from `test_unpack_known_prg_files` are accounted for.
- Start, end, and entry points are explicitly listed for both tools.
- `cargo test -p regenerator2000-core --lib unpacker::tests::test_unpack_known_prg_files` passes cleanly.
