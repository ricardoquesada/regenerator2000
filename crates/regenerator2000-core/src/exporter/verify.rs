//! Roundtrip assembly verification (Stage 3 of the exporter pipeline).
//!
//! [`verify_roundtrip`] and [`verify_all_assemblers`] export [`crate::state::AppState`] to a
//! temporary `.asm` bundle with [`ExportOptions::for_assembler`] and [`render_asm_with_options`].
//! This path avoids cloning [`crate::state::AppState`] and preserves all project, global, and
//! built-in enum definitions. It then runs the external cross-assembler and compares both the
//! 2-byte PRG load-address header and the reassembled payload byte-for-byte against `state.raw_data`.

use super::asm::render_asm_with_options;
use super::ir::ExportOptions;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Result of a roundtrip verification for one cross-assembler.
#[derive(Debug)]
pub struct VerifyResult {
    /// Target cross-assembler that ran verification.
    pub assembler: crate::state::Assembler,
    /// `true` when the 2-byte PRG load address and reassembled payload match `state`.
    pub success: bool,
    /// Diagnostic message when `success` is `false` (empty on success).
    pub message: String,
    /// Number of payload bytes that differ (`0` = identical).
    pub diff_count: usize,
    /// Total bytes compared against `state.raw_data`.
    pub total_bytes: usize,
}

impl std::fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.success {
            write!(
                f,
                "  ✓ {} — byte-identical ({} bytes)",
                self.assembler, self.total_bytes
            )
        } else {
            write!(f, "  ✗ {} — {}", self.assembler, self.message)
        }
    }
}

/// Runs an export, assemble, and byte-diff roundtrip for a [`crate::state::Assembler`].
///
/// Returns a [`VerifyResult`] that indicates whether the assembled PRG output is
/// byte-identical to the original binary.
///
/// This function performs five steps:
/// 1. Builds [`ExportOptions::for_assembler`] without cloning [`crate::state::AppState`].
/// 2. Renders and writes `.asm` and `.bin` sidecars to a unique temporary directory via
///    [`render_asm_with_options`] and [`super::ir::ExportBundle::write_to_disk`].
/// 3. Runs the external assembler (`64tass`, `acme`, `cl65`, or `KickAss.jar`).
/// 4. Compares the 2-byte PRG load-address header against `state.origin` and the payload
///    against `state.raw_data`.
/// 5. Deletes the temporary directory.
#[must_use]
pub fn verify_roundtrip(
    state: &crate::state::AppState,
    assembler: crate::state::Assembler,
) -> VerifyResult {
    // Create a unique subdirectory per call to prevent parallel test collisions.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique_id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "regenerator2000_verify_{}_{}",
        std::process::id(),
        unique_id
    ));
    let _ = std::fs::create_dir_all(&dir);

    let asm_stem = format!("verify_{}", assembler_file_suffix(assembler));
    let asm_file = dir.join(format!("{asm_stem}.asm"));
    let prg_file = dir.join(format!("{asm_stem}.prg"));

    // Remove stale files from prior runs if present.
    let _ = std::fs::remove_file(&asm_file);
    let _ = std::fs::remove_file(&prg_file);

    // 1. Export ASM using ExportOptions::for_assembler without cloning AppState.
    let options = ExportOptions::for_assembler(state, &asm_file, assembler);
    if let Err(e) = render_asm_with_options(state, &asm_file, &options).write_to_disk() {
        let _ = std::fs::remove_dir_all(&dir);
        return VerifyResult {
            assembler,
            success: false,
            message: format!("Export failed: {e}"),
            diff_count: 0,
            total_bytes: state.raw_data.len(),
        };
    }

    // 2. Assemble the exported source file.
    let assemble_result = run_assembler(
        assembler,
        &asm_file,
        &prg_file,
        &dir,
        state.settings.use_illegal_opcodes,
    );
    match assemble_result {
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return VerifyResult {
                assembler,
                success: false,
                message: e,
                diff_count: 0,
                total_bytes: state.raw_data.len(),
            };
        }
        Ok(stderr) => {
            if !stderr.is_empty() {
                log::debug!("Assembler stderr for {assembler}: {stderr}");
            }
        }
    }

    // 3. Compare the assembled PRG output against `state.origin` and `state.raw_data`.
    let result = match std::fs::read(&prg_file) {
        Ok(assembled_bytes) => {
            compare_assembled_prg(state.origin, &state.raw_data, &assembled_bytes, assembler)
        }
        Err(e) => VerifyResult {
            assembler,
            success: false,
            message: format!("could not read assembled output: {e}"),
            diff_count: 0,
            total_bytes: state.raw_data.len(),
        },
    };

    // 4. Delete the temporary directory.
    let _ = std::fs::remove_dir_all(&dir);

    result
}

