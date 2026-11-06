//! Commodore 64 `.crt` cartridge image parser.

use anyhow::{Result, anyhow};

/// Minimum `.crt` container header size in bytes.
const CRT_MIN_HEADER_LEN: usize = 0x40;

/// `.crt` container magic signature (`"C64 CARTRIDGE   "`).
const CRT_SIGNATURE: &[u8; 16] = b"C64 CARTRIDGE   ";

/// Minimum `CHIP` packet header size in bytes.
const CHIP_HEADER_LEN: usize = 0x10;

/// Exclusive upper bound of the 64 KiB 6502 address space.
const ADDRESS_SPACE_END: usize = 0x1_0000;

/// Single ROM or RAM bank (`CHIP` packet) inside a `.crt` cartridge image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrtChip {
    /// 6502 start address where this bank maps into memory.
    pub load_address: u16,
    /// Cartridge bank index.
    pub bank: u16,
    /// Chip memory type (`0` = ROM, `1` = RAM, `2` = Flash ROM).
    pub chip_type: u16,
    /// Raw ROM image bytes for this bank.
    pub data: Vec<u8>,
}

/// Parsed `.crt` cartridge header and its `CHIP` banks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrtHeader {
    /// Cartridge format version (for example, `0x0100` for version 1.0).
    pub version: u16,
    /// Numeric VICE cartridge hardware ID.
    pub hardware_type: u16,
    /// Initial `EXROM` line state (`0` = active low, `1` = inactive high).
    pub exrom: u8,
    /// Initial `GAME` line state (`0` = active low, `1` = inactive high).
    pub game: u8,
    /// Cartridge title from the 32-byte header field.
    pub name: String,
    /// Ordered list of `CHIP` packets stored in the cartridge.
    pub chips: Vec<CrtChip>,
}

