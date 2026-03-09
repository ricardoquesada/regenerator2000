/// Malformed-input tests for parsers: D64, CRT, T64, VSF
///
/// These tests ensure that the parsers return clear errors (never panic)
/// when given truncated, corrupted, or completely invalid data.

// ═══════════════════════════════════════════════════════════════════════════════
// D64 Parser
// ═══════════════════════════════════════════════════════════════════════════════

mod d64_malformed {
    use regenerator2000::parser::d64::{extract_file, parse_d64_directory};

    #[test]
    fn empty_data() {
        let res = parse_d64_directory(&[]);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Invalid disk image size")
        );
    }

    #[test]
    fn too_small_data() {
        let data = vec![0u8; 100];
        let res = parse_d64_directory(&data);
        assert!(res.is_err());
    }

    #[test]
    fn wrong_size_d64() {
        // Not a valid D64/D71/D81 size
        let data = vec![0u8; 100_000];
        let res = parse_d64_directory(&data);
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("Invalid disk image size"), "Got: {msg}");
    }

    #[test]
    fn valid_size_but_all_zeros() {
        // Standard D64 size, but all zeros => directory chain has track 0
        // which means "no more sectors", results in empty file list
        let data = vec![0u8; 174_848];
        let res = parse_d64_directory(&data);
        // Track 18, sector 1 should be accessible (all zeros).
        // Entry at first position has file_type_byte=0 (empty), so skip.
        // next_track=0 means end of chain, returns empty list.
        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
    }

    #[test]
    fn extract_file_with_too_small_image() {
        // extract_file requires >= D64_STANDARD_SIZE
        let data = vec![0u8; 1000];
        let dummy_entry = regenerator2000::parser::d64::D64FileEntry {
            filename: "TEST".to_string(),
            file_type: regenerator2000::parser::d64::FileType::PRG,
            track: 1,
            sector: 0,
            size_sectors: 1,
            disk_type: regenerator2000::parser::d64::DiskType::D64,
        };
        let res = extract_file(&data, &dummy_entry);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Invalid D64"));
    }

    #[test]
    fn extract_file_sector_chain_loop_protection() {
        // Create a D64 image where the sector chain loops back to itself
        let mut data = vec![0u8; 174_848];
        // Set track 1, sector 0 to point back to itself: next_track=1, next_sector=0
        // Offset for track 1 sector 0 = 0
        data[0] = 1; // next_track = 1
        data[1] = 0; // next_sector = 0
        // Fill remaining bytes with data
        for i in 2..256 {
            data[i] = 0xAA;
        }

        let entry = regenerator2000::parser::d64::D64FileEntry {
            filename: "LOOP".to_string(),
            file_type: regenerator2000::parser::d64::FileType::PRG,
            track: 1,
            sector: 0,
            size_sectors: 999,
            disk_type: regenerator2000::parser::d64::DiskType::D64,
        };
        let res = extract_file(&data, &entry);
        // Should hit the 1MB safety check and error out
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(
            msg.contains("too large") || msg.contains("corruption"),
            "Got: {msg}"
        );
    }

    #[test]
    fn d71_wrong_size() {
        // D71 expects 349_696 bytes
        let data = vec![0u8; 349_000]; // close but wrong
        let res = parse_d64_directory(&data);
        assert!(res.is_err());
    }

    #[test]
    fn d81_wrong_size() {
        // D81 expects 819_200 bytes
        let data = vec![0u8; 800_000]; // close but wrong
        let res = parse_d64_directory(&data);
        assert!(res.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CRT Parser
// ═══════════════════════════════════════════════════════════════════════════════

mod crt_malformed {
    use regenerator2000::parser::crt::{parse_crt, parse_crt_chips};

    #[test]
    fn empty_data() {
        let res = parse_crt_chips(&[]);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn too_short_for_header() {
        let data = vec![0u8; 0x3F]; // one byte short of min header
        let res = parse_crt_chips(&data);
        assert!(res.is_err());
    }

    #[test]
    fn wrong_signature() {
        let mut data = vec![0u8; 0x50];
        data[0..16].copy_from_slice(b"NOT A CARTRIDGE!");
        let res = parse_crt_chips(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Invalid CRT signature")
        );
    }

    #[test]
    fn valid_header_no_chips() {
        let mut data = vec![0u8; 0x40];
        data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
        data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes()); // header length

        let res = parse_crt_chips(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("No valid CHIP packets")
        );
    }

    #[test]
    fn chip_packet_too_short_length() {
        let mut data = vec![0u8; 0x60];
        data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
        data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
        // CHIP header at 0x40
        data[0x40..0x44].copy_from_slice(b"CHIP");
        // Set packet_len to 5 (< 0x10 minimum)
        data[0x44..0x48].copy_from_slice(&5u32.to_be_bytes());

        let res = parse_crt_chips(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Invalid CHIP packet length")
        );
    }

    #[test]
    fn chip_data_truncated() {
        let mut data = vec![0u8; 0x50];
        data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
        data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
        // CHIP header at 0x40
        data[0x40..0x44].copy_from_slice(b"CHIP");
        let packet_len = 0x10 + 1024u32; // claims 1024 bytes of ROM data
        data[0x44..0x48].copy_from_slice(&packet_len.to_be_bytes());
        // ROM size
        data[0x4E..0x50].copy_from_slice(&1024u16.to_be_bytes());
        // But we only have 0x50 bytes total — truncated

        let res = parse_crt_chips(&data);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("truncated"));
    }

    #[test]
    fn parse_crt_flat_with_bad_data() {
        let res = parse_crt(&[0xFF; 64]);
        assert!(res.is_err());
    }

    #[test]
    fn garbage_after_valid_header() {
        // Valid header but garbage instead of CHIP packets — should get "No valid CHIP packets"
        let mut data = vec![0u8; 0x80];
        data[0..16].copy_from_slice(b"C64 CARTRIDGE   ");
        data[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
        // Fill post-header with garbage (not "CHIP")
        for i in 0x40..0x80 {
            data[i] = 0xFF;
        }

        let res = parse_crt_chips(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("No valid CHIP packets")
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// T64 Parser
// ═══════════════════════════════════════════════════════════════════════════════

mod t64_malformed {
    use regenerator2000::parser::t64::{parse_t64, parse_t64_directory};

    #[test]
    fn empty_data() {
        let res = parse_t64_directory(&[]);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("too small"));
    }

    #[test]
    fn too_short_data() {
        let data = vec![0u8; 32]; // less than T64_HEADER_SIZE (64)
        let res = parse_t64_directory(&data);
        assert!(res.is_err());
    }

    #[test]
    fn wrong_signature() {
        let mut data = vec![0u8; 128];
        data[0..10].copy_from_slice(b"NOT A TAPE");
        let res = parse_t64_directory(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("Invalid T64 signature")
        );
    }

    #[test]
    fn zero_entries() {
        let mut data = vec![0u8; 128];
        data[0..3].copy_from_slice(b"C64");
        // used_entries at offset 36-37 = 0
        data[36] = 0;
        data[37] = 0;
        let res = parse_t64_directory(&data);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("no entries"));
    }

    #[test]
    fn entries_beyond_data() {
        // Claim 100 entries but only provide header
        let mut data = vec![0u8; 64];
        data[0..3].copy_from_slice(b"C64");
        data[36] = 100; // used_entries low
        data[37] = 0; // used_entries high
        // No actual entry data beyond header

        let res = parse_t64_directory(&data);
        // Should not panic; entries that don't fit are skipped
        assert!(res.is_ok());
        // With all zeros in entry data, file_type == 0 means free => skipped
        assert!(res.unwrap().is_empty());
    }

    #[test]
    fn parse_t64_no_type_1_entries() {
        // Has entries but none with file_type == 1 (normal tape file)
        let mut data = vec![0u8; 128];
        data[0..32].copy_from_slice(b"C64 tape image file\0\0\0\0\0\0\0\0\0\0\0\0\0");
        data[36] = 1; // 1 entry
        data[37] = 0;
        // Entry at offset 64, set file_type = 3 (not 1)
        data[64] = 3;
        data[65] = 0;
        data[66..68].copy_from_slice(&0x0801u16.to_le_bytes()); // start
        data[68..70].copy_from_slice(&0x0810u16.to_le_bytes()); // end
        data[72..76].copy_from_slice(&128u32.to_le_bytes()); // offset (beyond data)

        let res = parse_t64(&data);
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("No valid program files")
        );
    }

    #[test]
    fn truncated_file_data() {
        let mut data = vec![0u8; 96]; // header + 1 entry, but file data truncated
        data[0..32].copy_from_slice(b"C64 tape image file\0\0\0\0\0\0\0\0\0\0\0\0\0");
        data[36] = 1; // 1 entry
        data[37] = 0;
        // Directory entry at offset 64
        data[64] = 1; // file_type = 1 (Normal)
        data[65] = 0;
        data[66..68].copy_from_slice(&0x0801u16.to_le_bytes()); // start
        data[68..70].copy_from_slice(&0x0900u16.to_le_bytes()); // end (large)
        data[72..76].copy_from_slice(&96u32.to_le_bytes()); // offset = end of data

        // File claims 0x0900 - 0x0801 = 255 bytes, at offset 96, but data is only 96 bytes
        let res = parse_t64(&data);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Truncated"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VSF Parser
// ═══════════════════════════════════════════════════════════════════════════════

mod vsf_malformed {
    use regenerator2000::parser::vice_vsf::parse_vsf;

    /// Helper: assert `parse_vsf` returns Err and the error message contains `needle`.
    fn assert_vsf_err(data: &[u8], needle: &str) {
        match parse_vsf(data) {
            Err(e) => assert!(
                e.to_string().contains(needle),
                "Expected error containing {needle:?}, got: {e}"
            ),
            Ok(_) => panic!("Expected error containing {needle:?}, but got Ok"),
        }
    }

    #[test]
    fn empty_data() {
        assert_vsf_err(&[], "too short");
    }

    #[test]
    fn too_short_for_magic() {
        let data = vec![0u8; 20];
        assert!(parse_vsf(&data).is_err());
    }

    #[test]
    fn wrong_magic() {
        let mut data = vec![0u8; 100];
        data[0..19].copy_from_slice(b"Not A Snapshot!\x00\x00\x00\x1a");
        assert_vsf_err(&data, "Invalid VSF signature");
    }

    #[test]
    fn valid_magic_no_c64mem() {
        let mut data = Vec::new();
        data.extend_from_slice(b"VICE Snapshot File\x1a");
        data.push(0);
        data.push(0);
        data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0");
        assert_vsf_err(&data, "C64MEM module not found");
    }

    #[test]
    fn module_with_invalid_size() {
        let mut data = Vec::new();
        data.extend_from_slice(b"VICE Snapshot File\x1a");
        data.push(0);
        data.push(0);
        data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0");
        data.extend_from_slice(b"BADMODULE\0\0\0\0\0\0\0");
        data.push(0);
        data.push(0);
        data.extend_from_slice(&10u32.to_le_bytes());
        assert_vsf_err(&data, "Invalid module size");
    }

    #[test]
    fn module_truncated() {
        let mut data = Vec::new();
        data.extend_from_slice(b"VICE Snapshot File\x1a");
        data.push(0);
        data.push(0);
        data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0");
        data.extend_from_slice(b"C64MEM\0\0\0\0\0\0\0\0\0\0");
        data.push(0);
        data.push(0);
        data.extend_from_slice(&100_000u32.to_le_bytes());
        assert_vsf_err(&data, "truncated");
    }

    #[test]
    fn c64mem_too_small_data() {
        let mut data = Vec::new();
        data.extend_from_slice(b"VICE Snapshot File\x1a");
        data.push(0);
        data.push(0);
        data.extend_from_slice(b"C64\0\0\0\0\0\0\0\0\0\0\0\0\0");
        let module_data_size = 100;
        let total_size = 22 + module_data_size;
        data.extend_from_slice(b"C64MEM\0\0\0\0\0\0\0\0\0\0");
        data.push(0);
        data.push(0);
        data.extend_from_slice(&(total_size as u32).to_le_bytes());
        data.extend_from_slice(&vec![0u8; module_data_size]);
        assert_vsf_err(&data, "C64MEM module not found");
    }
}