/// Verifies roundtrip assembly for all four supported assemblers.
#[must_use]
pub fn verify_all_assemblers(state: &crate::state::AppState) -> Vec<VerifyResult> {
    use crate::state::Assembler;
    Assembler::all()
        .iter()
        .map(|asm| verify_roundtrip(state, *asm))
        .collect()
}

// ---- Helper functions for verification ----

fn compare_assembled_prg(
    origin: crate::state::Addr,
    expected: &[u8],
    assembled_bytes: &[u8],
    assembler: crate::state::Assembler,
) -> VerifyResult {
    let total = expected.len();
    if assembled_bytes.len() < 2 {
        return VerifyResult {
            assembler,
            success: false,
            message: format!(
                "assembled PRG is too short ({} bytes, expected 2-byte load address header)",
                assembled_bytes.len()
            ),
            diff_count: total,
            total_bytes: total,
        };
    }

    let assembled_origin = u16::from_le_bytes([assembled_bytes[0], assembled_bytes[1]]);
    // In 64tass, ACME, and KickAssembler, the origin directive sets the 2-byte PRG header.
    // In ca65, `c64-asm.cfg` always writes its default `$0801` linker start address.
    let origin_matches = assembler == crate::state::Assembler::Ca65 || assembled_origin == origin.0;
    let payload = &assembled_bytes[2..];
    let diff_count = count_diffs(expected, payload);

    if origin_matches && diff_count == 0 && payload.len() == total {
        return VerifyResult {
            assembler,
            success: true,
            message: String::new(),
            diff_count: 0,
            total_bytes: total,
        };
    }

    let mut msg = String::new();
    if !origin_matches {
        let _ = write!(
            msg,
            "load address mismatch: expected ${:04x}, got ${assembled_origin:04x}",
            origin.0
        );
    }
    if payload.len() != total {
        if !msg.is_empty() {
            msg.push_str("; ");
        }
        let _ = write!(
            msg,
            "size mismatch: original={total} assembled={}",
            payload.len()
        );
    }
    if diff_count > 0 {
        if !msg.is_empty() {
            msg.push_str("; ");
        }
        let compare_len = total.min(payload.len());
        let _ = write!(msg, "{diff_count} of {compare_len} bytes differ");

        // List the first five differing bytes for debugging.
        let mut shown = 0;
        for (i, (a, b)) in expected.iter().zip(payload.iter()).enumerate() {
            if a != b {
                if shown < 5 {
                    let addr = origin.wrapping_add(i as u16);
                    let _ = write!(msg, "\n    ${addr:04x}: expected ${a:02x}, got ${b:02x}");
                }
                shown += 1;
            }
        }
        if shown > 5 {
            let _ = write!(msg, "\n    ... and {} more", shown - 5);
        }
    }

    VerifyResult {
        assembler,
        success: false,
        message: msg,
        diff_count,
        total_bytes: total,
    }
}

fn assembler_file_suffix(asm: crate::state::Assembler) -> &'static str {
    use crate::state::Assembler;
    match asm {
        Assembler::Tass64 => "64tass",
        Assembler::Acme => "acme",
        Assembler::Ca65 => "ca65",
        Assembler::Kick => "kick",
    }
}

