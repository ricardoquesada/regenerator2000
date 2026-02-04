// D64 disk image parser
// Standard D64 format: 174,848 bytes (35 tracks)
// Track 18 = directory track
// Track 18, Sector 1+ = directory entries

use anyhow::{Result, anyhow};

const D64_STANDARD_SIZE: usize = 174_848;
const SECTOR_SIZE: usize = 256;
const DIR_TRACK: u8 = 18;
const DIR_SECTOR: u8 = 1;
const ENTRIES_PER_SECTOR: usize = 8;
const ENTRY_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct D64FileEntry {
    pub filename: String,
    pub file_type: FileType,
    pub track: u8,
    pub sector: u8,
    pub size_sectors: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    DEL,
    SEQ,
    PRG,
    USR,
    REL,
}

impl FileType {
    fn from_byte(byte: u8) -> Result<Self> {
        match byte & 0x0F {
            0 => Ok(FileType::DEL),
            1 => Ok(FileType::SEQ),
            2 => Ok(FileType::PRG),
            3 => Ok(FileType::USR),
            4 => Ok(FileType::REL),
            _ => Err(anyhow!("Unknown file type: {}", byte)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FileType::DEL => "DEL",
            FileType::SEQ => "SEQ",
            FileType::PRG => "PRG",
            FileType::USR => "USR",
            FileType::REL => "REL",
        }
    }
}

/// Calculate byte offset for a given track and sector
fn calculate_offset(track: u8, sector: u8) -> Result<usize> {
    if track == 0 || track > 40 {
        return Err(anyhow!("Invalid track: {}", track));
    }

    let max_sector = match track {
        1..=17 => 21,
        18..=24 => 19,
        25..=30 => 18,
        31..=40 => 17,
        _ => return Err(anyhow!("Invalid track: {}", track)),
    };

    if sector >= max_sector {
        return Err(anyhow!(
            "Invalid sector {} for track {} (max: {})",
            sector,
            track,
            max_sector - 1
        ));
    }

    let mut offset = 0usize;

    // Add sectors from previous tracks
    for t in 1..track {
        offset += match t {
            1..=17 => 21,
            18..=24 => 19,
            25..=30 => 18,
            31..=40 => 17,
            _ => unreachable!(),
        } * SECTOR_SIZE;
    }

    // Add sectors within current track
    offset += sector as usize * SECTOR_SIZE;

    Ok(offset)
}

/// Parse the D64 directory and return list of files
pub fn parse_d64_directory(data: &[u8]) -> Result<Vec<D64FileEntry>> {
    if data.len() < D64_STANDARD_SIZE {
        return Err(anyhow!(
            "Invalid D64 file size: {} (expected at least {})",
            data.len(),
            D64_STANDARD_SIZE
        ));
    }

    let mut files = Vec::new();
    let mut current_track = DIR_TRACK;
    let mut current_sector = DIR_SECTOR;

    // Follow the directory chain
    loop {
        let offset = calculate_offset(current_track, current_sector)?;

        if offset + SECTOR_SIZE > data.len() {
            return Err(anyhow!("Directory sector out of bounds"));
        }

        let sector_data = &data[offset..offset + SECTOR_SIZE];

        // Read next track/sector from first two bytes
        let next_track = sector_data[0];
        let next_sector = sector_data[1];

        // Parse directory entries (8 per sector, entries start at offset 2, overlapping the next/prev pointer in theory but actually shifted)
        // Standard D64 directory layout has entries at 2, 34, 66... etc.
        // The last entry (7) goes from 226 to 258 (truncating at 256).
        // However, relevant data (size at offset 28,29) fits within 256 bytes (ends at 255).
        for i in 0..ENTRIES_PER_SECTOR {
            let entry_offset = i * ENTRY_SIZE + 2;

            // Ensure we have enough data for the entry fields we care about (up to size at 29)
            if entry_offset + 30 > sector_data.len() {
                continue;
            }

            let limit = std::cmp::min(entry_offset + ENTRY_SIZE, sector_data.len());
            let entry = &sector_data[entry_offset..limit];

            // Check if entry is valid (file type byte indicates if entry is used)
            let file_type_byte = entry[0];
            if file_type_byte == 0x00 {
                // Empty entry, skip
                continue;
            }

            // Parse file type
            let file_type = match FileType::from_byte(file_type_byte) {
                Ok(ft) => ft,
                Err(_) => continue, // Skip invalid entries
            };

            // Parse start track/sector
            let track = entry[1];
            let sector = entry[2];

            // Parse filename (bytes 3-18, 16 bytes PETSCII)
            let filename_bytes = &entry[3..19];
            let filename = petscii_to_string(filename_bytes);

            // Parse size in sectors (bytes 28-29, little-endian)
            let size_sectors = u16::from_le_bytes([entry[28], entry[29]]);

            files.push(D64FileEntry {
                filename,
                file_type,
                track,
                sector,
                size_sectors,
            });
        }

        // Check if we've reached the end of the directory chain
        if next_track == 0 {
            break;
        }

        current_track = next_track;
        current_sector = next_sector;

        // Safety check to prevent infinite loops
        if files.len() > 144 {
            // Max 144 entries (18 sectors * 8 entries)
            return Err(anyhow!("Directory chain too long (possible corruption)"));
        }
    }

    Ok(files)
}

/// Extract a specific file from the disk image
pub fn extract_file(data: &[u8], entry: &D64FileEntry) -> Result<(u16, Vec<u8>)> {
    if data.len() < D64_STANDARD_SIZE {
        return Err(anyhow!("Invalid D64 file size"));
    }

    let mut file_data = Vec::new();
    let mut current_track = entry.track;
    let mut current_sector = entry.sector;

    // Follow the file's sector chain
    loop {
        let offset = calculate_offset(current_track, current_sector)?;

        if offset + SECTOR_SIZE > data.len() {
            return Err(anyhow!("File sector out of bounds"));
        }

        let sector_data = &data[offset..offset + SECTOR_SIZE];

        // Read next track/sector
        let next_track = sector_data[0];
        let next_sector = sector_data[1];

        // Determine how many bytes to read from this sector
        if next_track == 0 {
            // Last sector - next_sector contains the number of bytes to read (1-255)
            let bytes_to_read = if next_sector == 0 {
                254 // If 0, read all except first two bytes
            } else {
                next_sector as usize - 1
            };

            if bytes_to_read > 0 && bytes_to_read <= 254 {
                file_data.extend_from_slice(&sector_data[2..2 + bytes_to_read]);
            }
            break;
        } else {
            // Not the last sector, read all data bytes (254 bytes, skip first two)
            file_data.extend_from_slice(&sector_data[2..]);
            current_track = next_track;
            current_sector = next_sector;
        }

        // Safety check
        if file_data.len() > 1_000_000 {
            return Err(anyhow!("File too large (possible corruption)"));
        }
    }

    // Extract load address from first two bytes (little-endian)
    if file_data.len() < 2 {
        return Err(anyhow!("File too small (no load address)"));
    }

    let load_address = u16::from_le_bytes([file_data[0], file_data[1]]);
    let program_data = file_data[2..].to_vec();

    Ok((load_address, program_data))
}

/// Convert PETSCII bytes to UTF-8 string
fn petscii_to_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0xA0) // 0xA0 is shifted space (padding)
        .map(|&b| {
            match b {
                0x00 => '\0',
                0x20..=0x5F => b as char, // Standard ASCII range (includes A-Z)
                0x61..=0x7A => b as char, // a-z (PETSCII lowercase)
                0xA0 => ' ',              // Shifted space
                0xC1..=0xDA => (b - 0x80) as char, // Uppercase A-Z in PETSCII
                _ => '?',                 // Unknown character
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_offset() {
        // Track 1, Sector 0 should be at offset 0
        assert_eq!(calculate_offset(1, 0).unwrap(), 0);

        // Track 1, Sector 1 should be at 256
        assert_eq!(calculate_offset(1, 1).unwrap(), 256);

        // Track 2, Sector 0 should be at 21 * 256
        assert_eq!(calculate_offset(2, 0).unwrap(), 21 * 256);

        // Track 18 (directory track)
        let expected = 17 * 21 * 256; // Tracks 1-17
        assert_eq!(calculate_offset(18, 0).unwrap(), expected);

        // Track 25 (first track with 18 sectors)
        let expected = 17 * 21 * 256 + 7 * 19 * 256; // Tracks 1-17 + 18-24
        assert_eq!(calculate_offset(25, 0).unwrap(), expected);
    }

    #[test]
    fn test_calculate_offset_40_tracks() {
        // Track 36 (first extended track)
        // Standard 35 tracks = 683 sectors
        let expected = 683 * 256;
        assert_eq!(calculate_offset(36, 0).unwrap(), expected);
    }

    #[test]
    fn test_invalid_track_sector() {
        assert!(calculate_offset(0, 0).is_err());
        assert!(calculate_offset(41, 0).is_err());
        assert!(calculate_offset(1, 21).is_err()); // Track 1 has only 21 sectors (0-20)
        assert!(calculate_offset(18, 19).is_err()); // Track 18 has only 19 sectors (0-18)
    }

    #[test]
    fn test_file_type_from_byte() {
        assert_eq!(FileType::from_byte(0x82).unwrap(), FileType::PRG);
        assert_eq!(FileType::from_byte(0x80).unwrap(), FileType::DEL);
        assert_eq!(FileType::from_byte(0x81).unwrap(), FileType::SEQ);
        assert_eq!(FileType::from_byte(0x83).unwrap(), FileType::USR);
        assert_eq!(FileType::from_byte(0x84).unwrap(), FileType::REL);
    }

    #[test]
    fn test_petscii_to_string() {
        // Test basic ASCII
        let bytes = b"HELLO\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0";
        assert_eq!(petscii_to_string(bytes), "HELLO");

        // Test with padding
        let bytes = b"TEST.PRG\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0";
        assert_eq!(petscii_to_string(bytes), "TEST.PRG");
    }

    #[test]
    fn test_invalid_d64_size() {
        let data = vec![0u8; 1000];
        assert!(parse_d64_directory(&data).is_err());
    }

    #[test]
    fn test_parse_and_extract() {
        let mut data = vec![0u8; D64_STANDARD_SIZE];

        // Setup Track 18, Sector 1 (Directory)
        let dir_offset = calculate_offset(18, 1).unwrap();
        data[dir_offset] = 0; // Next track
        data[dir_offset + 1] = 255; // Next sector

        // Setup Entry 1 (PRG, Track 1, Sector 0, name "TEST", 1 sector)
        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82; // PRG
        data[entry_offset + 1] = 1; // Track
        data[entry_offset + 2] = 0; // Sector
        data[entry_offset + 3..entry_offset + 7].copy_from_slice(b"TEST");
        for i in 7..19 {
            data[entry_offset + i] = 0xA0; // Padding
        }
        data[entry_offset + 28] = 1; // Size low
        data[entry_offset + 29] = 0; // Size high

        // Setup File Data (Track 1, Sector 0)
        let file_offset = calculate_offset(1, 0).unwrap();
        data[file_offset] = 0; // Next track (last sector)
        data[file_offset + 1] = 6; // Next sector value (bytes used + 1 = 5 + 1 = 6)
        data[file_offset + 2] = 0x01; // Load address low
        data[file_offset + 3] = 0x08; // Load address high
        data[file_offset + 4] = 0xEA; // NOP
        data[file_offset + 5] = 0xEA; // NOP
        data[file_offset + 6] = 0x60; // RTS

        // Parse
        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "TEST");
        assert_eq!(files[0].file_type, FileType::PRG);
        assert_eq!(files[0].track, 1);
        assert_eq!(files[0].sector, 0);

        // Extract
        let (load_addr, extracted_data) = extract_file(&data, &files[0]).unwrap();
        assert_eq!(load_addr, 0x0801);
        assert_eq!(extracted_data, vec![0xEA, 0xEA, 0x60]);
    }
}
