//! Commodore `.prg` binary file parser.

use crate::state::types::System;
use anyhow::{Result, anyhow};

/// Parsed Commodore `.prg` file payload and inferred platform hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrgData {
    /// 16-bit load address read from the first 2 little-endian bytes of the `.prg` file.
    pub origin: u16,
    /// Binary payload following the 2-byte load address header.
    pub raw_data: Vec<u8>,
    /// Target [`System`] inferred from standard Commodore BASIC start addresses.
    pub suggested_system: Option<System>,
    /// Machine-code entry point parsed from a leading BASIC `SYS` stub, if present.
    pub suggested_entry_point: Option<u16>,
}

/// Parses a `.prg` byte slice into its load origin, payload, and platform hints.
///
/// # Errors
///
/// Returns an error if `data` has fewer than 2 bytes.
pub fn parse_prg(data: &[u8]) -> Result<PrgData> {
    if data.len() < 2 {
        return Err(anyhow!("PRG file too short"));
    }

    let origin = u16::from_le_bytes([data[0], data[1]]);
    let raw_data = data[2..].to_vec();

    let suggested_system = match origin {
        0x0801 => Some(System::C64),
        0x1C01 => Some(System::C128),
        0x1001 => Some(System::Plus4),
        0x0401 => Some(System::Pet40),
        0x1201 => Some(System::Vic20),
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

        let prg = parse_prg(&data).expect("Valid PRG must parse");
        assert_eq!(prg.origin, 0x0801);
        assert_eq!(prg.suggested_system, Some(System::new(System::C64)));
        assert_eq!(prg.suggested_entry_point, Some(2061));
    }

    #[test]
    fn test_parse_prg_suggests_platform_multi_line() {
        let data = vec![
            0x01, 0x08, // Load address $0801
            0x07, 0x08, // Next line pointer (offset 2 -> $0807 = offset 8)
            0x0A, 0x00, // Line number 10
            0x99, // PRINT token
            0x00, // Terminator
            // Line 2 starts at offset 8 ($0807)
            0x11, 0x08, // Next line pointer (offset 8 -> $0811 = offset 18)
            0x14, 0x00, // Line number 20
            0x9E, // SYS token
            0x31, 0x35, 0x33, 0x36, // "1536"
            0x00, // Terminator
            // End of program at offset 18 ($0811)
            0x00, 0x00,
        ];

        let prg = parse_prg(&data).expect("Multi-line PRG must parse");
        assert_eq!(prg.suggested_entry_point, Some(1536));
    }

    #[test]
    fn test_parse_prg_too_short() {
        let data = vec![0x01];
        assert!(parse_prg(&data).is_err());
    }
}
