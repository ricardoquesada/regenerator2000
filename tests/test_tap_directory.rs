use std::fs;

#[test]
fn test_burnin_rubber_tap_directory() {
    let data = fs::read("tests/6502/Burnin'_Rubber.tap").expect("Failed to read test file");

    let result = regenerator2000::parser::tap::parse_tap_directory(&data);

    match result {
        Ok(programs) => {
            println!("Found {} program(s) in TAP file:", programs.len());
            for (idx, program) in programs.iter().enumerate() {
                println!(
                    "  Program {}: ${:04X}-${:04X} ({} bytes)",
                    idx + 1,
                    program.start_addr,
                    program.end_addr,
                    (program.end_addr.saturating_sub(program.start_addr) as usize) + 1
                );
            }

            // Verify we found at least one program
            assert!(!programs.is_empty(), "Should find at least one program");

            // Test extracting the first program
            let first_program = &programs[0];
            match regenerator2000::parser::tap::extract_tap_program(&data, first_program) {
                Ok((addr, prog_data)) => {
                    println!("\nExtracted Program 1:");
                    println!("  Load address: 0x{:04x}", addr);
                    println!("  Data length: {} bytes", prog_data.len());
                    assert!(!prog_data.is_empty(), "Program data should not be empty");
                    assert_eq!(addr, first_program.start_addr, "Load address should match");
                }
                Err(e) => {
                    panic!("Failed to extract first program: {}", e);
                }
            }
        }
        Err(e) => {
            panic!("Failed to parse TAP directory: {}", e);
        }
    }
}
