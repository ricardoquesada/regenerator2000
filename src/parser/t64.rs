use anyhow::{Result, anyhow};
use std::convert::TryInto;

// T64 File Format Constants
const T64_HEADER_SIZE: usize = 64;
const T64_ENTRY_SIZE: usize = 32;

#[derive(Debug)]
pub struct T64Entry {
    #[allow(dead_code)]
    pub file_type: u8,
    pub start_address: u16,
    pub end_address: u16,
    pub offset: u32,
    pub filename: String,
}

pub fn parse_t64_directory(data: &[u8]) -> Result<Vec<T64Entry>> {
    if data.len() < T64_HEADER_SIZE {
        return Err(anyhow!("File too small to be a valid T64"));
    }

    // Check signature
    let signature = String::from_utf8_lossy(&data[0..32]);
    if !signature.starts_with("C64") {
        return Err(anyhow!("Invalid T64 signature: {}", signature));
    }

    // Read number of entries
    let used_entries = u16::from_le_bytes(data[36..38].try_into()?);

    if used_entries == 0 {
        return Err(anyhow!("T64 file contains no entries"));
    }

    let mut entries = Vec::new();

    for i in 0..used_entries {
        let offset = T64_HEADER_SIZE + (i as usize * T64_ENTRY_SIZE);
        if offset + T64_ENTRY_SIZE > data.len() {
            break;
        }

        let entry_data = &data[offset..offset + T64_ENTRY_SIZE];
        let file_type = entry_data[0];

        // 1 = Normal tape file, 3 = Raw, 4 = Roguelike?
        // 0 = Free
        // We accept non-free entries. Standard files are type 1.
        if file_type == 0 {
            continue;
        }

        let start_address = u16::from_le_bytes(entry_data[2..4].try_into()?);
        let end_address = u16::from_le_bytes(entry_data[4..6].try_into()?);

        // Offset is u16 in some docs, u32 in others?
        // Standard T64 entry:
        // 00: File type (u8)
        // 01: File type (u8) - 1541 type
        // 02-03: Start addr
        // 04-05: End addr
        // 06-07: Unused
        // 08-09: Offset (low word) ??
        // 0A-0B: Offset (high word) ??
        // Actually at 0x08 is offset (4 bytes).
        let offset_in_file = u32::from_le_bytes(entry_data[8..12].try_into()?);

        // Filename
        let filename_bytes = &entry_data[16..32];
        let mut filename = String::with_capacity(16);
        for &b in filename_bytes {
            match b {
                0x20..=0x5E | 0x61..=0x7A => filename.push(b as char),
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

pub fn extract_file(data: &[u8], entry: &T64Entry) -> Result<(u16, Vec<u8>)> {
    let offset = entry.offset as usize;
    // T64 end address is inclusive or exclusive? usually exclusive (address of byte AFTER last byte? or last byte?)
    // Docs say: "End address of the file in memory". Usually this means LAST BYTE address.
    // Length = End - Start + 1?
    // Let's check `parse_t64` original logic: `entry.end_address.wrapping_sub(entry.start_address)`.
    // Wait, original logic was `calc_len = (entry.end_address).wrapping_sub(entry.start_address) as usize;`.
    // If start is $0801 and end is $0801 (1 byte?), len is 0?
    // Usually C64 end addresses are "exclusive" implies size = end - start.
    // But if it is "inclusive" (pointer to last byte), size = end - start + 1.
    // Looking at common formats: PRG first 2 bytes are start. File length implies end.
    // T64 stores start/end.
    // Let's assume original logic was "correct enough" for now, but I suspect it might be off by 1 if end is inclusive.
    // However, looking at the test:
    // start $0801. Content len 3. End $0804.
    // $0804 - $0801 = 3.
    // So if end is $0804, it means the byte at $0804 is NOT included. (Exclusive).

    let calc_len = (entry.end_address).wrapping_sub(entry.start_address) as usize;

    if offset + calc_len > data.len() {
        return Err(anyhow!(
            "Truncated T64 file data for entry: {}",
            entry.filename
        ));
    }

    let file_content = data[offset..offset + calc_len].to_vec();
    Ok((entry.start_address, file_content))
}

pub fn parse_t64(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    let entries = parse_t64_directory(data)?;

    // Find first valid "PRG" like entry (type 1)
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
        // Construct a minimal valid T64 file
        let mut data = Vec::new();

        // Header
        data.extend_from_slice(b"C64 tape image file\0\0\0\0\0\0\0\0\0\0\0\0\0"); // 32 bytes signature
        data.extend_from_slice(&0x0100u16.to_le_bytes()); // Version
        data.extend_from_slice(&1u16.to_le_bytes()); // Max entries
        data.extend_from_slice(&1u16.to_le_bytes()); // Used entries
        data.extend_from_slice(&[0u8; 2]); // Unused
        data.extend_from_slice(b"TEST DISK NAME          "); // 24 bytes name

        // Ensure header is 64 bytes
        while data.len() < 64 {
            data.push(0);
        }

        // Directory Entry 1
        let start_addr: u16 = 0x0801;
        let content = vec![0xA9, 0x00, 0x00]; // LDA #$00, BRK
        let end_addr = start_addr + content.len() as u16; // 0x0804

        let offset: u32 = 64 + 32; // Header + 1 Entry

        data.push(1); // File type (Normal)
        data.push(0); // 1541 type
        data.extend_from_slice(&start_addr.to_le_bytes());
        data.extend_from_slice(&end_addr.to_le_bytes());
        data.extend_from_slice(&[0u8; 2]); // Unused
        data.extend_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]); // Unused
        data.extend_from_slice(b"TESTPRG         "); // 16 bytes filename

        // Data
        data.extend_from_slice(&content);

        let (load_addr, extracted_data) = parse_t64(&data).expect("Should parse successfully");

        assert_eq!(load_addr, 0x0801);
        assert_eq!(extracted_data, content);
    }
}
