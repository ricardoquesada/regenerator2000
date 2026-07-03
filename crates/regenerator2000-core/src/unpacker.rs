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
        // I/O at $D000-$DFFF is only visible when the C64 PLA maps it:
        //   - CHAREN (bit 2) must be set, AND
        //   - At least one of LORAM (bit 0) or HIRAM (bit 1) must be set.
        // When both LORAM and HIRAM are clear, RAM is visible regardless of CHAREN.
        if (0xD000..=0xDFFF).contains(&a) {
            let bank = self.mem[0x01];
            let io_visible = (bank & 0x04 != 0) && (bank & 0x03 != 0);
            if io_visible {
                return 0;
            }
        }
        self.mem[a]
    }

    fn set_byte(&mut self, addr: u16, val: u8) {
        let a = addr as usize;
        // Same PLA logic as get_byte: suppress writes only when I/O is mapped.
        if (0xD000..=0xDFFF).contains(&a) {
            let bank = self.mem[0x01];
            let io_visible = (bank & 0x04 != 0) && (bank & 0x03 != 0);
            if io_visible {
                return;
            }
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

    // Check bank register to see if ROM is mapped
    let bank = cpu.memory.mem[0x01] & 0x07;
    let basic_mapped = bank & 0x01 != 0; // Bit 0: BASIC ROM at $A000
    let kernal_mapped = bank & 0x02 != 0; // Bit 1: Kernal ROM at $E000

    // If user code was written here (depacker at $FF00+, etc.) AND the ROM
    // at this address is not currently mapped, let it run as RAM code.
    // When ROM IS mapped, the CPU reads from ROM regardless of RAM writes,
    // so we must still intercept. This matters when depackers decompress
    // to the full $0800-$FF3F range — RAM underneath ROM gets written, but
    // the CPU still executes ROM when it enters BASIC RUN or Kernal calls.
    if cpu.memory.written[pc as usize] {
        let rom_mapped_here =
            ((0xA000..=0xBFFF).contains(&pc) && basic_mapped) || (pc >= 0xE000 && kernal_mapped);
        if !rom_mapped_here {
            return RomAction::Continue;
        }
    }

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
    ret_addr: u16,
) -> Option<(u16, u16)> {
    let scan_start = ret_addr as usize;

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
    // boundary, and in that case we skip the trim heuristic for the final
    // scan so legitimate high-region output is not misidentified as workspace.
    let boundaries: &[usize] = &[0x9FFF, 0xCFFF, 0xFFFF];
    let mut skip_trim_next = false;

    for (i, &boundary) in boundaries.iter().enumerate() {
        if let Some((start, end, untrimmed_end)) =
            scan_hybrid_range(mem, snapshot, written, scan_start, boundary, skip_trim_next)
        {
            let is_last = i == boundaries.len() - 1;
            let near_ceiling = (untrimmed_end as usize) + 256 >= boundary;

            // Also escalate when there are written+diffed bytes above the
            // current boundary — in-place depackers write to a disjoint high
            // region while leaving unchanged data in between.
            let has_output_above = !is_last
                && (boundary + 1..=0xFFFF).any(|addr| {
                    written.get(addr).copied().unwrap_or(false)
                        && mem.get(addr).copied().unwrap_or(0)
                            != snapshot.get(addr).copied().unwrap_or(0)
                });

            if is_last || (!near_ceiling && !has_output_above) {
                return Some((start, end));
            }

            // If we're escalating due to an output gap (not a ceiling hit),
            // skip the trim heuristic on the next scan — the trim is designed
            // for depacker workspace at the end of a contiguous range and
            // would falsely cut valid high-region decompressed output.
            // Use |= so the flag persists across any subsequent ceiling-hit
            // escalations that follow the initial gap-triggered escalation.
            skip_trim_next |= !near_ceiling && has_output_above;
        }
    }

    // Fallback: scan $E000-$FFFF for packers that decompress only into
    // the Kernal ROM area.
    scan_hybrid_range(mem, snapshot, written, 0xE000, 0xFFFF, false).map(|(s, e, _)| (s, e))
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
) -> Option<(u16, u16, u16)> {
    let upper = end
        .min(written.len() - 1)
        .min(mem.len() - 1)
        .min(snapshot.len() - 1);

    // Find the first written byte (start of output)
    let mut first_written = None;
    for (addr, &was_written) in written.iter().enumerate().take(upper + 1).skip(start) {
        if was_written {
            first_written = Some(addr);
            break;
        }
    }
    let first = first_written?;

    // Find all diff bytes and identify the end of the "main" diff block
    // by trimming small trailing clusters separated by non-diff gaps.
    let mut last_diff = None;
    for addr in start..=upper {
        if mem[addr] != snapshot[addr] {
            last_diff = Some(addr);
        }
    }

    let diff_end = match last_diff {
        Some(d) => d,
        None => {
            // No diff found — use last written byte
            let mut last = first;
            for (addr, &was_written) in written.iter().enumerate().take(upper + 1).skip(first) {
                if was_written {
                    last = addr;
                }
            }
            return Some((first as u16, last as u16, last as u16));
        }
    };

    // Walk backward from diff_end to detect and trim small trailing clusters.
    // Only apply trimming when the diff extends near the scan boundary (within
    // 128 bytes). A clean gap between diff_end and the boundary means the
    // output ends naturally with no depacker workspace to separate — trimming
    // would only produce false positives on natural gaps in program data.
    // Skip trimming entirely when the caller signals an in-place depacker gap.
    let trimmed_end = if !skip_trim && diff_end + 128 >= upper {
        trim_trailing_clusters(mem, snapshot, first, diff_end)
    } else {
        diff_end
    };

    // Extend past the trimmed end through written bytes that match the snapshot
    // (trailing zero-fills that are part of the real output).
    let mut extended_end = trimmed_end;
    for addr in (trimmed_end + 1)..=upper {
        if written[addr] && mem[addr] == snapshot[addr] {
            extended_end = addr;
        } else {
            break;
        }
    }

    Some((first as u16, extended_end as u16, diff_end as u16))
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
fn trim_trailing_clusters(mem: &[u8], snapshot: &[u8], start: usize, end: usize) -> usize {
    if end <= start {
        return end;
    }

    let range_len = end - start;
    // Don't scan deeper than the last 15% of the range (allow workspaces up to ~10KB)
    let scan_floor = if range_len > 256 {
        start + range_len * 85 / 100
    } else {
        start
    };

    let mut pos = end;
    let mut is_first_gap = true;
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

        // Count trailing diff bytes after the gap
        let tail_diffs: usize = ((gap_end + 1)..=end)
            .filter(|&a| mem[a] != snapshot[a])
            .count();

        // gap length
        let gap_len = gap_end - (pos + 1) + 1;

        // Check 1: tiny trailing cluster (< 16 diff bytes) with a
        // significant gap (>= 4 bytes). Only apply to the very first
        // (rightmost) gap — deeper gaps in the depacker workspace also
        // have small tails but shouldn't trigger.
        if is_first_gap && gap_len >= 4 && tail_diffs < 16 {
            return pos;
        }
        is_first_gap = false;

        // Check 2: proportionally small tail range with a real gap (>= 2 bytes).
        // Track the deepest qualifying gap rather than returning at the first
        // one — the first gap might be inside the depacker workspace, while
        // the real output/workspace boundary is deeper.
        let tail_range = end - gap_end;
        let main_len = (pos + 1) - start;
        if gap_len >= 2 && main_len > 0 && tail_range > 128 && tail_range * 50 < main_len {
            best_trim_pos = Some(pos);
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

    // Set up memory
    let mut memory = UnpackerMemory::new();

    // Load binary into memory at load_addr
    let data_len = raw_data.len().min(0x10000 - load_addr as usize);
    for (i, &byte) in raw_data.iter().take(data_len).enumerate() {
        memory.mem[load_addr as usize + i] = byte;
    }

    // Initialize zero-page and system area
    init_zero_page(&mut memory, load_addr, data_len as u16);

    // Take snapshot before emulation (used for output range end detection)
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
    let load_end = load_addr.wrapping_add(data_len as u16);
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
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
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
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    pc,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
                );
            }
        }

        cpu.single_step();
        total_instructions += 1;
        if total_instructions.is_multiple_of(10_000)
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
    //
    // Some 2-pass depackers (e.g. TinyCrunch) jump back to BASIC mid-unpack.
    // We allow up to 3 BASIC-SYS redirects before treating the next one as
    // a final exit, preventing both infinite loops and premature termination.
    // -----------------------------------------------------------------------
    let load_end = load_addr.wrapping_add(data_len as u16);
    let mut basic_run_count: u8 = 0;

    loop {
        if total_instructions >= config.max_instructions {
            return Err(UnpackError::Phase2Timeout);
        }

        let pc = cpu.registers.program_counter;

        // Exit condition: PC is at or above ret_addr AND points to memory
        // that was written during emulation — the depacker finished and jumped
        // to freshly decompressed code.
        //
        // Skip this check when PC is in a ROM/IO region ($A000-$BFFF,
        // $D000-$FFFF). Even though the RAM *underneath* these areas may have
        // been written by the depacker, the CPU is executing from ROM, not
        // from the decompressed data. Letting execution continue ensures the
        let in_rom_or_io = (0xA000..=0xBFFF).contains(&pc) || (0xD000..=0xFFFF).contains(&pc);

        if pc >= ret_addr && !in_rom_or_io && cpu.memory.written[pc as usize] {
            if pc == ret_addr {
                // The depacker landed exactly at ret_addr ($0800). The original
                // program entry point may be stored as a JMP in the packed binary
                // (which got overwritten during decompression). Scan the snapshot
                // for the first plausible JMP target above ret_addr + $300.
                let entry_point =
                    find_entry_in_snapshot(&snapshot, load_addr, data_len, ret_addr).unwrap_or(pc);
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
                );
            } else if (0x0800..=0x0810).contains(&pc) {
                // Landed in BASIC area — re-parse SYS from freshly decompressed BASIC.
                // 2-pass depackers (e.g. TinyCrunch) may land here between passes.
                // Redirect to the SYS address and continue Phase 2 unless we have
                // already redirected the maximum number of times.
                if basic_run_count < 3
                    && let Some(new_entry) = find_sys_address(&cpu.memory.mem)
                {
                    basic_run_count += 1;
                    cpu.registers.program_counter = new_entry;
                    total_instructions += 1;
                    continue;
                }
                let entry_point = find_sys_address(&cpu.memory.mem).unwrap_or(pc);
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
                );
            } else {
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    pc,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
                );
            }
        }

        // Also exit if PC is above ret_addr but outside the original loaded
        // data region. This catches cases where the depacker decompresses
        // to memory that was empty/zero before loading.
        // Exclude I/O ($D000-$DFFF) and ROM ($A000-$BFFF, $E000-$FFFF) regions —
        // depackers may temporarily execute in these areas for bank switching
        // or hardware setup, but they are not valid program entry points.
        if pc >= ret_addr && !in_rom_or_io && (pc < load_addr || pc >= load_end) {
            // Only exit if significant decompression has happened
            let written_above = cpu
                .memory
                .written
                .iter()
                .skip(ret_addr as usize)
                .filter(|&&w| w)
                .count();
            if written_above > 64 {
                let entry_point = pc;
                return finish_unpack(
                    &cpu.memory.mem,
                    &snapshot,
                    &cpu.memory.written,
                    entry_point,
                    dep_addr,
                    ret_addr,
                    total_instructions,
                    load_addr,
                    load_end,
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
                // When exit/run fires from BASIC ROM, the PC is a ROM address
                // (e.g., $A659 inside CLR). The real entry point is the SYS
                // address in the freshly decompressed BASIC program.
                let entry_point = if (0xA000..=0xBFFF).contains(&pc) {
                    find_sys_address(&cpu.memory.mem).unwrap_or(pc)
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
                    load_addr,
                    load_end,
                );
            }
        }

        cpu.single_step();
        total_instructions += 1;
        if total_instructions.is_multiple_of(10_000)
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
    dep_addr: u16,
    ret_addr: u16,
    instructions_executed: u64,
    _load_addr: u16,
    load_end: u16,
) -> Result<UnpackResult, UnpackError> {
    let (mut start_addr, mut end_addr) =
        detect_output_range(mem, snapshot, written, ret_addr).ok_or(UnpackError::NothingWritten)?;

    // unp64 compatibility for Dali v0.3.3 / fast:
    // Dali copies its depacker to zero page and jumps to $1100 when done.
    // It leaves the compressed payload at the top of memory, which defeats our standard gap trim
    // because the gap is > 25KB and hits the scan_floor safeguard.
    // We can reliably find the true end of the decompressed data by finding the largest
    // contiguous block of unwritten memory.
    if mem.len() >= 0xED
        && mem[0xEA] == 0x4C
        && entry_point == u16::from_le_bytes([mem[0xEB], mem[0xEC]])
    {
        let mut max_gap_len = 0;
        let mut max_gap_start = 0;
        let mut current_gap_len = 0;
        let mut current_gap_start = 0;

        for (i, &is_written) in written
            .iter()
            .enumerate()
            .take(0x10000)
            .skip(start_addr as usize)
        {
            if !is_written {
                if current_gap_len == 0 {
                    current_gap_start = i;
                }
                current_gap_len += 1;
            } else {
                if current_gap_len > max_gap_len {
                    max_gap_len = current_gap_len;
                    max_gap_start = current_gap_start;
                }
                current_gap_len = 0;
            }
        }
        if current_gap_len > max_gap_len {
            max_gap_len = current_gap_len;
            max_gap_start = current_gap_start;
        }

        if max_gap_len > 256 {
            let e = max_gap_start.saturating_sub(1);
            if e >= start_addr as usize {
                end_addr = e as u16;
            }
        }
    }

    // unp64 compatibility for MC-Cracken Compressor:
    // MC-Cracken's first pass depacker jumps to $1100 for the second pass,
    // leaving the exclusive end address at zero page $AE-$AF. unp64 stops emulation
    // at the jump to $1100 and uses that as the entry point, while reporting
    // the unpacked range up to the value in $AE-$AF.
    if entry_point == 0x1100 && mem.len() >= 0xB0 && mem[0xAB..=0xAD] == [0x4C, 0x72, 0x01] {
        let reported_end = u16::from_le_bytes([mem[0xAE], mem[0xAF]]);
        if reported_end > start_addr {
            end_addr = reported_end.saturating_sub(1);
        }
    }

    // unp64 compatibility for Exomizer 3:
    // If we detect the Exomizer CLI; JMP signature near the end of the packed data,
    // unp64 takes that JMP target as the entry point, and skips $0800-$080C (the stub).
    // The user explicitly requested to use unp64 output as the source of truth.
    if start_addr == 0x0800 {
        let scan_start = load_end.saturating_sub(64) as usize;
        let scan_end = load_end.saturating_sub(3) as usize;
        for i in (scan_start..scan_end).rev() {
            // Check for CLI (0x58) followed by JMP (0x4C)
            if snapshot[i] == 0x58 && snapshot[i + 1] == 0x4C {
                let target = u16::from_le_bytes([snapshot[i + 2], snapshot[i + 3]]);
                if target >= 0x0800 {
                    entry_point = target;
                    start_addr = 0x080D;
                    break;
                }
            }
        }
    }

    // unp64 compatibility for ByteBoozer 2:
    // ByteBoozer 2 places its workspace immediately after the unpacked data without a gap,
    // making our standard cluster trimming fail. However, like unp64, we can detect it
    // by signature and read the end address it deposits in zero page $77.
    if snapshot.len() >= 0x8C4 {
        let b0 = snapshot[0x80D..0x811] == [0x78, 0xA9, 0x34, 0x85]; // SEI; LDA #$34; STA $01
        let b1 = snapshot[0x813..0x817] == [0xB7, 0xBD, 0x1E, 0x08]; // LDX #$B7; LDA $081E,X
        let b2 = snapshot[0x870..0x874] == [0xA8, 0x20, 0xAD, 0x00]; // TAY; JSR $00AD
        let b3 = snapshot[0x8C0..0x8C4] == [0xAE, 0xD0, 0x02, 0xE6]; // LDX abs; BNE +2; INC
        if b0 && b1 && b2 && b3 {
            let reported_end = u16::from_le_bytes([mem[0x77], mem[0x78]]);
            if reported_end > start_addr {
                // reported_end is the byte immediately following the unpacked data
                end_addr = reported_end.saturating_sub(1);
            }
        }
    }

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
        assert_eq!(result.start_addr, 0x081C);
        assert_eq!(result.end_addr, 0x0914);
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

        assert_eq!(result.start_addr, 0x0800, "Start address should be $0800");
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

    #[test]
    fn test_debug_exo_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_moving_tubes_lxt.exo3.prg").unwrap();
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
        let result = unpack(raw_data, load_addr, &config, None);
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
        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0x31FF);
        assert_eq!(result.entry_point, 0x2E00, "Entry point should be $2E00");
    }

    #[test]
    fn test_debug_pucrunch_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_moving_tubes_lxt.pucrunch.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        // End is $3209 (10 bytes of PUCrunch workspace included — trimming
        // is skipped because diff_end is far from the scan ceiling).
        assert_eq!(result.end_addr, 0x3209);
        assert_eq!(result.entry_point, 0x2E00);
    }

    #[test]
    fn test_debug_mule_dali_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_mule.dali.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0x9D19);
        assert_eq!(result.entry_point, 0x1100);
        // Dali copies its depacker to zero page ($0003-$00EC) and decompresses
        // to $0800+. With bank-aware I/O emulation ($01=$34 maps RAM at
        // $D000-$DFFF), the depacker runs to completion and exits via JMP $1100.
    }

    #[test]
    fn test_debug_mule_mccracken_compressor_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_mule.mccracken_compressor.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0x9D19);
        assert_eq!(result.entry_point, 0x1100);
    }

    #[test]
    fn test_debug_roma_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_roma.exo3.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x080D);
        assert_eq!(result.end_addr, 0xC8C5);
        assert_eq!(result.entry_point, 0x0820);
        assert_eq!(result.dep_addr, 0x0100);
    }

    #[test]
    fn test_debug_scoop_unpack() {
        let prg_data =
            std::fs::read("../../tests/6502/c64_thats_the_way_scoop.time_cruncher.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0xE750);
        assert_eq!(result.entry_point, 0x0801);
        assert_eq!(result.dep_addr, 0x0100);
    }

    #[test]
    fn test_debug_f600_unpack() {
        let prg_data = std::fs::read("../../tests/6502/c64_f600.exo.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x080D);
        assert_eq!(result.end_addr, 0xFEFF);
        assert_eq!(result.entry_point, 0x0810);
        assert_eq!(result.dep_addr, 0x0100);
    }

    #[test]
    fn test_debug_hw20131031_exo_unpack() {
        // This Exomizer variant finishes by triggering BASIC RUN ($A7AE→$A659).
        // The entry point must come from the freshly decompressed SYS line,
        // not the BASIC ROM address.
        let prg_data = std::fs::read("../../tests/6502/c64_hw20131031.exo.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0xFF3F);
        // Entry point is $3000 from the decompressed BASIC SYS line,
        // NOT $A659 (the BASIC ROM CLR routine where exit was detected).
        assert_eq!(result.entry_point, 0x3000);
    }

    #[test]
    fn test_unpack_traveller_tiny_crunch() {
        let prg_data = std::fs::read("../../tests/6502/c64_traveller.tiny_crunch.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0801);
        assert_eq!(result.end_addr, 0xfffd);
        assert_eq!(result.entry_point, 0x0911);
    }

    #[test]
    fn test_unpack_spectro_exo3() {
        let prg_data = std::fs::read("../../tests/6502/c64_spectro.exo3.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x080D);
        assert_eq!(result.end_addr, 0xE7FF);
        assert_eq!(result.entry_point, 0x08A1);
    }

    #[test]
    fn test_unpack_copperbooze_byte_boozer2() {
        let prg_data = std::fs::read("../../tests/6502/c64_CopperBooze.byte_boozer2.prg").unwrap();
        let load_addr = u16::from_le_bytes([prg_data[0], prg_data[1]]);
        let raw_data = &prg_data[2..];

        let config = UnpackConfig {
            max_instructions: 50_000_000,
            ..Default::default()
        };
        let result = unpack(raw_data, load_addr, &config, None).unwrap();

        assert_eq!(result.start_addr, 0x0800);
        assert_eq!(result.end_addr, 0xE7FF);
        assert_eq!(result.entry_point, 0x1300);
    }
}
