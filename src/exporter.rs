use crate::state::AppState;
use std::path::PathBuf;

pub fn export_asm(state: &AppState, path: &PathBuf) -> std::io::Result<()> {
    let mut output = String::new();

    // 1. Declare external addresses
    // Find all labels starting with 'e'
    let mut externals: Vec<(u16, &String)> = state
        .labels
        .iter()
        .filter(|(_, name)| name.starts_with('e'))
        .map(|(k, v)| (*k, v))
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
            output.push_str(&format!("{}\n", line.mnemonic));
            continue;
        }

        if line.mnemonic == ".BYTE" || line.mnemonic == ".WORD" {
            output.push_str(&format!("    {} {}\n", line.mnemonic, line.operand));
        } else {
            // Opcode
            output.push_str(&format!("    {} {}\n", line.mnemonic, line.operand));
        }
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
            opcode: None,
        });
        // STA $D020
        state.disassembly.push(DisassemblyLine {
            address: 0x1002,
            mnemonic: "STA".to_string(),
            operand: "$D020".to_string(),
            bytes: vec![0x8D, 0x20, 0xD0],
            comment: String::new(),
            opcode: None,
        });
        // RTS
        state.disassembly.push(DisassemblyLine {
            address: 0x1005,
            mnemonic: "RTS".to_string(),
            operand: "".to_string(),
            bytes: vec![0x60],
            comment: String::new(),
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
}
