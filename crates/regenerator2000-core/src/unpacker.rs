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

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::Bus;
use std::fmt;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

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
    /// Target system (default: None, defaults to C64 during execution).
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

/// Errors that can occur during unpacking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnpackError {
    /// The input data is empty.
    EmptyData,
    /// Could not find a SYS entry point in the BASIC header.
    NoEntryPoint,
    /// Phase 1 exceeded the instruction limit without finding the depacker.
    Phase1Timeout,
    /// Phase 2 exceeded the instruction limit without finishing decompression.
    Phase2Timeout,
    /// No memory was modified during decompression.
    NothingWritten,
    /// The detected entry point is outside the unpacked memory range.
    InvalidAddressRange {
        start_addr: u16,
        end_addr: u16,
        entry_point: u16,
    },
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyData => write!(f, "Empty input data"),
            Self::NoEntryPoint => write!(f, "Could not find SYS entry point"),
            Self::Phase1Timeout => write!(f, "Phase 1 timeout: depacker not found"),
            Self::Phase2Timeout => write!(f, "Phase 2 timeout: decompression did not finish"),
            Self::NothingWritten => write!(f, "No memory was modified during decompression"),
            Self::InvalidAddressRange {
                start_addr,
                end_addr,
                entry_point,
            } => write!(
                f,
                "Invalid unpacked range (${:04X}-${:04X}): entry point ${:04X} is outside range",
                start_addr, end_addr, entry_point
            ),
        }
    }
}

impl std::error::Error for UnpackError {}

// ---------------------------------------------------------------------------
// Memory bus
// ---------------------------------------------------------------------------

/// Custom memory bus for the unpacker.
///
/// Provides flat 64 KB RAM with per-byte write tracking and I/O suppression.
#[derive(Clone)]
pub struct UnpackerMemory {
    /// Flat 64 KB memory.
    pub(crate) mem: Vec<u8>,
    /// Per-byte write tracking across all phases.
    written: Vec<bool>,
    /// Per-byte write tracking during Phase 2.
    written_phase2: Vec<bool>,
    /// Target system.
    system: crate::state::types::System,
    /// Counter for read operations to simulate VIC-II raster beams.
    read_counter: u64,
    /// Whether emulation is in Phase 2.
    pub(crate) in_phase2: bool,
}

impl UnpackerMemory {
    /// Creates a new zeroed 64 KB memory.
    fn new(system: crate::state::types::System) -> Self {
        Self {
            mem: vec![0u8; 0x1_0000],
            written: vec![false; 0x1_0000],
            written_phase2: vec![false; 0x1_0000],
            system,
            read_counter: 0,
            in_phase2: false,
        }
    }
}

impl Bus for UnpackerMemory {
    fn get_byte(&mut self, addr: u16) -> u8 {
        let a = addr as usize;
        if self.system.is_c64() {
            // I/O at $D000-$DFFF is only visible when the C64 PLA maps it:
            //   - CHAREN (bit 2) must be set, AND
            //   - At least one of LORAM (bit 0) or HIRAM (bit 1) must be set.
            // When both LORAM and HIRAM are clear, RAM is visible regardless of CHAREN.
            if (0xD000..=0xDFFF).contains(&a) {
                let bank = self.mem[0x01];
                let io_visible = (bank & 0x04 != 0) && (bank & 0x03 != 0);
                if io_visible {
                    if addr == 0xD012 {
                        let val = (self.read_counter & 0xFF) as u8;
                        self.read_counter = self.read_counter.wrapping_add(1);
                        return val;
                    }
                    if addr == 0xD011 {
                        let val =
                            (((self.read_counter >> 1) & 0x80) as u8) | (self.mem[0xD011] & 0x7F);
                        self.read_counter = self.read_counter.wrapping_add(1);
                        return val;
                    }
                    return 0;
                }
            }
        }
        self.mem[a]
    }

    fn set_byte(&mut self, addr: u16, val: u8) {
        let a = addr as usize;
        if self.system.is_c64() {
            // Same PLA logic as get_byte: suppress writes only when I/O is mapped.
            if (0xD000..=0xDFFF).contains(&a) {
                let bank = self.mem[0x01];
                let io_visible = (bank & 0x04 != 0) && (bank & 0x03 != 0);
                if io_visible {
                    return;
                }
            }
        }
        self.mem[a] = val;
        self.written[a] = true;
        if self.in_phase2 {
            self.written_phase2[a] = true;
        }
    }
}

// ---------------------------------------------------------------------------
// BASIC SYS parser
// ---------------------------------------------------------------------------

/// BASIC token for `SYS`.
const SYS_TOKEN: u8 = 0x9E;

/// BASIC tokens for arithmetic operators.
const TOKEN_PLUS: u8 = 0xAA;
const TOKEN_MINUS: u8 = 0xAB;
const TOKEN_MULTIPLY: u8 = 0xAC;
const TOKEN_DIVIDE: u8 = 0xAD;

/// Parses a BASIC `SYS` line from memory at $0801 to find the entry point.
///
/// Handles:
/// - Simple: `SYS 2061`
/// - With spaces/parens: `SYS (2061)` or `SYS  2061`
/// - With arithmetic: `SYS 2048+16`, `SYS 2048*1+13`
#[must_use]
pub(crate) fn find_sys_address(mem: &[u8], basic_start: u16) -> Option<u16> {
    let start = (basic_start as usize) + 4;
    let limit = (basic_start as usize) + 0x100;
    if mem.len() < start + 1 {
        return None;
    }

    let mut pos = start;

    // Find SYS token
    while pos < mem.len() && pos < limit {
        if mem[pos] == 0x00 {
            return None; // End of line without SYS
        }
        if mem[pos] == SYS_TOKEN {
            pos += 1;
            break;
        }
        pos += 1;
    }

    if pos >= mem.len() || pos >= limit {
        return None;
    }

    // Skip spaces and opening parentheses
    while pos < mem.len() && (mem[pos] == b' ' || mem[pos] == b'(') {
        pos += 1;
    }

    // Parse first number
    let mut value: u32 = 0;
    let mut found_digit = false;
    while pos < mem.len() && mem[pos].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add(u32::from(mem[pos] - b'0'));
        found_digit = true;
        pos += 1;
    }

    if !found_digit {
        return None;
    }

    // Handle arithmetic operators (tokenized BASIC)
    while pos < mem.len() {
        let op = mem[pos];
        if op != TOKEN_PLUS && op != TOKEN_MINUS && op != TOKEN_MULTIPLY && op != TOKEN_DIVIDE {
            break;
        }
        pos += 1;

        // Skip spaces
        while pos < mem.len() && mem[pos] == b' ' {
            pos += 1;
        }

        // Parse next number
        let mut operand: u32 = 0;
        let mut found_operand = false;
        while pos < mem.len() && mem[pos].is_ascii_digit() {
            operand = operand
                .wrapping_mul(10)
                .wrapping_add(u32::from(mem[pos] - b'0'));
            found_operand = true;
            pos += 1;
        }

        if !found_operand {
            break;
        }

        match op {
            TOKEN_PLUS => value = value.wrapping_add(operand),
            TOKEN_MINUS => value = value.wrapping_sub(operand),
            TOKEN_MULTIPLY => value = value.wrapping_mul(operand),
            TOKEN_DIVIDE => {
                if let Some(result) = value.checked_div(operand) {
                    value = result;
                }
            }
            _ => break,
        }
    }

    if value <= 0xFFFF {
        Some(value as u16)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Zero-page & system initialization
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Target-system helpers
// ---------------------------------------------------------------------------

fn get_basic_start(load_addr: u16) -> u16 {
    if load_addr.is_multiple_of(2) {
        load_addr.wrapping_add(1)
    } else {
        load_addr
    }
}

fn is_in_basic_rom(pc: u16, system: &crate::state::types::System) -> bool {
    match system.as_str() {
        crate::state::types::System::C64 => (0xA000..=0xBFFF).contains(&pc),
        crate::state::types::System::C128 => (0x4000..=0xBFFF).contains(&pc),
        crate::state::types::System::VIC20 => (0xC000..=0xDFFF).contains(&pc),
        crate::state::types::System::PLUS4 => (0x8000..=0xBFFF).contains(&pc),
        _ => (0xA000..=0xBFFF).contains(&pc),
    }
}

fn is_in_kernal_rom(pc: u16, system: &crate::state::types::System) -> bool {
    match system.as_str() {
        crate::state::types::System::C64 => pc >= 0xE000,
        crate::state::types::System::C128 => pc >= 0xC000,
        crate::state::types::System::VIC20 => pc >= 0xE000,
        crate::state::types::System::PLUS4 => pc >= 0xE000,
        _ => pc >= 0xE000,
    }
}

fn is_basic_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        (mem[0x01] & 0x01) != 0
    } else {
        true
    }
}

