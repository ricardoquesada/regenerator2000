use regenerator2000_core::unpacker::{UnpackConfig, unpack};
use std::fs;
use std::process::Command;

fn main() {
    let unp64_bin = "/Users/ricardoq/bin/unp64";
    let test_dir = "tests/6502";

    println!("============================================================");
    println!("   Regenerator 2000 Unpacker vs unp64 Comparison Report");
    println!("============================================================");
    println!(
        "{:<45} {:<10} {:<10} {:<10} {:<10}",
        "File", "unp64", "R2000", "Payload", "Status"
    );
    println!("------------------------------------------------------------");

    let entries = fs::read_dir(test_dir).unwrap();
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();

    // Sort files alphabetically
    files.sort();

    let mut total = 0;
    let mut passed = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for file_path in files {
        let file_name = file_path.file_name().unwrap().to_str().unwrap();

        // Skip reference files (e.g. ones ending in .prg.XXXX or _2e00.prg)
        if file_name.ends_with("_2e00.prg")
            || file_name.contains(".prg.08")
            || file_name.contains(".prg.10")
            || file_name.contains(".prg.28")
            || file_name.contains(".prg.48")
            || file_name.contains(".prg.69")
        {
            continue;
        }

        total += 1;

        let prg_data = fs::read(&file_path).unwrap();
        if prg_data.len() <= 2 {
            println!("{:<45} Skipped (too short)", file_name);
            skipped += 1;
            continue;
        }

        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        // Run our unpacker
        // Some files might need higher instruction limit
        let config = UnpackConfig {
            max_instructions: 350_000_000,
            ..Default::default()
        };
        let r2000_res = unpack(raw_data, load_addr, &config, None);

        // Run unp64
        let tmp_out = std::env::temp_dir().join(format!("{}_unp64.tmp", file_name));
        let unp64_status = Command::new(unp64_bin)
            .arg(&file_path)
            .arg("-o")
            .arg(&tmp_out)
            .output();

        match (r2000_res, unp64_status) {
            (Ok(r2000), Ok(unp_out)) if unp_out.status.success() => {
                // unp64 succeeded and saved to tmp_out
                let unp_bytes = fs::read(&tmp_out).unwrap();
                let unp_load = u16::from_le_bytes([unp_bytes[0], unp_bytes[1]]);
                let unp_payload = &unp_bytes[2..];

                // unp64 output format matches load address
                // Compare start address
                let mut matches = true;
                let mut reason = Vec::new();

                if r2000.start_addr != unp_load {
                    matches = false;
                    reason.push(format!(
                        "Start mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.start_addr, unp_load
                    ));
                }

                let unp_end = unp_load
                    .saturating_add(unp_payload.len() as u16)
                    .saturating_sub(1);
                if r2000.end_addr != unp_end {
                    matches = false;
                    reason.push(format!(
                        "End mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.end_addr, unp_end
                    ));
                }

                // Parse entry point from stdout if possible
                // e.g. "Entry point: $080d" or "pass2, return to mem: $0834"
                let stdout_str = String::from_utf8_lossy(&unp_out.stdout);
                let unp_entry = parse_unp64_entry(&stdout_str).unwrap_or(0);

                if unp_entry != 0 && r2000.entry_point != unp_entry {
                    matches = false;
                    reason.push(format!(
                        "Entry mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.entry_point, unp_entry
                    ));
                }

                // Compare payload
                let offset = r2000.start_addr as i32 - unp_load as i32;
                if offset == 0 {
                    let min_len = r2000.data.len().min(unp_payload.len());
                    if r2000.data[..min_len] != unp_payload[..min_len] {
                        matches = false;
                        reason.push("Payload content mismatch".to_string());
                    }
                } else {
                    matches = false;
                    reason.push(format!("Load address offset mismatch: {}", offset));
                }

                if matches {
                    println!("{:<45} OK         OK         MATCH      PASS", file_name);
                    passed += 1;
                } else {
                    println!("{:<45} OK         OK         MISMATCH   FAIL", file_name);
                    for r in reason {
                        println!("  -> {}", r);
                    }
                    failed += 1;
                }

                let _ = fs::remove_file(tmp_out);
            }
            (Err(_e), Ok(unp_out)) if !unp_out.status.success() => {
                // Both failed (not a packed file or packer not supported by both)
                println!(
                    "{:<45} FAIL       FAIL       -          SKIP (Not packed)",
                    file_name
                );
                skipped += 1;
            }
            (Ok(r2000), Ok(unp_out)) if !unp_out.status.success() => {
                // R2000 succeeded, unp64 failed
                println!(
                    "{:<45} FAIL       OK (${:04X})  -          FAIL (unp64 failed)",
                    file_name, r2000.entry_point
                );
                failed += 1;
            }
            (Err(e), Ok(unp_out)) if unp_out.status.success() => {
                // R2000 failed, unp64 succeeded
                println!(
                    "{:<45} OK         FAIL       -          FAIL (R2000 failed: {:?})",
                    file_name, e
                );
                failed += 1;
                let _ = fs::remove_file(tmp_out);
            }
            _ => {
                println!(
                    "{:<45} ERROR      ERROR      -          FAIL (Execution error)",
                    file_name
                );
                failed += 1;
            }
        }
    }

    println!("------------------------------------------------------------");
    println!("Total PRG files analyzed: {}", total);
    println!("Passed (100% Match):      {}", passed);
    println!("Failed / Mismatched:      {}", failed);
    println!("Skipped (Not packed):     {}", skipped);
    println!("============================================================");
}

fn parse_unp64_entry(stdout: &str) -> Option<u16> {
    // Look for: "pass2, return to mem: \n$0834" or "pass2, return to mem: $0834"
    // Also "Entry point: $080d"
    // Let's try "pass2, return to mem: "
    if let Some(idx) = stdout.find("pass2, return to mem:") {
        let sub = &stdout[idx + "pass2, return to mem:".len()..];
        let target_sub = if let Some(arrow_idx) = sub.find("->") {
            &sub[arrow_idx + 2..]
        } else {
            sub
        };
        // Find the first hex number starting with $
        if let Some(dollar_idx) = target_sub.find('$') {
            let hex_str: String = target_sub[dollar_idx + 1..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .collect();
            if let Ok(val) = u16::from_str_radix(&hex_str, 16) {
                return Some(val);
            }
        }
    }
    None
}