/// Runs the external cross-assembler to produce a PRG file and returns `Ok(stderr)` on success.
fn run_assembler(
    assembler: crate::state::Assembler,
    asm_file: &Path,
    prg_file: &Path,
    work_dir: &Path,
    use_illegal_opcodes: bool,
) -> Result<String, String> {
    use crate::state::Assembler;
    use std::process::Command;

    let output = match assembler {
        Assembler::Tass64 => {
            let mut cmd = Command::new("64tass");
            cmd.arg("-C");
            if use_illegal_opcodes {
                cmd.arg("-i");
            }
            cmd.arg("-o")
                .arg(prg_file)
                .arg(asm_file)
                .current_dir(work_dir)
                .output()
        }
        Assembler::Acme => {
            let mut cmd = Command::new("acme");
            if use_illegal_opcodes {
                cmd.arg("--cpu").arg("6510");
            }
            cmd.arg("--format")
                .arg("cbm")
                .arg("-o")
                .arg(prg_file)
                .arg(asm_file)
                .current_dir(work_dir)
                .output()
        }
        Assembler::Ca65 => {
            // cl65 runs both the ca65 assembler and the ld65 linker in one step.
            let mut cmd = Command::new("cl65");
            cmd.arg("-t").arg("c64");
            if use_illegal_opcodes {
                cmd.arg("--cpu").arg("6502X");
            }
            cmd.arg("-C")
                .arg("c64-asm.cfg")
                .arg(asm_file)
                .arg("-o")
                .arg(prg_file)
                .current_dir(work_dir)
                .output()
        }
        Assembler::Kick => {
            // Resolve relative KICKASS_JAR paths before changing current_dir to work_dir.
            let raw_jar =
                std::env::var("KICKASS_JAR").unwrap_or_else(|_| "KickAss.jar".to_string());
            let jar_path =
                std::fs::canonicalize(&raw_jar).unwrap_or_else(|_| PathBuf::from(&raw_jar));
            Command::new("java")
                .arg("-jar")
                .arg(&jar_path)
                .arg(asm_file)
                .arg("-o")
                .arg(prg_file)
                .current_dir(work_dir)
                .output()
        }
    };

    match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if out.status.success() {
                Ok(stderr)
            } else {
                // KickAssembler reports "Unable to access jarfile" when the jar is not installed.
                if stderr.contains("Unable to access jarfile")
                    || stdout.contains("Unable to access jarfile")
                {
                    return Err(format!("{assembler} not found in PATH (skipped)"));
                }
                Err(format!(
                    "assembler exited with {}\nstdout: {}\nstderr: {}",
                    out.status, stdout, stderr
                ))
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Err(format!("{assembler} not found in PATH (skipped)"))
            } else {
                Err(format!("failed to execute {assembler}: {e}"))
            }
        }
    }
}

