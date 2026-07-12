use regenerator2000_core::unpacker::{UnpackConfig, unpack};
use std::fs;
use std::process::Command;

fn main() {
    let unp64_bin = std::env::var("UNP64")
        .or_else(|_| std::env::var("UNP64_PATH"))
        .unwrap_or_else(|_| {
            let candidates = [
                "/Users/ricardoq/.local/bin/unp64",
                "/Users/ricardoq/bin/unp64",
                "unp64",
            ];
            for c in candidates {
                if std::path::Path::new(c).exists() {
                    return c.to_string();
                }
            }
            "unp64".to_string()
        });
    let test_dir = "tests/6502";

    let allowed_files = [
        "c64_8_bit_ball.meanteam_cruncher.prg",
        "c64_lft-rodents-in-the-attic.exo3.prg",
        "c64_connection-8580.pucrunch.prg",
        "c64_f600.exo.prg",
        "c64_moving_tubes_lxt.dali.prg",
        "c64_moving_tubes_lxt.tscrunch_x.prg",
        "c64_moving_tubes_lxt.tscrunch_x2.prg",
        "c64_thats_the_way_scoop.time_cruncher.prg",
        "c64_traveller.tiny_crunch.prg",
        "c64_CopperBooze.byte_boozer2.prg",
        "c64_Bit_by_Bits-BZ!.exo3.prg",
        "c64_boilerplate.exo3.prg",
        "c64_druid_too.exo3.prg",
        "c64_endoskull.exo3.prg",
        "c64_leftovers-pl.exo3.prg",
        "c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg",
        "c64_sprite runners.exo3prg.prg",
        "c64_moving_tubes_lxt.exo3.prg",
        "c64_moving_tubes_lxt.pucrunch.prg",
        "c64_mule.dali.prg",
        "c64_mule.exo3.prg",
        "c64_mule.mccracken_compressor.prg",
        "c64_mule.pucrunch.prg",
        "c64_mule.tscrunch_x.prg",
        "c64_mule.tscrunch_x2.prg",
        "c64_roma.exe.exo3.prg",
        "c64_hw20131031.exo.prg",
        "c64_spectro.exo3.prg",
        "c64_cubicdream.exo3.prg",
        "c64_boo_alz64.prg",
        "c64_lft-nine.exo3.prg",
        "c64_HBFS.exo3.prg",
        "c64_Layers.exo3.prg",
        "c64_fantasy_intro.eca_compactor.prg",
        "c64_FppScroller.byte_boozer2.prg",
        "c64_little_things.exo3.prg",
        "c64_robot - not human.exo3.prg",
        "c64_soul_on_fire_unk.prg",
        "c64_gianna_sister_remix_badboy.tbc_multicompactor.prg",
        "c64_chiller.antiram_packer.prg",
    ];

    println!(
        "============================================================================================="
    );
    println!("   Regenerator 2000 Unpacker vs unp64 Comparison Report (Tests from unpacker.rs)");
    println!(
        "============================================================================================="
    );
    println!(
        "{:<45} {:<24} | {:<24} | Status",
        "File", "unp64 (range / entry)", "R2000 (range / entry)"
    );
    println!(
        "---------------------------------------------------------------------------------------------"
    );

    let entries = match fs::read_dir(test_dir) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("Failed to read test dir {}: {}", test_dir, err);
            return;
        }
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if !p.is_file() {
                return false;
            }
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                allowed_files.contains(&name)
            } else {
                false
            }
        })
        .collect();

    // Sort files alphabetically
    files.sort();

    let mut total = 0;
    let mut passed = 0;
    let mut failed = 0;

    for file_path in files {
        let file_name = match file_path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };
        total += 1;

        let prg_data = match fs::read(&file_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        if prg_data.len() < 2 {
            continue;
        }
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        // Run our unpacker
        let config = UnpackConfig {
            max_instructions: 350_000_000,
            ..Default::default()
        };
        let r2000_res = unpack(raw_data, load_addr, &config, None);

        // Run unp64
        let tmp_out = std::env::temp_dir().join(format!("{}_unp64.tmp", file_name));
        let unp64_status = Command::new(&unp64_bin)
            .arg(&file_path)
            .arg("-o")
            .arg(&tmp_out)
            .output();

        let unp64_ok = match &unp64_status {
            Ok(unp_out) => unp_out.status.success(),
            _ => false,
        };

        // Extract unp64 details if successful
        let mut unp_info = None;
        if unp64_ok
            && tmp_out.exists()
            && let Ok(unp_bytes) = fs::read(&tmp_out)
            && unp_bytes.len() >= 2
        {
            let unp_load = u16::from_le_bytes([unp_bytes[0], unp_bytes[1]]);
            let unp_payload = &unp_bytes[2..];
            let unp_end = unp_load
                .saturating_add(unp_payload.len() as u16)
                .saturating_sub(1);
            if let Ok(unp_out) = &unp64_status {
                let stdout_str = String::from_utf8_lossy(&unp_out.stdout);
                let unp_entry = parse_unp64_entry(&stdout_str).unwrap_or(0);
                unp_info = Some((unp_load, unp_end, unp_entry, unp_bytes));
            }
        }

        match (&r2000_res, &unp_info) {
            (Ok(r2000), Some((unp_load, unp_end, unp_entry, unp_bytes))) => {
                let unp_payload = &unp_bytes[2..];
                let mut matches = true;
                let mut reason = Vec::new();

                if r2000.start_addr != *unp_load {
                    matches = false;
                    reason.push(format!(
                        "Start mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.start_addr, unp_load
                    ));
                }

                if r2000.end_addr != *unp_end {
                    matches = false;
                    reason.push(format!(
                        "End mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.end_addr, unp_end
                    ));
                }

                if *unp_entry != 0 && r2000.entry_point != *unp_entry {
                    matches = false;
                    reason.push(format!(
                        "Entry mismatch: R2000=${:04X} vs unp64=${:04X}",
                        r2000.entry_point, unp_entry
                    ));
                }

                let offset = r2000.start_addr as i32 - *unp_load as i32;
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

                let unp64_range =
                    format!("${:04X}-${:04X} (${:04X})", unp_load, unp_end, unp_entry);
                let r2000_range = format!(
                    "${:04X}-${:04X} (${:04X})",
                    r2000.start_addr, r2000.end_addr, r2000.entry_point
                );

                if matches {
                    println!(
                        "{:<45} {:<24} | {:<24} | PASS",
                        file_name, unp64_range, r2000_range
                    );
                    passed += 1;
                } else {
                    println!(
                        "{:<45} {:<24} | {:<24} | FAIL (MISMATCH)",
                        file_name, unp64_range, r2000_range
                    );
                    for r in reason {
                        println!("  -> {}", r);
                    }
                    failed += 1;
                }
            }
            (Err(e), Some((unp_load, unp_end, unp_entry, _))) => {
                let unp64_range =
                    format!("${:04X}-${:04X} (${:04X})", unp_load, unp_end, unp_entry);
                println!(
                    "{:<45} {:<24} | {:<24} | FAIL (R2000 failed: {:?})",
                    file_name, unp64_range, "-", e
                );
                failed += 1;
            }
            (Ok(r2000), None) => {
                let r2000_range = format!(
                    "${:04X}-${:04X} (${:04X})",
                    r2000.start_addr, r2000.end_addr, r2000.entry_point
                );
                println!(
                    "{:<45} {:<24} | {:<24} | FAIL (unp64 failed)",
                    file_name, "-", r2000_range
                );
                failed += 1;
            }
            (Err(e), None) => {
                println!(
                    "{:<45} {:<24} | {:<24} | FAIL (Both failed: {:?})",
                    file_name, "-", "-", e
                );
                failed += 1;
            }
        }

        if tmp_out.exists() {
            let _ = fs::remove_file(tmp_out);
        }
    }

    println!(
        "---------------------------------------------------------------------------------------------"
    );
    println!("Total PRG files analyzed: {}", total);
    println!("Passed (100% Match):      {}", passed);
    println!("Failed / Mismatched:      {}", failed);
    println!(
        "================================================---------------------------------------------"
    );
}

fn parse_unp64_entry(stdout: &str) -> Option<u16> {
    if let Some(idx) = stdout.find("pass2, return to mem:") {
        let sub = &stdout[idx + "pass2, return to mem:".len()..];
        let target_sub = if let Some(arrow_idx) = sub.find("->") {
            &sub[arrow_idx + 2..]
        } else {
            sub
        };
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
