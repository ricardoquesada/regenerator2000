//! Commodore 64 `.t64` tape container parser.

use anyhow::{Result, anyhow};

const T64_HEADER_SIZE: usize = 64;
const T64_ENTRY_SIZE: usize = 32;

/// Directory entry inside a `.t64` tape image container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct T64Entry {
    /// T64 entry type (`0` = free, `1` = normal tape file, `3` = memory snapshot).
    pub file_type: u8,
    /// 6502 start address where the file loads in memory.
    pub start_address: u16,
    /// Exclusive 6502 end address of the file in memory.
    pub end_address: u16,
    /// Byte offset of the file payload within the `.t64` container.
    pub offset: u32,
    /// Trimmed filename converted from 16-byte PETSCII.
    pub filename: String,
}

/// Parses the directory table of a `.t64` tape container.
///
/// # Errors
///
/// Returns an error if `data` is shorter than 64 bytes, lacks a `"C64"` signature,
/// or declares zero directory entries.
pub fn parse_t64_directory(data: &[u8]) -> Result<Vec<T64Entry>> {
    if data.len() < T64_HEADER_SIZE {
        return Err(anyhow!("File too small to be a valid T64"));
    }

    if !data.starts_with(b"C64") {
        let signature = String::from_utf8_lossy(&data[0..32]);
        return Err(anyhow!("Invalid T64 signature: {signature}"));
    }

    let used_entries = u16::from_le_bytes([data[36], data[37]]);
    if used_entries == 0 {
        return Err(anyhow!("T64 file contains no entries"));
    }

    let mut entries = Vec::new();

    for i in 0..usize::from(used_entries) {
        let offset = T64_HEADER_SIZE + (i * T64_ENTRY_SIZE);
        if offset + T64_ENTRY_SIZE > data.len() {
            break;
        }

        let entry_data = &data[offset..offset + T64_ENTRY_SIZE];
        let file_type = entry_data[0];
        if file_type == 0 {
            continue;
        }

        let start_address = u16::from_le_bytes([entry_data[2], entry_data[3]]);
        let end_address = u16::from_le_bytes([entry_data[4], entry_data[5]]);
        let offset_in_file =
            u32::from_le_bytes([entry_data[8], entry_data[9], entry_data[10], entry_data[11]]);

        let filename_bytes = &entry_data[16..32];
        let mut filename = String::with_capacity(16);
        for &b in filename_bytes {
            match b {
                0x20..=0x5F | 0x61..=0x7A => filename.push(b as char),
                0xC1..=0xDA => filename.push((b - 0x80) as char),
                _ => filename.push(' '),
            }
        }
        let filename = filename.trim_end().to_string();

        entries.push(T64Entry {
            file_type,
            start_address,
            end_address,
            offset: offset_in_file,
            filename,
        });
    }

    Ok(entries)
}

/// Extracts `entry` from a `.t64` container as a `.prg`-formatted byte vector
/// (2-byte little-endian load address followed by payload bytes).
///
/// # Errors
///
/// Returns an error if `entry.end_address < entry.start_address` or if the payload
/// range extends past `data.len()`.
pub fn extract_file(data: &[u8], entry: &T64Entry) -> Result<Vec<u8>> {
    let offset = entry.offset as usize;
    let Some(calc_len) = entry
        .end_address
        .checked_sub(entry.start_address)
        .map(usize::from)
    else {
        return Err(anyhow!(
            "Invalid T64 address range (${:04X}-${:04X}) for entry: {}",
            entry.start_address,
            entry.end_address,
            entry.filename
        ));
    };

    let Some(end_offset) = offset.checked_add(calc_len) else {
        return Err(anyhow!(
            "Truncated T64 file data for entry: {}",
            entry.filename
        ));
    };
    if end_offset > data.len() {
        return Err(anyhow!(
            "Truncated T64 file data for entry: {}",
            entry.filename
        ));
    }

    let file_content = &data[offset..end_offset];
    let mut prg_data = Vec::with_capacity(file_content.len() + 2);
    prg_data.extend_from_slice(&entry.start_address.to_le_bytes());
    prg_data.extend_from_slice(file_content);
    Ok(prg_data)
}

/// Extracts the first normal program entry (`file_type == 1`) from a `.t64` container.
///
/// # Errors
///
/// Returns an error if [`parse_t64_directory`] fails, no type-1 entry exists,
/// or [`extract_file`] fails.
pub fn parse_t64(data: &[u8]) -> Result<Vec<u8>> {
    let entries = parse_t64_directory(data)?;

    if let Some(entry) = entries.iter().find(|e| e.file_type == 1) {
        extract_file(data, entry)
    } else {
        Err(anyhow!("No valid program files found in T64 container"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_t64_simple() {
        let mut data = Vec::new();

        data.extend_from_slice(b"C64 tape image file\0\0\0\0\0\0\0\0\0\0\0\0\0");
        data.extend_from_slice(&0x0100u16.to_le_bytes()); // Version
        data.extend_from_slice(&1u16.to_le_bytes()); // Max entries
        data.extend_from_slice(&1u16.to_le_bytes()); // Used entries
        data.extend_from_slice(&[0u8; 2]); // Unused
        data.extend_from_slice(b"TEST DISK NAME          ");
        data.resize(T64_HEADER_SIZE, 0);

        let start_addr: u16 = 0x0801;
        let content = vec![0xA9, 0x00, 0x00];
        let end_addr = start_addr + content.len() as u16;
        let offset: u32 = (T64_HEADER_SIZE + T64_ENTRY_SIZE) as u32;

        data.push(1); // File type (normal)
        data.push(0); // 1541 type
        data.extend_from_slice(&start_addr.to_le_bytes());
        data.extend_from_slice(&end_addr.to_le_bytes());
        data.extend_from_slice(&[0u8; 2]);
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(b"TESTPRG         ");
        data.extend_from_slice(&content);

        let prg_bytes = parse_t64(&data).expect("Should parse successfully");
        let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
        let extracted_data = prg_bytes[2..].to_vec();
        assert_eq!(load_addr, 0x0801);
        assert_eq!(extracted_data, content);
    }
}
