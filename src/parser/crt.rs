use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct CrtChip {
    pub load_address: u16,
    pub bank: u16,
    pub chip_type: u16,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CrtHeader {
    pub version: u16,
    pub hardware_type: u16,
    pub exrom: u8,
    pub game: u8,
    pub name: String,
    pub chips: Vec<CrtChip>,
}

impl CrtHeader {
    pub fn get_hardware_name(&self) -> String {
        match self.hardware_type {
            0 => "Normal Cartridge".to_string(),
            1 => "Action Replay".to_string(),
            2 => "KCS Power Cartridge".to_string(),
            3 => "Final Cartridge III".to_string(),
            4 => "Simons' BASIC".to_string(),
            5 => "Ocean (Type 1)".to_string(),
            6 => "Expert Cartridge".to_string(),
            7 => "Final Cartridge I/II".to_string(),
            8 => "Super Games".to_string(),
            9 => "Atomic Power".to_string(),
            10 => "Epyx FastLoad".to_string(),
            12 => "C64GS (System 3)".to_string(),
            13 => "Dinamic".to_string(),
            15 => "C64 Game System".to_string(),
            17 => "Mikro Assembler".to_string(),
            18 => "Final Cartridge Plus".to_string(),
            19 => "Magic Desk".to_string(),
            20 => "Super Snapshot V5".to_string(),
            32 => "EasyFlash".to_string(),
            33 => "GIDE".to_string(),
            43 => "Retro Replay".to_string(),
            _ => format!("Unknown Type ({})", self.hardware_type),
        }
    }
}

pub fn parse_crt_chips(data: &[u8]) -> Result<CrtHeader> {
    if data.len() < 0x40 {
        return Err(anyhow!("File too short to be a valid CRT file"));
    }

    // Check signature "C64 CARTRIDGE   "
    let signature = &data[0..16];
    let expected_sig = b"C64 CARTRIDGE   ";
    if signature != expected_sig {
        return Err(anyhow!("Invalid CRT signature"));
    }

    // Header length is at 0x10-0x13 (Big Endian)
    let header_len = u32::from_be_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]) as usize;

    // Version at 0x14-0x15
    let version = u16::from_be_bytes([data[0x14], data[0x15]]);

    // Hardware Type at 0x16-0x17
    let hardware_type = u16::from_be_bytes([data[0x16], data[0x17]]);

    // EXROM (0x18) and GAME (0x19) line status
    let exrom = data[0x18];
    let game = data[0x19];

    // Name at 0x20-0x3F (32 bytes max, stops at null)
    let name_bytes = &data[0x20..0x40];
    let end_index = name_bytes.iter().position(|&x| x == 0).unwrap_or(32);
    let name = String::from_utf8_lossy(&name_bytes[..end_index]).to_string();

    // Iterate through CHIP packets
    let mut current_offset = header_len;
    let mut chips = Vec::new();

    while current_offset < data.len() {
        if current_offset + 0x10 > data.len() {
            break; // Not enough data for a chip header
        }

        let chip_sig = &data[current_offset..current_offset + 4];
        if chip_sig != b"CHIP" {
            // Stop if we lose sync or find garbage
            break;
        }

        let packet_len = u32::from_be_bytes([
            data[current_offset + 4],
            data[current_offset + 5],
            data[current_offset + 6],
            data[current_offset + 7],
        ]) as usize;

        if packet_len < 0x10 {
            return Err(anyhow!(
                "Invalid CHIP packet length at offset {:x}",
                current_offset
            ));
        }

        // Chip type at 0x08-0x09 (Big Endian)
        let chip_type =
            u16::from_be_bytes([data[current_offset + 0x08], data[current_offset + 0x09]]);

        // Bank number at 0x0A-0x0B (Big Endian)
        let bank = u16::from_be_bytes([data[current_offset + 0x0A], data[current_offset + 0x0B]]);

        // Chip load address at 0x0C-0x0D (Big Endian)
        let load_address =
            u16::from_be_bytes([data[current_offset + 0x0C], data[current_offset + 0x0D]]);

        // ROM size at 0x0E-0x0F (Big Endian)
        let rom_size =
            u16::from_be_bytes([data[current_offset + 0x0E], data[current_offset + 0x0F]]) as usize;

        let rom_data_offset = current_offset + 0x10;
        if rom_data_offset + rom_size > data.len() {
            return Err(anyhow!(
                "CHIP packet data truncated at offset {:x}",
                current_offset
            ));
        }

        let rom_data = data[rom_data_offset..rom_data_offset + rom_size].to_vec();
        chips.push(CrtChip {
            load_address,
            bank,
            chip_type,
            data: rom_data,
        });

        current_offset += packet_len;
    }

    if chips.is_empty() {
        return Err(anyhow!("No valid CHIP packets found"));
    }

    Ok(CrtHeader {
        version,
        hardware_type,
        exrom,
        game,
        name,
        chips,
    })
}