fn is_kernal_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        (mem[0x01] & 0x02) != 0
    } else {
        true
    }
}

fn is_io_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        (mem[0x01] & 0x04) != 0
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Zero-page & system initialization
// ---------------------------------------------------------------------------

const C64_ZEROPAGE_TEMPLATE: [u8; 256] = [
    0x2F, 0x37, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x3C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0xA0, 0x30, 0xFD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C, 0x00, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x27, 0x00, 0x00, 0x00, 0x84, 0x84, 0x84, 0x84, 0x84, 0x84, 0x84,
    0x85, 0x85, 0x85, 0x85, 0x85, 0x85, 0x86, 0x86, 0x86, 0x86, 0x86, 0x86, 0x86, 0x87, 0x87, 0x87,
    0x87, 0x87, 0x87, 0x00, 0xD8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// Initializes target zero-page and system area defaults (per unp64 lines 572-620).
fn init_zero_page(mem: &mut UnpackerMemory, load_addr: u16, data_len: u16, basic_start: u16) {
    let end_addr = load_addr.wrapping_add(data_len);
    let system = mem.system.clone();

    if system.is_c64() {
        mem.mem[0..256].copy_from_slice(&C64_ZEROPAGE_TEMPLATE);
    } else if system.as_str() == crate::state::types::System::C128 {
        mem.mem[0x00] = 0x2F;
        mem.mem[0x01] = 0x37;
    }

    // BASIC text start (dynamically using basic_start)
    mem.mem[0x2B] = (basic_start & 0xFF) as u8;
    mem.mem[0x2C] = (basic_start >> 8) as u8;

    // Variables start = end of loaded data
    mem.mem[0x2D] = (end_addr & 0xFF) as u8;
    mem.mem[0x2E] = (end_addr >> 8) as u8;

    // Array start = same
    mem.mem[0x2F] = (end_addr & 0xFF) as u8;
    mem.mem[0x30] = (end_addr >> 8) as u8;

    // String start = same
    mem.mem[0x31] = (end_addr & 0xFF) as u8;
    mem.mem[0x32] = (end_addr >> 8) as u8;

    // BASIC end (top of memory for strings)
    if system.is_c64() {
        let ram_start = system.ram_start();
        mem.mem[0x37] = (ram_start & 0xFF) as u8;
        mem.mem[0x38] = (ram_start >> 8) as u8;
    }

    // First BASIC line number (read from loaded data)
    if data_len >= 4 {
        mem.mem[0x39] = mem.mem[basic_start as usize + 2];
        mem.mem[0x3A] = mem.mem[basic_start as usize + 3];
    }

    // End of program
    mem.mem[0xAE] = (end_addr & 0xFF) as u8;
    mem.mem[0xAF] = (end_addr >> 8) as u8;

    if let Some((vector_addr, handler_addr)) = system.default_irq() {
        mem.mem[vector_addr as usize] = (handler_addr & 0xFF) as u8;
        mem.mem[vector_addr as usize + 1] = (handler_addr >> 8) as u8;
    }

    if let Some(screen_range) = system.screen_ram() {
        for addr in screen_range {
            mem.mem[addr as usize] = 0x20;
        }
    }
}

// ---------------------------------------------------------------------------
// ROM interception
// ---------------------------------------------------------------------------

/// Action to take after ROM interception.
#[derive(Debug, PartialEq, Eq)]
enum RomAction {
    /// Not in ROM space, continue normally.
    Continue,
    /// Intercepted and handled; skip single_step.
    Handled,
    /// Hit an exit vector; break out of the current phase loop.
    Exit,
    /// Hit a BASIC RUN vector; re-parse SYS and redirect.
    BasicRun,
}

/// Simulated GETIN responses (cycling through them).
const GETIN_RESPONSES: [u8; 14] = [
    0x20, // SPACE
    0x00, 0x4E, // N
    0x00, 0x03, // RUN/STOP
    0x00, 0x5F, // ←
    0x00, 0x11, // CRSR-DOWN
    0x00, 0x0D, // RETURN
    0x00, 0x31, // 1
    0x00,
];

/// Checks whether the current PC is in ROM space and, if so, intercepts it.
///
/// Must be called BEFORE `single_step()` on each iteration.
fn handle_rom_entry(
    cpu: &mut CPU<UnpackerMemory, Nmos6502>,
    getin_index: &mut usize,
    phase: u8,
) -> RomAction {
    let pc = cpu.registers.program_counter;
    let system = &cpu.memory.system;

    let in_basic = is_in_basic_rom(pc, system);
    let in_kernal = is_in_kernal_rom(pc, system);

    // Not in ROM space — fast path for standard RAM code execution
    if !in_basic && !in_kernal {
        return RomAction::Continue;
    }

    let basic_mapped = is_basic_rom_mapped(&cpu.memory.mem, system);
    let kernal_mapped = is_kernal_rom_mapped(&cpu.memory.mem, system);

    // If user code was written here (depacker at $FF00+, etc.) AND the ROM
    // at this address is not currently mapped, let it run as RAM code.
    // When ROM IS mapped, the CPU reads from ROM regardless of RAM writes,
    // so we must still intercept. This matters when depackers decompress
    // to the full $0800-$FF3F range — RAM underneath ROM gets written, but
    // the CPU still executes ROM when it enters BASIC RUN or Kernal calls.
    if cpu.memory.written[pc as usize] {
        let rom_mapped_here = (in_basic && basic_mapped) || (in_kernal && kernal_mapped);
        if !rom_mapped_here {
            return RomAction::Continue;
        }
    }

    let is_c64 = system.is_c64();

    // BASIC ROM region
    if in_basic {
        if !basic_mapped {
            return RomAction::Continue; // RAM is visible, not ROM
        }

        // BASIC RUN detection (Phase 1 only triggers redirect; Phase 2 breaks)
        if is_c64
            && matches!(
                pc,
                0xA7AE | 0xA7B1 | 0xA7EA | 0xA474 | 0xA533 | 0xA871 | 0xA888 | 0xA8BC
            )
        {
            return RomAction::BasicRun;
        }

        // Phase 2 extended BASIC RUN detection
        if is_c64 && phase == 2 && ((0xA57C..=0xA659).contains(&pc) || pc == 0xA660 || pc == 0xA68E)
        {
            return RomAction::Exit;
        }

        // Fallback: force RTS
        force_rts(cpu);
        return RomAction::Handled;
    }

    // Kernal ROM region
    if in_kernal {
        if !kernal_mapped {
            return RomAction::Continue; // RAM is visible
        }

        match pc {
            // GETIN ($FFE4 / $F13E)
            0xFFE4 | 0xF13E if pc == 0xFFE4 || is_c64 => {
                cpu.registers.accumulator = GETIN_RESPONSES[*getin_index % GETIN_RESPONSES.len()];
                *getin_index += 1;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // CLRSCR / CINT ($E536 / $E544 / $FF5B)
            0xE536 | 0xE544 | 0xFF5B if pc == 0xFF5B || is_c64 => {
                // Fill screen with spaces
                if is_c64 {
                    for addr in 0x0400..=0x07E7 {
                        cpu.memory.mem[addr] = 0x20;
                    }
                }
                cpu.registers.accumulator = 0x00;
                cpu.registers.index_x = 0x00;
                cpu.registers.index_y = 0x00;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // CHROUT with A=$93 (clear screen)
            0xFFD2 => {
                if cpu.registers.accumulator == 0x93 && is_c64 {
                    for addr in 0x0400..=0x07E7 {
                        cpu.memory.mem[addr] = 0x20;
                    }
                }
                force_rts(cpu);
                return RomAction::Handled;
            }

            // SETNAM ($FFBD)
            0xFFBD => {
                cpu.memory.mem[0xB7] = cpu.registers.accumulator;
                cpu.memory.mem[0xBB] = cpu.registers.index_x;
                cpu.memory.mem[0xBC] = cpu.registers.index_y;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // IOINIT ($FDA3)
            0xFDA3 if is_c64 => {
                cpu.memory.mem[0x01] = 0xE7;
                cpu.registers.accumulator = 0xD7;
                cpu.registers.index_x = 0xFF;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // RESTOR ($FD15)
            0xFD15 if is_c64 => {
                cpu.registers.accumulator = 0x31;
                cpu.registers.index_x = 0x30;
                cpu.registers.index_y = 0xFF;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // LOAD ($FFD5 / $F4A2) — exit vector
            0xFFD5 | 0xF4A2 if pc == 0xFFD5 || is_c64 => {
                return RomAction::Exit;
            }

            // Warm start ($FCE2) — exit vector
            0xFCE2 if is_c64 => {
                return RomAction::Exit;
            }

            // IRQ handler range ($EA31-$EB76) — exit in Phase 2
            addr if phase == 2 && is_c64 && (0xEA31..=0xEB76).contains(&addr) => {
                return RomAction::Exit;
            }

            // Fallback: force RTS
            _ => {
                force_rts(cpu);
                return RomAction::Handled;
            }
        }
    }

    RomAction::Continue
}

/// Sets `mem[0] = $60` (RTS) and `PC = 0`, causing the CPU to execute an RTS
/// from zero-page which pops the return address from the stack.
fn force_rts(cpu: &mut CPU<UnpackerMemory, Nmos6502>) {
    cpu.memory.mem[0x0000] = 0x60; // RTS opcode
    cpu.registers.program_counter = 0x0000;
}

/// Emulates illegal opcodes that are not supported or have incorrect PC advancement in the base emulator.
/// Returns true if the opcode was handled (in which case PC and registers were updated).
fn emulate_illegal_opcode(cpu: &mut CPU<UnpackerMemory, Nmos6502>) -> bool {
    let pc = cpu.registers.program_counter;
    let opcode = cpu.memory.mem[pc as usize];
    match opcode {
        0xAB => {
            let imm = cpu.memory.mem[pc.wrapping_add(1) as usize];
            let val = (cpu.registers.accumulator | 0xEE) & imm;
            cpu.registers.accumulator = val;
            cpu.registers.index_x = val;
            cpu.registers
                .status
                .set(mos6502::registers::Status::PS_ZERO, val == 0);
            cpu.registers
                .status
                .set(mos6502::registers::Status::PS_NEGATIVE, (val & 0x80) != 0);
            cpu.registers.program_counter = pc.wrapping_add(2);
            true
        }
        0x9E => {
            // SHX
            let addr_low = cpu.memory.mem[pc.wrapping_add(1) as usize];
            let addr_high = cpu.memory.mem[pc.wrapping_add(2) as usize];
            let val = cpu.registers.index_x & addr_high.wrapping_add(1);
            let target_addr = u16::from_le_bytes([addr_low, addr_high])
                .wrapping_add(cpu.registers.index_y as u16);
            cpu.memory.mem[target_addr as usize] = val;
            cpu.memory.written[target_addr as usize] = true;
            cpu.registers.program_counter = pc.wrapping_add(3);
            true
        }
        0x9C => {
            // SHY
            let addr_low = cpu.memory.mem[pc.wrapping_add(1) as usize];
            let addr_high = cpu.memory.mem[pc.wrapping_add(2) as usize];
            let val = cpu.registers.index_y & addr_high.wrapping_add(1);
            let target_addr = u16::from_le_bytes([addr_low, addr_high])
                .wrapping_add(cpu.registers.index_x as u16);
            cpu.memory.mem[target_addr as usize] = val;
            cpu.memory.written[target_addr as usize] = true;
            cpu.registers.program_counter = pc.wrapping_add(3);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Output range detection
// ---------------------------------------------------------------------------

/// Detects the modified memory range using the write-tracking bitmap and
/// a pre-emulation snapshot.
///
/// `ret_addr` is the return-address boundary (typically `$0800`). Modifications
/// below this address are depacker workspace and are excluded from the output.
///
/// Uses a hybrid approach:
/// - **Start address**: determined by the `written` bitmap (catches all writes).
/// - **End address**: determined by the snapshot diff, then extended forward
///   through any bytes that were `written` but match the snapshot (trailing
///   zero-fills). This excludes depacker tables written past the output.
///
/// Returns `(start_addr, end_addr)` inclusive, or `None` if nothing was written.
#[must_use]
fn detect_output_range(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    _ret_addr: u16,
    load_addr: u16,
    system: &crate::state::types::System,
) -> Option<(u16, u16)> {
    let scan_start = (load_addr as usize).min(system.ram_start() as usize);

    // Cascading scans with progressively wider boundaries.
    // Each level is only tried if the previous scan's detected end is near
    // its ceiling (within 256 bytes), indicating the output continues past
    // that boundary. This keeps the scan range tight so the trim heuristics
    // work correctly with workspaces that fill high memory.
    //
    // Level 1: $0800..$9FFF — typical program area
    // Level 2: $0800..$CFFF — includes BASIC ROM area (all-RAM mode)
    // Level 3: $0800..$FFFF — includes I/O + Kernal ROM area (full RAM)
    //
    // In-place depackers (e.g. TinyCrunch) write to two disjoint regions:
    // a lower region (e.g. $0801-$7949) and a high region (e.g. $D000-$FFFD),
    // leaving a gap of unchanged bytes in the middle.  The gap means
    // `untrimmed_end` stops early and `near_ceiling` is false.  To handle
    // this we also escalate when written+diffed bytes exist above the current
    // boundary.
    let boundaries = system.memory_boundaries();

    for (i, &boundary) in boundaries.iter().enumerate() {
        if let Some((start, end, trimmed_end, has_diff)) =
            scan_hybrid_range(mem, snapshot, written, scan_start, boundary, false, system)
        {
            let is_last = i == boundaries.len() - 1;
            let near_ceiling = has_diff && (trimmed_end as usize) + 256 >= boundary;

            // Also escalate when there are written+diffed bytes above the
            // current boundary — in-place depackers write to a disjoint high
            // region while leaving unchanged data in between.
            let io_mapped = is_io_mapped(mem, system);
            let next_upper = if !is_last {
                boundaries[i + 1]
            } else {
                boundary
            };
            let has_output_above = !is_last
                && (trimmed_end as usize) >= boundary.saturating_sub(0x2000)
                && (boundary + 1..=next_upper)
                    .filter(|&addr| {
                        let is_io = (0xD000..=0xDFFF).contains(&addr) && io_mapped;
                        let diff = written.get(addr).copied().unwrap_or(false)
                            || mem.get(addr).copied().unwrap_or(0)
                                != snapshot.get(addr).copied().unwrap_or(0);
                        !is_io && diff
                    })
                    .count()
                    >= 4;

            if is_last || (!near_ceiling && !has_output_above) {
                return Some((start, end));
            }
        }
    }

    let mid_boundary = if boundaries.len() > 1 {
        boundaries[1]
    } else {
        boundaries[0]
    };
    let basic_mapped = is_basic_rom_mapped(mem, system);
    if (boundaries[0] + 1..=mid_boundary).any(|a| {
        let in_rom = is_in_basic_rom(a as u16, system) && basic_mapped;
        !in_rom
            && written.get(a).copied().unwrap_or(false)
            && mem.get(a).copied().unwrap_or(0) != snapshot.get(a).copied().unwrap_or(0)
    }) && let Some((s, e, _, _)) = scan_hybrid_range(
        mem,
        snapshot,
        written,
        scan_start,
        mid_boundary,
        false,
        system,
    ) {
        return Some((s, e));
    }

    // Fallback: scan $E000-$FFFF for packers that decompress only into
    // the Kernal ROM area.
    scan_hybrid_range(mem, snapshot, written, 0xE000, 0xFFFF, false, system)
        .map(|(s, e, _, _)| (s, e))
}

/// Scans a range using a hybrid of the `written` bitmap and snapshot diff.
///
/// - **Start**: first byte in the `written` bitmap.
/// - **End**: last byte where `mem != snapshot`, trimmed of any small trailing
///   diff clusters (depacker tables) separated by matching bytes, then extended
///   through written zero-fills (`mem == snapshot`).
///
/// If `skip_trim` is `true`, the `trim_trailing_clusters` heuristic is bypassed
/// and the last diff byte (or last written byte if no diff) is used directly.
/// This is used when escalating due to an in-place depacker output gap.
#[must_use]
fn scan_hybrid_range(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    start: usize,
    end: usize,
    skip_trim: bool,
    system: &crate::state::types::System,
) -> Option<(u16, u16, u16, bool)> {
    let upper = end
        .min(written.len() - 1)
        .min(mem.len() - 1)
        .min(snapshot.len() - 1);

    // Find the first written byte or first diff byte in RAM (start..=upper)
    let mut first_written = None;
    for addr in start..=upper {
        if written.get(addr).copied().unwrap_or(false) || mem[addr] != snapshot[addr] {
            first_written = Some(addr);
            break;
        }
    }
    let mut first = first_written?;
    let ram_start = system.ram_start() as usize;
    if first < ram_start {
        let mut diffs_below = 0;
        let mut gap_before_ram = 0;
        for a in first..ram_start {
            if mem[a] != snapshot[a] || written.get(a).copied().unwrap_or(false) {
                diffs_below += 1;
            } else {
                gap_before_ram += 1;
            }
        }
        if diffs_below < 64 && gap_before_ram > 128 {
            for a in ram_start..=upper {
                if written.get(a).copied().unwrap_or(false) || mem[a] != snapshot[a] {
                    first = a;
                    break;
                }
            }
        }
    }

    // Find all diff bytes and identify the end of the "main" diff block
    // by trimming small trailing clusters separated by non-diff gaps.
    let mut last_diff = None;
    for addr in start..=upper {
        if written.get(addr).copied().unwrap_or(false) || mem[addr] != snapshot[addr] {
            last_diff = Some(addr);
        }
    }

    let diff_end = last_diff?;

    // Walk backward from diff_end to detect and trim small trailing clusters.
    // Only apply trimming when the diff extends near the scan boundary (within
    // 128 bytes). A clean gap between diff_end and the boundary means the
    // output ends naturally with no depacker workspace to separate — trimming
    // would only produce false positives on natural gaps in program data.
    // Skip trimming entirely when the caller signals an in-place depacker gap.
    let trimmed_end = if !skip_trim {
        trim_trailing_clusters(mem, snapshot, written, first, diff_end)
    } else {
        diff_end
    };

    // Extend past the trimmed end through written bytes that match the snapshot
    // (trailing zero-fills that are part of the real output).
    let mut extended_end = trimmed_end;
    let max_extend = upper.min(trimmed_end + 512);
    for addr in (trimmed_end + 1)..=max_extend {
        if written[addr] && mem[addr] == snapshot[addr] {
            extended_end = addr;
        } else {
            break;
        }
    }

    Some((first as u16, extended_end as u16, trimmed_end as u16, true))
}

/// After the depacker transfers control to `ret_addr`, some packers execute
/// a brief init/bootstrap stub before jumping to the real program entry.
/// This function scans the **pre-decompression snapshot** for a `JMP $xxxx`
/// instruction that targets a plausible entry point (≥ `ret_addr + 0x100`).
///
/// The Dali packer, for example, stores `JMP $1100` at $090A in its packed
/// binary. Before decompression, the snapshot preserves this instruction even
/// though it gets overwritten by decompressed data.
///
/// Returns the discovered entry point, or `None` if none is found.
#[must_use]
fn find_entry_in_snapshot(
    snapshot: &[u8],
    load_addr: u16,
    load_size: usize,
    ret_addr: u16,
) -> Option<u16> {
    // Minimum plausible entry point: must be well past the depacker code
    // (ret_addr + 0x300 skips over common init stubs in the first 3 pages).
    let min_entry = ret_addr.saturating_add(0x300);
    // Only scan in the depacker's own code region: [ret_addr, ret_addr+0x400].
    // The depacker exit JMP is typically within the first few pages of the
    // loaded binary. We avoid scanning deeper to prevent false positives from
    // JMP instructions in the init/bootstrap code.
    let scan_start = ret_addr as usize;
    let scan_end = (ret_addr as usize)
        .saturating_add(0x400)
        .min(load_addr as usize + load_size)
        .min(snapshot.len().saturating_sub(2));

    // Scan for JMP $xxxx (opcode $4C) targeting a plausible entry address.
    // Use the LOWEST valid target — the Dali packer stores JMP $1100 as the
    // first JMP-to-entry in its depacker code.
    let mut best: Option<u16> = None;
    for i in scan_start..scan_end {
        if snapshot[i] == 0x4C {
            let lo = snapshot[i + 1];
            let hi = snapshot[i + 2];
            let target = u16::from_le_bytes([lo, hi]);
            // Target must be a plausible entry: above min_entry and in RAM (<$8000)
            if target >= min_entry && target < 0x8000 {
                // Prefer the LOWEST target — closest to the decompressed start
                match best {
                    None => best = Some(target),
                    Some(prev) if target < prev => best = Some(target),
                    _ => {}
                }
            }
        }
    }
    best
}

/// Trims trailing depacker workspace from the detected diff range.
///
/// Walks backward from `end` through the diff range, examining each gap
/// (run of same-as-snapshot bytes). Trims at the first gap where either:
///
/// 1. The trailing diff cluster is tiny (< 16 bytes) — handles depacker
///    tails like PUCrunch's 10-byte cluster.
/// 2. The trailing range is > 128 bytes AND proportionally small (< 2% of
///    the main region) — handles large depacker workspaces like ERA's
///    hundreds of bytes.
///
/// Stops scanning at 95% of the range to avoid false positives deep inside
/// the real output data.
#[must_use]
fn trim_trailing_clusters(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    start: usize,
    end: usize,
) -> usize {
    if end <= start {
        return end;
    }

    let scan_floor = start;
    let mut pos = end;
    let mut curr_cluster_end = end;
    let mut best_trim_pos: Option<usize> = None;

    while pos > scan_floor {
        // Walk backward through diff bytes
        while pos > scan_floor && mem[pos] != snapshot[pos] {
            pos -= 1;
        }

        if pos <= scan_floor {
            break;
        }

        // Found a matching byte — walk backward through the gap
        let gap_end = pos;
        while pos > start && mem[pos] == snapshot[pos] {
            pos -= 1;
        }

        // Count diff bytes in the cluster immediately following the gap
        let cluster_diffs: usize = ((gap_end + 1)..=curr_cluster_end)
            .filter(|&a| {
                written.get(a).copied().unwrap_or(false) || mem[a] != snapshot[a] || mem[a] != 0
            })
            .count();

        let gap_len = gap_end - (pos + 1) + 1;

        // Check 1: tiny trailing cluster (< 16 diff bytes) with a gap (>= 4 bytes).
        if gap_len >= 4 && cluster_diffs < 16 {
            return pos;
        }

        // Check 2: gap (>= 2 bytes) separating high workspace cluster from main decompressed payload.
        let main_len = (pos + 1).saturating_sub(start);
        if gap_end < 0xF000
            && (128..4096).contains(&gap_len)
            && main_len > 0
            && (cluster_diffs == 0 || cluster_diffs <= 512)
        {
            best_trim_pos = Some(pos);
            curr_cluster_end = pos;
        }
    }

    best_trim_pos.unwrap_or(end)
}

// ---------------------------------------------------------------------------
// Main unpack function
// ---------------------------------------------------------------------------

/// Unpacks a compressed C64 binary using 6502 emulation.
///
/// # Arguments
/// * `raw_data` — the raw binary data (without load address header)
/// * `load_addr` — the address where the binary is loaded in memory
/// * `config` — unpacker configuration
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
    if raw_data.is_empty() {
        return Err(UnpackError::EmptyData);
    }

    let system = config
        .target_system
        .clone()
        .unwrap_or_else(crate::state::types::default_system);

    let basic_start = get_basic_start(load_addr);

    // Set up memory
    let mut memory = UnpackerMemory::new(system.clone());

    // Load binary into memory at load_addr
    let data_len = raw_data.len().min(0x10000 - load_addr as usize);
    for (i, &byte) in raw_data.iter().take(data_len).enumerate() {
        memory.mem[load_addr as usize + i] = byte;
    }

    // Initialize zero-page and system area
    init_zero_page(&mut memory, load_addr, data_len as u16, basic_start);

    // Take snapshot before emulation (used for output range end detection)
    let snapshot = memory.mem.clone();

    // Find entry point
    let entry = if let Some(forced) = config.forced_entry {
        forced
    } else {
        find_sys_address(&memory.mem, basic_start)
            .or_else(|| find_sys_address(&memory.mem, system.default_basic_start()))
            .ok_or(UnpackError::NoEntryPoint)?
    };

    let ret_addr = config
        .forced_ret_addr
        .unwrap_or_else(|| load_addr.min(system.ram_start()));
    let load_end = (load_addr as usize + data_len).min(0x10000) as u16;
    let mut packer = crate::packers::detect_packer(&memory.mem, load_addr, load_end);

    // Apply patches for specific packers to bypass hardware/ROM checks during emulation
    if let Some(ref p) = packer {
        p.pre_emulate(&mut memory.mem, &system);
    }

    // Create CPU
    let mut cpu = CPU::new(memory, Nmos6502);
    cpu.registers.program_counter = entry;
    cpu.registers.stack_pointer = mos6502::registers::StackPointer(0xF6);

    let mut getin_index: usize = 0;
    let mut total_instructions: u64 = 0;

    // -----------------------------------------------------------------------
    // Phase 1: Find the depacker
    // Run from entry point. Loop until the depacker is reached.
    // Exit when PC matches a known depacker address, PC drops below ret_addr
    // (depacker found), or an exit vector is hit.
    // -----------------------------------------------------------------------
    let dep_addr;
    let load_end = load_addr.wrapping_add(data_len as u16);
    loop {
        if total_instructions >= config.max_instructions {
            return Err(UnpackError::Phase1Timeout);
        }

        let pc = cpu.registers.program_counter;

        let is_dep_addr = if let Some(ref p) = packer
            && let Some(known_dep) = p.info().dep_addr
            && known_dep >= ret_addr
        {
            pc == known_dep
        } else {
            pc < ret_addr && pc != 0x0000
        };

        if is_dep_addr {
            dep_addr = config.forced_dep_addr.unwrap_or(pc);
            break;
        }

        // ROM interception
        match handle_rom_entry(&mut cpu, &mut getin_index, 1) {
            RomAction::Continue => {}
            RomAction::Handled => {
                total_instructions += 1;
                continue;
            }
            RomAction::Exit => {
                // Packer finished via exit vector — no depacker phase needed
                dep_addr = config.forced_dep_addr.unwrap_or(pc);
                // Detect output and return
                let entry_point = pc;
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    basic_start,
                    load_end,
                    packer.as_deref(),
                    &system,
                    cpu.registers.index_y,
                );
            }
            RomAction::BasicRun => {
                // Re-parse SYS from memory and redirect
                if let Some(new_entry) = find_sys_address(&cpu.memory.mem, basic_start) {
                    cpu.registers.program_counter = new_entry;
                    total_instructions += 1;
                    continue;
                }
                // If we can't find a SYS, treat as exit
                dep_addr = config.forced_dep_addr.unwrap_or(pc);
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    pc,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    basic_start,
                    load_end,
                    packer.as_deref(),
                    &system,
                    cpu.registers.index_y,
                );
            }
        }

        if emulate_illegal_opcode(&mut cpu) {
            total_instructions += 1;
            continue;
        }

        cpu.single_step();
        total_instructions += 1;
        if total_instructions.is_multiple_of(30_000)
            && let Some(cb) = progress_callback
        {
            cb(total_instructions);
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2: Run decompression
    // Continues from where Phase 1 left off.
    //
    // Exit conditions:
    //  1. PC matches a known entry point (packer_info.entry_point) or BASIC RUN ($A7AE).
    //  2. Without a known entry point: PC >= ret_addr AND mem[PC] was written
    //     during emulation — the depacker finished and jumped to freshly unpacked code.
    //  3. Without a known entry point: PC >= ret_addr AND PC is outside the original
    //     loaded data range — the depacker jumped to an area that wasn't part of the
    //     original packed binary (e.g., it decompressed to a different region).
    //  4. ROM exit vector or BASIC RUN detection.
    //  5. Timeout.
    //
    // This handles inline packers (like Exomizer) that bounce between
    // depacker code below ret_addr (e.g., stack page) and depacker code
    // above ret_addr (e.g., $20B0) — those jumps back above ret_addr
    // are to the packer's own original code, not to unpacked data.
    //
    // Some 2-pass depackers (e.g. TinyCrunch) jump back to BASIC mid-unpack.
    // We allow up to 3 BASIC-SYS redirects before treating the next one as
    // a final exit, preventing both infinite loops and premature termination.
    // -----------------------------------------------------------------------
    let info = packer.as_ref().map(|p| p.info());
    let known_entry = info.as_ref().and_then(|i| i.entry_point);
    cpu.memory.in_phase2 = true;

    loop {
        if total_instructions >= config.max_instructions {
            return Err(UnpackError::Phase2Timeout);
        }

        if let Some(ref mut p) = packer {
            p.on_step(&mut cpu, 2);
        }

        let pc = cpu.registers.program_counter;
        let mut exit_triggered = false;
        let basic_mapped = is_basic_rom_mapped(&cpu.memory.mem, &system);
        let kernal_mapped = is_kernal_rom_mapped(&cpu.memory.mem, &system);
        let io_mapped = is_io_mapped(&cpu.memory.mem, &system);

        let in_rom_or_io = (is_in_basic_rom(pc, &system) && basic_mapped)
            || ((0xD000..=0xDFFF).contains(&pc) && io_mapped)
            || (is_in_kernal_rom(pc, &system) && kernal_mapped);

        let is_written_code = pc >= ret_addr
            && (pc as usize) < system.memory_boundaries()[0]
            && !in_rom_or_io
            && cpu.memory.written_phase2[pc as usize];

        if let Some(ke) = known_entry {
            let ke_hit = pc == ke;
            if (ke_hit && total_instructions > 10) || is_written_code {
                exit_triggered = true;
            }
        } else if is_written_code {
            exit_triggered = true;
        }

        if exit_triggered {
            let entry_point =
                if (basic_start..=basic_start.saturating_add(0x10)).contains(&pc) || pc == 0xA7AE {
                    find_sys_address(&cpu.memory.mem, basic_start).unwrap_or(pc)
                } else if pc == ret_addr {
                    find_entry_in_snapshot(&snapshot, load_addr, data_len, ret_addr).unwrap_or(pc)
                } else {
                    pc
                };
            return finish_unpack(
                &cpu.memory.mem,
                &snapshot,
                &cpu.memory.written,
                entry_point,
                dep_addr,
                ret_addr,
                total_instructions,
                basic_start,
                load_end,
                packer.as_deref(),
                &system,
                cpu.registers.index_y,
            );
        }

        // If the packer doesn't have a known entry point, exit when PC jumps
        // outside the original loaded data region (and above RAM $0800) to a written address.
        if known_entry.is_none()
            && pc >= system.ram_start()
            && (pc < load_addr || pc >= load_end)
            && cpu
                .memory
                .written
                .get(pc as usize)
                .copied()
                .unwrap_or(false)
        {
            let entry_point = pc;
            return finish_unpack(
                &cpu.memory.mem,
                &snapshot,
                &cpu.memory.written,
                entry_point,
                dep_addr,
                ret_addr,
                total_instructions,
                basic_start,
                load_end,
                packer.as_deref(),
                &system,
                cpu.registers.index_y,
            );
        }

        // ROM interception
        match handle_rom_entry(&mut cpu, &mut getin_index, 2) {
            RomAction::Continue => {}
            RomAction::Handled => {
                total_instructions += 1;
                continue;
            }
            RomAction::Exit | RomAction::BasicRun => {
                let entry_point = if is_in_basic_rom(pc, &system) {
                    find_sys_address(&cpu.memory.mem, basic_start).unwrap_or(pc)
                } else {
                    pc
                };
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    basic_start,
                    load_end,
                    packer.as_deref(),
                    &system,
                    cpu.registers.index_y,
                );
            }
        }

        if emulate_illegal_opcode(&mut cpu) {
            total_instructions += 1;
            continue;
        }

        cpu.single_step();
        total_instructions += 1;
        if total_instructions.is_multiple_of(30_000)
            && let Some(cb) = progress_callback
        {
            cb(total_instructions);
        }
    }
}

/// Extracts the unpacked result from memory after emulation completes.
#[allow(clippy::too_many_arguments)]
fn finish_unpack(
    mem: &[u8],
    snapshot: &[u8],
    written: &[bool],
    mut entry_point: u16,
    mut dep_addr: u16,
    ret_addr: u16,
    instructions_executed: u64,
    load_addr: u16,
    _load_end: u16,
    packer: Option<&dyn crate::packers::Packer>,
    system: &crate::state::types::System,
    y_reg: u8,
) -> Result<UnpackResult, UnpackError> {
    let (mut start_addr, mut end_addr) =
        detect_output_range(mem, snapshot, written, ret_addr, load_addr, system)
            .ok_or(UnpackError::NothingWritten)?;

    // Apply metadata overrides and post-emulation hooks from packer strategy
    if let Some(p) = packer {
        let info = p.info();
        if let Some(sa) = info.start_addr {
            start_addr = sa;
        }
        if let Some(ea) = info.end_addr {
            end_addr = ea;
        }
        if let Some(ea_ptr) = info.end_addr_ptr {
            let reported_end =
                u16::from_le_bytes([mem[ea_ptr as usize], mem[(ea_ptr + 1) as usize]]);
            if reported_end > start_addr && reported_end.saturating_sub(1) <= end_addr + 512 {
                end_addr = end_addr.max(reported_end.saturating_sub(1));
            }
        }
        if let Some(ep) = info.entry_point {
            entry_point = ep;
        }
        if let Some(da) = info.dep_addr {
            dep_addr = da;
        }

        let mut range = (start_addr, end_addr);
        p.post_emulate(
            mem,
            snapshot,
            written,
            &mut range,
            &mut entry_point,
            system,
            y_reg,
        );
        start_addr = range.0;
        end_addr = range.1;
    }

    // Override entry point with SYS target if entry point landed in ROM or BASIC stub range (load_addr..=load_addr+0x10)
    let basic_mapped = is_basic_rom_mapped(mem, system);
    let kernal_mapped = is_kernal_rom_mapped(mem, system);
    let is_rom_entry = (is_in_basic_rom(entry_point, system) && basic_mapped)
        || (is_in_kernal_rom(entry_point, system) && kernal_mapped);

    if ((load_addr..=load_addr.saturating_add(0x10)).contains(&entry_point) || is_rom_entry)
        && let Some(sys_ep) = find_sys_address(mem, load_addr)
    {
        entry_point = sys_ep;
        // If a BASIC SYS stub was found, the unpacked program output must include the stub.
        if start_addr > load_addr {
            start_addr = load_addr;
        }
    }

    // Invariant validation / adjustment:
    // Ensure start_addr <= entry_point and entry_point <= end_addr.
    if entry_point < start_addr && entry_point >= 0x0200 {
        start_addr = entry_point;
    }
    if entry_point > end_addr && (entry_point as usize) < mem.len() {
        end_addr = entry_point;
    }

    if entry_point < start_addr || entry_point > end_addr {
        return Err(UnpackError::InvalidAddressRange {
            start_addr,
            end_addr,
            entry_point,
        });
    }

    let data = mem[start_addr as usize..=end_addr as usize].to_vec();

    let packer_name = packer.map(|p| p.info().name.to_string());

    Ok(UnpackResult {
        data,
        start_addr,
        end_addr,
        entry_point,
        dep_addr,
        instructions_executed,
        packer_name,
    })
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
                exp_start: 0x0801,
                exp_end: 0x31FF,
                exp_entry: 0x2E00,
                exp_dep: Some(0x0100),
                exp_packer: Some("TSCrunch v1.3+-X2"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.tscrunch_x.prg",
                exp_start: 0x0800,
                exp_end: 0x9C1D,
                exp_entry: 0x1100,
                exp_dep: Some(0x0002),
                exp_packer: Some("TSCrunch v1.3+"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_mule.tscrunch_x2.prg",
                exp_start: 0x0801,
                exp_end: 0x9C1D,
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
                exp_start: 0x0801,
                exp_end: 0xC8C5,
                exp_entry: 0x0820,
                exp_dep: Some(0x01B2),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_thats_the_way_scoop.time_cruncher.prg",
                exp_start: 0x0801,
                exp_end: 0xE750,
                exp_entry: 0x0801,
                exp_dep: Some(0x0100),
                exp_packer: Some("Time Cruncher"),
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
                exp_end: 0xFA7A,
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
                exp_start: 0x080D,
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
                exp_start: 0x0801,
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
                exp_start: 0xEFB0,
                exp_end: 0xFEFF,
                exp_entry: 0xEFB0,
                exp_dep: Some(0x01AB),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: Some(150_000_000),
            },
            KnownUnpackCase {
                file: "c64_Layers.exo3.prg",
                exp_start: 0x080D,
                exp_end: 0xFBF1,
                exp_entry: 0x0834,
                exp_dep: Some(0x01C4),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: Some(350_000_000),
            },
            KnownUnpackCase {
                file: "c64_connection-8580.pucrunch.prg",
                exp_start: 0x0801,
                exp_end: 0xFF3F,
                exp_entry: 0x080D,
                exp_dep: Some(0x0116),
                exp_packer: Some("PUCrunch"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_lft-nine.exo3.prg",
                exp_start: 0x0801,
                exp_end: 0x7CBC,
                exp_entry: 0x080D,
                exp_dep: Some(0x0198),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_lft-rodents-in-the-attic.exo3.prg",
                exp_start: 0x0801,
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
                exp_start: 0x0801,
                exp_end: 0xCBE6,
                exp_entry: 0x0810,
                exp_dep: Some(0x01AB),
                exp_packer: Some("Exomizer 3.0"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_bluemarble4k_unk.prg",
                exp_start: 0x0800,
                exp_end: 0xF454,
                exp_entry: 0x0911,
                exp_dep: Some(0x07E8),
                exp_packer: None,
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_boo_alz64.prg",
                exp_start: 0x2A78,
                exp_end: 0x4D3C,
                exp_entry: 0x2A78,
                exp_dep: Some(0x005E),
                exp_packer: Some("ALZ64/Quiss"),
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_soul_on_fire_unk.prg",
                exp_start: 0x082B,
                exp_end: 0xE000,
                exp_entry: 0xE000,
                exp_dep: Some(0x005E),
                exp_packer: None,
                max_instructions: None,
            },
            KnownUnpackCase {
                file: "c64_323_ice_psm.1001_card_cruncher.prg",
                exp_start: 0x0801,
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
                0x0801,
                0xFEA4,
                0x1000,
                "c64_boilerplate.exo3.prg.1000",
            ),
            (
                "c64_druid_too.exo3.prg",
                0x0801,
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
                0x0801,
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
                0x0801,
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

            // Compare with unp64 output file if it exists
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

    // -----------------------------------------------------------------------
    // SYS parser tests
    // -----------------------------------------------------------------------

    /// Helper: build a minimal BASIC program in memory with a SYS line.
    fn make_basic_mem(tokens: &[u8]) -> Vec<u8> {
        let mut mem = vec![0u8; 0x1_0000];
        // BASIC program at $0801
        // Link pointer (can be anything non-zero)
        mem[0x0801] = 0x0B;
        mem[0x0802] = 0x08;
        // Line number 10
        mem[0x0803] = 0x0A;
        mem[0x0804] = 0x00;
        // Tokens
        for (i, &b) in tokens.iter().enumerate() {
            mem[0x0805 + i] = b;
        }
        // End of line
        mem[0x0805 + tokens.len()] = 0x00;
        mem
    }

    #[test]
    fn test_sys_simple() {
        // SYS 2061
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_spaces() {
        // SYS  2061
        let mem = make_basic_mem(&[0x9E, b' ', b' ', b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_parens() {
        // SYS(2061)
        let mem = make_basic_mem(&[0x9E, b'(', b'2', b'0', b'6', b'1', b')']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_addition() {
        // SYS 2048+16  → 2064
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAA, b'1', b'6']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2064));
    }

    #[test]
    fn test_sys_with_subtraction() {
        // SYS 2070-9  → 2061
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'7', b'0', 0xAB, b'9']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2061));
    }

    #[test]
    fn test_sys_with_multiplication() {
        // SYS 2048*1  → 2048
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAC, b'1']);
        assert_eq!(find_sys_address(&mem, 0x0801), Some(2048));
    }

    #[test]
    fn test_sys_not_found() {
        // No SYS token
        let mem = make_basic_mem(&[0x99, b'2', b'0', b'6', b'1']); // PRINT token
        assert_eq!(find_sys_address(&mem, 0x0801), None);
    }

    // -----------------------------------------------------------------------
    // Memory bus tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_write_tracking() {
        let mut mem = UnpackerMemory::new(crate::state::types::default_system());
        assert!(!mem.written[0x1000]);
        mem.set_byte(0x1000, 0x42);
        assert!(mem.written[0x1000]);
        assert_eq!(mem.get_byte(0x1000), 0x42);
    }

    #[test]
    fn test_memory_io_suppression() {
        let mut mem = UnpackerMemory::new(crate::state::types::default_system());
        // Set PLA bank register to default C64 value ($37) where I/O is visible
        mem.mem[0x01] = 0x37;
        mem.set_byte(0xD020, 0xFF); // VIC border color
        assert_eq!(mem.get_byte(0xD020), 0x00); // Reads return 0
        assert!(!mem.written[0xD020]); // Write not tracked
    }

    // -----------------------------------------------------------------------
    // Synthetic packer test
    // -----------------------------------------------------------------------

    #[test]
    fn test_synthetic_xor_decryptor() {
        // Build a program that:
        //   1) Copies a small depacker to $0003 (zero-page)
        //   2) Jumps to $0003 where the depacker XOR-decrypts 4 bytes to $0900
        //   3) The depacker then JMPs to $0900 (back above $0800)
        //
        // This properly exercises Phase 1 (PC drops from $080E → $0003)
        // and Phase 2 ($0003 runs depacker → JMP $0900).
        //
        // Layout at $0801:
        //   BASIC line: SYS 2062 (entry at $080E)
        //   $080E: LDX #$0F          ; depacker length - 1
        //   $0810: LDA $081D,X       ; load depacker byte
        //   $0813: STA $0003,X       ; store at zero-page
        //   $0816: DEX
        //   $0817: BPL $0810
        //   $0819: JMP $0003         ; jump to depacker (Phase 1 exit: PC < $0800)
        //
        // Depacker code (copied to $0003, runs in Phase 2):
        //   $0003: LDX #$03
        //   $0005: LDA $0022,X       ; load encrypted data (stored inline at $0022)
        //   $0008: EOR #$FF          ; decrypt
        //   $000A: STA $0900,X       ; store at $0900
        //   $000D: DEX
        //   $000E: BPL $0005
        //   $0010: JMP $0900         ; jump to unpacked code (Phase 2 exit: PC >= $0800)
        //
        // Encrypted data at $0022 (relative to zero-page):
        //   NOP NOP NOP RTS XOR'd with $FF = $15 $15 $15 $9F

        let mut raw = Vec::new();

        // BASIC line: 10 SYS 2062
        raw.extend_from_slice(&[
            0x14, 0x08, // Next line pointer
            0x0A, 0x00, // Line number 10
            0x9E, // SYS token
            b'2', b'0', b'6', b'2', // "2062"
            0x00, // End of BASIC line
            0x00, 0x00, // End of BASIC program
        ]);

        // Pad to $080E (offset 13)
        while raw.len() < 0x0E - 1 {
            raw.push(0x00);
        }

        // Copier code at $080E
        // Depacker is 19 bytes ($0003 to $0012) + 4 bytes encrypted data at $0022
        let depacker_len: u8 = 18; // 0-based: 0x12
        raw.extend_from_slice(&[
            0xA2,
            depacker_len, // LDX #depacker_len
            0xBD,
            0x1D,
            0x08, // LDA $081D,X (depacker source in PRG)
            0x9D,
            0x03,
            0x00, // STA $0003,X
            0xCA, // DEX
            0x10,
            0xF7, // BPL $0810
            0x4C,
            0x03,
            0x00, // JMP $0003
        ]);

        // Depacker code at $081D (will be copied to $0003)
        raw.extend_from_slice(&[
            0xA2, 0x03, // LDX #$03
            0xBD, 0x16, 0x00, // LDA $0016,X (encrypted data at $0016)
            0x49, 0xFF, // EOR #$FF
            0x9D, 0x00, 0x09, // STA $0900,X
            0xCA, // DEX
            0x10, 0xF5, // BPL $0005
            0x4C, 0x00, 0x09, // JMP $0900
        ]);

        // Pad depacker to full length (19 bytes)
        while raw.len() < (0x1D - 1) + 19 {
            raw.push(0x00);
        }

        // Encrypted data (will be at $0016 in zero-page after copy)
        // These are placed at the right offset so they end up at
        // $0003 + (0x16 - 0x03) = $0016 after the block copy
        // Since depacker is 19 bytes ($0003-$0015), encrypted data
        // starts at offset 19 ($0016). We need it at position
        // $081D + 19 = $0830 in the raw data.
        // Actually, the encrypted data needs to be in the copied block.
        // Let's adjust: the depacker loads from $0016 relative to zero-page,
        // which is absolute address $0016. The copy copies $0003-$0015 (19 bytes).
        // We need data at $0016-$0019 to be set up too.
        // Easiest fix: extend the copy to include the encrypted data.

        // Let me simplify: make the encrypted data part of the copy block.
        // Total block: depacker (13 bytes) + padding + encrypted data (4 bytes)
        // Adjust depacker_len to cover everything up to the encrypted data.

        // Actually, let's redesign more cleanly:
        raw.clear();

        // BASIC line: 10 SYS 2062
        raw.extend_from_slice(&[
            0x14, 0x08, // Next line pointer
            0x0A, 0x00, // Line number 10
            0x9E, // SYS token
            b'2', b'0', b'6', b'2', // "2062"
            0x00, // End of BASIC line
            0x00, 0x00, // End of BASIC program
        ]);

        // Pad to offset $0D (so code is at $080E)
        while raw.len() < 0x0D {
            raw.push(0x00);
        }

        // --- Depacker source (will be copied to $0003) ---
        // This is 17 bytes (indices 0..16, i.e. $0003..$0013)
        let depacker: Vec<u8> = vec![
            0xA2, 0x03, // $0003: LDX #$03
            0xBD, 0x14, 0x00, // $0005: LDA $0014,X  (data at $0014-$0017)
            0x49, 0xFF, // $0008: EOR #$FF
            0x9D, 0x00, 0x09, // $000A: STA $0900,X
            0xCA, // $000D: DEX
            0x10, 0xF5, // $000E: BPL $0005
            0x4C, 0x00, 0x09, // $0010: JMP $0900
        ];
        // Encrypted data (at $0014-$0017 after copy, i.e. offset 17..20 from $0003)
        let encrypted_data: Vec<u8> = vec![0x15, 0x15, 0x15, 0x9F]; // NOP NOP NOP RTS ^ $FF

        let total_copy_len = depacker.len() + encrypted_data.len(); // 21 bytes

        // Copier at $080E:
        //   LDX #(total_copy_len-1)
        //   LDA $source,X
        //   STA $0003,X
        //   DEX
        //   BPL loop
        //   JMP $0003
        let source_addr: u16 = 0x081C; // where depacker+data lives in the PRG
        raw.extend_from_slice(&[
            0xA2,
            (total_copy_len - 1) as u8, // LDX #20
            (source_addr & 0xFF) as u8, // placeholder, overwritten below
            0,                          // placeholder
            0,                          // placeholder
            0x9D,
            0x03,
            0x00, // STA $0003,X
            0xCA, // DEX
            0x10,
            0xF7, // BPL (back to LDA)
            0x4C,
            0x03,
            0x00, // JMP $0003
        ]);

        // Fix up the LDA absolute,X at offset $0D+2
        let lda_offset = 0x0D + 2; // position in raw
        let src_lo = (source_addr & 0xFF) as u8;
        let src_hi = (source_addr >> 8) as u8;
        raw[lda_offset] = 0xBD; // LDA abs,X
        raw[lda_offset + 1] = src_lo;
        raw[lda_offset + 2] = src_hi;

        // Pad until we reach source_addr offset in raw
        let source_offset = (source_addr - 0x0801) as usize;
        while raw.len() < source_offset {
            raw.push(0x00);
        }

        // Write depacker + encrypted data at source_addr
        raw.extend_from_slice(&depacker);
        raw.extend_from_slice(&encrypted_data);

        let config = UnpackConfig {
            max_instructions: 10_000,
            ..Default::default()
        };

        let result = unpack(&raw, 0x0801, &config, None).unwrap();
        assert_eq!(result.entry_point, 0x0900);
        assert_eq!(result.dep_addr, 0x0003);
        assert_eq!(result.start_addr, 0x0900);
        assert_eq!(result.end_addr, 0x0903);
    }

    // -----------------------------------------------------------------------
    // Real file test
    // -----------------------------------------------------------------------

    #[test]
    fn test_unpack_lxt_compressed() {
        let prg_data = std::fs::read("../../tests/6502/c64_moving_tubes_lxt.dali.prg").unwrap();

        // Parse PRG: first 2 bytes are load address (little-endian)
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        assert_eq!(load_addr, 0x0801, "Expected load address $0801");

        let config = UnpackConfig::default();
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        // Expected values from unp64 cross-validation:
        assert_eq!(result.dep_addr, 0x0003, "Depacker address should be $0003");
        assert_eq!(result.entry_point, 0x2E00, "Entry point should be $2E00");

        assert_eq!(result.start_addr, 0x0801, "Start address should be ");
        assert_eq!(result.end_addr, 0x31FF, "End address should be $31FF");

        // Should have executed a reasonable number of instructions
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

        // The unpacked data should be non-trivial
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
        let mut memory = UnpackerMemory::new(crate::state::types::default_system());
        memory.mem[0] = 0xCB; // SBX #$F8
        memory.mem[1] = 0xF8;
        let mut cpu = CPU::new(memory, Nmos6502);

        // Test case 1: 0xD8 SBX #$F8 -> (D8 & D8) - F8 = D8 - F8.
        // D8 < F8 -> Carry should be CLEAR!
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

        // Test case 2: 0xF8 SBX #$F8 -> (F8 & F8) - F8 = 00.
        // F8 >= F8 -> Carry should be SET!
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
