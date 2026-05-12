//! 6502 emulation-based binary unpacker for compressed C64 programs.
//!
//! Many C64 programs are distributed packed with tools like Dali/LXT, Exomizer,
//! PUCrunch, ByteBoiler, etc. This module emulates the 6502 CPU to run the
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
#[derive(Debug, Clone)]
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
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyData => write!(f, "Empty input data"),
            Self::NoEntryPoint => write!(f, "Could not find SYS entry point"),
            Self::Phase1Timeout => write!(f, "Phase 1 timeout: depacker not found"),
            Self::Phase2Timeout => write!(f, "Phase 2 timeout: decompression did not finish"),
            Self::NothingWritten => write!(f, "No memory was modified during decompression"),
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
struct UnpackerMemory {
    /// Flat 64 KB memory.
    mem: Vec<u8>,
    /// Per-byte write tracking.
    written: Vec<bool>,
}

impl UnpackerMemory {
    /// Creates a new zeroed 64 KB memory.
    fn new() -> Self {
        Self {
            mem: vec![0u8; 0x1_0000],
            written: vec![false; 0x1_0000],
        }
    }
}

impl Bus for UnpackerMemory {
    fn get_byte(&mut self, addr: u16) -> u8 {
        let a = addr as usize;
        // I/O region: return 0
        if (0xD000..=0xDFFF).contains(&a) {
            return 0;
        }
        self.mem[a]
    }

