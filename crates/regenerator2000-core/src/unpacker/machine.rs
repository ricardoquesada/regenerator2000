//! Machine profile of the unpacker sandbox.
//!
//! This module owns the platform knowledge that the emulation engine needs,
//! keyed by [`System`]:
//!
//! - [`RomTrapTable`]: the BASIC and KERNAL ROM addresses the sandbox
//!   intercepts instead of executing, and what each one does.
//! - [`KernalRoutine`]: the emulated effects of the trapped KERNAL routines.
//! - [`get_basic_start`]: where the BASIC program of a loaded PRG starts.
//!
//! The engine keeps the *control flow* around a trap: ROM banking, the
//! `written` bitmap, the forced `RTS`, and what an exit means for the result.
//!
//! # Behavior contract
//!
//! The tables reproduce the engine's historical behavior exactly, for every
//! [`System`] (Orchestrator rulings R4, R5, R7 and R18). In particular, the
//! generic KERNAL traps apply to every system, including the drives and
//! `Custom` systems, and the BASIC start convention does not depend on the
//! system. Any system-specific improvement is a deliberate, separately
//! reviewed behavior change.

use std::ops::RangeInclusive;

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;

use super::bus::C64Bus;
use crate::state::types::System;

// ===========================================================================
// Emulation phase
// ===========================================================================

/// Which half of the unp64-style algorithm the CPU is in.
///
/// Some traps only fire in [`Phase::Decompress`]. Example: on the C64, a jump
/// into the KERNAL IRQ handler (`$EA31`) ends decompression, but while the
/// depacker is still being located it is just a forced `RTS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Running from the entry point until the depacker is found.
    FindDepacker,
    /// Running the depacker until it hands control to the unpacked program.
    Decompress,
}

// ===========================================================================
// Trap vocabulary
// ===========================================================================

/// A KERNAL routine that the sandbox emulates instead of executing.
///
/// The effects are deliberately minimal: just enough register and memory
/// state for depackers that call these routines to carry on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernalRoutine {
    /// GETIN: returns the next simulated key press in A
    /// ([`GETIN_RESPONSES`], cycling).
    Getin,
    /// Clear screen / CINT: clears A, X and Y, and fills the system's screen
    /// RAM with spaces when the table says so
    /// ([`RomTrapTable::clears_screen_ram`]).
    ClearScreen,
    /// CHROUT: the character is dropped, except that
    /// [`PETSCII_CLEAR_SCREEN`] clears the screen like
    /// [`KernalRoutine::ClearScreen`] does (registers untouched).
    Chrout,
    /// SETNAM: stores A (name length) and X/Y (name pointer) in zero page.
    Setnam,
    /// IOINIT: leaves the processor port and registers as the real routine
    /// does.
    Ioinit,
    /// RESTOR: leaves the registers as the real routine does.
    Restor,
}

/// What a trapped KERNAL address does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernalTrap {
    /// Emulate the routine, then return to the caller.
    Routine(KernalRoutine),
    /// Stop emulating: the program left the depacker through an exit vector
    /// (LOAD, reset) or, while decompressing, the IRQ handler.
    Exit,
}

/// One trapped KERNAL address.
///
/// # Example
///
/// `KernalTrapEntry { addr: 0xFFD2, trap: KernalTrap::Routine(KernalRoutine::Chrout) }`
/// emulates every call to CHROUT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernalTrapEntry {
    /// Trapped program counter value.
    pub addr: u16,
    /// What happens when execution reaches [`Self::addr`].
    pub trap: KernalTrap,
}

// ===========================================================================
// Emulated routine constants
// ===========================================================================

/// Simulated GETIN responses, returned in order and then cycled.
///
/// Key presses alternate with "no key" (`$00`) so that "press any key" loops
/// and "wait for release" loops both make progress.
pub const GETIN_RESPONSES: [u8; 14] = [
    0x20, // SPACE
    0x00, 0x4E, // N
    0x00, 0x03, // RUN/STOP
    0x00, 0x5F, // ←
    0x00, 0x11, // CRSR-DOWN
    0x00, 0x0D, // RETURN
    0x00, 0x31, // 1
    0x00,
];

