// D64 disk image parser
// Standard D64 format: 174,848 bytes (35 tracks)
// Track 18 = directory track
// Track 18, Sector 1+ = directory entries

use anyhow::{Result, anyhow};

const D64_STANDARD_SIZE: usize = 174_848;
const D64_WITH_ERROR_INFO_SIZE: usize = 175_531;
const D64_40_TRACK_SIZE: usize = 196_608;
const D64_40_TRACK_WITH_ERROR_INFO_SIZE: usize = 197_376; // 196608 + 768 (40 * 19? no, usually 683 for 35 tracks, let's just allow > 40 track size)
const D71_STANDARD_SIZE: usize = 349_696;
const D81_STANDARD_SIZE: usize = 819_200;
const SECTOR_SIZE: usize = 256;
const DIR_TRACK: u8 = 18;
const DIR_SECTOR: u8 = 1;
const D81_DIR_TRACK: u8 = 40;
const D81_DIR_SECTOR: u8 = 3;
const ENTRIES_PER_SECTOR: usize = 8;
const ENTRY_SIZE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiskType {
    D64,
    D71,
    D81,
}

#[derive(Debug, Clone)]
pub struct D64FileEntry {
    pub filename: String,
    pub file_type: FileType,
    pub track: u8,
    pub sector: u8,
    pub size_sectors: u16,
    pub disk_type: DiskType,
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
fn calculate_offset(track: u8, sector: u8, disk_type: DiskType) -> Result<usize> {
    match disk_type {
        DiskType::D81 => {
            if !(1..=80).contains(&track) {
                return Err(anyhow!("Invalid track: {}", track));
            }
            if sector >= 40 {
                return Err(anyhow!("Invalid sector {} for track {}", sector, track));
            }
            // D81: 40 sectors per track, standard layout
            // Track 1 is at 0
            Ok((track as usize - 1) * 40 * SECTOR_SIZE + sector as usize * SECTOR_SIZE)
        }
        DiskType::D64 | DiskType::D71 => {
            if track == 0 || track > 70 {
                return Err(anyhow!("Invalid track: {}", track));
            }

            // Handle D71 second side (Tracks 36-70)
            // They map to the same geometry as Tracks 1-35 but offset by the size of a D64
            if track > 35 && disk_type == DiskType::D71 {
                let offset_in_side_2 = calculate_offset(track - 35, sector, DiskType::D64)?;
                return Ok(D64_STANDARD_SIZE + offset_in_side_2);
            }

            let max_sector = match track {
                1..=17 => 21,
                18..=24 => 19,
                25..=30 => 18,
                31..=40 => 17, // 40-track D64 support
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
    }
}

/// Parse the D64 directory and return list of files
pub fn parse_d64_directory(data: &[u8]) -> Result<Vec<D64FileEntry>> {
    // D64 size check (relaxed)
    // We accept:
    // 1. Standard D64 (174,848)
    // 2. D64 with error info (175,531)
    // 3. 40-track D64 (196,608)
    // 4. 40-track D64 with error info (197,376)
    // 5. D71 (349,696)
    // 6. D81 (819,200)

    let size = data.len();
    let is_valid_d64 = size == D64_STANDARD_SIZE
        || size == D64_WITH_ERROR_INFO_SIZE
        || size == D64_40_TRACK_SIZE
        || size == D64_40_TRACK_WITH_ERROR_INFO_SIZE;

    let is_valid_d71 = size == D71_STANDARD_SIZE;
    let is_valid_d81 = size == D81_STANDARD_SIZE;

    if !is_valid_d64 && !is_valid_d71 && !is_valid_d81 {
        return Err(anyhow!(
            "Invalid disk image size: {} (expected D64/D71/D81)",
            size
        ));
    }

    let disk_type = if is_valid_d81 {
        DiskType::D81
    } else if is_valid_d71 {
        DiskType::D71
    } else {
        DiskType::D64
    };

    let mut files = Vec::new();
    let (mut current_track, mut current_sector) = if disk_type == DiskType::D81 {
        (D81_DIR_TRACK, D81_DIR_SECTOR)
    } else {
        (DIR_TRACK, DIR_SECTOR)
    };

    // Follow the directory chain
    loop {
        let offset = calculate_offset(current_track, current_sector, disk_type)?;

        if offset + SECTOR_SIZE > data.len() {
            // Be lenient with D81 directory chain which might just end?
            // Actually D81 uses standard next_track/sector links
            return Err(anyhow!("Directory sector out of bounds"));
        }

        let sector_data = &data[offset..offset + SECTOR_SIZE];

        // Read next track/sector from first two bytes
        let next_track = sector_data[0];
        let next_sector = sector_data[1];

        // Parse directory entries
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
                disk_type,
            });
        }

        // Check if we've reached the end of the directory chain
        if next_track == 0 || (disk_type == DiskType::D81 && next_track == 255) {
            // D81 apparently can use 0 or 255? Standard CBM is 0.
            // 1581 disk format: last sector usually has $00 $FF.
            // Let's stick with 0 for now, add 255 if needed, but 0 is standard terminator.
            break;
        }

        current_track = next_track;
        current_sector = next_sector;

        // Safety check to prevent infinite loops
        // D81 has way more sectors, so directory could be larger?
        // 144 max entries for D64. D81 has max 296 entries? (Track 40 has 40 sectors... dedicated to dir?)
        // Let's increase limit for D81.
        let limit = if disk_type == DiskType::D81 {
            4000
        } else {
            144
        };
        if files.len() > limit {
            return Err(anyhow!("Directory chain too long (possible corruption)"));
        }
    }