    fn set_byte(&mut self, addr: u16, val: u8) {
        let a = addr as usize;
        // Suppress writes to I/O region
        if (0xD000..=0xDFFF).contains(&a) {
            return;
        }
        self.mem[a] = val;
        self.written[a] = true;
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
fn find_sys_address(mem: &[u8]) -> Option<u16> {
    // BASIC program starts at $0801
    // Format: [link_lo] [link_hi] [line_lo] [line_hi] [tokens...] [0x00]
    if mem.len() < 0x0806 {
        return None;
    }

    // Start scanning after the 4-byte header (link + line number)
    let start = 0x0805;
    let mut pos = start;

    // Find SYS token
    while pos < mem.len() && pos < 0x0900 {
        if mem[pos] == 0x00 {
            return None; // End of line without SYS
        }
        if mem[pos] == SYS_TOKEN {
            pos += 1;
            break;
        }
        pos += 1;
    }

    if pos >= mem.len() || pos >= 0x0900 {
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

/// Initializes C64 zero-page and system area defaults (per unp64 lines 572-620).
fn init_zero_page(mem: &mut UnpackerMemory, load_addr: u16, data_len: u16) {
    let end_addr = load_addr.wrapping_add(data_len);

    // Processor port
    mem.mem[0x00] = 0x2F; // DDR
    mem.mem[0x01] = 0x37; // Port: BASIC + Kernal mapped

    // BASIC text start
    mem.mem[0x2B] = 0x01;
    mem.mem[0x2C] = 0x08;

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
    mem.mem[0x37] = 0x00;
    mem.mem[0x38] = 0x08; // $0800 — this is an unusual default from unp64

    // First BASIC line number (read from loaded data)
    if data_len >= 4 {
        mem.mem[0x39] = mem.mem[load_addr as usize + 2];
        mem.mem[0x3A] = mem.mem[load_addr as usize + 3];
    }

    // End of program
    mem.mem[0xAE] = (end_addr & 0xFF) as u8;
    mem.mem[0xAF] = (end_addr >> 8) as u8;

    // IRQ vector
    mem.mem[0x0314] = 0x31;
    mem.mem[0x0315] = 0xEA;

    // Fill screen RAM with spaces
    for addr in 0x0400..=0x07E7 {
        mem.mem[addr] = 0x20;
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

    // Not in ROM space
    if pc < 0xA000 {
        return RomAction::Continue;
    }

    // If user code was written here (depacker at $FF00+, etc.), let it run
    if cpu.memory.written[pc as usize] {
        return RomAction::Continue;
    }

    // Check bank register to see if ROM is mapped
    let bank = cpu.memory.mem[0x01] & 0x07;
    let basic_mapped = bank & 0x01 != 0; // Bit 0: BASIC ROM at $A000
    let kernal_mapped = bank & 0x02 != 0; // Bit 1: Kernal ROM at $E000

    // BASIC ROM region $A000-$BFFF
    if (0xA000..=0xBFFF).contains(&pc) {
        if !basic_mapped {
            return RomAction::Continue; // RAM is visible, not ROM
        }

        // BASIC RUN detection (Phase 1 only triggers redirect; Phase 2 breaks)
        if matches!(
            pc,
            0xA7AE | 0xA7B1 | 0xA7EA | 0xA474 | 0xA533 | 0xA871 | 0xA888 | 0xA8BC
        ) {
            return RomAction::BasicRun;
        }

        // Phase 2 extended BASIC RUN detection
        if phase == 2 && ((0xA57C..=0xA659).contains(&pc) || pc == 0xA660 || pc == 0xA68E) {
            return RomAction::Exit;
        }

        // Fallback: force RTS
        force_rts(cpu);
        return RomAction::Handled;
    }

    // Kernal ROM region $E000-$FFFF
    if pc >= 0xE000 {
        if !kernal_mapped {
            return RomAction::Continue; // RAM is visible
        }

        match pc {
            // GETIN ($FFE4 / $F13E)
            0xFFE4 | 0xF13E => {
                cpu.registers.accumulator = GETIN_RESPONSES[*getin_index % GETIN_RESPONSES.len()];
                *getin_index += 1;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // CLRSCR / CINT ($E536 / $E544 / $FF5B)
            0xE536 | 0xE544 | 0xFF5B => {
                // Fill screen with spaces
                for addr in 0x0400..=0x07E7 {
                    cpu.memory.mem[addr] = 0x20;
                }
                cpu.registers.accumulator = 0x00;
                cpu.registers.index_x = 0x00;
                cpu.registers.index_y = 0x00;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // CHROUT with A=$93 (clear screen)
            0xFFD2 => {
                if cpu.registers.accumulator == 0x93 {
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
            0xFDA3 => {
                cpu.memory.mem[0x01] = 0xE7;
                cpu.registers.accumulator = 0xD7;
                cpu.registers.index_x = 0xFF;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // RESTOR ($FD15)
            0xFD15 => {
                cpu.registers.accumulator = 0x31;
                cpu.registers.index_x = 0x30;
                cpu.registers.index_y = 0xFF;
                force_rts(cpu);
                return RomAction::Handled;
            }

            // LOAD ($FFD5 / $F4A2) — exit vector
            0xFFD5 | 0xF4A2 => {
                return RomAction::Exit;
            }

            // Warm start ($FCE2) — exit vector
            0xFCE2 => {
                return RomAction::Exit;
            }

            // IRQ handler range ($EA31-$EB76) — exit in Phase 2
            addr if phase == 2 && (0xEA31..=0xEB76).contains(&addr) => {
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

// ---------------------------------------------------------------------------
// Output range detection
// ---------------------------------------------------------------------------

/// Detects the modified memory range by comparing against a pre-emulation snapshot.
///
/// Returns `(start_addr, end_addr)` inclusive, or `None` if nothing changed.
#[must_use]
fn detect_output_range(mem: &[u8], snapshot: &[u8]) -> Option<(u16, u16)> {
    // Primary scan: $0200-$9FFF (typical program area below ROM)
    if let Some(result) = scan_range(mem, snapshot, 0x0200, 0x9FFF) {
        return Some(result);
    }

    // Fallback scan: full memory excluding I/O
    // $0002-$CFFF
    if let Some((s1, e1)) = scan_range(mem, snapshot, 0x0002, 0xCFFF) {
        // Also check $E000-$FFFF
        if let Some((_s2, e2)) = scan_range(mem, snapshot, 0xE000, 0xFFFF) {
            return Some((s1, e2));
        }
        return Some((s1, e1));
    }

    // Just $E000-$FFFF
    scan_range(mem, snapshot, 0xE000, 0xFFFF)
}

/// Scans a memory range for differences against a snapshot.
#[must_use]
fn scan_range(mem: &[u8], snapshot: &[u8], start: usize, end: usize) -> Option<(u16, u16)> {
    let mut first = None;
    let mut last = None;

    for addr in start..=end {
        if addr < mem.len() && addr < snapshot.len() && mem[addr] != snapshot[addr] {
            if first.is_none() {
                first = Some(addr);
            }
            last = Some(addr);
        }
    }

    match (first, last) {
        (Some(f), Some(l)) => Some((f as u16, l as u16)),
        _ => None,
    }
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
/// Returns [`UnpackError`] if the binary cannot be unpacked.
pub fn unpack(
    raw_data: &[u8],
    load_addr: u16,
    config: &UnpackConfig,
) -> Result<UnpackResult, UnpackError> {
    if raw_data.is_empty() {
        return Err(UnpackError::EmptyData);
    }

    // Set up memory
    let mut memory = UnpackerMemory::new();

    // Load binary into memory at load_addr
    let data_len = raw_data.len().min(0x10000 - load_addr as usize);
    for (i, &byte) in raw_data.iter().take(data_len).enumerate() {
        memory.mem[load_addr as usize + i] = byte;
    }

    // Initialize zero-page and system area
    init_zero_page(&mut memory, load_addr, data_len as u16);

    // Take snapshot before emulation
    let snapshot = memory.mem.clone();

    // Find entry point
    let entry = if let Some(forced) = config.forced_entry {
        forced
    } else {
        find_sys_address(&memory.mem).ok_or(UnpackError::NoEntryPoint)?
    };

    let ret_addr = config.forced_ret_addr.unwrap_or(0x0800);

    // Create CPU
    let mut cpu = CPU::new(memory, Nmos6502);
    cpu.registers.program_counter = entry;
    cpu.registers.stack_pointer = mos6502::registers::StackPointer(0xFF);

    let mut getin_index: usize = 0;
    let mut total_instructions: u64 = 0;

    // -----------------------------------------------------------------------
    // Phase 1: Find the depacker
    // Run from entry point. Loop while PC >= ret_addr.
    // Exit when PC < ret_addr (depacker found) or exit vector hit.
    // -----------------------------------------------------------------------
    let dep_addr;
    loop {
        if total_instructions >= config.max_instructions {
            return Err(UnpackError::Phase1Timeout);
        }

        let pc = cpu.registers.program_counter;

        // Exit condition: PC dropped below ret_addr
        if pc < ret_addr {
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
                    entry_point,
                    dep_addr,
                    total_instructions,
                );
            }
            RomAction::BasicRun => {
                // Re-parse SYS from memory and redirect
                if let Some(new_entry) = find_sys_address(&cpu.memory.mem) {
                    cpu.registers.program_counter = new_entry;
                    total_instructions += 1;
                    continue;
                }
                // If we can't find a SYS, treat as exit
                dep_addr = config.forced_dep_addr.unwrap_or(pc);
                return finish_unpack(&cpu.memory.mem, &snapshot, pc, dep_addr, total_instructions);
            }
        }

        cpu.single_step();
        total_instructions += 1;
    }

    // -----------------------------------------------------------------------
    // Phase 2: Run decompression
    // Continues from where Phase 1 left off.
    //
    // Exit conditions:
    //  1. PC >= ret_addr AND mem[PC] was written during emulation — the
    //     depacker finished and jumped to freshly unpacked code.
    //  2. PC >= ret_addr AND PC is outside the original loaded data range —
    //     the depacker jumped to an area that wasn't part of the original
    //     packed binary (e.g., it decompressed to a different region).
    //  3. ROM exit vector or BASIC RUN detection.
    //  4. Timeout.
    //
    // This handles inline packers (like Exomizer) that bounce between
    // depacker code below ret_addr (e.g., stack page) and depacker code
    // above ret_addr (e.g., $20B0) — those jumps back above ret_addr
    // are to the packer's own original code, not to unpacked data.
    // -----------------------------------------------------------------------
    let load_end = load_addr.wrapping_add(data_len as u16);

    loop {
        if total_instructions >= config.max_instructions {
            return Err(UnpackError::Phase2Timeout);
        }

        let pc = cpu.registers.program_counter;

        // Exit condition: PC is above ret_addr AND points to written memory.
        // Written memory means the depacker put code there during emulation,
        // so this must be the unpacked program's entry point.
        if pc >= ret_addr && cpu.memory.written[pc as usize] {
            let entry_point = pc;
            return finish_unpack(
                &cpu.memory.mem,
                &snapshot,
                entry_point,
                dep_addr,
                total_instructions,
            );
        }

        // Also exit if PC is above ret_addr but outside the original loaded
        // data region. This catches cases where the depacker decompresses
        // to memory that was empty/zero before loading.
        if pc >= ret_addr && (pc < load_addr || pc >= load_end) {
            // Only exit if significant decompression has happened
            let written_above = cpu
                .memory
                .written
                .iter()
                .skip(ret_addr as usize)
                .filter(|&&w| w)
                .count();
            if written_above > 256 {
                let entry_point = pc;
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    entry_point,
                    dep_addr,
                    total_instructions,
                );
            }
        }

        // ROM interception
        match handle_rom_entry(&mut cpu, &mut getin_index, 2) {
            RomAction::Continue => {}
            RomAction::Handled => {
                total_instructions += 1;
                continue;
            }
            RomAction::Exit | RomAction::BasicRun => {
                let entry_point = pc;
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    entry_point,
                    dep_addr,
                    total_instructions,
                );
            }
        }

        cpu.single_step();
        total_instructions += 1;
    }
}

/// Extracts the unpacked result from memory after emulation completes.
fn finish_unpack(
    mem: &[u8],
    snapshot: &[u8],
    entry_point: u16,
    dep_addr: u16,
    instructions_executed: u64,
) -> Result<UnpackResult, UnpackError> {
    let (start_addr, end_addr) =
        detect_output_range(mem, snapshot).ok_or(UnpackError::NothingWritten)?;

    let data = mem[start_addr as usize..=end_addr as usize].to_vec();

    Ok(UnpackResult {
        data,
        start_addr,
        end_addr,
        entry_point,
        dep_addr,
        instructions_executed,
    })
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(find_sys_address(&mem), Some(2061));
    }

    #[test]
    fn test_sys_with_spaces() {
        // SYS  2061
        let mem = make_basic_mem(&[0x9E, b' ', b' ', b'2', b'0', b'6', b'1']);
        assert_eq!(find_sys_address(&mem), Some(2061));
    }

    #[test]
    fn test_sys_with_parens() {
        // SYS(2061)
        let mem = make_basic_mem(&[0x9E, b'(', b'2', b'0', b'6', b'1', b')']);
        assert_eq!(find_sys_address(&mem), Some(2061));
    }

    #[test]
    fn test_sys_with_addition() {
        // SYS 2048+16  → 2064
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAA, b'1', b'6']);
        assert_eq!(find_sys_address(&mem), Some(2064));
    }

    #[test]
    fn test_sys_with_subtraction() {
        // SYS 2070-9  → 2061
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'7', b'0', 0xAB, b'9']);
        assert_eq!(find_sys_address(&mem), Some(2061));
    }

    #[test]
    fn test_sys_with_multiplication() {
        // SYS 2048*1  → 2048
        let mem = make_basic_mem(&[0x9E, b'2', b'0', b'4', b'8', 0xAC, b'1']);
        assert_eq!(find_sys_address(&mem), Some(2048));
    }

    #[test]
    fn test_sys_not_found() {
        // No SYS token
        let mem = make_basic_mem(&[0x99, b'2', b'0', b'6', b'1']); // PRINT token
        assert_eq!(find_sys_address(&mem), None);
    }

    // -----------------------------------------------------------------------
    // Memory bus tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_write_tracking() {
        let mut mem = UnpackerMemory::new();
        assert!(!mem.written[0x1000]);
        mem.set_byte(0x1000, 0x42);
        assert!(mem.written[0x1000]);
        assert_eq!(mem.get_byte(0x1000), 0x42);
    }

    #[test]
    fn test_memory_io_suppression() {
        let mut mem = UnpackerMemory::new();
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

        let result = unpack(&raw, 0x0801, &config).unwrap();
        assert_eq!(result.entry_point, 0x0900);
        assert_eq!(result.dep_addr, 0x0003);
        // The output range should cover $0900
        assert!(result.start_addr <= 0x0900);
    }

    // -----------------------------------------------------------------------
    // Real file test
    // -----------------------------------------------------------------------

    #[test]
    fn test_unpack_lxt_compressed() {
        let prg_data = std::fs::read("../../tests/6502/moving_tubes_lxt_dali.prg").unwrap();

        // Parse PRG: first 2 bytes are load address (little-endian)
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        assert_eq!(load_addr, 0x0801, "Expected load address $0801");

        let config = UnpackConfig::default();
        let result = unpack(raw_data, load_addr, &config).unwrap();

        // Expected values from unp64 cross-validation:
        assert_eq!(result.dep_addr, 0x0003, "Depacker address should be $0003");
        assert_eq!(result.entry_point, 0x2E00, "Entry point should be $2E00");

        // Data range should start at $0800 (or close to it)
        assert!(
            result.start_addr <= 0x0801,
            "Start address ${:04X} should be <= $0801",
            result.start_addr
        );

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

    #[test]
    fn test_debug_exo_unpack() {
        let prg_data = std::fs::read("../../tests/6502/moving_tubes_lxt_exo.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        println!(
            "EXO: {} bytes, load=${:04X}, data={} bytes",
            prg_data.len(),
            load_addr,
            raw_data.len()
        );

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config);
        match &result {
            Ok(r) => {
                println!(
                    "SUCCESS: ${:04X}-${:04X}, entry=${:04X}, dep=${:04X}, instr={}",
                    r.start_addr, r.end_addr, r.entry_point, r.dep_addr, r.instructions_executed
                );
                println!("Unpacked data length: {}", r.data.len());
            }
            Err(e) => {
                println!("FAILED: {e:?}");
            }
        }
        let result = result.unwrap();
        // Exomizer should produce substantial output
        assert!(
            result.data.len() > 1000,
            "Expected >1KB output, got {} bytes",
            result.data.len()
        );
        // Entry point should be $2E00 (same as the Dali/LXT version)
        assert_eq!(result.entry_point, 0x2E00, "Entry point should be $2E00");
    }
}