pub fn parse_crt(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    let header = parse_crt_chips(data)?;
    let chips = header.chips;

    // Calculate total memory range
    let mut min_addr = 0xFFFF;
    let mut max_addr = 0x0000;

    for chip in &chips {
        let start = chip.load_address;
        let end = start as usize + chip.data.len();
        if start < min_addr {
            min_addr = start;
        }
        if end > max_addr {
            max_addr = end;
        }
    }

    if min_addr > max_addr as u16 {
        // Should not happen if chips is not empty
        return Err(anyhow!("Could not determine memory range"));
    }

    // Create flat buffer
    let size = max_addr - min_addr as usize;
    let mut memory = vec![0u8; size];

    // Map chips into buffer
    // Note: Iterate in reverse so that earlier chips (e.g. Bank 0) overwrite later ones in the flat memory model.
    // This ensures that the main code (usually in the first chips) is what we see in a flat disassembly.
    for chip in chips.iter().rev() {
        let offset = (chip.load_address - min_addr) as usize;
        let len = chip.data.len();
        if offset + len <= memory.len() {
            memory[offset..offset + len].copy_from_slice(&chip.data);
        }
    }

    Ok((min_addr, memory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_crt_simple() {
        let mut data = Vec::new();
        // CRT Header (0x40 bytes)
        data.extend_from_slice(b"C64 CARTRIDGE   "); // 0x00: Signature
        data.extend_from_slice(&0x40u32.to_be_bytes()); // 0x10: Header length
        data.extend_from_slice(&[0x01, 0x00]); // 0x14: Version
        data.extend_from_slice(&[0x00, 0x00]); // 0x16: Hardware type (0=Normal)
        data.extend_from_slice(&[0; 6]); // 0x18: EXROM/GAME/Reserved
        data.extend_from_slice(&[0; 32]); // 0x20: Name

        while data.len() < 0x40 {
            data.push(0);
        }

        // CHIP Packet 1 (at 0x40)
        // Load at 0x8000, size 0x10 (tiny)
        let chip_data = b"1234567890abcdef";
        let packet_len = 0x10 + chip_data.len() as u32;

        data.extend_from_slice(b"CHIP"); // 0x00
        data.extend_from_slice(&packet_len.to_be_bytes()); // 0x04: Total header length
        data.extend_from_slice(&[0x00, 0x00]); // 0x08: Type
        data.extend_from_slice(&[0x00, 0x00]); // 0x0A: Bank
        data.extend_from_slice(&0x8000u16.to_be_bytes()); // 0x0C: Load Address
        data.extend_from_slice(&(chip_data.len() as u16).to_be_bytes()); // 0x0E: ROM Size
        data.extend_from_slice(chip_data); // 0x10: Data

        let (origin, mem) = parse_crt(&data).expect("Should parse valid CRT");
        assert_eq!(origin, 0x8000);
        assert_eq!(mem.len(), 16);
        assert_eq!(mem, chip_data);
    }

    #[test]
    fn test_parse_crt_overlapping() {
        // Build a minimal CRT header
        let mut data = Vec::new();
        data.extend_from_slice(b"C64 CARTRIDGE   "); // 0x00
        data.extend_from_slice(&0x40u32.to_be_bytes()); // 0x10 Header len
        data.extend_from_slice(&[0x01, 0x00]); // 0x14 Version
        data.extend_from_slice(&[0x00, 0x00]); // 0x16 Hardware type (0=Normal)
        data.extend_from_slice(&[0; 6]); // 0x18
        data.extend_from_slice(&[0; 32]); // 0x20
        // Pad header to 0x40
        while data.len() < 0x40 {
            data.push(0);
        }

        // Chip 1: Bank 0 (Main Code) - "MAIN_CODE_BLOCK_"
        let chip1_content = b"MAIN_CODE_BLOCK_";
        let pkt1_len = 0x10 + chip1_content.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&pkt1_len.to_be_bytes()); // Packet 1 Len
        data.extend_from_slice(&[0x00, 0x00]); // Type: ROM
        data.extend_from_slice(&[0x00, 0x00]); // Bank: 0
        data.extend_from_slice(&0x8000u16.to_be_bytes()); // Load: 8000
        data.extend_from_slice(&(chip1_content.len() as u16).to_be_bytes()); // Size
        data.extend_from_slice(chip1_content);

        // Chip 2: Bank 1 (Data that overwrites Code) - "OVERWRITE_DATA__"
        let chip2_content = b"OVERWRITE_DATA__";
        let pkt2_len = 0x10 + chip2_content.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&pkt2_len.to_be_bytes()); // Packet 2 Len
        data.extend_from_slice(&[0x00, 0x00]); // Type: ROM
        data.extend_from_slice(&[0x00, 0x01]); // Bank: 1
        data.extend_from_slice(&0x8000u16.to_be_bytes()); // Load: 8000
        data.extend_from_slice(&(chip2_content.len() as u16).to_be_bytes()); // Size
        data.extend_from_slice(chip2_content);

        let (origin, mem) = parse_crt(&data).expect("Should parse valid CRT");

        assert_eq!(origin, 0x8000);
        // We expect the FIRST chip (Bank 0) to be visible, not overwriten by Bank 1
        assert_eq!(mem, chip1_content);
    }
}
