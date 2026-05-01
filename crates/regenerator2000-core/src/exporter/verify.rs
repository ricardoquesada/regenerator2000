use super::asm::export_asm;

/// Result of a roundtrip verification for a single assembler
#[derive(Debug)]
pub struct VerifyResult {
    pub assembler: crate::state::Assembler,
    pub success: bool,
    pub message: String,
    /// Number of bytes that differ (0 = identical)
    pub diff_count: usize,
    /// Total bytes compared
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

/// Perform export → assemble → diff roundtrip for a specific assembler.
///
/// Returns a `VerifyResult` indicating whether the assembled output is
/// byte-identical to the original binary.
///
/// The function:
/// 1. Temporarily sets the assembler in state settings
/// 2. Exports ASM to a temp file
/// 3. Runs the appropriate assembler
/// 4. Compares the assembled output (the raw bytes after the PRG load-address header)
///    against `state.raw_data`
/// 5. Cleans up temp files
pub fn verify_roundtrip(
    state: &crate::state::AppState,
    assembler: crate::state::Assembler,
) -> VerifyResult {
    // We need a mutable clone of settings to change the assembler
    let mut state_clone = clone_state_for_verify(state);
    state_clone.settings.assembler = assembler;

    // Use a unique subdirectory per invocation to avoid parallel test conflicts
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

    // Cleanup any previous run artifacts
    let _ = std::fs::remove_file(&asm_file);
    let _ = std::fs::remove_file(&prg_file);

    // 1. Export ASM
    if let Err(e) = export_asm(&state_clone, &asm_file) {
        let _ = std::fs::remove_dir_all(&dir);
        return VerifyResult {
            assembler,
            success: false,
            message: format!("export failed: {e}"),
            diff_count: 0,
            total_bytes: state.raw_data.len(),
        };
    }

    // 2. Assemble
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
                message: e.clone(),
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

    // 3. Compare
    let result = match std::fs::read(&prg_file) {
        Ok(assembled_bytes) => {
            // PRG files have a 2-byte load address header — strip it for comparison.
            // ca65/cl65 also produces a PRG with load address.
            // KickAssembler produces a PRG by default.
            let payload = if assembled_bytes.len() >= 2 {
                &assembled_bytes[2..]
            } else {
                &assembled_bytes[..]
            };

            let total = state.raw_data.len();
            let diff_count = count_diffs(&state.raw_data, payload);

            if diff_count == 0 && payload.len() == total {
                VerifyResult {
                    assembler,
                    success: true,
                    message: String::new(),
                    diff_count: 0,
                    total_bytes: total,
                }
            } else {
                let mut msg = String::new();
                if payload.len() != total {
                    msg.push_str(&format!(
                        "size mismatch: original={} assembled={}",
                        total,
                        payload.len()
                    ));
                }
                if diff_count > 0 {
                    if !msg.is_empty() {
                        msg.push_str("; ");
                    }
                    let compare_len = total.min(payload.len());
                    msg.push_str(&format!("{diff_count} of {compare_len} bytes differ"));

                    // Show first few diffs for debugging
                    let mut shown = 0;
                    for (i, (a, b)) in state.raw_data.iter().zip(payload.iter()).enumerate() {
                        if a != b {
                            if shown < 5 {
                                let addr = state.origin.wrapping_add(i as u16);
                                msg.push_str(&format!(
                                    "\n    ${addr:04x}: expected ${a:02x}, got ${b:02x}"
                                ));
                            }
                            shown += 1;
                        }
                    }
                    if shown > 5 {
                        msg.push_str(&format!("\n    ... and {} more", shown - 5));
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
        }
        Err(e) => VerifyResult {
            assembler,
            success: false,
            message: format!("could not read assembled output: {e}"),
            diff_count: 0,
            total_bytes: state.raw_data.len(),
        },
    };

    // 4. Cleanup — remove the entire unique temp directory
    let _ = std::fs::remove_dir_all(&dir);

    result
}

/// Verify roundtrip for all 4 assemblers. Returns results for each.
#[must_use]
pub fn verify_all_assemblers(state: &crate::state::AppState) -> Vec<VerifyResult> {
    use crate::state::Assembler;
    Assembler::all()
        .iter()
        .map(|asm| verify_roundtrip(state, *asm))
        .collect()
}

// ---- Helper functions for verification ----

fn assembler_file_suffix(asm: crate::state::Assembler) -> &'static str {
    use crate::state::Assembler;
    match asm {
        Assembler::Tass64 => "64tass",
        Assembler::Acme => "acme",
        Assembler::Ca65 => "ca65",
        Assembler::Kick => "kick",
    }
}

/// Run the assembler and produce a PRG file. Returns `Ok(stderr)` on success.
fn run_assembler(
    assembler: crate::state::Assembler,
    asm_file: &std::path::Path,
    prg_file: &std::path::Path,
    work_dir: &std::path::Path,
    use_illegal_opcodes: bool,
) -> Result<String, String> {
    use crate::state::Assembler;
    use std::process::Command;

    let asm_path = asm_file.to_str().unwrap_or("verify.asm");
    let prg_path = prg_file.to_str().unwrap_or("verify.prg");

    let output = match assembler {
        Assembler::Tass64 => {
            let mut cmd = Command::new("64tass");
            if use_illegal_opcodes {
                cmd.arg("-i");
            }
            cmd.arg("-o")
                .arg(prg_path)
                .arg(asm_path)
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
                .arg(prg_path)
                .arg(asm_path)
                .current_dir(work_dir)
                .output()
        }
        Assembler::Ca65 => {
            // ca65 requires a two-step process: assemble then link
            // cl65 wraps both steps
            let mut cmd = Command::new("cl65");
            cmd.arg("-t").arg("c64");
            if use_illegal_opcodes {
                cmd.arg("--cpu").arg("6502X");
            }
            cmd.arg("-C")
                .arg("c64-asm.cfg")
                .arg(asm_path)
                .arg("-o")
                .arg(prg_path)
                .current_dir(work_dir)
                .output()
        }
        Assembler::Kick => {
            // KickAssembler: java -jar KickAss.jar input.asm -o output.prg
            // Use KICKASS_JAR env var for the absolute path to the jar file,
            // since current_dir is set to a temp directory.
            let jar_path =
                std::env::var("KICKASS_JAR").unwrap_or_else(|_| "KickAss.jar".to_string());
            Command::new("java")
                .arg("-jar")
                .arg(&jar_path)
                .arg(asm_path)
                .arg("-o")
                .arg(prg_path)
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
                // KickAssembler reports "Unable to access jarfile" when not installed
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

/// Create a lightweight clone of `AppState` suitable for roundtrip verification.
/// Only copies the fields needed for `export_asm`.
fn clone_state_for_verify(state: &crate::state::AppState) -> crate::state::AppState {
    let mut clone = crate::state::AppState::new();
    clone.origin = state.origin;
    clone.raw_data = state.raw_data.clone();
    clone.block_types = state.block_types.clone();
    clone.labels = state.labels.clone();
    clone.settings = state.settings.clone();
    clone.platform_comments = state.platform_comments.clone();
    clone.user_side_comments = state.user_side_comments.clone();
    clone.user_line_comments = state.user_line_comments.clone();
    clone.immediate_value_formats = state.immediate_value_formats.clone();
    clone.cross_refs = state.cross_refs.clone();
    clone.collapsed_blocks = state.collapsed_blocks.clone();
    clone.splitters = state.splitters.clone();
    clone.scopes = state.scopes.clone();
    clone.disassembly = state.disassembly.clone();
    clone
}

fn count_diffs(a: &[u8], b: &[u8]) -> usize {
    let min_len = a.len().min(b.len());
    let mut diffs = 0;
    for i in 0..min_len {
        if a[i] != b[i] {
            diffs += 1;
        }
    }
    // Bytes beyond the shorter slice count as diffs
    diffs += a.len().abs_diff(b.len());
    diffs
}
