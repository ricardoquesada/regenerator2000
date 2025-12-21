use crate::state::AppState;
use std::path::PathBuf;

pub fn export_asm(state: &AppState, path: &PathBuf) -> std::io::Result<()> {
    let mut output = String::new();

    // 1. Declare external addresses
    // Find all labels starting with 'e' OR that are technically external addresses
    let data_len = state.raw_data.len();
    let mut externals: Vec<(u16, &String)> = state
        .labels
        .iter()
        .filter(|(addr, label)| {
            label.name.starts_with('e') || is_external(**addr, state.origin, data_len)
        })
        .map(|(k, v)| (*k, &v.name))
        .collect();
    externals.sort_by_key(|(k, _)| *k);

    for (addr, name) in externals {
        output.push_str(&format!("{} = ${:04X}\n", name, addr));
    }
    output.push('\n');

    output.push_str(&format!("    * = ${:04X}\n", state.origin));

    for line in &state.disassembly {
        // Label line
        if line.mnemonic.ends_with(':') {
            if !line.comment.is_empty() {
                output.push_str(&format!("{:<40} ; {}\n", line.mnemonic, line.comment));
            } else {
                output.push_str(&format!("{}\n", line.mnemonic));
            }
            continue;
        }

        // Check for mid-instruction labels
        // Only for instructions/data that have bytes.
        // If we have a multi-byte instruction/data, we check if any byte inside has a label.
        // We start from 1 because 0 is the address itself (handled above as label line).
        if line.bytes.len() > 1 {
            for i in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(i as u16);
                if let Some(label) = state.labels.get(&mid_addr) {
                    output.push_str(&format!("{} = * + {}\n", label.name, i));
                }
            }
        }

        let line_out = if line.mnemonic == ".BYTE" || line.mnemonic == ".WORD" {
            format!("    {} {}", line.mnemonic, line.operand)
        } else {
            format!("    {} {}", line.mnemonic, line.operand)
        };

        if !line.comment.is_empty() {
            output.push_str(&format!("{:<40} ; {}\n", line_out, line.comment));
        } else {
            output.push_str(&format!("{}\n", line_out));
        }
    }

    std::fs::write(path, output)
}

fn is_external(addr: u16, origin: u16, len: usize) -> bool {
    let end = origin.wrapping_add(len as u16);
    if origin < end {
        addr < origin || addr >= end
    } else {
        // Wrap around case
        !(addr >= origin || addr < end)
    }
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
            label: None,
            opcode: None,
        });
        // STA $D020
        state.disassembly.push(DisassemblyLine {
            address: 0x1002,
            mnemonic: "STA".to_string(),
            operand: "$D020".to_string(),
            bytes: vec![0x8D, 0x20, 0xD0],
            comment: String::new(),
            label: None,
            opcode: None,
        });
        // RTS
        state.disassembly.push(DisassemblyLine {
            address: 0x1005,
            mnemonic: "RTS".to_string(),
            operand: "".to_string(),
            bytes: vec![0x60],
            comment: String::new(),
            label: None,
            opcode: None,
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
            crate::state::Label {
                name: "aC000".to_string(),
                kind: crate::state::LabelKind::User,
                names: std::collections::HashMap::new(),
                refs: Vec::new(),
            },
        );
        state.labels.insert(
            0xC001,
            crate::state::Label {
                name: "aC001".to_string(),
                kind: crate::state::LabelKind::User,
                names: std::collections::HashMap::new(),
                refs: Vec::new(),
            },
        );
        state.labels.insert(
            0xC002,
            crate::state::Label {
                name: "aC002".to_string(),
                kind: crate::state::LabelKind::User,
                names: std::collections::HashMap::new(),
                refs: Vec::new(),
            },
        );

        // Disassembly line for the STA instruction
        state.disassembly.push(DisassemblyLine {
            address: 0xC000,
            mnemonic: "aC000:".to_string(), // The label line
            operand: "".to_string(),
            bytes: vec![],
            comment: String::new(),
            label: Some("aC000".to_string()),
            opcode: None,
        });

        state.disassembly.push(DisassemblyLine {
            address: 0xC000,
            mnemonic: "STA".to_string(),
            operand: "$1234".to_string(),
            bytes: vec![0x8D, 0x34, 0x12],
            comment: String::new(),
            label: Some("aC000".to_string()),
            opcode: None,
        });

        // Next instruction using those labels
        // LDA aC001 -> AD 01 C0
        state.disassembly.push(DisassemblyLine {
            address: 0xC003,
            mnemonic: "LDA".to_string(),
            operand: "aC001".to_string(),
            bytes: vec![0xAD, 0x01, 0xC0],
            comment: String::new(),
            label: None,
            opcode: None,
        });

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
        assert!(content.contains("aC001 = * + 1"));
        assert!(content.contains("aC002 = * + 2"));

        // It should look like:
        // aC000:
        // aC001 = * + 1
        // aC002 = * + 2
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
            crate::state::Label {
                name: "MyLabel".to_string(),
                kind: crate::state::LabelKind::User,
                names: std::collections::HashMap::new(),
                refs: vec![0x2000, 0x3000], // Two refs
            },
        );

        // Disassembly line for the label
        // Note: Disassembler creates the comment. Here we manually fake it
        // because we are testing EXPORTER, not disassembler integration here.
        // BUT, real AppState uses disassembler to generate lines.
        // Ideally we should call disassembler logic or manually construct the line AS IF it came from disassembler.
        // Disassembler logic puts "; x-ref: ..." in the comment field.

        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "MyLabel:".to_string(),
            operand: "".to_string(),
            bytes: vec![],
            comment: "x-ref: 2000, 3000".to_string(), // Simulated disassembler output without semicolon
            label: Some("MyLabel".to_string()),
            opcode: None,
        });

        // Instruction at 1000
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: "".to_string(),
            label: Some("MyLabel".to_string()),
            opcode: None,
        });

        let file_name = "test_xref_export.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // Check for padding. MyLabel: is 8 chars.
        // Format is {:-40} ; {comment}
        // "MyLabel:                                 ; x-ref: 2000, 3000"
        // Just checking it contains the aligned semi-colon and content is safer than exact spacing if we calculate wrong.
        // But let's check basic structure.
        assert!(content.contains("MyLabel:"));
        assert!(content.contains("; x-ref: 2000, 3000"));
        // Check for correct separation (at least 20 spaces)
        assert!(content.contains("                    ; x-ref"));

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
            crate::state::Label {
                name: "f0002".to_string(),
                kind: crate::state::LabelKind::Auto,
                names: std::collections::HashMap::new(),
                refs: vec![],
            },
        );
        state.labels.insert(
            0xFFD2,
            crate::state::Label {
                name: "sFFD2".to_string(),
                kind: crate::state::LabelKind::Auto,
                names: std::collections::HashMap::new(),
                refs: vec![],
            },
        );

        // Disassembly: invalid but unimportant for this test
        state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            bytes: vec![0xEA],
            comment: String::new(),
            label: None,
            opcode: None,
        });

        let file_name = "test_external_fields.asm";
        let path = PathBuf::from(file_name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let res = export_asm(&state, &path);
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        println!("Content:\n{}", content);

        // These assertions should currently FAIL because they don't start with 'e'
        assert!(content.contains("f0002 = $0002"));
        assert!(content.contains("sFFD2 = $FFD2"));

        let _ = std::fs::remove_file(&path);
    }
}