/// PETSCII "clear screen" control code. CHROUT clears the screen for it.
pub const PETSCII_CLEAR_SCREEN: u8 = 0x93;

/// Screen code of a space, which fills a cleared screen.
pub const SCREEN_CODE_SPACE: u8 = 0x20;

/// Zero-page address of the file name length (SETNAM's A).
const ZP_FILE_NAME_LEN: usize = 0xB7;
/// Zero-page address of the file name pointer, low byte (SETNAM's X).
const ZP_FILE_NAME_PTR_LO: usize = 0xBB;
/// Zero-page address of the file name pointer, high byte (SETNAM's Y).
const ZP_FILE_NAME_PTR_HI: usize = 0xBC;

/// Processor port (`$01`) address.
const PROCESSOR_PORT: usize = 0x01;
/// Processor port value IOINIT leaves behind.
const IOINIT_PROCESSOR_PORT: u8 = 0xE7;
/// A and X as IOINIT leaves them (Y is untouched).
const IOINIT_A: u8 = 0xD7;
const IOINIT_X: u8 = 0xFF;

/// A, X and Y as RESTOR leaves them.
const RESTOR_A: u8 = 0x31;
const RESTOR_X: u8 = 0x30;
const RESTOR_Y: u8 = 0xFF;

impl KernalRoutine {
    /// Applies the routine's effects to `cpu`. The caller performs the
    /// return to the calling code (the forced `RTS`).
    ///
    /// `getin_index` is the position in [`GETIN_RESPONSES`]; it persists
    /// across calls within one emulation run.
    pub(crate) fn emulate(
        self,
        cpu: &mut CPU<C64Bus, Nmos6502>,
        traps: &RomTrapTable,
        getin_index: &mut usize,
    ) {
        let registers = &mut cpu.registers;
        match self {
            Self::Getin => {
                registers.accumulator = GETIN_RESPONSES[*getin_index % GETIN_RESPONSES.len()];
                *getin_index += 1;
            }
            Self::ClearScreen => {
                traps.clear_screen(&mut cpu.memory.mem, &cpu.memory.system);
                registers.accumulator = 0x00;
                registers.index_x = 0x00;
                registers.index_y = 0x00;
            }
            Self::Chrout => {
                if registers.accumulator == PETSCII_CLEAR_SCREEN {
                    traps.clear_screen(&mut cpu.memory.mem, &cpu.memory.system);
                }
            }
            Self::Setnam => {
                let mem = &mut cpu.memory.mem;
                mem[ZP_FILE_NAME_LEN] = registers.accumulator;
                mem[ZP_FILE_NAME_PTR_LO] = registers.index_x;
                mem[ZP_FILE_NAME_PTR_HI] = registers.index_y;
            }
            Self::Ioinit => {
                cpu.memory.mem[PROCESSOR_PORT] = IOINIT_PROCESSOR_PORT;
                registers.accumulator = IOINIT_A;
                registers.index_x = IOINIT_X;
            }
            Self::Restor => {
                registers.accumulator = RESTOR_A;
                registers.index_x = RESTOR_X;
                registers.index_y = RESTOR_Y;
            }
        }
    }
}

// ===========================================================================
// Trap tables
// ===========================================================================

