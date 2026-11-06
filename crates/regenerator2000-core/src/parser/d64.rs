//! Commodore 1541, 1571, and 1581 disk image parser (`.d64`, `.d71`, `.d81`).

use anyhow::{Result, anyhow};
use std::collections::HashSet;

const D64_STANDARD_SIZE: usize = 174_848;
const D64_WITH_ERROR_INFO_SIZE: usize = 175_531;
const D64_40_TRACK_SIZE: usize = 196_608;
const D64_40_TRACK_WITH_ERROR_INFO_SIZE: usize = 197_376;
const D64_42_TRACK_SIZE: usize = 205_312;
const D71_STANDARD_SIZE: usize = 349_696;
/// D71 image size with 40 tracks per side (80 tracks total, `2 * 196_608` bytes).
const D71_80_TRACK_SIZE: usize = 393_216;
const D81_STANDARD_SIZE: usize = 819_200;
const SECTOR_SIZE: usize = 256;
const DIR_TRACK: u8 = 18;
const DIR_SECTOR: u8 = 1;
const D81_DIR_TRACK: u8 = 40;
const D81_DIR_SECTOR: u8 = 3;
const ENTRIES_PER_SECTOR: usize = 8;
const ENTRY_SIZE: usize = 32;

/// Disk image geometry variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskType {
    /// Single-sided 1541 image (35, 40, or 42 tracks).
    D64,
    /// Double-sided 1571 image with 35 tracks per side (70 tracks total).
    D71_70,
    /// Double-sided 1571 image with 40 tracks per side (80 tracks total).
    D71_80,
    /// Double-sided 3.5-inch 1581 image (80 tracks, 40 sectors per track).
    D81,
}

/// Directory entry parsed from a `.d64`, `.d71`, or `.d81` disk image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D64FileEntry {
    /// Trimmed UTF-8 filename converted from 16-byte PETSCII.
    pub filename: String,
    /// Commodore DOS file type.
    pub file_type: FileType,
    /// 1-based starting track number of the file's sector chain.
    pub track: u8,
    /// 0-based starting sector number of the file's sector chain.
    pub sector: u8,
    /// File size in 254-byte disk blocks as recorded in the directory entry.
    pub size_sectors: u16,
    /// Disk image geometry that owns this entry.
    pub disk_type: DiskType,
}

/// Commodore DOS directory file type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Deleted (`DEL`) file entry.
    DEL,
    /// Sequential (`SEQ`) data file.
    SEQ,
    /// Program (`PRG`) binary file with a 2-byte load address header.
    PRG,
    /// User (`USR`) file.
    USR,
    /// Relative (`REL`) record file.
    REL,
}

impl FileType {
    fn from_byte(byte: u8) -> Result<Self> {
        match byte & 0x0F {
            0 => Ok(Self::DEL),
            1 => Ok(Self::SEQ),
            2 => Ok(Self::PRG),
            3 => Ok(Self::USR),
            4 => Ok(Self::REL),
            _ => Err(anyhow!("Unknown file type: {byte}")),
        }
    }

    /// Returns the 3-letter Commodore DOS mnemonic (`"DEL"`, `"SEQ"`, `"PRG"`, `"USR"`, or `"REL"`).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DEL => "DEL",
            Self::SEQ => "SEQ",
            Self::PRG => "PRG",
            Self::USR => "USR",
            Self::REL => "REL",
        }
    }
}

/// Returns the number of 256-byte sectors on `track` (1..=42) for a single-sided 1541 layout.
#[must_use]
const fn d64_sectors_per_track(track: u8) -> Option<usize> {
    match track {
        1..=17 => Some(21),
        18..=24 => Some(19),
        25..=30 => Some(18),
        31..=42 => Some(17),
        _ => None,
    }
}

