#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// Fuzz / edge-case tests for file format parsers (CRT, D64, T64, VSF).
///
/// These tests feed crafted and random-ish data to the parsers to exercise
/// edge cases that could cause panics, overflows, or infinite loops.
///
/// Categories:
/// 1. **Minimal valid** – smallest possible valid file
/// 2. **Truncated** – valid header then unexpected EOF
/// 3. **Corrupt fields** – valid structure but bogus lengths / offsets
/// 4. **Randomised** – pseudo-random blobs at various sizes
use regenerator_core::parser::{crt, d64, t64, vice_vsf};

// ─────────────────────── helpers ───────────────────────

/// Deterministic pseudo-random byte generator (xorshift32)
fn pseudo_random_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        out.push(state as u8);
    }
    out
}

// ═══════════════════════ CRT ═══════════════════════

#[test]
fn test_crt_empty() {
    assert!(crt::parse_crt(&[]).is_err());
    assert!(crt::parse_crt_chips(&[]).is_err());
}

#[test]
fn test_crt_too_short() {
    let data = vec![0u8; 0x3F]; // 1 byte short of minimum header
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_bad_signature() {
    let mut data = vec![0u8; 0x50];
    data[0..16].copy_from_slice(b"NOT A CARTRIDGE ");
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_header_len_zero() {
    let mut data = vec![0u8; 0x50];
    data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
    // header_len = 0 → chips start at 0, which is the signature itself
    data[0x10..0x14].copy_from_slice(&0u32.to_be_bytes());
    // There won't be a valid "CHIP" at offset 0 so it should return an error (no CHIP packets)
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_header_len_huge() {
    let mut data = vec![0u8; 0x50];
    data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
    // header_len > file size → no CHIP packets found
    data[0x10..0x14].copy_from_slice(&0xFFFFFFFFu32.to_be_bytes());
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_chip_packet_len_too_small() {
    let mut data = vec![0u8; 0x50];
    data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
    data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
    // Place CHIP at 0x40
    data[0x40..0x44].copy_from_slice(b"CHIP");
    // packet_len = 0x04 (< 0x10 minimum) → invalid
    data[0x44..0x48].copy_from_slice(&0x04u32.to_be_bytes());
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_chip_rom_truncated() {
    let mut data = vec![0u8; 0x60];
    data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
    data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
    // CHIP at 0x40
    data[0x40..0x44].copy_from_slice(b"CHIP");
    data[0x44..0x48].copy_from_slice(&0x20u32.to_be_bytes()); // packet_len = 0x20
    data[0x4C..0x4E].copy_from_slice(&0x8000u16.to_be_bytes()); // load_address
    data[0x4E..0x50].copy_from_slice(&0xFF00u16.to_be_bytes()); // rom_size = way bigger than remaining data
    assert!(crt::parse_crt(&data).is_err());
}

#[test]
fn test_crt_random_data() {
    for seed in 0..100 {
        for size in [0, 1, 15, 16, 63, 64, 65, 128, 256, 1024] {
            let data = pseudo_random_bytes(seed, size);
            // Must not panic — error is fine
            let _ = crt::parse_crt(&data);
            let _ = crt::parse_crt_chips(&data);
        }
    }
}

// ═══════════════════════ T64 ═══════════════════════

#[test]
fn test_t64_empty() {
    assert!(t64::parse_t64(&[]).is_err());
    assert!(t64::parse_t64_directory(&[]).is_err());
}

#[test]
fn test_t64_too_short() {
    let data = vec![0u8; 63]; // 1 byte less than header
    assert!(t64::parse_t64(&data).is_err());
}

#[test]
fn test_t64_bad_signature() {
    let mut data = vec![0u8; 96]; // header + 1 entry
    data[0..3].copy_from_slice(b"ZZZ");
    assert!(t64::parse_t64(&data).is_err());
}

#[test]
fn test_t64_zero_entries() {
    let mut data = vec![0u8; 96];
    data[0..3].copy_from_slice(b"C64");
    data[36..38].copy_from_slice(&0u16.to_le_bytes()); // used_entries = 0
    assert!(t64::parse_t64(&data).is_err());
}

#[test]
fn test_t64_entry_offset_out_of_bounds() {
    let mut data = vec![0u8; 96];
    data[0..3].copy_from_slice(b"C64");
    data[36..38].copy_from_slice(&1u16.to_le_bytes()); // 1 entry
    // Entry at offset 64
    data[64] = 1; // file_type = normal
    data[66..68].copy_from_slice(&0x0801u16.to_le_bytes()); // start
    data[68..70].copy_from_slice(&0x0810u16.to_le_bytes()); // end → 15 bytes
    data[72..76].copy_from_slice(&0xFFFFu32.to_le_bytes()); // offset way past end
    let entries = t64::parse_t64_directory(&data);
    assert!(entries.is_ok());
    // extract_file should fail
    if let Ok(e) = entries
        && let Some(entry) = e.first()
    {
        assert!(t64::extract_file(&data, entry).is_err());
    }
}

#[test]
fn test_t64_end_before_start() {
    let mut data = vec![0u8; 128];
    data[0..3].copy_from_slice(b"C64");
    data[36..38].copy_from_slice(&1u16.to_le_bytes());
    data[64] = 1;
    data[66..68].copy_from_slice(&0x1000u16.to_le_bytes()); // start
    data[68..70].copy_from_slice(&0x0800u16.to_le_bytes()); // end < start (wrapping)
    data[72..76].copy_from_slice(&96u32.to_le_bytes());
    let _ = t64::parse_t64(&data); // must not panic
}

#[test]
fn test_t64_random_data() {
    for seed in 0..100 {
        for size in [0, 1, 32, 63, 64, 65, 96, 128, 256, 1024] {
            let data = pseudo_random_bytes(seed + 1000, size);
            let _ = t64::parse_t64(&data);
            let _ = t64::parse_t64_directory(&data);
        }
    }
}

// ═══════════════════════ D64 ═══════════════════════

#[test]
fn test_d64_empty() {
    assert!(d64::parse_d64_directory(&[]).is_err());
}

#[test]
fn test_d64_wrong_size() {
    // Not a valid D64/D71/D81 size
    let data = vec![0u8; 1000];
    assert!(d64::parse_d64_directory(&data).is_err());
}

#[test]
fn test_d64_dir_chain_loop() {
    // Standard D64 size, but directory sector points back to itself
    let mut data = vec![0u8; 174_848];
    // Track 18, Sector 1 is the first directory sector.
    // We manually compute offset based on D64 geometry.
    // Tracks 1-17 = 17*21*256 = 91392 bytes, Sector 1 of Track 18 = +256
    let dir_offset = 17 * 21 * 256 + 256;
    data[dir_offset] = 18; // next_track = 18 (same track!)
    data[dir_offset + 1] = 1; // next_sector = 1 (same sector!)
    // Add a valid entry so we don't skip
    data[dir_offset + 2] = 0x82; // PRG file type
    data[dir_offset + 3] = 1; // track
    data[dir_offset + 4] = 0; // sector
    data[dir_offset + 5..dir_offset + 21]
        .copy_from_slice(b"LOOPTEST\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0");
    // This will create a directory chain loop — parser must not hang
    let result = d64::parse_d64_directory(&data);
    // It should eventually hit the safety limit
    assert!(result.is_ok() || result.is_err()); // either is fine, no panic
}

#[test]
fn test_d64_corrupt_file_chain() {
    // Corrupt sector chain in a file
    let mut data = vec![0u8; 174_848];
    let dir_offset = 17 * 21 * 256 + 256;
    data[dir_offset] = 0; // no next dir sector
    data[dir_offset + 1] = 255;
    // Valid entry pointing to track 1, sector 0
    data[dir_offset + 2] = 0x82;
    data[dir_offset + 3] = 1;
    data[dir_offset + 4] = 0;
    data[dir_offset + 5..dir_offset + 21]
        .copy_from_slice(b"TEST\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0\xa0");
    data[dir_offset + 30] = 1;

    // File at Track 1, Sector 0: points to invalid track
    data[0] = 99; // next_track = 99 (invalid)
    data[1] = 0;
    let files = d64::parse_d64_directory(&data);
    assert!(files.is_ok());
    if let Ok(f) = files
        && let Some(entry) = f.first()
    {
        assert!(d64::extract_file(&data, entry).is_err());
    }
}

#[test]
fn test_d64_random_at_valid_size() {
    // Random data at the right size — many paths exercised
    for seed in 0..10 {
        let data = pseudo_random_bytes(seed + 2000, 174_848);
        let _ = d64::parse_d64_directory(&data); // must not panic
    }
}

#[test]
fn test_d71_random_at_valid_size() {
    for seed in 0..5 {
        let data = pseudo_random_bytes(seed + 3000, 349_696);
        let _ = d64::parse_d64_directory(&data);
    }
}

#[test]
fn test_d81_random_at_valid_size() {
    for seed in 0..5 {
        let data = pseudo_random_bytes(seed + 4000, 819_200);
        let _ = d64::parse_d64_directory(&data);
    }
}

// ═══════════════════════ VSF ═══════════════════════

#[test]
fn test_vsf_empty() {
    assert!(vice_vsf::parse_vsf(&[]).is_err());
}

#[test]
fn test_vsf_too_short() {
    let data = vec![0u8; 36]; // too short for header
    assert!(vice_vsf::parse_vsf(&data).is_err());
}

#[test]
fn test_vsf_bad_magic() {
    let mut data = vec![0u8; 100];
    data[0..19].copy_from_slice(b"NOT A Snapshot File");
    assert!(vice_vsf::parse_vsf(&data).is_err());
}

#[test]
fn test_vsf_module_size_too_small() {
    let mut data = Vec::new();
    data.extend_from_slice(b"VICE Snapshot File\x1a");
    data.push(0); // major
    data.push(0); // minor
    data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0"); // machine name
    // Module with size < 22
    data.extend_from_slice(b"BADMOD\0\0\0\0\0\0\0\0\0\0"); // name (16 bytes)
    data.push(0); // major
    data.push(0); // minor
    data.extend_from_slice(&10u32.to_le_bytes()); // size = 10 (invalid, < 22)
    assert!(vice_vsf::parse_vsf(&data).is_err());
}

#[test]
fn test_vsf_no_c64mem() {
    let mut data = Vec::new();
    data.extend_from_slice(b"VICE Snapshot File\x1a");
    data.push(0);
    data.push(0);
    data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0");
    // Valid module but not C64MEM
    data.extend_from_slice(b"MAINCPU\0\0\0\0\0\0\0\0\0"); // name (16 bytes)
    data.push(0);
    data.push(0);
    let module_data = vec![0u8; 20]; // 20 bytes of module data
    let total = 22 + module_data.len();
    data.extend_from_slice(&(total as u32).to_le_bytes());
    data.extend_from_slice(&module_data);
    // Should fail: no C64MEM found
    assert!(vice_vsf::parse_vsf(&data).is_err());
}

#[test]
fn test_vsf_random_data() {
    for seed in 0..50 {
        for size in [0, 1, 36, 37, 100, 256, 1024, 4096] {
            let data = pseudo_random_bytes(seed + 5000, size);
            let _ = vice_vsf::parse_vsf(&data); // must not panic
        }
    }
}

// ═══════════════════════ VICE Labels ═══════════════════════

#[test]
fn test_vice_labels_empty() {
    let result = regenerator_core::parser::vice_lbl::parse_vice_labels("");
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_vice_labels_garbage() {
    let garbage = "this is not a label file\n!@#$%^&*\n\0\0\0";
    let result = regenerator_core::parser::vice_lbl::parse_vice_labels(garbage);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_vice_labels_huge_address() {
    // Address bigger than u16 max — should be silently ignored
    let content = "al $FFFFF .too_big\nal $1000 .valid\n";
    let result = regenerator_core::parser::vice_lbl::parse_vice_labels(content);
    assert!(result.is_ok());
    let labels = result.unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].0, 0x1000);
}