/// KERNAL ROM entry points the sandbox intercepts for one target machine,
/// plus the BASIC ROM addresses with special meaning.
///
/// The engine consults the table only for program counters inside a mapped
/// BASIC or KERNAL ROM (see `engine::handle_rom_entry` for the full order).
/// A ROM address with no entry gets a forced `RTS`.
///
/// # Example
///
/// ```
/// use regenerator2000_core::state::types::TargetSystem;
/// use regenerator2000_core::unpacker::machine::{KernalRoutine, KernalTrap, Phase, RomTrapTable};
///
/// let c64 = RomTrapTable::for_system(&TargetSystem::C64);
/// assert!(c64.is_basic_run_entry(0xA7AE)); // BASIC RUN
/// assert_eq!(
///     c64.kernal_trap(0xFFD2, Phase::FindDepacker),
///     Some(KernalTrap::Routine(KernalRoutine::Chrout))
/// );
/// // The IRQ handler only ends decompression.
/// assert_eq!(c64.kernal_trap(0xEA31, Phase::FindDepacker), None);
/// assert_eq!(c64.kernal_trap(0xEA31, Phase::Decompress), Some(KernalTrap::Exit));
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct RomTrapTable {
    /// BASIC interpreter entries that mean "run the BASIC program". The
    /// engine then takes the entry point from the SYS line. Checked before
    /// the banking and `written` checks, because packers jump here with RAM
    /// banked in.
    pub basic_run_entries: &'static [u16],
    /// BASIC ROM range that ends decompression ([`Phase::Decompress`] only).
    pub basic_exit_range: Option<RangeInclusive<u16>>,
    /// Single BASIC ROM addresses that end decompression, outside
    /// [`Self::basic_exit_range`] ([`Phase::Decompress`] only).
    pub basic_exit_addrs: &'static [u16],
    /// Trapped KERNAL addresses, as layers searched in order (first match
    /// wins; addresses are unique across layers, so the order is not
    /// significant). Every table includes [`GENERIC_KERNAL_TRAPS`].
    pub kernal_traps: &'static [&'static [KernalTrapEntry]],
    /// KERNAL IRQ handler range that ends decompression
    /// ([`Phase::Decompress`] only).
    pub irq_handler: Option<RangeInclusive<u16>>,
    /// Whether the clear-screen traps fill [`System::screen_ram`] with
    /// [`SCREEN_CODE_SPACE`]. Only the C64 table does (ruling R5).
    pub clears_screen_ram: bool,
}

/// KERNAL traps shared by every system, at their KERNAL jump table
/// addresses: GETIN `$FFE4`, CINT `$FF5B`, CHROUT `$FFD2`, SETNAM `$FFBD`
/// and LOAD `$FFD5` (exit vector).
///
/// The engine has always trapped these on every system whose KERNAL range
/// contains them, which is every [`System`] (ruling R18).
pub const GENERIC_KERNAL_TRAPS: [KernalTrapEntry; 5] = [
    KernalTrapEntry {
        addr: 0xFFE4,
        trap: KernalTrap::Routine(KernalRoutine::Getin),
    },
    KernalTrapEntry {
        addr: 0xFF5B,
        trap: KernalTrap::Routine(KernalRoutine::ClearScreen),
    },
    KernalTrapEntry {
        addr: 0xFFD2,
        trap: KernalTrap::Routine(KernalRoutine::Chrout),
    },
    KernalTrapEntry {
        addr: 0xFFBD,
        trap: KernalTrap::Routine(KernalRoutine::Setnam),
    },
    KernalTrapEntry {
        addr: 0xFFD5,
        trap: KernalTrap::Exit,
    },
];

/// C64-only KERNAL traps: routine bodies that depackers call directly,
/// bypassing the jump table, and the reset entry.
pub const C64_KERNAL_TRAPS: [KernalTrapEntry; 7] = [
    // GETIN body.
    KernalTrapEntry {
        addr: 0xF13E,
        trap: KernalTrap::Routine(KernalRoutine::Getin),
    },
    // Clear-screen entry points.
    KernalTrapEntry {
        addr: 0xE536,
        trap: KernalTrap::Routine(KernalRoutine::ClearScreen),
    },
    KernalTrapEntry {
        addr: 0xE544,
        trap: KernalTrap::Routine(KernalRoutine::ClearScreen),
    },
    // IOINIT body.
    KernalTrapEntry {
        addr: 0xFDA3,
        trap: KernalTrap::Routine(KernalRoutine::Ioinit),
    },
    // RESTOR body.
    KernalTrapEntry {
        addr: 0xFD15,
        trap: KernalTrap::Routine(KernalRoutine::Restor),
    },
    // LOAD body (exit vector).
    KernalTrapEntry {
        addr: 0xF4A2,
        trap: KernalTrap::Exit,
    },
    // Reset routine, historically the engine's "warm start" exit vector.
    KernalTrapEntry {
        addr: 0xFCE2,
        trap: KernalTrap::Exit,
    },
];

