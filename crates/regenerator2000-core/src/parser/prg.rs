use crate::state::types::Platform;
use anyhow::{Result, anyhow};

pub struct PrgData {
    pub origin: u16,
    pub raw_data: Vec<u8>,
    pub suggested_platform: Option<Platform>,
    pub suggested_entry_point: Option<u16>,
}

pub fn parse_prg(data: &[u8]) -> Result<PrgData> {
    if data.len() < 2 {
        return Err(anyhow!("PRG file too short"));
    }

    let origin = u16::from(data[1]) << 8 | u16::from(data[0]);
    let raw_data = data[2..].to_vec();

    let suggested_platform = match origin {
        0x0801 => Some(Platform::new(Platform::C64)),
        0x1C01 => Some(Platform::new(Platform::C128)),
        0x1001 => Some(Platform::new(Platform::PLUS4)),
        0x0401 => Some(Platform::new(Platform::PET)),
        0x1201 => Some(Platform::new(Platform::VIC20)),
        _ => None,
    };

    let mut suggested_entry_point = None;

    // Try to find SYS address if it looks like a BASIC program
    if suggested_platform.is_some() && data.len() >= 7 {
        let mut offset = 2;
        while offset + 4 < data.len() {
            let next_ptr = u16::from(data[offset]) | (u16::from(data[offset + 1]) << 8);
            if next_ptr == 0 {
                break;
            }

            // Calculate the offset of the next line in the file.
            // We assume the pointers in the file are relative to the origin.
            let next_offset = if let Some(off) = (next_ptr as usize).checked_sub(origin as usize) {
                off.saturating_add(2)
            } else {
                break; // Invalid pointer
            };

            if next_offset <= offset + 4 || next_offset > data.len() {
                // Pointer doesn't make sense or goes out of bounds
                break;
            }

            // The line content is from offset + 4 to next_offset - 1 (terminator is at next_offset - 1)
            let line_end = next_offset - 1;

            // Search for SYS token (0x9E)
            for i in offset + 4..line_end {
                if data[i] == 0x9E {
                    let mut addr_str = String::new();
                    let mut parsing_digits = false;
                    for &b in &data[i + 1..line_end] {
                        if b.is_ascii_digit() {
                            addr_str.push(b as char);
                            parsing_digits = true;
                        } else if b == b' ' && !parsing_digits {
                            continue; // skip spaces before digits
                        } else {
                            break;
                        }
                    }
                    if let Ok(sys_addr) = addr_str.parse::<u16>() {
                        suggested_entry_point = Some(sys_addr);
                        break;
                    }
                }
            }

            if suggested_entry_point.is_some() {
                break;
            }

            offset = next_offset;
        }
    }

    Ok(PrgData {
        origin,
        raw_data,
        suggested_platform,
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
        assert_eq!(prg.suggested_platform, Some(Platform::new(Platform::C64)));
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
