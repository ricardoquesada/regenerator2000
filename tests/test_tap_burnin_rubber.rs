use std::fs;

#[test]
fn test_burnin_rubber_tap() {
    let data = fs::read("tests/6502/Burnin'_Rubber.tap").expect("Failed to read test file");

    let result = regenerator2000::parser::tap::parse_tap(&data);

    match result {
        Ok((addr, prog_data)) => {
            println!("Successfully parsed TAP file!");
            println!("  Start address: 0x{:04x}", addr);
            println!("  Data length: {} bytes", prog_data.len());

            // Print first 64 bytes to check if it looks like valid data
            println!("  First 64 bytes:");
            for (i, chunk) in prog_data.iter().take(64).enumerate() {
                if i % 16 == 0 {
                    print!("\n    {:04x}: ", i);
                }
                print!("{:02x} ", chunk);
            }
            println!("\n");

            // Check for BASIC stub indicators
            if prog_data.len() >= 2 {
                let link = u16::from_le_bytes([prog_data[0], prog_data[1]]);
                println!("  BASIC link address: 0x{:04x}", link);

                // Look for SYS token
                for (i, &byte) in prog_data.iter().enumerate().take(30) {
                    if byte == 0x9E {
                        println!("  Found SYS token at offset {}", i);
                        break;
                    }
                }
            }

            // Basic validation
            assert!(!prog_data.is_empty(), "Program data should not be empty");

            // Check it's not all the same byte
            let first_byte = prog_data[0];
            let all_same = prog_data.iter().take(100).all(|&b| b == first_byte);
            assert!(
                !all_same,
                "Data appears to be all the same byte - likely decoding error"
            );
        }
        Err(e) => {
            panic!("Failed to parse Burnin' Rubber TAP: {}", e);
        }
    }
}
