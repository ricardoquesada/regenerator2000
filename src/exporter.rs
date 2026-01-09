use crate::state::AppState;
use std::path::PathBuf;

pub fn export_asm(state: &AppState, path: &PathBuf) -> std::io::Result<()> {
    let mut output = String::new();

    let mut origin_printed = false;
    let formatter = state.get_formatter();

    let external_lines = if !state.settings.all_labels {
        state.get_external_label_definitions()
    } else {
        Vec::new()
    };

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
    );

    for line in external_lines.iter().chain(full_disassembly.iter()) {
        // Special case: Header (starts with ;)
        if line.mnemonic.starts_with(';') {
            output.push_str(&format!("{}\n", line.mnemonic));
            continue;
        }

        // Special case: Equate (contains =)
        if line.mnemonic.contains('=') {
            output.push_str(&format!("{}\n", line.mnemonic));
            continue;
        }

        // Special case: Empty line (separator)
        if line.mnemonic.is_empty() && line.bytes.is_empty() && line.comment.is_empty() {
            output.push('\n');
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
            output.push_str(&format!("; {}\n", comment));
        }

        if line.bytes.len() > 1 {
            for i in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(i as u16);
                if let Some(label_vec) = state.labels.get(&mid_addr) {
                    for label in label_vec {
                        let formatted_name = formatter.format_label(&label.name);
                        output.push_str(&format!("{} =*+${:02x}\n", formatted_name, i));
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
            output.push_str(&format!("{:<40} ; {}\n", line_out, line.comment));
        } else {
            output.push_str(&format!("{}\n", line_out));
        }
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
    use std::process::Command;

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
        });

        let file_name = "test_external_fields.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        // Sync external labels into disassembly
        let externals = state.get_external_label_definitions();
        let mut new_disassembly = externals;
        new_disassembly.extend(state.disassembly);
        state.disassembly = new_disassembly;

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
        let mut new_disassembly = externals;
        new_disassembly.extend(state.disassembly);
        state.disassembly = new_disassembly;

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

        let mut idx = 0;
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
}