/// C64 BASIC entries that run the BASIC program (RUN at `$A7AE` and the
/// interpreter loop and statement dispatch addresses packers jump to).
pub const C64_BASIC_RUN_ENTRIES: [u16; 9] = [
    0xA7AE, 0xA7B1, 0xA7EA, 0xA474, 0xA483, 0xA533, 0xA871, 0xA888, 0xA8BC,
];

/// The C64 machine profile.
static C64_TRAPS: RomTrapTable = RomTrapTable {
    basic_run_entries: &C64_BASIC_RUN_ENTRIES,
    basic_exit_range: Some(0xA57C..=0xA659),
    basic_exit_addrs: &[0xA660, 0xA68E],
    kernal_traps: &[&GENERIC_KERNAL_TRAPS, &C64_KERNAL_TRAPS],
    irq_handler: Some(0xEA31..=0xEB76),
    clears_screen_ram: true,
};

/// The profile of every other system: the generic KERNAL traps only, no
/// BASIC traps and no screen fill.
static GENERIC_TRAPS: RomTrapTable = RomTrapTable {
    basic_run_entries: &[],
    basic_exit_range: None,
    basic_exit_addrs: &[],
    kernal_traps: &[&GENERIC_KERNAL_TRAPS],
    irq_handler: None,
    clears_screen_ram: false,
};

impl RomTrapTable {
    /// Returns the trap table for `system`.
    ///
    /// The C64 profile is selected with [`System::is_c64`], the predicate the
    /// bus and the engine use for C64 banking. Every other system, including
    /// the drives and `Custom` systems (even one named like the C64), gets
    /// the generic profile ([`GENERIC_KERNAL_TRAPS`]), because the engine has
    /// always trapped those addresses on every system (ruling R18).
    #[must_use]
    pub fn for_system(system: &System) -> &'static RomTrapTable {
        if system.is_c64() {
            &C64_TRAPS
        } else {
            &GENERIC_TRAPS
        }
    }

    /// Returns `true` if `pc` is a BASIC entry that runs the BASIC program.
    #[must_use]
    pub fn is_basic_run_entry(&self, pc: u16) -> bool {
        self.basic_run_entries.contains(&pc)
    }

    /// Returns `true` if reaching BASIC ROM address `pc` in `phase` ends
    /// emulation.
    #[must_use]
    pub fn is_basic_exit(&self, pc: u16, phase: Phase) -> bool {
        if phase != Phase::Decompress {
            return false;
        }
        self.basic_exit_range
            .as_ref()
            .is_some_and(|range| range.contains(&pc))
            || self.basic_exit_addrs.contains(&pc)
    }

    /// Returns what reaching KERNAL ROM address `pc` in `phase` does, or
    /// `None` if it is not trapped (the engine then forces an `RTS`).
    #[must_use]
    pub fn kernal_trap(&self, pc: u16, phase: Phase) -> Option<KernalTrap> {
        let listed = self
            .kernal_traps
            .iter()
            .flat_map(|layer| layer.iter())
            .find(|entry| entry.addr == pc)
            .map(|entry| entry.trap);
        if listed.is_some() {
            return listed;
        }
        let in_irq_handler = self
            .irq_handler
            .as_ref()
            .is_some_and(|range| range.contains(&pc));
        (phase == Phase::Decompress && in_irq_handler).then_some(KernalTrap::Exit)
    }

    /// Fills `system`'s screen RAM in `mem` with [`SCREEN_CODE_SPACE`] if this
    /// table clears the screen ([`Self::clears_screen_ram`]); otherwise, or
    /// if the system has no screen RAM, leaves `mem` unchanged.
    pub fn clear_screen(&self, mem: &mut [u8], system: &System) {
        if !self.clears_screen_ram {
            return;
        }
        let Some(screen) = system.screen_ram() else {
            return;
        };
        let range = usize::from(*screen.start())..=usize::from(*screen.end());
        if let Some(cells) = mem.get_mut(range) {
            cells.fill(SCREEN_CODE_SPACE);
        }
    }
}

// ===========================================================================
// BASIC start
// ===========================================================================

/// Load address of the Card Cruncher convention (ruling R7): a PRG loaded at
/// `$07C0` starts its BASIC line one byte later, at `$07C1`.
pub const CARD_CRUNCHER_LOAD_ADDR: u16 = 0x07C0;

