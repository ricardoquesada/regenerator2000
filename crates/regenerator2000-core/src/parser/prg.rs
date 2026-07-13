use crate::state::types::System;
use anyhow::{Result, anyhow};

pub struct PrgData {
    pub origin: u16,
    pub raw_data: Vec<u8>,
    pub suggested_system: Option<System>,
    pub suggested_entry_point: Option<u16>,
}

/// Parses a PRG file content to extract data and platform info.
///
/// # Errors
/// Returns an error if the data is too short to contain a valid PRG header.
pub fn parse_prg(data: &[u8]) -> Result<PrgData> {
    if data.len() < 2 {
        return Err(anyhow!("PRG file too short"));
    }

    let origin = u16::from(data[1]) << 8 | u16::from(data[0]);
    let raw_data = data[2..].to_vec();

    let suggested_system = match origin {
        0x0801 => Some(System::new(System::C64)),
        0x1C01 => Some(System::new(System::C128)),
        0x1001 => Some(System::new(System::PLUS4)),
        0x0401 => Some(System::new(System::PET)),
        0x1201 => Some(System::new(System::VIC20)),
        _ => None,
    };

    let suggested_entry_point =
        crate::parser::basic::find_sys_address(data, 2, Some(origin), suggested_system.as_ref());

    Ok(PrgData {
        origin,
        raw_data,
        suggested_system,
        suggested_entry_point,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prg_suggests_platform() {
        let data = vec![
            0x01, 0x08, // Load address $0801
            0x0B, 0x08, // Next line pointer
            0x0A, 0x00, // Line number 10
            0x9E, // SYS token
            0x32, 0x30, 0x36, 0x31, // "2061"
            0x00, // Terminator
        ];

        let result = parse_prg(&data);
        assert!(result.is_ok());
        let prg = result.unwrap();
        assert_eq!(prg.origin, 0x0801);
        assert_eq!(prg.suggested_system, Some(System::new(System::C64)));
        assert_eq!(prg.suggested_entry_point, Some(2061));
    }

    #[test]
    fn test_parse_prg_suggests_platform_multi_line() {
        let data = vec![
            0x01, 0x08, // Load address $0801
            0x07, 0x08, // Next line pointer (Offset 2, points to $0807 = offset 8)
            0x0A, 0x00, // Line number 10
            0x99, // PRINT token
            0x00, // Terminator
            // Line 2 starts at offset 8 (which is $0807)
            0x11, 0x08, // Next line pointer (Offset 8, points to $0811 = offset 18)
            0x14, 0x00, // Line number 20
            0x9E, // SYS token
            0x31, 0x35, 0x33, 0x36, // "1536"
            0x00, // Terminator
            // Line 3 starts at offset 18 (which is $0811)
            0x00, 0x00, // End of program
        ];

        let result = parse_prg(&data);
        assert!(result.is_ok());
        let prg = result.unwrap();
        assert_eq!(prg.suggested_entry_point, Some(1536));
    }

    #[test]
    fn test_parse_prg_too_short() {
        let data = vec![0x01];
        let result = parse_prg(&data);
        assert!(result.is_err());
    }
}