impl CrtHeader {
    /// Returns the human-readable cartridge hardware name for [`Self::hardware_type`].
    #[must_use]
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

/// Parses the header and `CHIP` packets from a `.crt` file byte slice.
///
/// # Errors
///
/// Returns an error if `data` is shorter than 64 bytes, has an invalid signature,
/// contains a malformed or truncated `CHIP` packet, or contains no valid `CHIP` packets.
pub fn parse_crt_chips(data: &[u8]) -> Result<CrtHeader> {
    if data.len() < CRT_MIN_HEADER_LEN {
        return Err(anyhow!("File too short to be a valid CRT file"));
    }

    if &data[0..16] != CRT_SIGNATURE {
        return Err(anyhow!("Invalid CRT signature"));
    }

    let header_len = u32::from_be_bytes([data[0x10], data[0x11], data[0x12], data[0x13]]) as usize;
    let version = u16::from_be_bytes([data[0x14], data[0x15]]);
    let hardware_type = u16::from_be_bytes([data[0x16], data[0x17]]);
    let exrom = data[0x18];
    let game = data[0x19];

    let name_bytes = &data[0x20..0x40];
    let end_index = name_bytes.iter().position(|&x| x == 0).unwrap_or(32);
    let name = String::from_utf8_lossy(&name_bytes[..end_index]).to_string();

    let mut current_offset = header_len;
    let mut chips = Vec::new();

    while current_offset < data.len() {
        let Some(rom_data_offset) = current_offset.checked_add(CHIP_HEADER_LEN) else {
            break;
        };
        if rom_data_offset > data.len() {
            break;
        }

        if &data[current_offset..current_offset + 4] != b"CHIP" {
            break;
        }

        let packet_len = u32::from_be_bytes([
            data[current_offset + 4],
            data[current_offset + 5],
            data[current_offset + 6],
            data[current_offset + 7],
        ]) as usize;

        if packet_len < CHIP_HEADER_LEN {
            return Err(anyhow!(
                "Invalid CHIP packet length at offset {current_offset:x}"
            ));
        }

        let chip_type =
            u16::from_be_bytes([data[current_offset + 0x08], data[current_offset + 0x09]]);
        let bank = u16::from_be_bytes([data[current_offset + 0x0A], data[current_offset + 0x0B]]);
        let load_address =
            u16::from_be_bytes([data[current_offset + 0x0C], data[current_offset + 0x0D]]);
        let rom_size =
            u16::from_be_bytes([data[current_offset + 0x0E], data[current_offset + 0x0F]]) as usize;

        let Some(rom_end) = rom_data_offset.checked_add(rom_size) else {
            return Err(anyhow!(
                "CHIP packet data truncated at offset {current_offset:x}"
            ));
        };
        if rom_end > data.len() {
            return Err(anyhow!(
                "CHIP packet data truncated at offset {current_offset:x}"
            ));
        }

        if packet_len < CHIP_HEADER_LEN + rom_size {
            return Err(anyhow!(
                "Invalid CHIP packet length at offset {current_offset:x}"
            ));
        }

        let rom_data = data[rom_data_offset..rom_end].to_vec();
        chips.push(CrtChip {
            load_address,
            bank,
            chip_type,
            data: rom_data,
        });

        let Some(next_offset) = current_offset.checked_add(packet_len) else {
            break;
        };
        current_offset = next_offset;
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

/// Parses a `.crt` file and flattens all `CHIP` banks into one contiguous memory buffer.
///
/// Returns `(origin, memory)` where `origin` is the lowest `CHIP` load address.
/// Earlier `CHIP` banks (such as Bank 0) take precedence over later overlapping banks.
///
/// # Errors
///
/// Returns an error if [`parse_crt_chips`] fails or if the combined memory range is empty.
pub fn parse_crt(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    let header = parse_crt_chips(data)?;
    let chips = header.chips;

    let mut min_addr = usize::MAX;
    let mut max_addr = 0usize;

    for chip in &chips {
        if chip.data.is_empty() {
            continue;
        }
        let start = usize::from(chip.load_address);
        let end = start.saturating_add(chip.data.len()).min(ADDRESS_SPACE_END);
        if start < min_addr {
            min_addr = start;
        }
        if end > max_addr {
            max_addr = end;
        }
    }

    if min_addr >= max_addr || min_addr > usize::from(u16::MAX) {
        return Err(anyhow!("Could not determine memory range"));
    }

    let size = max_addr - min_addr;
    let mut memory = vec![0u8; size];

    // Iterate in reverse so earlier chips (such as Bank 0) overwrite later overlapping banks.
    for chip in chips.iter().rev() {
        let start = usize::from(chip.load_address);
        if start < min_addr {
            continue;
        }
        let offset = start - min_addr;
        if offset >= memory.len() {
            continue;
        }
        let copy_len = chip.data.len().min(memory.len() - offset);
        memory[offset..offset + copy_len].copy_from_slice(&chip.data[..copy_len]);
    }

    Ok((min_addr as u16, memory))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_crt_header() -> Vec<u8> {
        let mut data = Vec::with_capacity(CRT_MIN_HEADER_LEN);
        data.extend_from_slice(CRT_SIGNATURE);
        data.extend_from_slice(&(CRT_MIN_HEADER_LEN as u32).to_be_bytes());
        data.extend_from_slice(&[0x01, 0x00]); // Version 1.0
        data.extend_from_slice(&[0x00, 0x00]); // Hardware type 0 (Normal)
        data.resize(CRT_MIN_HEADER_LEN, 0);
        data
    }

    #[test]
    fn test_parse_crt_simple() {
        let mut data = make_crt_header();

        let chip_data = b"1234567890abcdef";
        let packet_len = 0x10 + chip_data.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&packet_len.to_be_bytes());
        data.extend_from_slice(&[0x00, 0x00]); // Type
        data.extend_from_slice(&[0x00, 0x00]); // Bank
        data.extend_from_slice(&0x8000u16.to_be_bytes()); // Load address
        data.extend_from_slice(&(chip_data.len() as u16).to_be_bytes()); // ROM size
        data.extend_from_slice(chip_data);

        let (origin, mem) = parse_crt(&data).expect("Should parse valid CRT");
        assert_eq!(origin, 0x8000);
        assert_eq!(mem.len(), 16);
        assert_eq!(mem, chip_data);
    }

    #[test]
    fn test_parse_crt_overlapping() {
        let mut data = make_crt_header();

        // Chip 1: Bank 0 (Main Code)
        let chip1_content = b"MAIN_CODE_BLOCK_";
        let pkt1_len = 0x10 + chip1_content.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&pkt1_len.to_be_bytes());
        data.extend_from_slice(&[0x00, 0x00]); // Type: ROM
        data.extend_from_slice(&[0x00, 0x00]); // Bank: 0
        data.extend_from_slice(&0x8000u16.to_be_bytes());
        data.extend_from_slice(&(chip1_content.len() as u16).to_be_bytes());
        data.extend_from_slice(chip1_content);

        // Chip 2: Bank 1 at the same load address
        let chip2_content = b"OVERWRITE_DATA__";
        let pkt2_len = 0x10 + chip2_content.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&pkt2_len.to_be_bytes());
        data.extend_from_slice(&[0x00, 0x00]); // Type: ROM
        data.extend_from_slice(&[0x00, 0x01]); // Bank: 1
        data.extend_from_slice(&0x8000u16.to_be_bytes());
        data.extend_from_slice(&(chip2_content.len() as u16).to_be_bytes());
        data.extend_from_slice(chip2_content);

        let (origin, mem) = parse_crt(&data).expect("Should parse valid CRT");
        assert_eq!(origin, 0x8000);
        assert_eq!(mem, chip1_content);
    }

    #[test]
    fn test_parse_crt_ultimax_top_of_memory() {
        // An 8 KiB Ultimax/KERNAL bank at $E000..=$FFFF has exclusive end 0x10000.
        let mut data = make_crt_header();
        let chip_data = vec![0xEAu8; 0x2000];
        let packet_len = 0x10 + chip_data.len() as u32;

        data.extend_from_slice(b"CHIP");
        data.extend_from_slice(&packet_len.to_be_bytes());
        data.extend_from_slice(&[0x00, 0x00]);
        data.extend_from_slice(&[0x00, 0x00]);
        data.extend_from_slice(&0xE000u16.to_be_bytes());
        data.extend_from_slice(&(chip_data.len() as u16).to_be_bytes());
        data.extend_from_slice(&chip_data);

        let (origin, mem) = parse_crt(&data).expect("Ultimax $E000-$FFFF CRT must parse");
        assert_eq!(origin, 0xE000);
        assert_eq!(mem.len(), 0x2000);
    }
}