/// Calculates the byte offset in a disk image for `track` and `sector`.
fn calculate_offset(track: u8, sector: u8, disk_type: DiskType) -> Result<usize> {
    match disk_type {
        DiskType::D81 => {
            if !(1..=80).contains(&track) {
                return Err(anyhow!("Invalid track: {track}"));
            }
            if sector >= 40 {
                return Err(anyhow!("Invalid sector {sector} for track {track}"));
            }
            Ok((track as usize - 1) * 40 * SECTOR_SIZE + sector as usize * SECTOR_SIZE)
        }
        DiskType::D64 | DiskType::D71_70 => {
            if track == 0 || track > 70 {
                return Err(anyhow!("Invalid track: {track}"));
            }

            // Side 2 of a 70-track D71 (tracks 36-70) mirrors tracks 1-35 after `D64_STANDARD_SIZE`.
            if track > 35 && disk_type == DiskType::D71_70 {
                let offset_in_side_2 = calculate_offset(track - 35, sector, DiskType::D64)?;
                return Ok(D64_STANDARD_SIZE + offset_in_side_2);
            }

            let Some(max_sector) = d64_sectors_per_track(track) else {
                return Err(anyhow!("Invalid track: {track}"));
            };

            if usize::from(sector) >= max_sector {
                return Err(anyhow!(
                    "Invalid sector {} for track {} (max: {})",
                    sector,
                    track,
                    max_sector - 1
                ));
            }

            let mut offset = 0usize;
            for t in 1..track {
                if let Some(sectors) = d64_sectors_per_track(t) {
                    offset += sectors * SECTOR_SIZE;
                }
            }
            offset += usize::from(sector) * SECTOR_SIZE;

            Ok(offset)
        }
        DiskType::D71_80 => {
            // Side 2 of an 80-track D71 (tracks 41-80) mirrors tracks 1-40 after `D64_40_TRACK_SIZE`.
            if track == 0 || track > 80 {
                return Err(anyhow!("Invalid track: {track}"));
            }

            if track > 40 {
                let offset_in_side_2 = calculate_offset(track - 40, sector, DiskType::D64)?;
                return Ok(D64_40_TRACK_SIZE + offset_in_side_2);
            }

            calculate_offset(track, sector, DiskType::D64)
        }
    }
}