fn count_diffs(a: &[u8], b: &[u8]) -> usize {
    let min_len = a.len().min(b.len());
    let mut diffs = 0;
    for i in 0..min_len {
        if a[i] != b[i] {
            diffs += 1;
        }
    }
    // Count bytes beyond the shorter slice as differences.
    diffs += a.len().abs_diff(b.len());
    diffs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState, Assembler, BlockType, EnumDefinition};
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn test_compare_assembled_prg_checks_load_address_header_and_payload() {
        let origin = Addr(0x0801);
        let payload = [0xA9, 0x00, 0x60];

        // 1. Valid 2-byte little-endian load address ($0801 -> 01 08) + matching payload
        let valid_prg = [0x01, 0x08, 0xA9, 0x00, 0x60];
        let ok_res = compare_assembled_prg(origin, &payload, &valid_prg, Assembler::Tass64);
        assert!(ok_res.success);
        assert_eq!(ok_res.diff_count, 0);

        // 2. Wrong load address ($1000 -> 00 10) with matching payload must fail verification
        let wrong_origin_prg = [0x00, 0x10, 0xA9, 0x00, 0x60];
        let bad_origin =
            compare_assembled_prg(origin, &payload, &wrong_origin_prg, Assembler::Tass64);
        assert!(!bad_origin.success);
        assert!(
            bad_origin
                .message
                .contains("load address mismatch: expected $0801, got $1000"),
            "Unexpected message: {}",
            bad_origin.message
        );

        // 3. Truncated PRG (< 2 bytes) must fail verification
        let truncated_prg = [0x01];
        let short_res = compare_assembled_prg(origin, &payload, &truncated_prg, Assembler::Tass64);
        assert!(!short_res.success);
        assert!(short_res.message.contains("assembled PRG is too short"));
    }

    #[test]
    fn test_verify_roundtrip_preserves_enums_without_cloning_state() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // LDA #$00, LDX #$01, LDY #$02, RTS
        state.raw_data = vec![0xA9, 0x00, 0xA2, 0x01, 0xA0, 0x02, 0x60];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.settings.assembler = Assembler::Tass64;

        // 1. Project enum (`state.enums`) at $1000
        let mut proj_variants = BTreeMap::new();
        proj_variants.insert(0, "PROJ_BLACK".to_string());
        state.enums.insert(
            "ProjectColors".to_string(),
            EnumDefinition {
                name: "ProjectColors".to_string(),
                description: None,
                variants: proj_variants,
                source_file: None,
            },
        );
        state.annotations.update(Addr(0x1000), |e| {
            e.enum_usage = Some("ProjectColors".to_string());
        });

        // 2. User global enum (`state.user_global_enums`) at $1002
        let mut global_variants = BTreeMap::new();
        global_variants.insert(1, "MODE_PLAY".to_string());
        state.user_global_enums.insert(
            "GlobalModes".to_string(),
            EnumDefinition {
                name: "GlobalModes".to_string(),
                description: None,
                variants: global_variants,
                source_file: None,
            },
        );
        state.annotations.update(Addr(0x1002), |e| {
            e.enum_usage = Some("GlobalModes".to_string());
        });

        // 3. Builtin enum (`state.builtin_enums`) at $1004
        let mut builtin_variants = BTreeMap::new();
        builtin_variants.insert(2, "FLAG_ACTIVE".to_string());
        state.builtin_enums.insert(
            "BuiltinFlags".to_string(),
            EnumDefinition {
                name: "BuiltinFlags".to_string(),
                description: None,
                variants: builtin_variants,
                source_file: None,
            },
        );
        state.annotations.update(Addr(0x1004), |e| {
            e.enum_usage = Some("BuiltinFlags".to_string());
        });

        let dummy_path = Path::new("verify_enums.asm");

        for &asm in Assembler::all() {
            // Verify in-memory rendering preserves all three enum pools without mutating `state`
            let options = ExportOptions::for_assembler(&state, dummy_path, asm);
            let bundle = render_asm_with_options(&state, dummy_path, &options);
            assert!(
                bundle.primary_content.contains("PROJ_BLACK"),
                "{asm} rendered output missing ProjectColors::PROJ_BLACK:\n{}",
                bundle.primary_content
            );
            assert!(
                bundle.primary_content.contains("MODE_PLAY"),
                "{asm} rendered output missing GlobalModes::MODE_PLAY:\n{}",
                bundle.primary_content
            );
            assert!(
                bundle.primary_content.contains("FLAG_ACTIVE"),
                "{asm} rendered output missing BuiltinFlags::FLAG_ACTIVE:\n{}",
                bundle.primary_content
            );
            assert_eq!(state.settings.assembler, Assembler::Tass64);

            // Verify actual roundtrip assembly when assembler binary is installed
            let result = verify_roundtrip(&state, asm);
            if result.message.contains("not found in PATH") {
                continue;
            }
            assert!(
                result.success,
                "{asm} roundtrip failed for binary with enums: {}",
                result.message
            );
            assert_eq!(result.diff_count, 0);
        }
    }
}
