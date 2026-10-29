//! 6502 emulation-based binary unpacker for compressed Commodore 8-bit programs.
//!
//! Many Commodore 8-bit programs are distributed packed with tools like Dali, Exomizer,
//! PUCrunch, TSCrunch, or ByteBoozer. This module emulates the MOS 6502 CPU in a
//! target-system sandbox to run the packer's decompression stub, tracks modified
//! memory and bound provenance in [`UnpackReport`], and extracts the unpacked binary.
//!
//! The algorithm is inspired by the **unp64** utility and uses a two-phase state machine:
//!
//! - [`Phase::FindDepacker`](machine::Phase::FindDepacker) (Phase 1): Runs from the entry point
//!   until the program counter reaches the depacker stub or drops below the return boundary.
//! - [`Phase::Decompress`](machine::Phase::Decompress) (Phase 2): Runs the depacker stub while
//!   tracking RAM writes until execution jumps to the unpacked program or hits a trapped exit vector.

pub mod bus;
pub mod cia;
pub mod detector;
pub mod engine;
pub mod machine;

pub use crate::error::UnpackError;
pub use crate::packers::PackerId;
pub use bus::{C64Bus, MemoryAccessHook, UnpackerMemory};
pub use engine::{BoundSource, ExitReason, TRACE_TAIL_CAPACITY, UnpackEngine, UnpackReport};

/// Configuration for the unpacker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackConfig {
    /// Force a specific entry point (skip BASIC `SYS` parsing and packer `forced_entry` hints).
    pub forced_entry: Option<u16>,
    /// Force the depacker address recorded when transitioning into [`machine::Phase::Decompress`].
    pub forced_dep_addr: Option<u16>,
    /// Force the return address boundary (defaults to `load_addr.min(system.ram_start())`,
    /// e.g. `$0800` on C64).
    pub forced_ret_addr: Option<u16>,
    /// Maximum instructions across both phases before timeout (default: 50 million).
    pub max_instructions: u64,
    /// Optional 8 KB BASIC ROM image (`$A000`–`$BFFF` on C64).
    pub basic_rom: Option<Vec<u8>>,
    /// Optional 8 KB KERNAL ROM image (`$E000`–`$FFFF` on C64).
    pub kernal_rom: Option<Vec<u8>>,
    /// Optional 4 KB Character ROM image (`$D000`–`$DFFF` on C64).
    pub char_rom: Option<Vec<u8>>,
    /// Target system machine architecture (default: `None`, which defaults to C64 during execution).
    /// Controls memory boundary ceilings, default RAM and BASIC start addresses,
    /// hardware vector locations, and target-specific memory mapping during decompression emulation.
    pub target_system: Option<crate::state::types::System>,
    /// Record the last [`TRACE_TAIL_CAPACITY`] program counters before exit in
    /// [`UnpackReport::trace_tail`] (default: `false`).
    pub trace: bool,
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
            trace: false,
        }
    }
}

/// Result of a successful unpack operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackResult {
    /// The unpacked binary data (`mem[start_addr..=end_addr]`).
    pub data: Vec<u8>,
    /// Start address of the unpacked region.
    pub start_addr: u16,
    /// End address (inclusive) of the unpacked region.
    pub end_addr: u16,
    /// Entry point of the unpacked program.
    pub entry_point: u16,
    /// Address where the depacker code was found.
    pub dep_addr: u16,
    /// Total instructions executed across both phases.
    pub instructions_executed: u64,
    /// Stable identifier of the detected [`crate::packers::PackerSpec`], if any.
    pub packer: Option<PackerId>,
    /// Display name of the detected packer, if any.
    pub packer_name: Option<String>,
    /// Structured provenance report for how the unpacker reached its bounds and exit.
    pub report: UnpackReport,
}

/// Unpacks a compressed Commodore 8-bit binary using 6502 emulation.
///
/// # Arguments
///
/// * `raw_data` — the raw binary data (without the 2-byte PRG load address header)
/// * `load_addr` — the address where the binary is loaded in memory
/// * `config` — unpacker configuration
/// * `progress_callback` — optional callback invoked periodically with the instruction count
///
/// # Errors
///
/// Returns [`UnpackError`] if the input is empty, no entry point is found, nothing is written
/// to memory, the resolved entry point lies outside the output range, or emulation exceeds
/// [`UnpackConfig::max_instructions`] without completing.
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

    /// T1.A3.1: the `.d64` container path. The fixture is resolved from
    /// `CARGO_MANIFEST_DIR` so the test does not depend on the working
    /// directory, and a missing fixture fails the test instead of skipping it.
    #[test]
    fn test_unpack_moving_tubes_d64() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/6502/c64_moving_tubes_lxt.d64");
        let data = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("missing D64 fixture {}: {e}", path.display()));
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
        // T1.B.2: the output starts at the original load address, one byte
        // below the packed load address, so it keeps the original's first
        // byte (Design Doc Open Question 1).
        assert_eq!(result.start_addr, 0x0800, "Start address should be $0800");
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

        assert_eq!(
            result.data.len(),
            usize::from(result.end_addr - result.start_addr) + 1,
            "Unpacked data should cover exactly $0800-$31FF"
        );
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

    /// T4.1.13: [`UnpackResult`], [`UnpackReport`], [`BoundSource`], and [`ExitReason`]
    /// implement `Send + Clone + Debug`, and `UnpackResult::packer` carries `Option<PackerId>`.
    #[test]
    fn unpack_result_is_send_clone_debug() {
        fn assert_traits<T: Send + Clone + std::fmt::Debug>() {}

        assert_traits::<UnpackResult>();
        assert_traits::<UnpackReport>();
        assert_traits::<BoundSource>();
        assert_traits::<ExitReason>();

        let prg_data = std::fs::read("../../tests/6502/c64_moving_tubes_lxt.dali.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let result = unpack(&prg_data[2..], load_addr, &UnpackConfig::default(), None).unwrap();
        assert_eq!(result.packer, Some("dali"));
        assert_eq!(result.packer_name.as_deref(), Some("Dali"));
        assert_eq!(
            result.report.start_source,
            BoundSource::WriteDiff,
            "Dali start_addr comes from generic write diff"
        );
        assert_eq!(result.report.end_source, BoundSource::WriteDiff);
        assert_eq!(
            result.report.entry_source,
            BoundSource::PackerSpec { id: "dali" }
        );
    }
}
