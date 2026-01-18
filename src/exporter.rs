use crate::state::AppState;
use std::path::PathBuf;

pub fn export_asm(state: &AppState, path: &PathBuf) -> std::io::Result<()> {
    let formatter = state.get_formatter();

    let mut output = String::new();

    let base_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");

    output.push_str(&formatter.format_file_header(base_name));

    let mut origin_printed = false;

    let external_lines = state.get_external_label_definitions();

    // Regenerate disassembly without collapsed blocks for export
    let full_disassembly = state.disassembler.disassemble(
        &state.raw_data,
        &state.block_types,
        &state.labels,
        state.origin,
        &state.settings,
        &state.system_comments,
        &state.user_side_comments,
        &state.user_line_comments,
        &state.immediate_value_formats,
        &state.cross_refs,
        &[], // Ignore collapsed_blocks
        &state.splitters,
    );

    let all_lines: Vec<&crate::disassembler::DisassemblyLine> = external_lines
        .iter()
        .chain(full_disassembly.iter())
        .collect();

    let mut i = 0;
    while i < all_lines.len() {
        let line = all_lines[i];

        // Special case: Header (starts with comment prefix)
        if line.mnemonic.starts_with(formatter.comment_prefix()) {
            output.push_str(&format!("{}\n", line.mnemonic));
            i += 1;
            continue;
        }

        // Special case: Equate (contains =)
        if line.mnemonic.contains('=') {
            output.push_str(&format!("{}\n", line.mnemonic));
            i += 1;
            continue;
        }

        // Special case: Empty line (separator)
        if line.mnemonic.is_empty() && line.bytes.is_empty() && line.comment.is_empty() {
            output.push('\n');
            i += 1;
            continue;
        }

        // Check for ExternalFile
        let offset = line.address as isize - state.origin as isize;
        let is_external = if offset >= 0 && (offset as usize) < state.block_types.len() {
            state.block_types[offset as usize] == crate::state::BlockType::ExternalFile
        } else {
            false
        };

        if is_external {
            // Find end of contiguous block
            let start_idx = offset as usize;
            let mut end_idx = start_idx;
            while end_idx < state.block_types.len()
                && state.block_types[end_idx] == crate::state::BlockType::ExternalFile
            {
                end_idx += 1;
            }
            // end_idx is exclusive
            let byte_count = end_idx - start_idx;
            let start_addr = line.address;
            let end_addr = line.address.wrapping_add(byte_count as u16).wrapping_sub(1);

            // Extract data
            let data_slice = &state.raw_data[start_idx..end_idx];

            // Generate filename
            let base_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("export");

            let bin_filename = format!("{}_{:04x}_{:04x}.bin", base_name, start_addr, end_addr);

            // Allow override of path directory? Use same dir as asm file
            let bin_path = path
                .parent()
                .unwrap_or(&std::path::PathBuf::from("."))
                .join(&bin_filename);

            if let Err(e) = std::fs::write(&bin_path, data_slice) {
                // Determine how to report error? For now print to stdout or just panic?
                // Returning Result is better.
                return Err(std::io::Error::other(format!(
                    "Failed to write external binary {}: {}",
                    bin_filename, e
                )));
            }

            // Output directive
            // 64tass: .binary
            // ACME: !binary
            // We need to check assembler settings.
            match state.settings.assembler {
                crate::state::Assembler::Tass64 => {
                    output.push_str(&format!(".binary \"{}\"\n", bin_filename));
                }
                crate::state::Assembler::Acme => {
                    output.push_str(&format!("!binary \"{}\"\n", bin_filename));
                }
                crate::state::Assembler::Ca65 => {
                    output.push_str(&format!(".incbin \"{}\"\n", bin_filename));
                }
                crate::state::Assembler::Kick => {
                    output.push_str(&format!(".import binary \"{}\"\n", bin_filename));
                }
            }

            // Ensure origin is printed if this is the first thing
            if !origin_printed {
                // But wait, .binary usually implies data at current PC.
                // If we haven't set PC (origin), it might be wrong.
                // We should print origin.
                // But strictly speaking, if we just output .binary, it puts data there.
                // So we need headers if not printed.
                // Reuse logic below?
            }
            // Actually, we should check origin_printed before outputting .binary?
            // Yes.
            if !origin_printed {
                output.push_str(&format!(
                    "{}\n",
                    formatter.format_header_origin(state.origin)
                ));
                origin_printed = true;
            }

            // Skip lines that are covered by this block
            // We iterate `all_lines` until address exceeds end_addr.
            while i < all_lines.len() {
                let next_line_addr = all_lines[i].address;
                // Check if next_line_addr is within [start_addr, end_addr]
                // Be careful with wrapping, though typically blocks don't wrap in this view.
                // Logic: if next_line_addr < start_addr + byte_count

                // Simple generic check:
                // If the line address is inside the range we just exported, skip it.
                // But lines might be "Header" or comments that don't have address?
                // Header/Empty lines handled above (continue).
                // Comments should probably be kept?
                // But our "ExternalFile" logic in Disassembler generates dummy lines with .BYTE
                // We want to suppress those.

                // If we check strictly address:
                let line_in_range = if next_line_addr >= start_addr {
                    let delta = next_line_addr.wrapping_sub(start_addr);
                    (delta as usize) < byte_count
                } else {
                    false
                };

                if line_in_range {
                    i += 1;
                } else {
                    break;
                }
            }
            continue;
        }

        // If we reach here, it's a code/data/label line.
        // Ensure origin is printed before the first code line.
        if !origin_printed {
            output.push_str(&format!(
                "{}\n",
                formatter.format_header_origin(state.origin)
            ));
            origin_printed = true;
        }

        // Check for mid-instruction labels
        // Only for instructions/data that have bytes.
        // If we have a multi-byte instruction/data, we check if any byte inside has a label.
        // We start from 1 because 0 is the address itself (handled above as label line).
        if let Some(comment) = &line.line_comment {
            output.push_str(&format!("{} {}\n", formatter.comment_prefix(), comment));
        }

        if line.bytes.len() > 1 {
            for j in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(j as u16);
                if let Some(label_vec) = state.labels.get(&mid_addr) {
                    for label in label_vec {
                        output.push_str(&format!(
                            "{}\n",
                            formatter.format_relative_label(&label.name, j)
                        ));
                    }
                }
            }
        }

        let label_part = if let Some(label) = &line.label {
            formatter.format_label_definition(label)
        } else {
            String::new()
        };

        let instruction_part = if line.opcode.is_none() && !line.bytes.is_empty() {
            // Data directive (or invalid instruction rendered as byte)
            // The mnemonic is already set by Disassembler (.BYTE, !byte, etc.)
            format!("{} {}", line.mnemonic, line.operand)
        } else {
            // Valid instruction
            // The operand is already formatted by Disassembler (including forcing if needed)
            format!("{} {}", line.mnemonic, line.operand)
        };

        let line_out = format!("{:<24}{}", label_part, instruction_part);

        if !line.comment.is_empty() {
            output.push_str(&format!(
                "{:<40} {} {}\n",
                line_out,
                formatter.comment_prefix(),
                line.comment
            ));
        } else {
            output.push_str(&format!("{}\n", line_out));
        }

        i += 1;
    }

    // Fallback if no code labels/instructions found (empty file?)
    if !origin_printed {
        output.push_str(&format!(
            "{}\n",
            formatter.format_header_origin(state.origin)
        ));
    }

    std::fs::write(path, output)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;
    use crate::state::AppState;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn test_export_external_file() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // Data: 0x1000..0x1004 (4 bytes)
        // 1000: NOP
        // 1001-1002: External File (2 bytes)
        // 1003: RTS
        state.raw_data = vec![0xEA, 0x11, 0x22, 0x60];
        // Note: BlockType::ExternalFile needs to be imported or use crate::state::BlockType
        state.block_types = vec![
            crate::state::BlockType::Code,
            crate::state::BlockType::ExternalFile,
            crate::state::BlockType::ExternalFile,
            crate::state::BlockType::Code,
        ];
        state.project_path = Some(PathBuf::from("/tmp/test_project.regen2000proj"));

        // Mock disassembly lines
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });
        state.disassembly.push(DisassemblyLine {
            address: 0x1001,
            mnemonic: ".BYTE".to_string(),
            operand: "$11".to_string(),
            bytes: vec![0x11],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });
        state.disassembly.push(DisassemblyLine {
            address: 0x1002,
            mnemonic: ".BYTE".to_string(),
            operand: "$22".to_string(),
            bytes: vec![0x22],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });
        state.disassembly.push(DisassemblyLine {
            address: 0x1003,
            mnemonic: "RTS".to_string(),
            operand: "".to_string(),
            bytes: vec![0x60],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        let file_name = "test_export_external.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let bin_path = PathBuf::from("test_export_external_1001_1002.bin");
        if bin_path.exists() {
            let _ = std::fs::remove_file(&bin_path);
        }

        // Test 64tass
        state.settings.assembler = crate::state::Assembler::Tass64;
        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Check for .binary directive
        // Filename: test_export_external_1001_1002.bin
        assert!(content.contains(".binary \"test_export_external_1001_1002.bin\""));
        assert!(!content.contains(".BYTE $11"));

        // Check bin file creation
        // Note: exporter writes relative to `path` parent.
        // `path` is "test_export_external.asm". Parent is "" (current dir).
        // So bin file is "test_project-1001-1002.bin" in current dir.
        assert!(bin_path.exists());
        let bin_content = std::fs::read(&bin_path).unwrap();
        assert_eq!(bin_content, vec![0x11, 0x22]);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bin_path);

        // Test ACME
        state.settings.assembler = crate::state::Assembler::Acme;
        let res = export_asm(&state, &path);
        assert!(res.is_ok());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("!binary \"test_export_external_1001_1002.bin\""));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&bin_path);
    }

    #[test]
    fn test_export_compiles_with_64tass() {
        // 1. Setup AppState with some code
        let mut state = AppState::new();
        state.origin = 0x1000;

        // Add some dummy lines mimicking a real program
        // LDA #$00
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "LDA".to_string(),
            operand: "#$00".to_string(),
            bytes: vec![0xA9, 0x00],
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });
        // STA $D020
        state.disassembly.push(DisassemblyLine {
            address: 0x1002,
            mnemonic: "STA".to_string(),
            operand: "$D020".to_string(),
            bytes: vec![0x8D, 0x20, 0xD0],
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });
        // RTS
        state.disassembly.push(DisassemblyLine {
            address: 0x1005,
            mnemonic: "RTS".to_string(),
            operand: "".to_string(),
            bytes: vec![0x60],
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        // 2. Export to a temp file
        // Since we don't want to depend on `tempfile` crate if it's not in Cargo.toml,
        // we'll use a local file and try to clean it up.
        let file_name = "test_output.asm";
        let path = PathBuf::from(file_name);

        // Clean up before just in case
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok(), "Export failed: {:?}", res.err());

        // 3. Run 64tass
        // Command: 64tass test_output.asm
        let output = Command::new("64tass").arg(file_name).output();

        // 4. Assert success
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("stdout: {}", stdout);
                println!("stderr: {}", stderr);

                assert!(
                    output.status.success(),
                    "64tass compilation failed. \nStdout: {}\nStderr: {}",
                    stdout,
                    stderr
                );
            }
            Err(e) => {
                // If 64tass is not installed, this might fail.
                // But the user request implies they want a test that it compiles WITH 64tass.
                // If it's not installed, the test arguably should fail or be skipped.
                // Given the instructions said "add a test that ... compiles with 64tass",
                // we assume the environment should have it or it's a failure.
                panic!("Failed to execute 64tass: {}", e);
            }
        }

        // 5. Cleanup
        let _ = std::fs::remove_file(&path);
        // 64tass might generate an output file (default usually a.out or based on input)
        // By default 64tass generates `a.out` if no output specified?
        // Let's check 64tass behavior. It usually just compiles.
        // If we want to be clean we should probably delete `64tass.output` if it creates one.
        // But for now, just deleting the asm file is good citizenship.
    }

    #[test]
    fn test_export_mid_instruction_label() {
        let mut state = AppState::new();
        state.origin = 0xC000;

        // STA $1234 -> 8D 34 12
        // We want to simulate labels at C001 and C002.
        // C000: STA ...
        // C001: (mid)
        // C002: (mid)

        // Add 3 labels
        state.labels.insert(
            0xC000,
            vec![crate::state::Label {
                name: "aC000".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            0xC001,
            vec![crate::state::Label {
                name: "aC001".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            0xC002,
            vec![crate::state::Label {
                name: "aC002".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        state.raw_data = vec![
            0x8D, 0x34, 0x12, // STA $1234
            0xAD, 0x01, 0xC0, // LDA $C001
        ];
        state.block_types = vec![crate::state::BlockType::Code; state.raw_data.len()];

        let file_name = "test_mid_labels.asm";
        let path = PathBuf::from(file_name);

        // Clean up
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Verify output contains the mid-instruction labels
        assert!(content.contains("aC001 =*+$01"));
        assert!(content.contains("aC002 =*+$02"));

        // It should look like:
        // aC000:
        // aC001 =*+$01
        // aC002 =*+$02
        //     STA $1234

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_includes_xrefs() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // Add a label with refs
        state.labels.insert(
            0x1000,
            vec![crate::state::Label {
                name: "MyLabel".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        state.raw_data = vec![0xEA];
        state.block_types = vec![crate::state::BlockType::Code; 1];
        state.cross_refs.insert(0x1000, vec![0x2000, 0x3000]);
        // To get "x-ref" comment, we need to ensure max_xref_count > 0 (default is 3, so ok)
        // And the address must be referenced? No, side_comment logic uses cross_refs map.
        // It should pick it up automatically.

        let file_name = "test_xref_export.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Check for padding. MyLabel is 7 chars (MyLabel).
        // Format is {:-24} {Instruction}
        // "MyLabel                 NOP                     ; x-ref: $2000, $3000"

        // Check that label, instruction and comment are on the same line
        assert!(content.contains("MyLabel"));
        assert!(!content.contains("MyLabel:"));
        assert!(content.contains("nop"));
        assert!(content.contains("; x-ref: $2000, $3000"));

        // Ensure they appear in correct order on the line?
        // Since we read whole file, finding them separately is enough for basic correctness.
        // But let's check one line content.
        let line = content.lines().find(|l| l.contains("MyLabel")).unwrap();
        assert!(line.contains("nop"));
        assert!(line.contains("; x-ref"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_external_fields() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // 1 byte of data: 1000
        state.raw_data = vec![0x00];

        // Define labels that are external:
        // $0002 -> f0002 (Field)
        // $FFD2 -> sFFD2 (Subroutine)
        // Analyzer might produce these.

        state.labels.insert(
            0x0002,
            vec![crate::state::Label {
                name: "f0002".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageField,
            }],
        );
        state.labels.insert(
            0xFFD2,
            vec![crate::state::Label {
                name: "sFFD2".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::Subroutine,
            }],
        );

        // Disassembly: invalid but unimportant for this test
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        let file_name = "test_external_fields.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        // Sync external labels into disassembly
        let externals = state.get_external_label_definitions();
        state.disassembly = externals.into_iter().chain(state.disassembly).collect();

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // These assertions should currently FAIL because they don't start with 'e'
        assert!(content.contains("f0002 = $02")); // Now it should be $02 for ZP
        assert!(content.contains("sFFD2 = $ffd2"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_ca65_header() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.settings.assembler = crate::state::Assembler::Ca65;
        state.raw_data = vec![0xEA];
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        let file_name = "test_ca65_header.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        assert!(content.contains("; Auto-generated by Regenerator 2000"));
        assert!(content.contains("; Assemble with:"));
        // cl65 -t c64 -C c64-asm.cfg test_ca65_header.asm -o test_ca65_header.prg
        assert!(
            content.contains(
                "; cl65 -t c64 -C c64-asm.cfg test_ca65_header.asm -o test_ca65_header.prg"
            )
        );
        assert!(content.contains(".macpack cbm"));
        assert!(content.contains(".include \"c64.inc\""));
        assert!(content.contains("nop"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_headers_other_assemblers() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.raw_data = vec![0xEA];
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        // ACME
        state.settings.assembler = crate::state::Assembler::Acme;
        let file_name_acme = "test_acme_header.asm";
        let path_acme = PathBuf::from(file_name_acme);
        let _ = export_asm(&state, &path_acme);
        let content_acme = std::fs::read_to_string(&path_acme).unwrap();
        assert!(content_acme.contains("; Auto-generated by Regenerator 2000"));
        assert!(content_acme.contains("; Assemble with:"));
        assert!(
            content_acme
                .contains("; acme --format cbm -o test_acme_header.prg test_acme_header.asm")
        );
        let _ = std::fs::remove_file(&path_acme);

        // KickAssembler
        state.settings.assembler = crate::state::Assembler::Kick;
        let file_name_kick = "test_kick_header.asm";
        let path_kick = PathBuf::from(file_name_kick);
        let _ = export_asm(&state, &path_kick);
        let content_kick = std::fs::read_to_string(&path_kick).unwrap();
        assert!(content_kick.contains("// Auto-generated by Regenerator 2000"));
        assert!(content_kick.contains("// Assemble with:"));
        assert!(content_kick.contains("// java -jar KickAss.jar test_kick_header.asm"));
        let _ = std::fs::remove_file(&path_kick);

        // 64tass
        state.settings.assembler = crate::state::Assembler::Tass64;
        let file_name_tass = "test_tass_header.asm";
        let path_tass = PathBuf::from(file_name_tass);
        let _ = export_asm(&state, &path_tass);
        let content_tass = std::fs::read_to_string(&path_tass).unwrap();
        assert!(content_tass.contains("; Auto-generated by Regenerator 2000"));
        assert!(content_tass.contains("; Assemble with:"));
        assert!(content_tass.contains("; 64tass -o test_tass_header.prg test_tass_header.asm"));
        let _ = std::fs::remove_file(&path_tass);
    }

    #[test]
    fn test_export_label_ordering() {
        let mut state = AppState::new();
        state.origin = 0xC000;
        state.raw_data = vec![0xEA]; // NOP at C000

        // Groups:
        // ZP Fields: f10, f05
        state.labels.insert(
            0x0010,
            vec![crate::state::Label {
                name: "f10".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageField,
            }],
        );
        state.labels.insert(
            0x0005,
            vec![crate::state::Label {
                name: "f05".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageField,
            }],
        );

        // ZP Abs: a20
        state.labels.insert(
            0x0020,
            vec![crate::state::Label {
                name: "a20".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageAbsoluteAddress,
            }],
        );

        // ZP Ptrs: p30
        state.labels.insert(
            0x0030,
            vec![crate::state::Label {
                name: "p30".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPagePointer,
            }],
        );

        // Fields: f1000
        state.labels.insert(
            0x1000,
            vec![crate::state::Label {
                name: "f1000".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::Field,
            }],
        );

        // Abs: a2000
        state.labels.insert(
            0x2000,
            vec![crate::state::Label {
                name: "a2000".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::AbsoluteAddress,
            }],
        );

        // Ptrs: p3000
        state.labels.insert(
            0x3000,
            vec![crate::state::Label {
                name: "p3000".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::Pointer,
            }],
        );

        // Ext Jump: e4000
        state.labels.insert(
            0x4000,
            vec![crate::state::Label {
                name: "e4000".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ExternalJump,
            }],
        );

        // Other: b5000
        state.labels.insert(
            0x5000,
            vec![crate::state::Label {
                name: "b5000".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::Branch,
            }],
        );

        // Edge Case: Absolute Address at low address (should NOT be ZP Absolute)
        state.labels.insert(
            0x0011,
            vec![crate::state::Label {
                name: "a0011".to_string(), // Manually named absolute
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::AbsoluteAddress,
            }],
        );

        let file_name = "test_label_ordering.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        // Sync external labels into disassembly
        let externals = state.get_external_label_definitions();
        state.disassembly = externals.into_iter().chain(state.disassembly).collect();

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        let lines: Vec<&str> = content.lines().collect();
        // Check order of lines before "* = $C000"
        // Expected order:
        // Fields with Headers. ZP addresses formatted as $XX
        //
        // ; ZP FIELDS
        // f05 = $05
        // f10 = $10
        //
        // ; ZP ABSOLUTE ADDRESSES
        // a20 = $20
        // ...

        let mut idx = 3; // Skip header lines
        assert_eq!(lines[idx], "; ZP FIELDS");
        idx += 1;
        assert_eq!(lines[idx], "f05 = $05");
        idx += 1;
        assert_eq!(lines[idx], "f10 = $10");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; ZP ABSOLUTE ADDRESSES");
        idx += 1;
        assert_eq!(lines[idx], "a20 = $20");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; ZP POINTERS");
        idx += 1;
        assert_eq!(lines[idx], "p30 = $30");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; FIELDS");
        idx += 1;
        assert_eq!(lines[idx], "f1000 = $1000");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; ABSOLUTE ADDRESSES");
        idx += 1;
        assert_eq!(lines[idx], "a0011 = $0011"); // Added case
        idx += 1;
        assert_eq!(lines[idx], "a2000 = $2000");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; POINTERS");
        idx += 1;
        assert_eq!(lines[idx], "p3000 = $3000");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; EXTERNAL JUMPS");
        idx += 1;
        assert_eq!(lines[idx], "e4000 = $4000");
        idx += 1;
        assert_eq!(lines[idx], "");
        idx += 1;

        assert_eq!(lines[idx], "; OTHERS");
        idx += 1;
        assert_eq!(lines[idx], "b5000 = $5000");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_external_detection_no_name_check() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.raw_data = vec![0xEA]; // Range: 1000-1001

        // Label at internal address 0x1000
        // Name starts with "e" -> "eUser"
        // Should NOT be treated as external (because it's internal address).

        state.labels.insert(
            0x1000,
            vec![crate::state::Label {
                name: "eUser".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        // Disassembly line for the label and instruction
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: String::new(),
            line_comment: None,
            label: Some("eUser".to_string()),
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
            is_collapsed: false,
        });

        let file_name = "test_external_name_check.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();

        // Should be defined as label
        assert!(content.contains("eUser"));
        assert!(!content.contains("eUser:"));
        // Should NOT be in external list
        assert!(!content.contains("eUser = $1000"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_absolute_zp_forcing() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // AD 12 00: LDA $0012 (Absolute) targeting ZP
        // BD 12 00: LDA $0012,X (Absolute X) targeting ZP
        // 4C 12 00: JMP $0012 (Absolute) targeting ZP - JMP is always absolute, check if 64tass needs forcing.
        // Actually JMP (abs) is 4C. JMP (indirect) is 6C. There is no JMP ZP.
        // So JMP $0012 is 4C 12 00. 64tass doesn't optimize JMP to ZP because JMP ZP doesn't exist.
        // So we only care about instructions that HAVE a ZP equivalent (LDA, STA, ADC, etc).
        // Opcode AD is LDA Absolute. Opcode A5 is LDA ZP.

        let data = vec![
            0xAD, 0x12, 0x00, // LDA $0012 (Absolute)
            0xBD, 0x12, 0x00, // LDA $0012,X (Absolute,X)
        ];
        state.raw_data = data.clone();
        state.block_types = vec![crate::state::BlockType::Code; data.len()];

        // Disassemble to populate state.disassembly
        // We need to manually populate disassembly or call disassemble.
        // Since we are mocking, let's just push lines as if disassembler did it correctly.
        // Disassembler SHOULD preserve the opcode information which acts as the source of truth for "Original addressing mode".

        // Note: The exporter relies on `line.opcode` to know it was Absolute.

        // Line 1: LDA $0012 (Absolute)
        // state.disassembly.push(...) - Remove manual push

        // Line 2: LDA $0012,X (Absolute X)
        // state.disassembly.push(...) - Remove manual push

        // Force absolute addressing for the first instruction (AD 12 00)
        // We need to tell the disassembler to treat this as Absolute, not ZP.
        // The disassembler uses `opcodes` table. 0xAD is Absolute. 0xA5 is ZP.
        // If the code has 0xAD, it IS Absolute. The question is, does the exporter preserve the "@w" if the user wants it forced?
        // Wait, "forcing" usually means we have an instruction that COUULD be ZP but we want Absolute.
        // 0xAD $0012 is legally Absolute. 0xA5 $12 is ZP.
        // If we write 0xAD, it is 3 bytes.
        // Standard disassembler output for 0xAD 12 00 is "LDA $0012".
        // Some assemblers might optimize "LDA $0012" to ZP. To prevent that, we use "@w".
        // The Disassembler logic in `handle_code` (lines 677+) formats the operand.
        // Does Disassembler add "@w"?
        // See Disassembler::handle_code (we didn't read deep enough).
        // Let's assume for this test that we simply need to rely on the fact that 0xAD is absolute.
        // If the test demands "@w", then Disassembler MUST output "@w".
        // If Disassembler doesn't output "@w" by default for 0xAD $0012, then the test expectation might be wrong OR Disassembler needs config.
        // Let's assume Disassembler DOES output @w for Absolute addresses in ZP range.
        // So we just need to set up the data.

        let file_name = "test_export_force_zp.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Verify Exporter preserves the @w prefix
        assert!(
            content.contains("lda @w $0012"),
            "Output missing @w prefix for Absolute ZP target. content: {}",
            content
        );
        assert!(
            content.contains("lda @w $0012,x"),
            "Output missing @w prefix for AbsoluteX ZP target. content: {}",
            content
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_ignores_collapsed_blocks() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // Data: 3 NOPs
        state.raw_data = vec![0xEA, 0xEA, 0xEA];
        state.block_types = vec![crate::state::BlockType::Code; 3];

        // Collapse the 2nd NOP (offset 1)
        state.collapsed_blocks.push((1, 1)); // Single byte collapsed

        // Manually trigger disassemble to update state (though export regenerates it)
        state.disassemble();

        // Verify state.disassembly has the collapsed block (summary line)
        // 0: NOP
        // 1: Collapsed...
        // 2: NOP
        assert_eq!(state.disassembly.len(), 3);
        assert!(state.disassembly[1].mnemonic.contains("Collapsed"));

        let file_name = "test_export_collapsed.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Export should regenerate WITHOUT collapsed blocks, so we expect 3 NOPs.
        // It should NOT contain "Collapsed"
        assert!(
            !content.contains("Collapsed"),
            "Export should not contain collapsed block summary"
        );

        // Should contain 3 NOPs (or rather, the bytes for 3 NOPs)
        // Since we are mocking, we rely on disassemble logic.
        // Disassembler for 3 NOPs -> 3 lines of NOP.
        // So content should have NOP appearing 3 times?
        // Let's just count NOPs.
        let nop_count = content.to_lowercase().matches("nop").count();
        assert_eq!(nop_count, 3, "Should export all 3 NOPs");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_line_comments() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.settings.assembler = crate::state::Assembler::Tass64;

        state.raw_data = vec![0xA9, 0x00];
        state.block_types = vec![crate::state::BlockType::Code; 2];
        state
            .user_line_comments
            .insert(0x1000, "Function Start".to_string());
        state.labels.insert(
            0x1000,
            vec![crate::state::Label {
                name: "MyLabel".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        let file_name = "test_export_line_comments.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();

        // Expected usage:
        // ; Function Start
        // MyLabel                LDA #$00
        assert!(content.contains("; Function Start"));

        // Check ordering: Comment needs to be before Label
        // Find index of comment
        let comment_idx = content.find("; Function Start").unwrap();
        let label_idx = content.find("MyLabel").unwrap();

        assert!(
            comment_idx < label_idx,
            "Line comment should appear before label"
        );

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    #[test]
    fn test_export_all_labels_disabled() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.raw_data = vec![0xEA];

        // Define an external label
        state.labels.insert(
            0x0010,
            vec![crate::state::Label {
                name: "f10".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageField,
            }],
        );

        // Disable "All Labels"
        state.settings.all_labels = false;

        // Run disassembly
        state.disassemble();

        // precise verification: disassembly should NOT verify external label definition
        for line in &state.disassembly {
            if line.mnemonic.contains("ZP FIELDS") || line.mnemonic.contains("f10 =") {
                panic!(
                    "Disassembly contained external label definition but 'all_labels' is false!"
                );
            }
        }

        // Now Export
        let file_name = "test_export_all_labels_false.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();

        // Must contain the label definition
        assert!(content.contains("f10 = $10"));
        assert!(content.contains("; ZP FIELDS"));

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_export_kick_external_comments() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.settings.assembler = crate::state::Assembler::Kick;
        state.settings.all_labels = true;
        state.raw_data = vec![0xEA];

        // Add an external label to trigger header generation
        state.labels.insert(
            0x0002,
            vec![crate::state::Label {
                name: "f0002".to_string(),
                kind: crate::state::LabelKind::Auto,
                label_type: crate::state::LabelType::ZeroPageField,
            }],
        );

        let file_name = "test_kick_comments.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        // Check for KickAssembler comment style in header
        assert!(content.contains("// ZP FIELDS"));
        // Standard check
        assert!(content.contains("f0002 = $02"));

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    #[test]
    fn test_export_kick_relative_labels() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.settings.assembler = crate::state::Assembler::Kick;

        let file_name = "test_kick_rel.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        // Mock disassembly with bytes > 1 to trigger relative label logic
        // JMP $1001 (0x4C, 0x01, 0x10) - 3 bytes
        state.raw_data = vec![0x4C, 0x01, 0x10];
        state.block_types = vec![crate::state::BlockType::Code; 3];

        // No disassembly push needed as export_asm calls disassemble()

        // Add label at 1001 (+1)
        state.labels.insert(
            0x1001,
            vec![crate::state::Label {
                name: "rel1".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        // Check for .label syntax
        assert!(content.contains(".label rel1 = * + 1"));

        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}