    Ok(files)
}

/// Extract a specific file from the disk image
pub fn extract_file(data: &[u8], entry: &D64FileEntry) -> Result<(u16, Vec<u8>)> {
    if data.len() < D64_STANDARD_SIZE {
        return Err(anyhow!("Invalid D64/D71/D81 file size"));
    }

    let mut file_data = Vec::new();
    let mut current_track = entry.track;
    let mut current_sector = entry.sector;

    // Follow the file's sector chain
    loop {
        let offset = calculate_offset(current_track, current_sector, entry.disk_type)?;

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
        assert_eq!(calculate_offset(1, 0, DiskType::D64).unwrap(), 0);

        // Track 1, Sector 1 should be at 256
        assert_eq!(calculate_offset(1, 1, DiskType::D64).unwrap(), 256);

        // Track 2, Sector 0 should be at 21 * 256
        assert_eq!(calculate_offset(2, 0, DiskType::D64).unwrap(), 21 * 256);

        // Track 18 (directory track)
        let expected = 17 * 21 * 256; // Tracks 1-17
        assert_eq!(calculate_offset(18, 0, DiskType::D64).unwrap(), expected);

        // Track 25 (first track with 18 sectors)
        let expected = 17 * 21 * 256 + 7 * 19 * 256; // Tracks 1-17 + 18-24
        assert_eq!(calculate_offset(25, 0, DiskType::D64).unwrap(), expected);
    }

    #[test]
    fn test_calculate_offset_40_tracks() {
        // Track 36 (first extended track)
        // Standard 35 tracks = 683 sectors
        let expected = 683 * 256;
        assert_eq!(calculate_offset(36, 0, DiskType::D64).unwrap(), expected);
    }

    #[test]
    fn test_invalid_track_sector() {
        assert!(calculate_offset(0, 0, DiskType::D64).is_err());
        assert!(calculate_offset(71, 0, DiskType::D64).is_err());
        assert!(calculate_offset(1, 21, DiskType::D64).is_err()); // Track 1 has only 21 sectors (0-20)
        assert!(calculate_offset(18, 19, DiskType::D64).is_err()); // Track 18 has only 19 sectors (0-18)
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
        let dir_offset = calculate_offset(18, 1, DiskType::D64).unwrap();
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
        let file_offset = calculate_offset(1, 0, DiskType::D64).unwrap();
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
    #[test]
    fn test_calculate_offset_d71() {
        // Test D71 offsets (Tracks 36-70)

        // Track 36, Sector 0 (First sector of Side 2)
        // Should be at D64_STANDARD_SIZE + offset of Track 1, Sector 0
        let expected = D64_STANDARD_SIZE;
        assert_eq!(calculate_offset(36, 0, DiskType::D71).unwrap(), expected);

        // Track 36, Sector 1
        let expected = D64_STANDARD_SIZE + 256;
        assert_eq!(calculate_offset(36, 1, DiskType::D71).unwrap(), expected);

        // Track 53 (Directory track for Side 2 - mirrors Track 18)
        // Track 53 matches Track 18 geometry wise.
        // Track 18 offset is start of directory.
        // So Track 53 offset = D64_STANDARD_SIZE + calculate_offset(18, 0)
        let t18_offset = calculate_offset(18, 0, DiskType::D64).unwrap();
        let expected = D64_STANDARD_SIZE + t18_offset;
        assert_eq!(calculate_offset(53, 0, DiskType::D71).unwrap(), expected);

        // Track 70 (Last track)
        // Matches Track 35 geometry.
        let t35_offset = calculate_offset(35, 0, DiskType::D64).unwrap();
        let expected = D64_STANDARD_SIZE + t35_offset;
        assert_eq!(calculate_offset(70, 0, DiskType::D71).unwrap(), expected);
    }

    #[test]
    fn test_d71_feature() {
        let mut data = vec![0u8; D71_STANDARD_SIZE];

        // 1. Setup Directory on Side 1 (Track 18) to point to a file on Side 2 (Track 36)
        let dir_offset = calculate_offset(18, 1, DiskType::D71).unwrap();
        data[dir_offset] = 0; // Next track
        data[dir_offset + 1] = 255; // Next sector

        // Setup Entry
        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82; // PRG
        data[entry_offset + 1] = 36; // Track 36 (Side 2)
        data[entry_offset + 2] = 0; // Sector 0
        data[entry_offset + 3..entry_offset + 8].copy_from_slice(b"SIDE2");
        for i in 8..19 {
            data[entry_offset + i] = 0xA0; // Padding
        }
        data[entry_offset + 28] = 1; // Size low
        data[entry_offset + 29] = 0; // Size high

        // 2. Setup File Data on Side 2 (Track 36, Sector 0)
        let file_offset = calculate_offset(36, 0, DiskType::D71).unwrap();
        // This is physically at D64_STANDARD_SIZE + 0
        assert_eq!(file_offset, D64_STANDARD_SIZE);

        data[file_offset] = 0; // Next track (last sector)
        data[file_offset + 1] = 4; // Next sector value (bytes used + 1 = 3 + 1 = 4)
        data[file_offset + 2] = 0x00; // Load address low
        data[file_offset + 3] = 0x10; // Load address high (0x1000)
        data[file_offset + 4] = 0x42; // Data byte

        // Parse
        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "SIDE2");
        assert_eq!(files[0].track, 36);

        // Extract
        let (load_addr, extracted_data) = extract_file(&data, &files[0]).unwrap();
        assert_eq!(load_addr, 0x1000);
        assert_eq!(extracted_data, vec![0x42]);
    }

    #[test]
    fn test_d64_size_variants() {
        // Standard D64
        let data = vec![0u8; D64_STANDARD_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        // D64 with error info
        let data = vec![0u8; D64_WITH_ERROR_INFO_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        // 40-track D64
        let data = vec![0u8; D64_40_TRACK_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        // 40-track D64 with error info
        let data = vec![0u8; D64_40_TRACK_WITH_ERROR_INFO_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        // Invalid size
        let data = vec![0u8; D64_STANDARD_SIZE + 100];
        assert!(parse_d64_directory(&data).is_err());
    }

    #[test]
    fn test_calculate_offset_d81() {
        // Track 1, Sector 0 => 0
        assert_eq!(calculate_offset(1, 0, DiskType::D81).unwrap(), 0);

        // Track 1, Sector 1 => 256
        assert_eq!(calculate_offset(1, 1, DiskType::D81).unwrap(), 256);

        // Track 2, Sector 0 => 40 * 256 = 10240
        assert_eq!(calculate_offset(2, 0, DiskType::D81).unwrap(), 40 * 256);

        // Track 40 (Dir), Sector 0
        assert_eq!(
            calculate_offset(40, 0, DiskType::D81).unwrap(),
            39 * 40 * 256
        );

        // Track 80, Sector 39 (Last sector)
        let expected = 79 * 40 * 256 + 39 * 256;
        assert_eq!(calculate_offset(80, 39, DiskType::D81).unwrap(), expected);
    }

    #[test]
    fn test_d81_parsing() {
        let mut data = vec![0u8; D81_STANDARD_SIZE];

        // Setup D81 Directory (Track 40, Sector 3)
        let dir_offset = calculate_offset(40, 3, DiskType::D81).unwrap();
        data[dir_offset] = 0;
        data[dir_offset + 1] = 255;

        // Entry
        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82; // PRG
        data[entry_offset + 1] = 1; // Track 1
        data[entry_offset + 2] = 0; // Sector 0
        data[entry_offset + 3..entry_offset + 10].copy_from_slice(b"D81TEST");
        for i in 10..19 {
            data[entry_offset + i] = 0xA0;
        }
        data[entry_offset + 28] = 0;
        data[entry_offset + 29] = 0;

        // Setup file data at Track 1, Sector 0
        let file_offset = calculate_offset(1, 0, DiskType::D81).unwrap();
        data[file_offset] = 0;
        data[file_offset + 1] = 4;
        data[file_offset + 2] = 0;
        data[file_offset + 3] = 0x20;
        data[file_offset + 4] = 0xFF;

        // Parse
        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "D81TEST");
        assert_eq!(files[0].disk_type, DiskType::D81);

        // Extract
        let (load_addr, extracted) = extract_file(&data, &files[0]).unwrap();
        assert_eq!(load_addr, 0x2000);
        assert_eq!(extracted, vec![0xFF]);
    }
}
