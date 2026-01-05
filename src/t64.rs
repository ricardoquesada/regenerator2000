use anyhow::{anyhow, Result};
use std::convert::TryInto;

const HEADER_SIZE: usize = 64;
const ENTRY_SIZE: usize = 32;
const SIGNATURE_SIZE: usize = 32;
const FILENAME_SIZE: usize = 16;
const OFFSET_COUNT: usize = 36;
const OFFSET_ENTRY_TYPE: usize = 0;
const OFFSET_ENTRY_START: usize = 2;
const OFFSET_ENTRY_END: usize = 4;
const OFFSET_ENTRY_OFFSET: usize = 8;
const OFFSET_ENTRY_NAME: usize = 16;
const FILE_TYPE_NORMAL: u8 = 1;
const SIGNATURE_PREFIX: &str = "C64";


pub fn parse_t64(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    if data.len() < HEADER_SIZE {
        return Err(anyhow!("File too small to be a valid T64"));
    }

    let signature = String::from_utf8_lossy(&data[..SIGNATURE_SIZE]);
    if !signature.starts_with(SIGNATURE_PREFIX) {
        return Err(anyhow!("Invalid T64 signature: {}", signature));
    }

    let used_entries = u16::from_le_bytes(data[OFFSET_COUNT..OFFSET_COUNT + 2].try_into()?);

    if used_entries == 0 {
        return Err(anyhow!("T64 file contains no entries"));
    }

    let entry_slice = (0..used_entries)
        .map(|i| HEADER_SIZE + (i as usize * ENTRY_SIZE))
        .take_while(|&base| base + ENTRY_SIZE <= data.len())
        .map(|base| &data[base..base + ENTRY_SIZE])
        .find(|slice| slice[OFFSET_ENTRY_TYPE] == FILE_TYPE_NORMAL)
        .ok_or_else(|| anyhow!("No valid program files found in T64 container"))?;

    let start_address = u16::from_le_bytes(entry_slice[OFFSET_ENTRY_START..OFFSET_ENTRY_START + 2].try_into()?);
    let end_address = u16::from_le_bytes(entry_slice[OFFSET_ENTRY_END..OFFSET_ENTRY_END + 2].try_into()?);
    let offset = u32::from_le_bytes(entry_slice[OFFSET_ENTRY_OFFSET..OFFSET_ENTRY_OFFSET + 4].try_into()?);

    let offset_usize = offset as usize;
    let length = end_address
        .wrapping_sub(start_address)
        .saturating_add(1) as usize;

    if offset_usize + length > data.len() {
        let name_bytes = &entry_slice[OFFSET_ENTRY_NAME..OFFSET_ENTRY_NAME + FILENAME_SIZE];
        let filename = String::from_utf8_lossy(name_bytes).trim().to_string();
        return Err(anyhow!(
            "Truncated T64 file data for entry: {}",
            filename
        ));
    }

    Ok((start_address, data[offset_usize..offset_usize + length].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_t64_simple() {
        let mut data = Vec::new();
        let mut signature = vec![0u8; SIGNATURE_SIZE];
        let sig_bytes = b"C64 tape image file";
        signature[..sig_bytes.len()].copy_from_slice(sig_bytes);
        data.extend_from_slice(&signature);

        data.extend_from_slice(&0x0100u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&[0u8; 2]);
        data.extend_from_slice(&[0x20u8; 24]);

        while data.len() < HEADER_SIZE {
            data.push(0);
        }

        let start_addr: u16 = 0x0801;
        let content = vec![0xA9, 0x00, 0x00];
        let end_addr = start_addr + (content.len() as u16) - 1;
        let offset_val = (HEADER_SIZE + ENTRY_SIZE) as u32;

        data.push(FILE_TYPE_NORMAL);
        data.push(0);
        data.extend_from_slice(&start_addr.to_le_bytes());
        data.extend_from_slice(&end_addr.to_le_bytes());
        data.extend_from_slice(&[0u8; 2]);
        data.extend_from_slice(&offset_val.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]);
        
        let mut name = vec![0x20u8; FILENAME_SIZE];
        let name_bytes = b"TESTPRG";
        name[..name_bytes.len()].copy_from_slice(name_bytes);
        data.extend_from_slice(&name);

        data.extend_from_slice(&content);

        let (load_addr, extracted) = parse_t64(&data).expect("Should parse");

        assert_eq!(load_addr, 0x0801);
        assert_eq!(extracted, content);
    }
}