/// The BASIC start of a PRG loaded at `load_addr`.
///
/// This is the single source of truth for the BASIC start. The engine uses
/// it, and so does the corpus classifier's `padding_byte` rule (ruling R16).
///
/// A PRG loaded one byte below the C64's default BASIC start (the `$0800`
/// padding byte, derived from [`System::default_basic_start`] of the C64), or
/// at [`CARD_CRUNCHER_LOAD_ADDR`], starts its BASIC line at the next byte. Any
/// other PRG starts it at its load address.
///
/// The convention is the same for every target system (ruling R18): the
/// engine has always applied it regardless of the system, so, for example, a
/// C128 load at `$1C00` gives `$1C00`, not the C128's `$1C01`. A
/// system-aware BASIC start is a recorded follow-up, not part of this
/// behavior-preserving profile.
///
/// # Example
///
/// ```
/// use regenerator2000_core::unpacker::machine::get_basic_start;
///
/// assert_eq!(get_basic_start(0x0800), 0x0801); // C64 padding byte
/// assert_eq!(get_basic_start(0x0801), 0x0801);
/// assert_eq!(get_basic_start(0x07C0), 0x07C1); // Card Cruncher
/// assert_eq!(get_basic_start(0x1000), 0x1000);
/// ```
#[must_use]
pub fn get_basic_start(load_addr: u16) -> u16 {
    let c64_padding_byte = System::C64.default_basic_start().checked_sub(1);
    let padded = load_addr == CARD_CRUNCHER_LOAD_ADDR || c64_padding_byte == Some(load_addr);
    if padded {
        // A padded load is `$07C0` or `$0800`, so this never wraps; the
        // wrapping form keeps the function total.
        load_addr.wrapping_add(1)
    } else {
        load_addr
    }
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::TargetSystem;

    fn non_c64_systems() -> [TargetSystem; 11] {
        [
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Pet20,
            TargetSystem::Pet40,
            TargetSystem::Plus4,
            TargetSystem::C16,
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
            TargetSystem::Custom("Commodore 64".into()),
            TargetSystem::Custom("Homebrew 6502".into()),
        ]
    }

    /// Every `(pc, phase)` whose KERNAL trap is `Some`, over the whole
    /// address space.
    fn trapped(table: &RomTrapTable, phase: Phase) -> Vec<(u16, KernalTrap)> {
        (0..=u16::MAX)
            .filter_map(|pc| table.kernal_trap(pc, phase).map(|trap| (pc, trap)))
            .collect()
    }

    fn sorted(mut traps: Vec<(u16, KernalTrap)>) -> Vec<(u16, KernalTrap)> {
        traps.sort_by_key(|&(pc, _)| pc);
        traps
    }

    fn routine(addr: u16, routine: KernalRoutine) -> (u16, KernalTrap) {
        (addr, KernalTrap::Routine(routine))
    }

    /// The generic traps, as `(pc, trap)` pairs.
    fn generic_traps() -> Vec<(u16, KernalTrap)> {
        sorted(vec![
            routine(0xFFE4, KernalRoutine::Getin),
            routine(0xFF5B, KernalRoutine::ClearScreen),
            routine(0xFFD2, KernalRoutine::Chrout),
            routine(0xFFBD, KernalRoutine::Setnam),
            (0xFFD5, KernalTrap::Exit),
        ])
    }

    /// T2.1.1: exactly the nine BASIC RUN entries, and nothing next to them.
    #[test]
    fn c64_basic_run_entries_exact() {
        let c64 = RomTrapTable::for_system(&TargetSystem::C64);
        let expected = [
            0xA7AE, 0xA7B1, 0xA7EA, 0xA474, 0xA483, 0xA533, 0xA871, 0xA888, 0xA8BC,
        ];
        assert_eq!(c64.basic_run_entries, expected);
        let all: Vec<u16> = (0..=u16::MAX)
            .filter(|&pc| c64.is_basic_run_entry(pc))
            .collect();
        let mut expected_sorted = expected.to_vec();
        expected_sorted.sort_unstable();
        assert_eq!(all, expected_sorted);
    }

    /// T2.1.2: the BASIC exit is `$A57C..=$A659`, `$A660` and `$A68E`, only
    /// while decompressing, and nothing else.
    #[test]
    fn c64_basic_exit_covers_range_and_singletons() {
        let c64 = RomTrapTable::for_system(&TargetSystem::C64);
        let decompress: Vec<u16> = (0..=u16::MAX)
            .filter(|&pc| c64.is_basic_exit(pc, Phase::Decompress))
            .collect();
        let mut expected: Vec<u16> = (0xA57C..=0xA659).collect();
        expected.extend([0xA660, 0xA68E]);
        assert_eq!(decompress, expected);
        assert!(
            (0..=u16::MAX).all(|pc| !c64.is_basic_exit(pc, Phase::FindDepacker)),
            "no BASIC exit while finding the depacker"
        );
    }

    /// T2.1.3: GETIN, clear screen, CHROUT, SETNAM, IOINIT, RESTOR, the
    /// LOAD and reset exits, and the IRQ range (decompress only), exactly.
    #[test]
    fn c64_kernal_traps_exact() {
        let c64 = RomTrapTable::for_system(&TargetSystem::C64);
        let mut listed = generic_traps();
        listed.extend([
            routine(0xF13E, KernalRoutine::Getin),
            routine(0xE536, KernalRoutine::ClearScreen),
            routine(0xE544, KernalRoutine::ClearScreen),
            routine(0xFDA3, KernalRoutine::Ioinit),
            routine(0xFD15, KernalRoutine::Restor),
            (0xF4A2, KernalTrap::Exit),
            (0xFCE2, KernalTrap::Exit),
        ]);
        let listed = sorted(listed);
        assert_eq!(trapped(c64, Phase::FindDepacker), listed);

        let mut with_irq = listed;
        with_irq.extend((0xEA31..=0xEB76).map(|pc| (pc, KernalTrap::Exit)));
        assert_eq!(trapped(c64, Phase::Decompress), sorted(with_irq));
        assert!(c64.clears_screen_ram);
    }

    /// Addresses are unique across a table's layers, so the search order
    /// cannot change behavior.
    #[test]
    fn kernal_trap_addresses_are_unique() {
        for table in [&C64_TRAPS, &GENERIC_TRAPS] {
            let mut addrs: Vec<u16> = table
                .kernal_traps
                .iter()
                .flat_map(|layer| layer.iter().map(|entry| entry.addr))
                .collect();
            let count = addrs.len();
            addrs.sort_unstable();
            addrs.dedup();
            assert_eq!(addrs.len(), count, "{table:?}");
        }
    }

    /// T2.1.4: on every non-C64 system only the five generic KERNAL traps
    /// apply, in both phases, with no BASIC traps and no screen fill.
    #[test]
    fn non_c64_trap_table_matches_todays_guards() {
        for system in non_c64_systems() {
            let table = RomTrapTable::for_system(&system);
            for phase in [Phase::FindDepacker, Phase::Decompress] {
                assert_eq!(trapped(table, phase), generic_traps(), "{system} {phase:?}");
                assert!(
                    (0..=u16::MAX).all(|pc| !table.is_basic_exit(pc, phase)),
                    "{system} {phase:?}"
                );
            }
            assert!(
                (0..=u16::MAX).all(|pc| !table.is_basic_run_entry(pc)),
                "{system}"
            );
            assert!(!table.clears_screen_ram, "{system}");
        }
    }

    /// T2.1.5: the C64 profile follows `is_c64()`, not `PartialEq`.
    #[test]
    fn for_system_uses_is_c64_predicate() {
        let c64 = RomTrapTable::for_system(&TargetSystem::C64);
        assert!(std::ptr::eq(c64, &C64_TRAPS));
        assert!(std::ptr::eq(
            RomTrapTable::for_system(&TargetSystem::new("c64")),
            &C64_TRAPS
        ));

        let lookalike = TargetSystem::Custom("Commodore 64".into());
        assert_eq!(lookalike, TargetSystem::C64, "equal under PartialEq");
        assert!(!lookalike.is_c64());
        assert!(std::ptr::eq(
            RomTrapTable::for_system(&lookalike),
            &GENERIC_TRAPS
        ));
    }

    /// T2.1.6 (ruling R18): the drives and `Custom` systems get the generic
    /// table, not an empty one, because the engine has always trapped the
    /// generic KERNAL addresses there.
    #[test]
    fn for_system_drive_and_custom_return_generic_table() {
        let systems = [
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
            TargetSystem::Custom("Homebrew 6502".into()),
        ];
        for system in systems {
            let table = RomTrapTable::for_system(&system);
            assert!(std::ptr::eq(table, &GENERIC_TRAPS), "{system}");
            assert_eq!(table.kernal_traps, [&GENERIC_KERNAL_TRAPS[..]], "{system}");
            assert!(table.basic_run_entries.is_empty(), "{system}");
            assert_eq!(table.basic_exit_range, None, "{system}");
            assert!(table.basic_exit_addrs.is_empty(), "{system}");
            assert_eq!(table.irq_handler, None, "{system}");
        }
    }

    // -----------------------------------------------------------------------
    // Clear-screen effects (ruling R5)
    // -----------------------------------------------------------------------

    const START_A: u8 = 0x93;
    const START_X: u8 = 0x5A;
    const START_Y: u8 = 0xA5;
    /// Background byte for memory, so a fill is visible.
    const BACKGROUND: u8 = 0xAA;

    fn cpu_for(system: TargetSystem, a: u8) -> CPU<C64Bus, Nmos6502> {
        let mut bus = C64Bus::new(system, None, None, None);
        bus.mem.fill(BACKGROUND);
        let mut cpu = CPU::new(bus, Nmos6502);
        cpu.registers.accumulator = a;
        cpu.registers.index_x = START_X;
        cpu.registers.index_y = START_Y;
        cpu
    }

    fn emulate(cpu: &mut CPU<C64Bus, Nmos6502>, routine: KernalRoutine) {
        let traps = RomTrapTable::for_system(&cpu.memory.system);
        let mut getin_index = 0;
        routine.emulate(cpu, traps, &mut getin_index);
    }

    /// Addresses whose value is not [`BACKGROUND`].
    fn changed(cpu: &CPU<C64Bus, Nmos6502>) -> Vec<usize> {
        (0..cpu.memory.mem.len())
            .filter(|&addr| cpu.memory.mem[addr] != BACKGROUND)
            .collect()
    }

    fn screen_addrs(system: &TargetSystem) -> Vec<usize> {
        system
            .screen_ram()
            .map(|screen| (usize::from(*screen.start())..=usize::from(*screen.end())).collect())
            .unwrap_or_default()
    }

    /// T2.1.7: on the C64 the clear-screen trap fills exactly
    /// `system.screen_ram()` with spaces and clears A, X and Y.
    #[test]
    fn clear_screen_trap_fills_system_screen_ram_on_c64() {
        let mut cpu = cpu_for(TargetSystem::C64, START_A);
        emulate(&mut cpu, KernalRoutine::ClearScreen);

        let screen = screen_addrs(&TargetSystem::C64);
        assert_eq!(screen.first(), Some(&0x0400));
        assert_eq!(screen.last(), Some(&0x07E7));
        assert_eq!(changed(&cpu), screen);
        assert!(
            screen
                .iter()
                .all(|&addr| cpu.memory.mem[addr] == SCREEN_CODE_SPACE)
        );
        let registers = &cpu.registers;
        assert_eq!(
            (registers.accumulator, registers.index_x, registers.index_y),
            (0, 0, 0)
        );
    }

    /// T2.1.8 (ruling R5): the C128 has screen RAM, but its trap does not
    /// fill it; neither does any other non-C64 system's. Registers are
    /// still cleared.
    #[test]
    fn clear_screen_trap_does_not_fill_on_c128() {
        assert!(TargetSystem::C128.screen_ram().is_some());
        for system in non_c64_systems() {
            let mut cpu = cpu_for(system.clone(), START_A);
            emulate(&mut cpu, KernalRoutine::ClearScreen);
            assert_eq!(changed(&cpu), Vec::<usize>::new(), "{system}");
            let registers = &cpu.registers;
            assert_eq!(
                (registers.accumulator, registers.index_x, registers.index_y),
                (0, 0, 0),
                "{system}"
            );
        }
    }

    /// T2.1.9: CHROUT clears the screen only for A = `$93`, and never touches
    /// the registers.
    #[test]
    fn chrout_clear_screen_only_for_a_equals_93() {
        let screen = screen_addrs(&TargetSystem::C64);
        for a in [0x00, 0x0D, 0x13, 0x92, 0x93, 0x94, 0xD3] {
            let mut cpu = cpu_for(TargetSystem::C64, a);
            emulate(&mut cpu, KernalRoutine::Chrout);
            let expected = if a == PETSCII_CLEAR_SCREEN {
                screen.clone()
            } else {
                Vec::new()
            };
            assert_eq!(changed(&cpu), expected, "A = ${a:02X}");
            let registers = &cpu.registers;
            assert_eq!(
                (registers.accumulator, registers.index_x, registers.index_y),
                (a, START_X, START_Y),
                "A = ${a:02X}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // BASIC start (rulings R7, R18)
    // -----------------------------------------------------------------------

    /// T2.1b.1: the C64 cases, including the explicit Card Cruncher load.
    #[test]
    fn basic_start_c64_cases() {
        let cases = [
            (0x0800, 0x0801),
            (0x0801, 0x0801),
            (0x07C0, 0x07C1),
            (0x1000, 0x1000),
            (0x07BF, 0x07BF),
            (0x07C1, 0x07C1),
            (0x07FF, 0x07FF),
            (0x0802, 0x0802),
        ];
        for (load, expected) in cases {
            assert_eq!(get_basic_start(load), expected, "load ${load:04X}");
        }
        // The padding byte is derived from the C64 profile, which is also the
        // C64's RAM start.
        assert_eq!(TargetSystem::C64.default_basic_start(), 0x0801);
        assert_eq!(TargetSystem::C64.ram_start(), 0x0800);
    }

    /// T2.1b.2 (ruling R18): the convention does not depend on the system.
    /// Loads at each system's RAM start and default BASIC start keep their
    /// historical BASIC start; only `$0800` and `$07C0` are padded. Example:
    /// a C128 load at `$1C00` stays `$1C00` (a system-aware `$1C01` is a
    /// recorded follow-up).
    #[test]
    fn basic_start_other_systems() {
        // (system, RAM start → BASIC start, default BASIC start → BASIC start)
        let rows = [
            (TargetSystem::C128, (0x1C00, 0x1C00), (0x1C01, 0x1C01)),
            (TargetSystem::Vic20, (0x1000, 0x1000), (0x1001, 0x1001)),
            (TargetSystem::Pet20, (0x0400, 0x0400), (0x0401, 0x0401)),
            (TargetSystem::Pet40, (0x0400, 0x0400), (0x0401, 0x0401)),
            (TargetSystem::Plus4, (0x1000, 0x1000), (0x1001, 0x1001)),
            (TargetSystem::C16, (0x1000, 0x1000), (0x1001, 0x1001)),
            (
                TargetSystem::Custom("Homebrew 6502".into()),
                (0x0800, 0x0801),
                (0x0801, 0x0801),
            ),
        ];
        for (system, (ram_start, from_ram), (basic, from_basic)) in rows {
            assert_eq!(system.ram_start(), ram_start, "{system}");
            assert_eq!(system.default_basic_start(), basic, "{system}");
            assert_eq!(get_basic_start(ram_start), from_ram, "{system} RAM start");
            assert_eq!(get_basic_start(basic), from_basic, "{system} BASIC start");
        }
    }

    /// T2.1b.3: the drives have a default BASIC start of `$0000`; the
    /// derivation neither underflows nor wraps at either end of memory.
    #[test]
    fn basic_start_drive_systems_no_underflow() {
        for system in [
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
        ] {
            assert_eq!(system.default_basic_start(), 0x0000, "{system}");
            assert_eq!(system.ram_start(), 0x0000, "{system}");
        }
        assert_eq!(get_basic_start(0x0000), 0x0000);
        assert_eq!(get_basic_start(0xFFFF), 0xFFFF);
        assert_eq!(get_basic_start(0x0001), 0x0001);
    }
}
