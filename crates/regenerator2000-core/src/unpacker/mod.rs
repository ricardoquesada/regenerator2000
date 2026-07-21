//! 6502 emulation-based binary unpacker for compressed C64 programs.
//!
//! Many C64 programs are distributed packed with tools like Dali, Exomizer,
//! PUCrunch, etc. This module emulates the 6502 CPU to run the
//! packer's own decompression stub, then extracts the unpacked binary.
//!
//! The algorithm is based on the **unp64** utility and uses a two-phase approach:
//! - Phase 1: Find the depacker (runs from the SYS entry point until PC drops
//!   below the return address)
//! - Phase 2: Run decompression (continues until PC jumps back above the return
//!   address, indicating the depacker finished)

pub mod bus;
pub mod cia;
pub mod detector;
pub mod engine;

pub use crate::error::UnpackError;
pub use bus::{C64Bus, MemoryAccessHook, UnpackerMemory};
pub use engine::UnpackEngine;

/// Configuration for the unpacker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackConfig {
    /// Force a specific entry point (skip SYS parsing).
    pub forced_entry: Option<u16>,
    /// Force the depacker address.
    pub forced_dep_addr: Option<u16>,
    /// Force the return address boundary (default: `$0800`).
    pub forced_ret_addr: Option<u16>,
    /// Maximum instructions before timeout (default: 50 million).
    pub max_instructions: u64,
    /// Optional 8 KB BASIC ROM image (`$A000`–`$BFFF`).
    pub basic_rom: Option<Vec<u8>>,
    /// Optional 8 KB Kernal ROM image (`$E000`–`$FFFF`).
    pub kernal_rom: Option<Vec<u8>>,
    /// Optional 4 KB Character ROM image (`$D000`–`$DFFF`).
    pub char_rom: Option<Vec<u8>>,
    /// Target system machine architecture (default: `None`, which defaults to C64 during execution).
    /// Controls memory boundary ceilings, default RAM and BASIC start addresses,
    /// hardware vector locations, and target-specific memory mapping during decompression emulation.
    pub target_system: Option<crate::state::types::System>,
}

impl Default for UnpackConfig {
    fn default() -> Self {
        Self {
            forced_entry: None,
            forced_dep_addr: None,
            forced_ret_addr: None,
            max_instructions: 50_000_000,
            basic_rom: None,
            kernal_rom: None,
            char_rom: None,
            target_system: None,
        }
    }
}

/// Result of a successful unpack operation.
#[derive(Debug, Clone)]
pub struct UnpackResult {
    /// The unpacked binary data.
    pub data: Vec<u8>,
    /// Start address of the unpacked region.
    pub start_addr: u16,
    /// End address (inclusive) of the unpacked region.
    pub end_addr: u16,
    /// Entry point of the unpacked program (PC when Phase 2 exits).
    pub entry_point: u16,
    /// Address where the depacker code was found.
    pub dep_addr: u16,
    /// Total instructions executed across both phases.
    pub instructions_executed: u64,
    /// Name of the detected packer, if any.
    pub packer_name: Option<String>,
}

/// Unpacks a compressed C64 binary using 6502 emulation.
///
/// # Arguments
/// * `raw_data` — the raw binary data (without load address header)
/// * `load_addr` — the address where the binary is loaded in memory
/// * `config` — unpacker configuration
/// * `progress_callback` — optional callback invoked periodically with instruction count
///
/// # Errors
///
/// Returns [`UnpackError`] if the input is empty, no entry point is found,
/// or the emulation exceeds the instruction limit without completing.
pub fn unpack(
    raw_data: &[u8],
    load_addr: u16,
    config: &UnpackConfig,
    progress_callback: Option<&dyn Fn(u64)>,
) -> Result<UnpackResult, UnpackError> {
    UnpackEngine::new(config, progress_callback).run(raw_data, load_addr)
}

