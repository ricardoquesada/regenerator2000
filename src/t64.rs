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

pub fn parse_t64(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    if data.len() < T64_HEADER_SIZE {
        return Err(anyhow!("File too small to be a valid T64"));
    }

    // Check signature
    let signature = String::from_utf8_lossy(&data[0..32]);
    if !signature.starts_with("C64") {
        return Err(anyhow!("Invalid T64 signature: {}", signature));
    }

    // Read number of entries
    // max_entries at 34..36 is unused here
    let used_entries = u16::from_le_bytes(data[36..38].try_into()?);

    if used_entries == 0 {
        return Err(anyhow!("T64 file contains no entries"));
    }

    // Parse directory entries
    let mut best_entry: Option<T64Entry> = None;

    for i in 0..used_entries {
        let offset = T64_HEADER_SIZE + (i as usize * T64_ENTRY_SIZE);
        if offset + T64_ENTRY_SIZE > data.len() {
            break;
        }

        let entry_data = &data[offset..offset + T64_ENTRY_SIZE];
        let file_type = entry_data[0];

        // We are looking for file_type 1 (Normal tape file) usually.
        if file_type != 1 {
            continue;
        }

        let start_address = u16::from_le_bytes(entry_data[2..4].try_into()?);
        let end_address = u16::from_le_bytes(entry_data[4..6].try_into()?);
        let data_offset = u32::from_le_bytes(entry_data[8..12].try_into()?);

        // Filename
        let filename_bytes = &entry_data[16..32];
        let filename = String::from_utf8_lossy(filename_bytes).trim().to_string();

        let entry = T64Entry {
            file_type,
            start_address,
            end_address,
            offset: data_offset,
            filename,
        };

        // Pick the first valid entry
        best_entry = Some(entry);
        break;
    }

    if let Some(entry) = best_entry {
        let offset = entry.offset as usize;

        let calc_len = (entry.end_address).wrapping_sub(entry.start_address) as usize;

        if offset + calc_len > data.len() {
            return Err(anyhow!(
                "Truncated T64 file data for entry: {}",
                entry.filename
            ));
        }

        let file_content = data[offset..offset + calc_len].to_vec();
        Ok((entry.start_address, file_content))
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