/// Parses the directory chain of a `.d64`, `.d71`, or `.d81` disk image.
///
/// # Errors
///
/// Returns an error if `data` does not match a supported disk image size or if the
/// directory sector chain is out of bounds or contains a cycle.
pub fn parse_d64_directory(data: &[u8]) -> Result<Vec<D64FileEntry>> {
    let size = data.len();
    let is_valid_d64 = matches!(
        size,
        D64_STANDARD_SIZE
            | D64_WITH_ERROR_INFO_SIZE
            | D64_40_TRACK_SIZE
            | D64_40_TRACK_WITH_ERROR_INFO_SIZE
            | D64_42_TRACK_SIZE
    );
    let is_valid_d71 = size == D71_STANDARD_SIZE || size == D71_80_TRACK_SIZE;
    let is_valid_d81 = size == D81_STANDARD_SIZE;

    if !is_valid_d64 && !is_valid_d71 && !is_valid_d81 {
        return Err(anyhow!(
            "Invalid disk image size: {size} (expected D64/D71/D81)"
        ));
    }

    let disk_type = if is_valid_d81 {
        DiskType::D81
    } else if size == D71_80_TRACK_SIZE {
        DiskType::D71_80
    } else if is_valid_d71 {
        DiskType::D71_70
    } else {
        DiskType::D64
    };

    let mut files = Vec::new();
    let mut visited_sectors = HashSet::new();
    let (mut current_track, mut current_sector) = if disk_type == DiskType::D81 {
        (D81_DIR_TRACK, D81_DIR_SECTOR)
    } else {
        (DIR_TRACK, DIR_SECTOR)
    };

    loop {
        if !visited_sectors.insert((current_track, current_sector)) {
            return Err(anyhow!("Directory chain too long (possible corruption)"));
        }

        let offset = calculate_offset(current_track, current_sector, disk_type)?;
        if offset + SECTOR_SIZE > data.len() {
            return Err(anyhow!("Directory sector out of bounds"));
        }

        let sector_data = &data[offset..offset + SECTOR_SIZE];
        let next_track = sector_data[0];
        let next_sector = sector_data[1];

        for i in 0..ENTRIES_PER_SECTOR {
            let entry_offset = i * ENTRY_SIZE + 2;
            if entry_offset + 30 > sector_data.len() {
                continue;
            }

            let limit = (entry_offset + ENTRY_SIZE).min(sector_data.len());
            let entry = &sector_data[entry_offset..limit];

            let file_type_byte = entry[0];
            if file_type_byte == 0x00 {
                continue;
            }

            let Ok(file_type) = FileType::from_byte(file_type_byte) else {
                continue;
            };

            let track = entry[1];
            let sector = entry[2];
            let filename = petscii_to_string(&entry[3..19]);
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

        if next_track == 0 || (disk_type == DiskType::D81 && next_track == 255) {
            break;
        }

        current_track = next_track;
        current_sector = next_sector;

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

/// Extracts the raw byte stream for `entry` by following its sector chain in `data`.
///
/// # Errors
///
/// Returns an error if `data` is smaller than a standard D64 image, a sector is out of bounds,
/// the sector chain contains a cycle, or the extracted payload has fewer than 2 bytes.
pub fn extract_file(data: &[u8], entry: &D64FileEntry) -> Result<Vec<u8>> {
    if data.len() < D64_STANDARD_SIZE {
        return Err(anyhow!("Invalid D64/D71/D81 file size"));
    }

    let mut file_data = Vec::new();
    let mut visited_sectors = HashSet::new();
    let mut current_track = entry.track;
    let mut current_sector = entry.sector;

    loop {
        if !visited_sectors.insert((current_track, current_sector)) {
            return Err(anyhow!("File too large (possible corruption)"));
        }

        let offset = calculate_offset(current_track, current_sector, entry.disk_type)?;
        if offset + SECTOR_SIZE > data.len() {
            return Err(anyhow!("File sector out of bounds"));
        }

        let sector_data = &data[offset..offset + SECTOR_SIZE];
        let next_track = sector_data[0];
        let next_sector = sector_data[1];

        if next_track == 0 {
            let bytes_to_read = if next_sector == 0 {
                254
            } else {
                usize::from(next_sector) - 1
            };

            if bytes_to_read > 0 && bytes_to_read <= 254 {
                file_data.extend_from_slice(&sector_data[2..2 + bytes_to_read]);
            }
            break;
        }

        file_data.extend_from_slice(&sector_data[2..]);
        current_track = next_track;
        current_sector = next_sector;

        if file_data.len() > 1_000_000 {
            return Err(anyhow!("File too large (possible corruption)"));
        }
    }

    if file_data.len() < 2 {
        return Err(anyhow!("File too small (no load address)"));
    }

    Ok(file_data)
}

/// Converts a 16-byte PETSCII directory filename slice into a trimmed UTF-8 [`String`].
fn petscii_to_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|&&b| b != 0xA0 && b != 0x00)
        .map(|&b| match b {
            0x20..=0x5F | 0x61..=0x7A => b as char,
            0xC1..=0xDA => (b - 0x80) as char,
            _ => '?',
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
        assert_eq!(calculate_offset(1, 0, DiskType::D64).unwrap(), 0);
        assert_eq!(calculate_offset(1, 1, DiskType::D64).unwrap(), 256);
        assert_eq!(calculate_offset(2, 0, DiskType::D64).unwrap(), 21 * 256);

        let expected = 17 * 21 * 256;
        assert_eq!(calculate_offset(18, 0, DiskType::D64).unwrap(), expected);

        let expected = 17 * 21 * 256 + 7 * 19 * 256;
        assert_eq!(calculate_offset(25, 0, DiskType::D64).unwrap(), expected);
    }

    #[test]
    fn test_calculate_offset_40_tracks() {
        let expected = 683 * 256;
        assert_eq!(calculate_offset(36, 0, DiskType::D64).unwrap(), expected);
    }

    #[test]
    fn test_calculate_offset_42_tracks() {
        let expected_41 = 768 * 256;
        assert_eq!(calculate_offset(41, 0, DiskType::D64).unwrap(), expected_41);

        let expected_42 = (768 + 17) * 256;
        assert_eq!(calculate_offset(42, 0, DiskType::D64).unwrap(), expected_42);

        assert_eq!(
            calculate_offset(42, 16, DiskType::D64).unwrap(),
            expected_42 + 16 * 256
        );

        assert!(calculate_offset(43, 0, DiskType::D64).is_err());
    }

    #[test]
    fn test_d64_42_track_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/6502/d64_42_tracks.d64"
        );
        let Ok(data) = std::fs::read(path) else {
            return;
        };
        assert_eq!(data.len(), D64_42_TRACK_SIZE, "unexpected file size");

        let files = parse_d64_directory(&data).unwrap();
        assert!(!files.is_empty(), "expected at least one file in the image");

        for entry in &files {
            let result = extract_file(&data, entry);
            assert!(
                result.is_ok(),
                "failed to extract '{}': {:?}",
                entry.filename,
                result.err()
            );
        }
    }

    #[test]
    fn test_invalid_track_sector() {
        assert!(calculate_offset(0, 0, DiskType::D64).is_err());
        assert!(calculate_offset(71, 0, DiskType::D64).is_err());
        assert!(calculate_offset(1, 21, DiskType::D64).is_err());
        assert!(calculate_offset(18, 19, DiskType::D64).is_err());
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
        let bytes = b"HELLO\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0";
        assert_eq!(petscii_to_string(bytes), "HELLO");

        let bytes = b"TEST.PRG\xA0\xA0\xA0\xA0\xA0\xA0\xA0\xA0";
        assert_eq!(petscii_to_string(bytes), "TEST.PRG");
    }

    #[test]
    fn test_invalid_d64_size() {
        let data = vec![0u8; 1000];
        assert!(parse_d64_directory(&data).is_err());
    }

    #[test]
    fn test_empty_dir_chain_loop_terminates() {
        // Directory sector points to itself while containing zero valid entries.
        let mut data = vec![0u8; D64_STANDARD_SIZE];
        let dir_offset = calculate_offset(18, 1, DiskType::D64).unwrap();
        data[dir_offset] = 18;
        data[dir_offset + 1] = 1;
        assert!(parse_d64_directory(&data).is_err());
    }

    #[test]
    fn test_parse_and_extract() {
        let mut data = vec![0u8; D64_STANDARD_SIZE];

        let dir_offset = calculate_offset(18, 1, DiskType::D64).unwrap();
        data[dir_offset] = 0;
        data[dir_offset + 1] = 255;

        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82; // PRG
        data[entry_offset + 1] = 1; // Track
        data[entry_offset + 2] = 0; // Sector
        data[entry_offset + 3..entry_offset + 7].copy_from_slice(b"TEST");
        for i in 7..19 {
            data[entry_offset + i] = 0xA0;
        }
        data[entry_offset + 28] = 1;
        data[entry_offset + 29] = 0;

        let file_offset = calculate_offset(1, 0, DiskType::D64).unwrap();
        data[file_offset] = 0;
        data[file_offset + 1] = 6;
        data[file_offset + 2] = 0x01;
        data[file_offset + 3] = 0x08;
        data[file_offset + 4] = 0xEA;
        data[file_offset + 5] = 0xEA;
        data[file_offset + 6] = 0x60;

        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "TEST");
        assert_eq!(files[0].file_type, FileType::PRG);
        assert_eq!(files[0].track, 1);
        assert_eq!(files[0].sector, 0);

        let prg_bytes = extract_file(&data, &files[0]).unwrap();
        let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
        let extracted_data = prg_bytes[2..].to_vec();
        assert_eq!(load_addr, 0x0801);
        assert_eq!(extracted_data, vec![0xEA, 0xEA, 0x60]);
    }

    #[test]
    fn test_calculate_offset_d71() {
        let expected = D64_STANDARD_SIZE;
        assert_eq!(calculate_offset(36, 0, DiskType::D71_70).unwrap(), expected);

        let expected = D64_STANDARD_SIZE + 256;
        assert_eq!(calculate_offset(36, 1, DiskType::D71_70).unwrap(), expected);

        let t18_offset = calculate_offset(18, 0, DiskType::D64).unwrap();
        let expected = D64_STANDARD_SIZE + t18_offset;
        assert_eq!(calculate_offset(53, 0, DiskType::D71_70).unwrap(), expected);

        let t35_offset = calculate_offset(35, 0, DiskType::D64).unwrap();
        let expected = D64_STANDARD_SIZE + t35_offset;
        assert_eq!(calculate_offset(70, 0, DiskType::D71_70).unwrap(), expected);
    }

    #[test]
    fn test_d71_feature() {
        let mut data = vec![0u8; D71_STANDARD_SIZE];

        let dir_offset = calculate_offset(18, 1, DiskType::D71_70).unwrap();
        data[dir_offset] = 0;
        data[dir_offset + 1] = 255;

        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82;
        data[entry_offset + 1] = 36;
        data[entry_offset + 2] = 0;
        data[entry_offset + 3..entry_offset + 8].copy_from_slice(b"SIDE2");
        for i in 8..19 {
            data[entry_offset + i] = 0xA0;
        }
        data[entry_offset + 28] = 1;
        data[entry_offset + 29] = 0;

        let file_offset = calculate_offset(36, 0, DiskType::D71_70).unwrap();
        assert_eq!(file_offset, D64_STANDARD_SIZE);

        data[file_offset] = 0;
        data[file_offset + 1] = 4;
        data[file_offset + 2] = 0x00;
        data[file_offset + 3] = 0x10;
        data[file_offset + 4] = 0x42;

        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "SIDE2");
        assert_eq!(files[0].track, 36);

        let prg_bytes = extract_file(&data, &files[0]).unwrap();
        let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
        let extracted_data = prg_bytes[2..].to_vec();
        assert_eq!(load_addr, 0x1000);
        assert_eq!(extracted_data, vec![0x42]);
    }

    #[test]
    fn test_calculate_offset_d71_80_track() {
        let expected = D64_40_TRACK_SIZE;
        assert_eq!(calculate_offset(41, 0, DiskType::D71_80).unwrap(), expected);

        assert_eq!(
            calculate_offset(41, 1, DiskType::D71_80).unwrap(),
            D64_40_TRACK_SIZE + 256
        );

        let t18_offset = calculate_offset(18, 0, DiskType::D64).unwrap();
        assert_eq!(
            calculate_offset(58, 0, DiskType::D71_80).unwrap(),
            D64_40_TRACK_SIZE + t18_offset
        );

        let t40_offset = calculate_offset(40, 0, DiskType::D64).unwrap();
        assert_eq!(
            calculate_offset(80, 0, DiskType::D71_80).unwrap(),
            D64_40_TRACK_SIZE + t40_offset
        );

        assert!(calculate_offset(81, 0, DiskType::D71_80).is_err());
    }

    #[test]
    fn test_d71_80_track_feature() {
        let mut data = vec![0u8; D71_80_TRACK_SIZE];

        let dir_offset = calculate_offset(18, 1, DiskType::D71_80).unwrap();
        data[dir_offset] = 0;
        data[dir_offset + 1] = 255;

        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82;
        data[entry_offset + 1] = 41;
        data[entry_offset + 2] = 0;
        data[entry_offset + 3..entry_offset + 9].copy_from_slice(b"SIDE2B");
        for i in 9..19 {
            data[entry_offset + i] = 0xA0;
        }
        data[entry_offset + 28] = 1;
        data[entry_offset + 29] = 0;

        let file_offset = calculate_offset(41, 0, DiskType::D71_80).unwrap();
        assert_eq!(file_offset, D64_40_TRACK_SIZE);
        data[file_offset] = 0;
        data[file_offset + 1] = 5;
        data[file_offset + 2] = 0x00;
        data[file_offset + 3] = 0x20;
        data[file_offset + 4] = 0xDE;
        data[file_offset + 5] = 0xAD;

        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "SIDE2B");
        assert_eq!(files[0].track, 41);
        assert_eq!(files[0].disk_type, DiskType::D71_80);

        let prg_bytes = extract_file(&data, &files[0]).unwrap();
        let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
        let payload = prg_bytes[2..].to_vec();
        assert_eq!(load_addr, 0x2000);
        assert_eq!(payload, vec![0xDE, 0xAD]);
    }

    #[test]
    fn test_d71_80_track_real_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/6502/d71_80_tracks.d71"
        );
        let Ok(data) = std::fs::read(path) else {
            return;
        };
        assert_eq!(data.len(), D71_80_TRACK_SIZE, "unexpected file size");

        let files = parse_d64_directory(&data).unwrap();
        assert!(!files.is_empty(), "expected at least one file in the image");

        for entry in &files {
            let result = extract_file(&data, entry);
            assert!(
                result.is_ok(),
                "failed to extract '{}': {:?}",
                entry.filename,
                result.err()
            );
        }
    }

    #[test]
    fn test_d64_size_variants() {
        let data = vec![0u8; D64_STANDARD_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D64_WITH_ERROR_INFO_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D64_40_TRACK_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D64_40_TRACK_WITH_ERROR_INFO_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D64_42_TRACK_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D71_STANDARD_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D71_80_TRACK_SIZE];
        assert!(parse_d64_directory(&data).is_ok());

        let data = vec![0u8; D64_STANDARD_SIZE + 100];
        assert!(parse_d64_directory(&data).is_err());
    }

    #[test]
    fn test_calculate_offset_d81() {
        assert_eq!(calculate_offset(1, 0, DiskType::D81).unwrap(), 0);
        assert_eq!(calculate_offset(1, 1, DiskType::D81).unwrap(), 256);
        assert_eq!(calculate_offset(2, 0, DiskType::D81).unwrap(), 40 * 256);
        assert_eq!(
            calculate_offset(40, 0, DiskType::D81).unwrap(),
            39 * 40 * 256
        );

        let expected = 79 * 40 * 256 + 39 * 256;
        assert_eq!(calculate_offset(80, 39, DiskType::D81).unwrap(), expected);
    }

    #[test]
    fn test_d81_parsing() {
        let mut data = vec![0u8; D81_STANDARD_SIZE];

        let dir_offset = calculate_offset(40, 3, DiskType::D81).unwrap();
        data[dir_offset] = 0;
        data[dir_offset + 1] = 255;

        let entry_offset = dir_offset + 2;
        data[entry_offset] = 0x82;
        data[entry_offset + 1] = 1;
        data[entry_offset + 2] = 0;
        data[entry_offset + 3..entry_offset + 10].copy_from_slice(b"D81TEST");
        for i in 10..19 {
            data[entry_offset + i] = 0xA0;
        }
        data[entry_offset + 28] = 0;
        data[entry_offset + 29] = 0;

        let file_offset = calculate_offset(1, 0, DiskType::D81).unwrap();
        data[file_offset] = 0;
        data[file_offset + 1] = 4;
        data[file_offset + 2] = 0;
        data[file_offset + 3] = 0x20;
        data[file_offset + 4] = 0xFF;

        let files = parse_d64_directory(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "D81TEST");
        assert_eq!(files[0].disk_type, DiskType::D81);

        let prg_bytes = extract_file(&data, &files[0]).unwrap();
        let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
        let extracted = prg_bytes[2..].to_vec();
        assert_eq!(load_addr, 0x2000);
        assert_eq!(extracted, vec![0xFF]);
    }
}
