use anyhow::{Result, anyhow};

pub struct PrgData {
    pub origin: u16,
    pub raw_data: Vec<u8>,
    pub suggested_platform: Option<String>,
    pub suggested_entry_point: Option<u16>,
}

pub fn parse_prg(data: &[u8]) -> Result<PrgData> {
    if data.len() < 2 {
        return Err(anyhow!("PRG file too short"));
    }

    let origin = u16::from(data[1]) << 8 | u16::from(data[0]);
    let raw_data = data[2..].to_vec();

    let suggested_platform = match origin {
        0x0801 => Some("Commodore 64".to_string()),
        0x1C01 => Some("Commodore 128".to_string()),
        0x1001 => Some("Commodore Plus4".to_string()),
        0x0401 => Some("Commodore PET 4.0".to_string()),
        0x1201 => Some("Commodore VIC-20".to_string()),
        _ => None,
    };

    let mut suggested_entry_point = None;

    // Try to find SYS address if it looks like a BASIC program
    if suggested_platform.is_some() && data.len() >= 9 {
        // data[2..] is BASIC program
        // 2 bytes pointer + 2 bytes line number + 1 byte token
        // So token should be at data[6]
        if data[6] == 0x9E {
            let mut addr_str = String::new();
            let mut parsing_digits = false;
            for &b in &data[7..] {
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
            }
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
        assert_eq!(prg.suggested_platform, Some("Commodore 64".to_string()));
        assert_eq!(prg.suggested_entry_point, Some(2061));
    }

    #[test]
    fn test_parse_prg_too_short() {
        let data = vec![0x01];
        let result = parse_prg(&data);
        assert!(result.is_err());
    }
}