/// Scans memory for a BASIC `SYS` token and extracts the target jump address.
#[must_use]
pub fn find_sys_address(mem: &[u8], basic_start: u16) -> Option<u16> {
    crate::parser::basic::find_sys_address(mem, basic_start as usize, None, None)
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct KnownUnpackCase {
        file: &'static str,
        exp_start: u16,
        exp_end: u16,
        exp_entry: u16,
        exp_dep: Option<u16>,
        exp_packer: Option<&'static str>,
        max_instructions: Option<u64>,
    }

    #[test]
    fn test_unpack_known_prg_files() {
        use std::fs;

        let cases = [
            KnownUnpackCase {
                file: "c64_8_bit_ball.meanteam_cruncher.prg",
                exp_start: 0x0801,
                exp_end: 0xFF9E,
                exp_entry: 0x8100,
                exp_dep: None,
                exp_packer: None,
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_moving_tubes_lxt.dali.prg",
                exp_start: 0x0801,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: Some(0x0003),
                exp_packer: Some("Dali"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_moving_tubes_lxt.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: None,
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_moving_tubes_lxt.pucrunch.prg",
                exp_start: 0x0800,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: None,
                exp_packer: Some("PUCrunch"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_moving_tubes_lxt.tscrunch_x.prg",
                exp_start: 0x0800,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: Some(0x0002),
                exp_packer: Some("TSCrunch v1.3+"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_moving_tubes_lxt.tscrunch_x2.prg",
                exp_start: 0x0800,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: Some(0x0100),
                exp_packer: Some("TSCrunch v1.3+-X2"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.tscrunch_x.prg",
                exp_start: 0x0800,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: Some(0x0002),
                exp_packer: Some("TSCrunch v1.3+"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.tscrunch_x2.prg",
                exp_start: 0x0800,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: Some(0x0100),
                exp_packer: Some("TSCrunch v1.3+-X2"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.dali.prg",
                exp_start: 0x0801,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: None,
                exp_packer: Some("Dali"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: None,
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: Some(350_000_000),
            },
            KnownUnpackCase {
                file: "c64_mule.mccracken_compressor.prg",
                exp_start: 0x0800,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: None,
                exp_packer: Some("McCracken Compressor"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.pucrunch.prg",
                exp_start: 0x0800,
                exp_end: 0x9D19,
                exp_entry: 0x1100,
                exp_dep: None,
                exp_packer: Some("PUCrunch"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_roma.exe.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0xC8C5,
                exp_entry: 0x0820,
                exp_dep: Some(0x01B2),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_f600.exo.prg",
                exp_start: 0x0801,
                exp_end: 0xFEFF,
                exp_entry: 0x0810,
                exp_dep: Some(0x0134),
                exp_packer: Some("Exomizer 2.x"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_hw20131031.exo.prg",
                exp_start: 0x0801,
                exp_end: 0xF481,
                exp_entry: 0x3000,
                exp_dep: Some(0x0134),
                exp_packer: Some("Exomizer 2.x"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_traveller.tiny_crunch.prg",
                exp_start: 0x0801,
                exp_end: 0x7949,
                exp_entry: 0x0911,
                exp_dep: None,
                exp_packer: Some("TinyCrunch"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_spectro.exo3.prg",
                exp_start: 0x0801,
                exp_end: 0xE7FF,
                exp_entry: 0x08A1,
                exp_dep: None,
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_CopperBooze.byte_boozer2.prg",
                exp_start: 0x0800,
                exp_end: 0xE7FF,
                exp_entry: 0x1300,
                exp_dep: None,
                exp_packer: Some("ByteBoozer"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_cubicdream.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0xEF2A,
                exp_entry: 0x080D,
                exp_dep: Some(0x01B2),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_FppScroller.byte_boozer2.prg",
                exp_start: 0x0801,
                exp_end: 0xA057,
                exp_entry: 0x080D,
                exp_dep: Some(0x0010),
                exp_packer: Some("ByteBoozer"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_HBFS.exo3.prg",
                exp_start: 0x0801,
                exp_end: 0xFEFF,
                exp_entry: 0xEFB0,
                exp_dep: Some(0x01AB),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: Some(150_000_000),
            },
            KnownUnpackCase {
                file: "c64_Layers.exo3.prg",
                exp_start: 0x0801,
                exp_end: 0xFBF1,
                exp_entry: 0x0834,
                exp_dep: Some(0x01C4),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: Some(350_000_000),
            },
            KnownUnpackCase {
                file: "c64_connection-8580.pucrunch.prg",
                exp_start: 0x0801,
                exp_end: 0xF87D,
                exp_entry: 0x080D,
                exp_dep: Some(0x0116),
                exp_packer: Some("PUCrunch"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_lft-nine.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0x7CBC,
                exp_entry: 0x080D,
                exp_dep: Some(0x0198),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_lft-rodents-in-the-attic.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0xC56B,
                exp_entry: 0x080D,
                exp_dep: Some(0x01A1),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_little_things.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0x98FF,
                exp_entry: 0x080D,
                exp_dep: Some(0x01AB),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_robot - not human.exo3.prg",
                exp_start: 0x0800,
                exp_end: 0xCBE6,
                exp_entry: 0x0810,
                exp_dep: Some(0x01AB),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_bluemarble4k_unk.prg",
                exp_start: 0x0800,
                exp_end: 0xFFEF,
                exp_entry: 0x0911,
                exp_dep: Some(0x07E8),
                exp_packer: None,
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_boo_alz64.prg",
                exp_start: 0x0801,
                exp_end: 0x4D3C,
                exp_entry: 0x2A78,
                exp_dep: Some(0x005E),
                exp_packer: Some("ALZ64/Quiss"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_soul_on_fire_unk.prg",
                exp_start: 0x0801,
                exp_end: 0xE000,
                exp_entry: 0xE000,
                exp_dep: Some(0x005E),
                exp_packer: None,
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_323_ice_psm.1001_card_cruncher.prg",
                exp_start: 0x07C1,
                exp_end: 0x319C,
                exp_entry: 0x3197,
                exp_dep: Some(0x0100),
                exp_packer: Some("1001 CardCruncher ACM"),
                max_instructions: None,
            },
        ];

        for case in cases {
            let p1 = format!("tests/6502/{}", case.file);
            let p2 = format!("../../tests/6502/{}", case.file);
            let data = fs::read(&p1)
                .or_else(|_| fs::read(&p2))
                .unwrap_or_else(|e| panic!("Failed to read test PRG {}: {}", case.file, e));
            assert!(data.len() > 2, "File {} too small", case.file);
            let load_addr = u16::from_le_bytes([data[0], data[1]]);
            let config = UnpackConfig {
                max_instructions: case.max_instructions.unwrap_or(50_000_000),
                ..Default::default()
            };
            let res = unpack(&data[2..], load_addr, &config, None)
                .unwrap_or_else(|e| panic!("Failed to unpack {}: {e}", case.file));

            assert_eq!(
                res.start_addr, case.exp_start,
                "Start mismatch for {}",
                case.file
            );
            assert_eq!(res.end_addr, case.exp_end, "End mismatch for {}", case.file);
            assert_eq!(
                res.entry_point, case.exp_entry,
                "Entry mismatch for {}",
                case.file
            );
            if let Some(exp_dep) = case.exp_dep {
                assert_eq!(
                    res.dep_addr, exp_dep,
                    "Depacker addr mismatch for {}",
                    case.file
                );
            }
            if let Some(exp_packer) = case.exp_packer {
                assert_eq!(
                    res.packer_name.as_deref(),
                    Some(exp_packer),
                    "Packer name mismatch for {}",
                    case.file
                );
            }
        }
    }

    #[test]
    fn test_unpack_moving_tubes_d64() {
        use std::fs;
        let path = "../../tests/6502/c64_moving_tubes_lxt.d64";
        let data = match fs::read(path) {
            Ok(d) => d,
            Err(_) => return,
        };
        let entries = crate::parser::d64::parse_d64_directory(&data).expect("Should parse D64");
        let prg_entries: Vec<_> = entries
            .into_iter()
            .filter(|e| e.file_type == crate::parser::d64::FileType::PRG)
            .collect();
        assert!(!prg_entries.is_empty(), "D64 should contain PRG files");

        let mut unpacked_count = 0;
        for entry in &prg_entries {
            if let Ok(prg_bytes) = crate::parser::d64::extract_file(&data, entry)
                && prg_bytes.len() > 2
            {
                let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
                let config = UnpackConfig::default();
                if let Ok(res) = unpack(&prg_bytes[2..], load_addr, &config, None) {
                    assert!(res.start_addr <= res.entry_point && res.entry_point <= res.end_addr);
                    assert!(!res.data.is_empty());
                    unpacked_count += 1;
                }
            }
        }
        assert!(
            unpacked_count > 0,
            "At least one PRG in D64 should be unpacked"
        );
    }

    #[test]
    fn test_compare_moving_tubes_with_unp64_and_reference() {
        use std::fs;
        let ref_path = "../../tests/6502/c64_moving_tubes_lxt_2e00.prg";
        let ref_bytes = match fs::read(ref_path) {
            Ok(b) => b,
            Err(_) => return,
        };
        assert!(ref_bytes.len() > 2);
        let ref_load = u16::from_le_bytes([ref_bytes[0], ref_bytes[1]]);
        let ref_payload = &ref_bytes[2..];

        let test_files = [
            "c64_moving_tubes_lxt.dali.prg",
            "c64_moving_tubes_lxt.exo3.prg",
            "c64_moving_tubes_lxt.pucrunch.prg",
        ];

        for f in test_files {
            let path = format!("../../tests/6502/{f}");
            let data = fs::read(&path).unwrap();
            let load_addr = u16::from_le_bytes([data[0], data[1]]);
            let config = UnpackConfig::default();
            let res = unpack(&data[2..], load_addr, &config, None)
                .unwrap_or_else(|e| panic!("Failed to unpack {f}: {e}"));

            assert_eq!(res.entry_point, 0x2E00, "Entry point mismatch for {f}");

            let offset = (res.start_addr - ref_load) as usize;
            let compare_len = res.data.len().min(ref_payload.len().saturating_sub(offset));
            assert_eq!(
                &res.data[..compare_len],
                &ref_payload[offset..offset + compare_len],
                "Decompressed data for {f} does not match reference"
            );
        }

        let d64_path = "../../tests/6502/c64_moving_tubes_lxt.d64";
        let d64_data = fs::read(d64_path).unwrap();
        let entries = crate::parser::d64::parse_d64_directory(&d64_data).unwrap();
        for entry in &entries {
            if entry.file_type == crate::parser::d64::FileType::PRG {
                let prg_bytes = crate::parser::d64::extract_file(&d64_data, entry).unwrap();
                let load_addr = u16::from_le_bytes([prg_bytes[0], prg_bytes[1]]);
                let config = UnpackConfig::default();
                if let Ok(res) = unpack(&prg_bytes[2..], load_addr, &config, None) {
                    assert_eq!(res.entry_point, 0x2E00);
                    let offset = (res.start_addr - ref_load) as usize;
                    let compare_len = res.data.len().min(ref_payload.len().saturating_sub(offset));
                    assert_eq!(
                        &res.data[..compare_len],
                        &ref_payload[offset..offset + compare_len],
                        "D64 PRG unpacked data does not match reference"
                    );
                }
            }
        }
    }

    #[test]
    fn test_unpack_untracked_prg_files_with_unp64_comparison() {
        use std::fs;
        let cases = [
            (
                "c64_Bit_by_Bits-BZ!.exo3.prg",
                0x0800,
                0xEF83,
                0x083A,
                "c64_Bit_by_Bits-BZ!.exo3.prg.083a",
            ),
            (
                "c64_boilerplate.exo3.prg",
                0x0800,
                0xFEA4,
                0x1000,
                "c64_boilerplate.exo3.prg.1000",
            ),
            (
                "c64_druid_too.exo3.prg",
                0x0800,
                0xCE1F,
                0x4800,
                "c64_druid_too.exo3.prg.4800",
            ),
            (
                "c64_endoskull.exo3.prg",
                0x0800,
                0xFF29,
                0x0810,
                "c64_endoskull.exo3.prg.0810",
            ),
            (
                "c64_leftovers-pl.exo3.prg",
                0x0800,
                0xF3F0,
                0x080E,
                "c64_leftovers-pl.exo3.prg.080e",
            ),
            (
                "c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg",
                0x0800,
                0x9FFF,
                0x2800,
                "c64_radiant-every_time_i_go_on_pouet.byte_boozer2prg.prg.2800",
            ),
            (
                "c64_sprite_runners.exo3.prg",
                0x0800,
                0x95BF,
                0x6900,
                "c64_sprite_runners.exo3.prg.6900",
            ),
        ];

        for (f, exp_start, exp_end, exp_entry, unp64_out_file) in cases {
            let path = format!("../../tests/6502/{f}");
            let data = match fs::read(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            assert!(data.len() > 2);
            let load_addr = u16::from_le_bytes([data[0], data[1]]);
            let config = UnpackConfig::default();
            let res = unpack(&data[2..], load_addr, &config, None)
                .unwrap_or_else(|e| panic!("Failed to unpack {f}: {e}"));

            assert_eq!(res.start_addr, exp_start, "Start addr mismatch for {f}");
            assert_eq!(res.end_addr, exp_end, "End addr mismatch for {f}");
            assert_eq!(res.entry_point, exp_entry, "Entry point mismatch for {f}");
            assert!(
                res.start_addr <= res.entry_point && res.entry_point <= res.end_addr,
                "File {f}: entry ${:04X} outside range [${:04X}, ${:04X}]",
                res.entry_point,
                res.start_addr,
                res.end_addr
            );

            let unp_path = format!("../../tests/6502/{unp64_out_file}");
            if let Ok(unp_bytes) = fs::read(&unp_path) {
                let unp_payload = if unp_bytes.len() >= 2 {
                    &unp_bytes[2..]
                } else {
                    &unp_bytes[..]
                };
                let unp_start = if unp_bytes.len() >= 2 {
                    u16::from_le_bytes([unp_bytes[0], unp_bytes[1]])
                } else {
                    exp_start
                };
                let offset = (res.start_addr - unp_start) as usize;
                let compare_len = res.data.len().min(unp_payload.len().saturating_sub(offset));
                assert_eq!(
                    &res.data[..compare_len],
                    &unp_payload[offset..offset + compare_len],
                    "Decompressed data for {f} does not match unp64 output"
                );
            }
        }
    }

    fn make_basic_mem(tokens: &[u8]) -> Vec<u8> {
        let mut mem = vec![0u8; 0x1_0000];
        let next_line = 0x0805 + tokens.len() + 1;
        mem[0x0801] = (next_line & 0xFF) as u8;
        mem[0x0802] = (next_line >> 8) as u8;
        mem[0x0803] = 0x0A;
        mem[0x0804] = 0x00;
        for (i, &b) in tokens.iter().enumerate() {
            mem[0x0805 + i] = b;
        }
        mem[0x0805 + tokens.len()] = 0x00;
        mem
    }

    #[test]
    fn test_sys_simple() {
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_spaces() {
        let mem = make_basic_mem(&[0x9E, b' ', b' ', b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_parens() {
        let mem = make_basic_mem(&[0x9E, b'(', b'2', b'0', b'6', b'1', b')']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_addition() {
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAA, b'1', b'6']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2064));
    }

    #[test]
    fn test_sys_with_subtraction() {
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'7', b'0', 0xAB, b'9']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_multiplication() {
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAC, b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2048));
    }

    #[test]
    fn test_sys_not_found() {
        let mem = make_basic_mem(&[0x99, b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), None);
    }

    #[test]
    fn test_synthetic_xor_decryptor() {
        let mut raw = Vec::new();

        raw.extend_from_slice(&[
            0x14, 0x08, 0x0A, 0x00, 0x9E, b'2', b'0', b'6', b'2', 0x00, 0x00, 0x00,
        ]);

        while raw.len() < 0x0D {
            raw.push(0x00);
        }

        let depacker: Vec<u8> = vec![
            0xA2, 0x03, 0xBD, 0x14, 0x00, 0x49, 0xFF, 0x9D, 0x00, 0x09, 0xCA, 0x10, 0xF5, 0x4C,
            0x00, 0x09,
        ];
        let encrypted_data: Vec<u8> = vec![0x15, 0x15, 0x15, 0x9F];

        let total_copy_len = depacker.len() + encrypted_data.len();

        let source_addr: u16 = 0x081C;
        raw.extend_from_slice(&[
            0xA2,
            (total_copy_len - 1) as u8,
            0xBD,
            (source_addr & 0xFF) as u8,
            (source_addr >> 8) as u8,
            0x9D,
            0x03,
            0x00,
            0xCA,
            0x10,
            0xF7,
            0x4C,
            0x03,
            0x00,
        ]);

        let source_offset = (source_addr - 0x0801) as usize;
        while raw.len() < source_offset {
            raw.push(0x00);
        }

        raw.extend_from_slice(&depacker);
        raw.extend_from_slice(&encrypted_data);

        let config = UnpackConfig {
            max_instructions: 10_000,
            ..Default::default()
        };

        let result = unpack(&raw, 0x0801, &config, None).unwrap();
        assert_eq!(result.entry_point, 0x0900);
        assert_eq!(result.dep_addr, 0x0003);
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0x0903);
    }

    #[test]
    fn test_unpack_lxt_compressed() {
        let prg_data = std::fs::read("../../tests/6502/c64_moving_tubes_lxt.dali.prg").unwrap();

        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        assert_eq!(load_addr, 0x0801, "Expected load address $0801");

        let config = UnpackConfig::default();
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.dep_addr, 0x0003, "Depacker address should be $0003");
        assert_eq!(result.entry_point, 0x2E00, "Entry point should be $2E00");
        assert_eq!(result.start_addr, 0x0801, "Start address should be $0801");
        assert_eq!(result.end_addr, 0x31FF, "End address should be $31FF");

        assert!(
            result.instructions_executed > 100_000,
            "Expected >100K instructions, got {}",
            result.instructions_executed
        );
        assert!(
            result.instructions_executed < 1_000_000,
            "Expected <1M instructions, got {}",
            result.instructions_executed
        );

        assert!(
            result.data.len() > 1000,
            "Unpacked data should be >1KB, got {} bytes",
            result.data.len()
        );
    }

    fn find_unp64_bin() -> Option<std::path::PathBuf> {
        if let Ok(path) = std::env::var("UNP64_PATH").or_else(|_| std::env::var("UNP64_BIN")) {
            let p = std::path::PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(out) = std::process::Command::new("unp64").arg("-h").output()
            && (out.status.success() || !out.stdout.is_empty() || !out.stderr.is_empty())
        {
            return Some(std::path::PathBuf::from("unp64"));
        }
        None
    }

    #[test]
    fn test_unpack_gianna_sister_remix() {
        let prg_path = "../../tests/6502/c64_gianna_sister_remix_badboy.tbc_multicompactor.prg";
        let prg_data = std::fs::read(prg_path).unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];
        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0xC947);
        assert_eq!(result.entry_point, 0x0810);
        assert_eq!(result.dep_addr, 0x0100);

        if let Some(unp64_bin) = find_unp64_bin() {
            let tmp_out = std::env::temp_dir().join("gianna_sister_remix_unp64.prg");
            let status = std::process::Command::new(&unp64_bin)
                .arg(prg_path)
                .arg(&tmp_out)
                .status()
                .unwrap();
            if status.success() {
                let actual_out = if tmp_out.exists() {
                    tmp_out.clone()
                } else {
                    std::path::PathBuf::from(format!("{}.0810", tmp_out.display()))
                };
                if actual_out.exists() {
                    let unp64_bytes = std::fs::read(&actual_out).unwrap();
                    let unp64_load_addr = u16::from_le_bytes([unp64_bytes[0], unp64_bytes[1]]);
                    assert_eq!(result.start_addr, unp64_load_addr);
                    assert_eq!(result.data, &unp64_bytes[2..]);
                    let _ = std::fs::remove_file(actual_out);
                }
            }
        }
    }

    #[test]
    fn test_unpack_fantasy_intro() {
        let prg_path = "../../tests/6502/c64_fantasy_intro.eca_compactor.prg";
        let prg_data = std::fs::read(prg_path).unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];
        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();
        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0x37FF);
        assert_eq!(result.entry_point, 0x3000);
        assert_eq!(result.dep_addr, 0x0100);

        if let Some(unp64_bin) = find_unp64_bin() {
            let tmp_out = std::env::temp_dir().join("fantasy_intro_unp64.prg");
            let status = std::process::Command::new(&unp64_bin)
                .arg(prg_path)
                .arg("-o")
                .arg(&tmp_out)
                .status()
                .unwrap();
            if status.success() {
                let actual_out = if tmp_out.exists() {
                    tmp_out.clone()
                } else {
                    std::path::PathBuf::from(format!("{}.3000", tmp_out.display()))
                };
                if actual_out.exists() {
                    let unp64_bytes = std::fs::read(&actual_out).unwrap();
                    let unp64_load_addr = u16::from_le_bytes([unp64_bytes[0], unp64_bytes[1]]);
                    assert_eq!(result.start_addr, unp64_load_addr);
                    assert_eq!(result.data, &unp64_bytes[2..]);
                    let _ = std::fs::remove_file(actual_out);
                }
            }
        }
    }

    #[test]
    fn test_unpack_time_cruncher_debug() {
        let prg_path = "../../tests/6502/c64_thats_the_way_scoop.time_cruncher.prg";
        let prg_data = std::fs::read(prg_path).unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];
        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let res = unpack(raw_data, load_addr, &config, None).unwrap();
        assert_eq!(res.start_addr, 0x0801);
        assert_eq!(res.end_addr, 0xE750);
        assert_eq!(res.entry_point, 0x0801);
        assert_eq!(res.dep_addr, 0x0100);

        if let Some(unp64_bin) = find_unp64_bin() {
            let tmp_out = std::env::temp_dir().join("time_cruncher_unp64_diff.prg");
            let status = std::process::Command::new(&unp64_bin)
                .arg(prg_path)
                .arg("-o")
                .arg(&tmp_out)
                .status()
                .unwrap();
            if status.success() && tmp_out.exists() {
                let unp64_bytes = std::fs::read(&tmp_out).unwrap();
                let unp64_payload = &unp64_bytes[2..];
                assert_eq!(
                    res.start_addr,
                    u16::from_le_bytes([unp64_bytes[0], unp64_bytes[1]])
                );
                assert_eq!(res.data.len(), unp64_payload.len());
                let _ = std::fs::remove_file(tmp_out);
            }
        }
    }

    #[test]
    fn test_unpack_c64_chiller_antiram() {
        let prg_path = "../../tests/6502/c64_chiller.antiram_packer.prg";
        let prg_data = std::fs::read(prg_path).unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];
        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();
        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0xCE00);
        assert_eq!(result.entry_point, 0x0818);
        assert_eq!(result.dep_addr, 0xFF00);

        if let Some(unp64_bin) = find_unp64_bin() {
            let tmp_out = std::env::temp_dir().join("chiller_unp64.prg");
            let status = std::process::Command::new(&unp64_bin)
                .arg(prg_path)
                .arg("-o")
                .arg(&tmp_out)
                .status()
                .unwrap();
            if status.success() {
                let actual_out = if tmp_out.exists() {
                    tmp_out.clone()
                } else {
                    std::path::PathBuf::from(format!("{}.0818", tmp_out.display()))
                };
                if actual_out.exists() {
                    let unp64_bytes = std::fs::read(&actual_out).unwrap();
                    let unp64_load_addr = u16::from_le_bytes([unp64_bytes[0], unp64_bytes[1]]);
                    assert_eq!(result.start_addr, unp64_load_addr);
                    assert_eq!(result.data, &unp64_bytes[2..]);
                    let _ = std::fs::remove_file(actual_out);
                }
            }
        }
    }

    #[test]
    fn test_sbx_flags_behavior() {
        use mos6502::cpu::CPU;
        use mos6502::instruction::Nmos6502;
        let mut memory =
            UnpackerMemory::new(crate::state::types::default_system(), None, None, None);
        memory.mem[0] = 0xCB; // SBX #$F8
        memory.mem[1] = 0xF8;
        let mut cpu = CPU::new(memory, Nmos6502);

        cpu.registers.accumulator = 0xD8;
        cpu.registers.index_x = 0xD8;
        cpu.registers
            .status
            .set(mos6502::registers::Status::PS_CARRY, true);
        cpu.single_step();
        let carry1 = cpu
            .registers
            .status
            .contains(mos6502::registers::Status::PS_CARRY);
        println!("Test 1: X={:02X}, Carry={}", cpu.registers.index_x, carry1);

        cpu.registers.program_counter = 0;
        cpu.registers.accumulator = 0xF8;
        cpu.registers.index_x = 0xF8;
        cpu.registers
            .status
            .set(mos6502::registers::Status::PS_CARRY, false);
        cpu.single_step();
        let carry2 = cpu
            .registers
            .status
            .contains(mos6502::registers::Status::PS_CARRY);
        println!("Test 2: X={:02X}, Carry={}", cpu.registers.index_x, carry2);
    }
}
