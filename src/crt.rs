use anyhow::{anyhow, Result};

pub fn parse_crt(data: &[u8]) -> Result<(u16, Vec<u8>)> {
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

        let rom_data = &data[rom_data_offset..rom_data_offset + rom_size];
        chips.push((load_address, rom_data));

        current_offset += packet_len;
    }

    if chips.is_empty() {
        return Err(anyhow!("No valid CHIP packets found"));
    }

    // Calculate total memory range
    let mut min_addr = 0xFFFF;
    let mut max_addr = 0x0000;

    for (addr, rom) in &chips {
        let start = *addr;
        let end = start as usize + rom.len();
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
    // Note: Later chips overwrite earlier ones, which is crude bank switching support
    for (addr, rom) in &chips {
        let offset = (*addr - min_addr) as usize;
        let len = rom.len();
        if offset + len <= memory.len() {
            memory[offset..offset + len].copy_from_slice(rom);
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
}
