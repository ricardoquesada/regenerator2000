//! Decompression execution engine driving 2-phase MOS 6502 emulation.

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::Bus;

use super::bus::{C64Bus, is_basic_rom_mapped, is_io_mapped, is_kernal_rom_mapped};
use super::detector::{detect_output_range, find_entry_in_snapshot};
use super::machine::{KernalTrap, Phase, RomTrapTable, SCREEN_CODE_SPACE, get_basic_start};
use super::{UnpackConfig, UnpackError, UnpackResult, find_sys_address};
use crate::packers::{
    AddrSource, Bounds, Hints, LoadedImage, PackerHooks, PackerId, PackerInfo, PackerSpec,
    PointerGuard, PointerMonitor, RefineContext, Registers, StepContext, registry,
};
use crate::state::types::System;
use std::fmt;

/// Default C64 post-reset zero-page RAM image (`$0000`–`$00FF`) copied before packer
/// detection when `system.is_c64()` holds.
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

/// Size of the 6502 64 KB address space.
const ADDRESS_SPACE_SIZE: usize = 0x1_0000;

/// Initial 6502 stack pointer (`$F6`) set before emulation begins.
const INITIAL_STACK_POINTER: u8 = 0xF6;

/// Minimum global instruction count (`> 10`) required before a packer's
/// `known_entry` hint is allowed to end [`Phase::Decompress`] (Invariant 3).
const MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY: u64 = 10;

/// Instruction interval between invocations of the optional progress callback.
const PROGRESS_CALLBACK_INTERVAL: u64 = 30_000;

/// Maximum forward slack (in bytes) past the current `end_addr` within which a
/// packer's `end_addr_ptr` hint is accepted (Invariant 5).
const END_PTR_SLACK: u16 = 512;

/// Minimum valid entry point address (`$0200`, above zero page and stack page)
/// allowed to clamp `start_addr` downward in [`finish`] (Invariant 5).
const MIN_VALID_ENTRY_ADDR: u16 = 0x0200;

/// Maximum number of recent program counter values retained in [`UnpackReport::trace_tail`]
/// when [`UnpackConfig::trace`] is enabled.
pub const TRACE_TAIL_CAPACITY: usize = 32;

/// Why emulation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// [`Phase::Decompress`] reached an address in RAM that was written during emulation.
    WrittenCodeReached {
        /// Program counter at exit.
        pc: u16,
    },
    /// [`Phase::Decompress`] reached the packer's known entry point after the
    /// initial 10-instruction guard.
    KnownEntryReached {
        /// Program counter at exit.
        pc: u16,
    },
    /// [`Phase::Decompress`] jumped outside the original loaded data region
    /// (at or above `ram_start`) to a written address.
    LeftLoadedRegion {
        /// Program counter at exit.
        pc: u16,
    },
    /// Execution reached a trapped KERNAL or BASIC ROM exit vector.
    RomExitVector {
        /// Program counter at exit.
        pc: u16,
    },
    /// Execution reached a trapped BASIC `RUN` entry vector.
    BasicRun,
}

impl fmt::Display for ExitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrittenCodeReached { pc } => write!(f, "written_code(${pc:04X})"),
            Self::KnownEntryReached { pc } => write!(f, "known_entry(${pc:04X})"),
            Self::LeftLoadedRegion { pc } => write!(f, "left_loaded_region(${pc:04X})"),
            Self::RomExitVector { pc } => write!(f, "rom_exit_vector(${pc:04X})"),
            Self::BasicRun => f.write_str("basic_run"),
        }
    }
}

/// Which step of `finish` or exit resolution produced an output bound or the entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundSource {
    /// Generic write + snapshot-diff scan (`detect_output_range`).
    WriteDiff,
    /// Static [`AddrSource`] ([`AddrSource::Const`], [`AddrSource::BasicStart`],
    /// [`AddrSource::OperandLe`], [`AddrSource::AbsoluteLe`]) or resolved hint from a [`PackerSpec`].
    PackerSpec {
        /// Identifier of the [`PackerSpec`] that supplied the bound.
        id: PackerId,
    },
    /// Zero-page or RAM pointer read after Phase 2 (`AddrSource::PointerAfterRun`).
    PointerAfterRun {
        /// Address of the low byte of the pointer read after Phase 2.
        addr: u16,
    },
    /// `PackerHooks::refine` modified this bound after emulation.
    PackerRefine {
        /// Identifier of the `PackerSpec` whose `refine` hook modified the bound.
        id: PackerId,
    },
    /// Lowered `start_addr` to `basic_start` because a BASIC `SYS` line was found.
    SysLineClamp,
    /// Clamped `start_addr` or `end_addr` so `entry_point` lies inside `start_addr..=end_addr`.
    EntryClamp,
    /// Forced via [`UnpackConfig`] or a packer's `forced_entry` hint.
    Forced,
    /// Entry point taken directly from the CPU program counter at Phase 2 exit (Ruling R6).
    ExitPc,
    /// Entry point recovered from the pre-emulation snapshot via `find_entry_in_snapshot` (Ruling R6).
    SnapshotEntry,
    /// Entry point resolved from a BASIC `SYS` line at `basic_start` (Ruling R6).
    SysLineLookup,
}

impl fmt::Display for BoundSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteDiff => f.write_str("write_diff"),
            Self::PackerSpec { id } => write!(f, "packer_spec({id})"),
            Self::PointerAfterRun { addr } => write!(f, "pointer_after_run(${addr:04X})"),
            Self::PackerRefine { id } => write!(f, "packer_refine({id})"),
            Self::SysLineClamp => f.write_str("sys_line_clamp"),
            Self::EntryClamp => f.write_str("entry_clamp"),
            Self::Forced => f.write_str("forced"),
            Self::ExitPc => f.write_str("exit_pc"),
            Self::SnapshotEntry => f.write_str("snapshot_entry"),
            Self::SysLineLookup => f.write_str("sys_line_lookup"),
        }
    }
}

/// Structured provenance report explaining how [`UnpackResult`] bounds, entry point,
/// and exit condition were determined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackReport {
    /// Why emulation stopped.
    pub exit_reason: ExitReason,
    /// Instructions executed in `Phase::FindDepacker` (Phase 1).
    pub find_depacker_instructions: u64,
    /// Instructions executed in `Phase::Decompress` (Phase 2).
    pub decompress_instructions: u64,
    /// Provenance of [`UnpackResult::start_addr`].
    pub start_source: BoundSource,
    /// Provenance of [`UnpackResult::end_addr`].
    pub end_source: BoundSource,
    /// Provenance of [`UnpackResult::entry_point`].
    pub entry_source: BoundSource,
    /// Last [`TRACE_TAIL_CAPACITY`] program counters executed before exit when
    /// [`UnpackConfig::trace`] is `true`; empty when `false`.
    pub trace_tail: Vec<u16>,
}

/// Fixed-capacity ring buffer recording the last [`TRACE_TAIL_CAPACITY`] executed PCs.
#[derive(Debug, Clone)]
struct TraceRing {
    buf: [u16; TRACE_TAIL_CAPACITY],
    head: usize,
    len: usize,
}

impl TraceRing {
    const fn new() -> Self {
        Self {
            buf: [0; TRACE_TAIL_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, pc: u16) {
        if self.len < TRACE_TAIL_CAPACITY {
            self.buf[(self.head + self.len) % TRACE_TAIL_CAPACITY] = pc;
            self.len += 1;
        } else {
            self.buf[self.head] = pc;
            self.head = (self.head + 1) % TRACE_TAIL_CAPACITY;
        }
    }

    fn to_vec(&self) -> Vec<u16> {
        (0..self.len)
            .map(|i| self.buf[(self.head + i) % TRACE_TAIL_CAPACITY])
            .collect()
    }
}

/// Outcome of a single emulation state-machine step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Step completed; continue executing in the current [`Phase`].
    Continue,
    /// Depacker reached; transition from [`Phase::FindDepacker`] to [`Phase::Decompress`].
    ToPhase2 { dep_addr: u16 },
    /// Emulation completed; proceed to [`finish`].
    Done(ExitReason),
    /// Global instruction budget exhausted in the given [`Phase`].
    Timeout(Phase),
}

/// Action to take after ROM interception.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Core 2-phase decompression execution engine.
pub struct UnpackEngine<'a> {
    config: &'a UnpackConfig,
    progress_callback: Option<&'a dyn Fn(u64)>,
}

impl<'a> UnpackEngine<'a> {
    /// Creates a new decompression engine instance.
    #[must_use]
    pub fn new(config: &'a UnpackConfig, progress_callback: Option<&'a dyn Fn(u64)>) -> Self {
        Self {
            config,
            progress_callback,
        }
    }

    /// Executes the 2-phase depacking process on `raw_data` loaded at `load_addr`.
    ///
    /// # Errors
    ///
    /// Returns [`UnpackError`] if input is empty, no entry point is found,
    /// or emulation times out.
    pub fn run(&self, raw_data: &[u8], load_addr: u16) -> Result<UnpackResult, UnpackError> {
        let mut state =
            EmulationState::new(self.config, self.progress_callback, raw_data, load_addr)?;
        state.run()
    }
}

/// Per-run state machine driving [`Phase::FindDepacker`] and [`Phase::Decompress`].
struct EmulationState<'a> {
    config: &'a UnpackConfig,
    progress_callback: Option<&'a dyn Fn(u64)>,
    system: System,
    basic_start: u16,
    load_addr: u16,
    data_len: usize,
    load_end: u16,
    ret_addr: u16,
    traps: &'static RomTrapTable,
    snapshot: Vec<u8>,
    hooks: Option<Box<dyn PackerHooks>>,
    /// Stable identifier of the matched [`PackerSpec`], if any.
    packer_id: Option<PackerId>,
    /// Packer metadata hints resolved once at detection time (`P2.3.2`).
    hints: Option<PackerInfo>,
    /// Declarative [`Hints`] from the matched [`PackerSpec`], if any.
    raw_hints: Option<Hints>,
    /// Whether the initial entry point was forced (`config.forced_entry` or `Hints::forced_entry`).
    entry_was_forced: bool,
    cpu: CPU<C64Bus, Nmos6502>,
    phase: Phase,
    dep_addr: u16,
    getin_index: usize,
    /// Single global instruction counter across both phases (Invariant 4).
    total_instructions: u64,
    /// Instructions executed in [`Phase::FindDepacker`].
    find_depacker_instructions: u64,
    /// Instructions executed in [`Phase::Decompress`].
    decompress_instructions: u64,
    /// Bounded PC ring buffer populated when [`UnpackConfig::trace`] is `true`.
    trace_ring: TraceRing,
}

impl<'a> EmulationState<'a> {
    /// Initializes memory, zero page, packer detection, entry point, patches, snapshot, and CPU state.
    fn new(
        config: &'a UnpackConfig,
        progress_callback: Option<&'a dyn Fn(u64)>,
        raw_data: &[u8],
        load_addr: u16,
    ) -> Result<Self, UnpackError> {
        Self::new_with_specs(
            config,
            progress_callback,
            raw_data,
            load_addr,
            registry::PACKERS,
        )
    }

    /// Initializes [`EmulationState`] using the provided `specs` registry slice.
    ///
    /// Strictly enforces the M3.6a initialization order (`P3.6.1`):
    /// 1. `init_zero_page(&mut memory, ...)`
    /// 2. `registry::detect_spec_in(specs, &LoadedImage::new(&memory.mem, load_addr, load_end, &system))`
    /// 3. Entry resolution (`config.forced_entry` > `spec.forced_entry` > `find_sys_address`)
    /// 4. Apply `sig_match.resolved.patches` to `memory.mem`
    /// 5. `let snapshot = memory.mem.clone();`
    fn new_with_specs(
        config: &'a UnpackConfig,
        progress_callback: Option<&'a dyn Fn(u64)>,
        raw_data: &[u8],
        load_addr: u16,
        specs: &[&'static PackerSpec],
    ) -> Result<Self, UnpackError> {
        if raw_data.is_empty() {
            return Err(UnpackError::EmptyData);
        }

        let system = config
            .target_system
            .clone()
            .unwrap_or_else(crate::state::types::default_system);

        let basic_start = get_basic_start(load_addr);
        let traps = RomTrapTable::for_system(&system);

        let basic_rom = config
            .basic_rom
            .clone()
            .or_else(|| crate::assets::default_basic_rom(&system));
        let kernal_rom = config
            .kernal_rom
            .clone()
            .or_else(|| crate::assets::default_kernal_rom(&system));
        let char_rom = config
            .char_rom
            .clone()
            .or_else(|| crate::assets::default_char_rom(&system));

        // Set up memory
        let mut memory = C64Bus::new(system.clone(), basic_rom, kernal_rom, char_rom);

        // Load binary into memory at load_addr
        let data_len = raw_data
            .len()
            .min(ADDRESS_SPACE_SIZE - usize::from(load_addr));
        for (i, &byte) in raw_data.iter().take(data_len).enumerate() {
            memory.mem[usize::from(load_addr) + i] = byte;
        }

        // 1. Initialize zero-page and system area before detection so `Anchor::BasicStart`
        //    and spec resolvers see initialized zero-page pointers (`T3.6a.4`).
        init_zero_page(&mut memory, load_addr, data_len as u16, basic_start);

        let ret_addr = config
            .forced_ret_addr
            .unwrap_or_else(|| load_addr.min(system.ram_start()));
        // Note: `load_end` wraps to `$0000` when `load_addr + data_len == 0x1_0000` (pinned by T2.0.15).
        let load_end = load_addr.wrapping_add(data_len as u16);

        // 2. Detect packer spec against the zero-page-initialized image (`P3.5.3`, `P3.6.1`).
        let detected = {
            let image = LoadedImage::new(&memory.mem, load_addr, load_end, &system);
            registry::detect_spec_in(specs, &image).map(|(spec, sig_match)| {
                let info = PackerInfo::resolve(&image, &sig_match);
                let spec_forced_entry = sig_match
                    .resolved
                    .hints
                    .forced_entry
                    .and_then(|src| src.resolve_static(&image, sig_match.anchor_addr));
                let hooks = spec.hooks.map(|factory| factory(&image, &sig_match));
                (info, sig_match, spec_forced_entry, hooks)
            })
        };

        // 3. Resolve entry point: user `config.forced_entry` > spec `forced_entry` > BASIC `SYS` (`T3.6.12`).
        let spec_forced_entry = detected.as_ref().and_then(|(_, _, fe, _)| *fe);
        let entry_was_forced = config.forced_entry.is_some() || spec_forced_entry.is_some();
        let entry = if let Some(forced) = config.forced_entry {
            forced
        } else if let Some(spec_entry) = spec_forced_entry {
            spec_entry
        } else {
            find_sys_address(&memory.mem, basic_start)
                .or_else(|| find_sys_address(&memory.mem, system.default_basic_start()))
                .ok_or(UnpackError::NoEntryPoint)?
        };

        // 4. Apply pre-emulation patches to `memory.mem` before capturing `snapshot` (`P3.6.1`).
        let (packer_id, hints, raw_hints, hooks) =
            if let Some((info, sig_match, _, hooks)) = detected {
                for patch in sig_match.resolved.patches {
                    patch.apply(&mut memory.mem, sig_match.anchor_addr);
                }
                (
                    Some(sig_match.spec_id),
                    Some(info),
                    Some(sig_match.resolved.hints),
                    hooks,
                )
            } else {
                (None, None, None, None)
            };

        // 5. Take snapshot before emulation (after zero-page init and spec patches).
        let snapshot = memory.mem.clone();

        // Create CPU
        let mut cpu = CPU::new(memory, Nmos6502);
        cpu.registers.program_counter = entry;
        cpu.registers.stack_pointer = mos6502::registers::StackPointer(INITIAL_STACK_POINTER);

        Ok(Self {
            config,
            progress_callback,
            system,
            basic_start,
            load_addr,
            data_len,
            load_end,
            ret_addr,
            traps,
            snapshot,
            hooks,
            packer_id,
            hints,
            raw_hints,
            entry_was_forced,
            cpu,
            phase: Phase::FindDepacker,
            dep_addr: 0,
            getin_index: 0,
            total_instructions: 0,
            find_depacker_instructions: 0,
            decompress_instructions: 0,
            trace_ring: TraceRing::new(),
        })
    }

    /// Drives the single emulation state-machine loop and finalizes the result via [`finish`].
    fn run(&mut self) -> Result<UnpackResult, UnpackError> {
        let exit_reason = loop {
            let step = match self.phase {
                Phase::FindDepacker => self.step_find_depacker(),
                Phase::Decompress => self.step_decompress(),
            };

            match step {
                Step::Continue => {}
                Step::ToPhase2 { dep_addr } => {
                    self.dep_addr = dep_addr;
                    self.cpu.memory.in_phase2 = true;
                    self.phase = Phase::Decompress;
                }
                Step::Done(reason) => break reason,
                Step::Timeout(Phase::FindDepacker) => return Err(UnpackError::Phase1Timeout),
                Step::Timeout(Phase::Decompress) => return Err(UnpackError::Phase2Timeout),
            }
        };

        let (entry_point, entry_source, dep_addr) = self.resolve_exit_with_source(exit_reason);
        finish(FinishContext {
            mem: &self.cpu.memory.mem,
            snapshot: &self.snapshot,
            written: &self.cpu.memory.written,
            entry_point,
            entry_source,
            dep_addr,
            ret_addr: self.ret_addr,
            instructions_executed: self.total_instructions,
            find_depacker_instructions: self.find_depacker_instructions,
            decompress_instructions: self.decompress_instructions,
            exit_reason,
            trace_tail: if self.config.trace {
                self.trace_ring.to_vec()
            } else {
                Vec::new()
            },
            packer_id: self.packer_id,
            basic_start: self.basic_start,
            hooks: self.hooks.as_deref(),
            hints: self.hints.as_ref(),
            raw_hints: self.raw_hints.as_ref(),
            system: &self.system,
            y_reg: self.cpu.registers.index_y,
        })
    }

    /// Records one executed instruction at `pc`, updating the global counter, the active
    /// phase counter, and (when [`UnpackConfig::trace`] is enabled) [`Self::trace_ring`].
    fn record_executed_instruction(&mut self, pc: u16) {
        if self.config.trace {
            self.trace_ring.push(pc);
        }
        self.total_instructions += 1;
        match self.phase {
            Phase::FindDepacker => self.find_depacker_instructions += 1,
            Phase::Decompress => self.decompress_instructions += 1,
        }
    }

    /// Executes one iteration of [`Phase::FindDepacker`].
    fn step_find_depacker(&mut self) -> Step {
        // Invariant 4 (Global instruction budget): Checked at the top of each step
        // against the single `total_instructions` counter.
        if self.total_instructions >= self.config.max_instructions {
            return Step::Timeout(Phase::FindDepacker);
        }

        // Invariant 2 (Hook order): In `Phase::FindDepacker`, capture `pc` and update
        // `current_pc` BEFORE invoking `hooks.on_step`. The depacker-entry check below
        // evaluates the pre-hook `pc` (though `on_step` may set `in_phase2`).
        let pc = self.cpu.registers.program_counter;
        self.cpu.memory.current_pc = pc;

        if let Some(ref mut hooks) = self.hooks {
            let mut ctx = StepContext::from_cpu(&mut self.cpu, Phase::FindDepacker);
            hooks.on_step(&mut ctx);
        }

        // Invariant 1 (Bus phase flag): `C64Bus::set_byte` sets `in_phase2 = true` when
        // code executing below `ram_start` writes at or above `ram_start`. Read `in_phase2`
        // from the bus here alongside the PC threshold check.
        let is_dep_addr = self.cpu.memory.in_phase2
            || (if let Some(known_dep) = self.hints.as_ref().and_then(|h| h.dep_addr)
                && known_dep >= self.ret_addr
            {
                pc == known_dep
            } else {
                pc < self.ret_addr && pc != 0x0000 && pc != 0x0002
            });

        if is_dep_addr {
            // Transition does NOT increment `total_instructions`; `Phase::Decompress`
            // resumes at the same `pc` before this instruction executes.
            let dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
            return Step::ToPhase2 { dep_addr };
        }

        // ROM interception
        match handle_rom_entry(
            &mut self.cpu,
            self.traps,
            &mut self.getin_index,
            Phase::FindDepacker,
        ) {
            RomAction::Continue => {}
            RomAction::Handled => {
                // Invariant 4: ROM `Handled` steps increment `total_instructions` by 1
                // and skip `single_step` (and the progress callback).
                self.record_executed_instruction(pc);
                return Step::Continue;
            }
            RomAction::Exit => {
                return Step::Done(ExitReason::RomExitVector { pc });
            }
            RomAction::BasicRun => {
                return Step::Done(ExitReason::BasicRun);
            }
        }

        self.execute_instruction()
    }

    /// Executes one iteration of [`Phase::Decompress`].
    fn step_decompress(&mut self) -> Step {
        // Invariant 4 (Global instruction budget): Uses the same `total_instructions`
        // counter as `Phase::FindDepacker`, without resetting at the phase transition.
        if self.total_instructions >= self.config.max_instructions {
            return Step::Timeout(Phase::Decompress);
        }

        // Invariant 2 (Hook order): In `Phase::Decompress`, invoke `hooks.on_step`
        // BEFORE reading `pc = cpu.registers.program_counter`. This is deliberately reversed
        // relative to `Phase::FindDepacker`: `TSCrunch` clears `written[reloc..]` inside
        // `on_step` when PC enters `$0021..=$0090` or `$0121..=$0190`, which must happen
        // before the exit checks below inspect `written`.
        if let Some(ref mut hooks) = self.hooks {
            let mut ctx = StepContext::from_cpu(&mut self.cpu, Phase::Decompress);
            hooks.on_step(&mut ctx);
        }

        let pc = self.cpu.registers.program_counter;
        self.cpu.memory.current_pc = pc;

        let basic_mapped = is_basic_rom_mapped(&self.cpu.memory.mem, &self.system);
        let kernal_mapped = is_kernal_rom_mapped(&self.cpu.memory.mem, &self.system);
        let io_mapped = is_io_mapped(&self.cpu.memory.mem, &self.system);

        let in_rom_or_io = (is_in_basic_rom(pc, &self.system) && basic_mapped)
            || (self.system.is_in_io(pc) && io_mapped)
            || (is_in_kernal_rom(pc, &self.system) && kernal_mapped);

        // Invariant 3 (`known_entry` 10-instruction guard) and `exact_return` (unp64 `RtAFrc`):
        // - A packer's `known_entry` triggers a `Decompress` exit only when the global
        //   `total_instructions` counter exceeds `MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY` (10),
        //   preventing premature exit during initial stub setup.
        // - When `exact_return` is `false` (default), reaching any written RAM code at
        //   `pc >= self.ret_addr` (`is_written_code`) or jumping outside the loaded region
        //   (`LeftLoadedRegion`, when `known_entry.is_none()`) ends decompression.
        // - When `exact_return` is `true`, intermediate written RAM addresses are ignored and
        //   decompression exits on RAM code only at the exact target return address
        //   (`known_entry.unwrap_or(self.ret_addr)`).
        let known_entry = self.hints.as_ref().and_then(|h| h.entry_point);
        let exact_return = self.raw_hints.is_some_and(|h| h.exact_return);
        let exact_target = known_entry.unwrap_or(self.ret_addr);
        let is_written_code = (if exact_return {
            pc == exact_target
        } else {
            pc >= self.ret_addr
        }) && usize::from(pc) < self.system.memory_boundaries()[0]
            && !in_rom_or_io
            && (self.cpu.memory.written[usize::from(pc)]
                || self.cpu.memory.written_phase2[usize::from(pc)]);

        if (known_entry == Some(pc) || (exact_return && pc == exact_target))
            && self.total_instructions > MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY
        {
            return Step::Done(ExitReason::KnownEntryReached { pc });
        }
        if is_written_code {
            return Step::Done(ExitReason::WrittenCodeReached { pc });
        }

        // If the packer doesn't have a known entry point (and does not require an exact
        // return address), exit when PC jumps outside the original loaded data region
        // (and at/above `ram_start`) to a written address.
        // Note: checks `written` only (not `written_phase2`), preserving TSCrunch relocation clearing.
        if known_entry.is_none()
            && !exact_return
            && pc >= self.system.ram_start()
            && (pc < self.load_addr || pc >= self.load_end)
            && self
                .cpu
                .memory
                .written
                .get(usize::from(pc))
                .copied()
                .unwrap_or(false)
        {
            return Step::Done(ExitReason::LeftLoadedRegion { pc });
        }

        // ROM interception
        match handle_rom_entry(
            &mut self.cpu,
            self.traps,
            &mut self.getin_index,
            Phase::Decompress,
        ) {
            RomAction::Continue => {}
            RomAction::Handled => {
                // Invariant 4: ROM `Handled` steps increment `total_instructions` by 1
                // and skip `single_step` (and the progress callback).
                self.record_executed_instruction(pc);
                return Step::Continue;
            }
            RomAction::Exit => {
                return Step::Done(ExitReason::RomExitVector { pc });
            }
            RomAction::BasicRun => {
                return Step::Done(ExitReason::BasicRun);
            }
        }

        self.execute_instruction()
    }

    /// Executes an illegal opcode or a standard CPU step, updating `total_instructions`
    /// and firing the progress callback at multiples of [`PROGRESS_CALLBACK_INTERVAL`].
    fn execute_instruction(&mut self) -> Step {
        let pc = self.cpu.registers.program_counter;
        // Invariant 4: Emulated illegal opcodes count as 1 instruction and skip
        // `single_step` (and the progress callback).
        if emulate_illegal_opcode(&mut self.cpu) {
            self.record_executed_instruction(pc);
            return Step::Continue;
        }

        self.cpu.single_step();
        self.record_executed_instruction(pc);
        if self
            .total_instructions
            .is_multiple_of(PROGRESS_CALLBACK_INTERVAL)
            && let Some(cb) = self.progress_callback
        {
            cb(self.total_instructions);
        }
        Step::Continue
    }

    /// Resolves `(entry_point, dep_addr)` from `reason` and the active [`Phase`].
    #[cfg(test)]
    fn resolve_exit(&self, reason: ExitReason) -> (u16, u16) {
        let (entry_point, _, dep_addr) = self.resolve_exit_with_source(reason);
        (entry_point, dep_addr)
    }

    /// Resolves `(entry_point, entry_source, dep_addr)` from `reason` and the active [`Phase`].
    fn resolve_exit_with_source(&self, reason: ExitReason) -> (u16, BoundSource, u16) {
        let mem = &self.cpu.memory.mem;
        let default_source = if self.entry_was_forced {
            BoundSource::Forced
        } else {
            BoundSource::ExitPc
        };
        match (self.phase, reason) {
            (Phase::FindDepacker, ExitReason::RomExitVector { pc }) => {
                let dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
                (pc, default_source, dep_addr)
            }
            (Phase::FindDepacker, ExitReason::BasicRun) => {
                let pc = self.cpu.memory.current_pc;
                // Subtle phase difference: in `FindDepacker`, `BasicRun` falls back to
                // `basic_start` when no `SYS` line is found, whereas in `Decompress`
                // `Exit | BasicRun` falls back to `pc`.
                let (entry_point, entry_source) = match find_sys_address(mem, self.basic_start) {
                    Some(sys_ep) => (sys_ep, BoundSource::SysLineLookup),
                    None => (self.basic_start, default_source),
                };
                let dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
                (entry_point, entry_source, dep_addr)
            }
            (
                Phase::Decompress,
                ExitReason::KnownEntryReached { pc } | ExitReason::WrittenCodeReached { pc },
            ) => {
                let (entry_point, entry_source) =
                    if self.system.is_basic_exec_entry(pc) || pc == self.basic_start {
                        match find_sys_address(mem, self.basic_start) {
                            Some(sys_ep) => (sys_ep, BoundSource::SysLineLookup),
                            None => (pc, default_source),
                        }
                    } else if pc == self.ret_addr {
                        match find_entry_in_snapshot(
                            &self.snapshot,
                            self.load_addr,
                            self.data_len,
                            self.ret_addr,
                        ) {
                            Some(snap_ep) => (snap_ep, BoundSource::SnapshotEntry),
                            None => (pc, default_source),
                        }
                    } else {
                        (pc, default_source)
                    };
                (entry_point, entry_source, self.dep_addr)
            }
            (Phase::Decompress, ExitReason::LeftLoadedRegion { pc }) => {
                (pc, default_source, self.dep_addr)
            }
            (Phase::Decompress, ExitReason::RomExitVector { .. } | ExitReason::BasicRun) => {
                let pc = match reason {
                    ExitReason::RomExitVector { pc } => pc,
                    _ => self.cpu.memory.current_pc,
                };
                // Subtle phase difference: in `Decompress`, `Exit | BasicRun` looks up the
                // `SYS` line only when `pc` is inside BASIC ROM and falls back to `pc`.
                let (entry_point, entry_source) = if is_in_basic_rom(pc, &self.system) {
                    match find_sys_address(mem, self.basic_start) {
                        Some(sys_ep) => (sys_ep, BoundSource::SysLineLookup),
                        None => (pc, default_source),
                    }
                } else {
                    (pc, default_source)
                };
                (entry_point, entry_source, self.dep_addr)
            }
            (
                Phase::FindDepacker,
                ExitReason::KnownEntryReached { pc }
                | ExitReason::WrittenCodeReached { pc }
                | ExitReason::LeftLoadedRegion { pc },
            ) => (
                pc,
                default_source,
                self.config.forced_dep_addr.unwrap_or(pc),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// System & zero-page initialization helpers
// ---------------------------------------------------------------------------

fn is_in_basic_rom(pc: u16, system: &System) -> bool {
    system.is_in_basic_rom(pc)
}

fn is_in_kernal_rom(pc: u16, system: &System) -> bool {
    system.is_in_kernal_rom(pc)
}

fn init_zero_page(mem: &mut C64Bus, load_addr: u16, data_len: u16, basic_start: u16) {
    let end_addr = load_addr.wrapping_add(data_len);
    let system = &mem.system;

    if system.is_c64() {
        mem.mem[0..256].copy_from_slice(&C64_ZEROPAGE_TEMPLATE);
    } else if *system == crate::state::types::TargetSystem::C128 {
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
            mem.mem[addr as usize] = SCREEN_CODE_SPACE;
        }
    }
}

// ---------------------------------------------------------------------------
// ROM interception
// ---------------------------------------------------------------------------

/// Intercepts execution at a ROM address, using the machine's trap table.
///
/// The order of the checks is part of the engine's behavior:
///
/// 1. Outside BASIC and KERNAL ROM space, the code runs (`Continue`).
/// 2. A BASIC RUN entry wins before the `written` and banking checks:
///    packers jump to it (C64: `$A7AE`) even with RAM banked in.
/// 3. Code written under an unmapped ROM runs as RAM.
/// 4. BASIC ROM: unmapped runs as RAM; a decompress-stage BASIC exit ends
///    emulation; anything else gets a forced `RTS`.
/// 5. KERNAL ROM: unmapped runs as RAM; an exit vector ends emulation; a
///    trapped routine is emulated and returns; anything else gets a forced
///    `RTS`.
fn handle_rom_entry(
    cpu: &mut CPU<C64Bus, Nmos6502>,
    traps: &RomTrapTable,
    getin_index: &mut usize,
    phase: Phase,
) -> RomAction {
    let pc = cpu.registers.program_counter;
    let system = &cpu.memory.system;

    let in_basic = is_in_basic_rom(pc, system);
    let in_kernal = is_in_kernal_rom(pc, system);

    // Not in ROM space — fast path for standard RAM code execution
    if !in_basic && !in_kernal {
        return RomAction::Continue;
    }

    // BASIC RUN detection (check even if written, as packers jump to BASIC RUN)
    if traps.is_basic_run_entry(pc) {
        return RomAction::BasicRun;
    }

    let basic_mapped = is_basic_rom_mapped(&cpu.memory.mem, system);
    let kernal_mapped = is_kernal_rom_mapped(&cpu.memory.mem, system);

    // If user code was written here (depacker at $FF00+, etc.) AND the ROM
    // at this address is not currently mapped, let it run as RAM code.
    let rom_mapped_here = (in_basic && basic_mapped) || (in_kernal && kernal_mapped);
    if cpu.memory.written[pc as usize] && !rom_mapped_here {
        return RomAction::Continue;
    }

    // The BASIC and KERNAL ROM ranges never overlap on any system, so
    // everything past this branch is a KERNAL address.
    if in_basic {
        return handle_basic_rom_entry(cpu, traps, basic_mapped, phase);
    }
    handle_kernal_rom_entry(cpu, traps, kernal_mapped, getin_index, phase)
}

/// The BASIC ROM part of [`handle_rom_entry`].
fn handle_basic_rom_entry(
    cpu: &mut CPU<C64Bus, Nmos6502>,
    traps: &RomTrapTable,
    basic_mapped: bool,
    phase: Phase,
) -> RomAction {
    if !basic_mapped {
        return RomAction::Continue; // RAM is visible, not ROM
    }
    if traps.is_basic_exit(cpu.registers.program_counter, phase) {
        return RomAction::Exit;
    }
    // Fallback: force RTS
    force_rts(cpu);
    RomAction::Handled
}

/// The KERNAL ROM part of [`handle_rom_entry`].
fn handle_kernal_rom_entry(
    cpu: &mut CPU<C64Bus, Nmos6502>,
    traps: &RomTrapTable,
    kernal_mapped: bool,
    getin_index: &mut usize,
    phase: Phase,
) -> RomAction {
    if !kernal_mapped {
        return RomAction::Continue; // RAM is visible
    }
    match traps.kernal_trap(cpu.registers.program_counter, phase) {
        Some(KernalTrap::Exit) => RomAction::Exit,
        Some(KernalTrap::Routine(routine)) => {
            routine.emulate(cpu, traps, getin_index);
            force_rts(cpu);
            RomAction::Handled
        }
        // Fallback: force RTS
        None => {
            force_rts(cpu);
            RomAction::Handled
        }
    }
}

fn force_rts(cpu: &mut CPU<C64Bus, Nmos6502>) {
    let sp = cpu.registers.stack_pointer.0;
    let low = cpu.memory.mem[0x0100 | usize::from(sp.wrapping_add(1))];
    let high = cpu.memory.mem[0x0100 | usize::from(sp.wrapping_add(2))];
    cpu.registers.stack_pointer.0 = sp.wrapping_add(2);
    cpu.registers.program_counter = u16::from_le_bytes([low, high]).wrapping_add(1);
}

fn emulate_illegal_opcode(cpu: &mut CPU<C64Bus, Nmos6502>) -> bool {
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
            cpu.memory.step_cycles(2);
            true
        }
        0x9E => {
            let addr_low = cpu.memory.mem[pc.wrapping_add(1) as usize];
            let addr_high = cpu.memory.mem[pc.wrapping_add(2) as usize];
            let val = cpu.registers.index_x & addr_high.wrapping_add(1);
            let target_addr = u16::from_le_bytes([addr_low, addr_high])
                .wrapping_add(cpu.registers.index_y as u16);
            cpu.memory.set_byte(target_addr, val);
            cpu.memory.step_cycles(4);
            cpu.registers.program_counter = pc.wrapping_add(3);
            true
        }
        0x9C => {
            let addr_low = cpu.memory.mem[pc.wrapping_add(1) as usize];
            let addr_high = cpu.memory.mem[pc.wrapping_add(2) as usize];
            let val = cpu.registers.index_y & addr_high.wrapping_add(1);
            let target_addr = u16::from_le_bytes([addr_low, addr_high])
                .wrapping_add(cpu.registers.index_x as u16);
            cpu.memory.set_byte(target_addr, val);
            cpu.memory.step_cycles(4);
            cpu.registers.program_counter = pc.wrapping_add(3);
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Post-emulation range & entry finalization (`finish`)
// ---------------------------------------------------------------------------

/// Inputs required by [`finish`] to compute the final [`UnpackResult`].
struct FinishContext<'a> {
    /// Final 64 KB emulated RAM image.
    mem: &'a [u8],
    /// Pre-emulation 64 KB RAM snapshot (captured after zero-page initialization).
    snapshot: &'a [u8],
    /// Per-byte write tracking across both emulation phases (`written`, not `written_phase2`).
    written: &'a [bool],
    /// Initial entry point resolved from the exit condition.
    entry_point: u16,
    /// Provenance of the initial `entry_point` resolved from the exit condition.
    entry_source: BoundSource,
    /// Depacker address resolved at the Phase 1 → Phase 2 transition (or Phase 1 exit).
    dep_addr: u16,
    /// Return address boundary (`min(load_addr, system.ram_start())` unless forced).
    ret_addr: u16,
    /// Total instructions executed across both phases.
    instructions_executed: u64,
    /// Instructions executed in [`Phase::FindDepacker`].
    find_depacker_instructions: u64,
    /// Instructions executed in [`Phase::Decompress`].
    decompress_instructions: u64,
    /// Exit condition that ended emulation.
    exit_reason: ExitReason,
    /// Last [`TRACE_TAIL_CAPACITY`] executed PCs when [`UnpackConfig::trace`] is `true`.
    trace_tail: Vec<u16>,
    /// Stable identifier of the matched [`PackerSpec`], if any.
    packer_id: Option<PackerId>,
    /// Effective BASIC start address for the loaded binary (renamed from the misleading
    /// `load_addr` parameter of `finish_unpack`; `T2.3.15`).
    basic_start: u16,
    /// Optional lifecycle hooks from the detected [`PackerSpec`].
    hooks: Option<&'a dyn PackerHooks>,
    /// Packer metadata hints resolved once at detection time, if any.
    hints: Option<&'a PackerInfo>,
    /// Declarative [`Hints`] from the matched [`PackerSpec`], if any.
    raw_hints: Option<&'a Hints>,
    /// Target machine system profile.
    system: &'a System,
    /// CPU Y index register at the moment emulation exited.
    y_reg: u8,
}

/// Finalizes an unpack run by detecting the output address range and applying
/// packer overrides, post-emulation hooks, and entry/range clamps in a fixed order.
///
/// # Invariant 5 (`finish` override order)
///
/// The bounds and entry point are resolved in this exact 10-step sequence:
/// 1. [`detect_output_range`] over `mem`, `snapshot`, and `written` ([`UnpackError::NothingWritten`] if `None`).
/// 2. `hints.start_addr` override.
/// 3. `hints.end_addr` override.
/// 4. `hints.end_addr_ptr` override, guarded against the *already overridden* `end_addr`
///    (`reported_end > start_addr && reported_end.saturating_sub(1) <= end_addr.saturating_add(END_PTR_SLACK)`)
///    using `ea_ptr.wrapping_add(1)` for the high byte (Ruling R19, `T2.3.9`).
/// 5. `hints.entry_point` override.
/// 6. `hints.dep_addr` override.
/// 7. Post-emulation refinement (`PointerGuard::McCrackenEndOfProgram` + `PackerHooks::refine`).
/// 8. BASIC `SYS` line clamp (`find_sys_address(mem, basic_start)`): replaces `entry_point`
///    when `is_rom_entry || entry_point == basic_start`, and lowers `start_addr` to
///    `basic_start` when `start_addr > basic_start`.
/// 9. Entry clamps: lowers `start_addr` to `entry_point` if `entry_point < start_addr` and
///    `entry_point >= MIN_VALID_ENTRY_ADDR` (`$0200`), and raises `end_addr` to `entry_point`
///    if `entry_point > end_addr`.
/// 10. Range validation: returns [`UnpackError::InvalidAddressRange`] if `entry_point` lies
///     outside `start_addr..=end_addr`.
fn finish(ctx: FinishContext<'_>) -> Result<UnpackResult, UnpackError> {
    let FinishContext {
        mem,
        snapshot,
        written,
        mut entry_point,
        mut entry_source,
        mut dep_addr,
        ret_addr,
        instructions_executed,
        find_depacker_instructions,
        decompress_instructions,
        exit_reason,
        trace_tail,
        packer_id,
        basic_start,
        hooks,
        hints,
        raw_hints,
        system,
        y_reg,
    } = ctx;

    // 1. Detect initial output range from memory write tracking and pre-emulation snapshot.
    let (mut start_addr, mut end_addr) =
        detect_output_range(mem, snapshot, written, ret_addr, basic_start, system)
            .ok_or(UnpackError::NothingWritten)?;
    let mut start_source = BoundSource::WriteDiff;
    let mut end_source = BoundSource::WriteDiff;
    let spec_id = packer_id
        .or_else(|| hints.map(|info| info.name))
        .unwrap_or("");

    if let Some(info) = hints {
        // 2. Packer start_addr hint override.
        if let Some(sa) = info.start_addr {
            start_addr = sa;
            start_source = BoundSource::PackerSpec { id: spec_id };
        }
        // 3. Packer end_addr hint override.
        if let Some(ea) = info.end_addr {
            end_addr = ea;
            end_source = BoundSource::PackerSpec { id: spec_id };
        }
        // 4. Packer end_addr_ptr override, guarded against the already overridden `end_addr`.
        // Uses `wrapping_add(1)` so a pointer at `$FFFF` wraps its high byte to `$0000`
        // without panicking in debug builds (Ruling R19).
        if let Some(ea_ptr) = info.end_addr_ptr {
            let lo = mem[usize::from(ea_ptr)];
            let hi = mem[usize::from(ea_ptr.wrapping_add(1))];
            let reported_end = u16::from_le_bytes([lo, hi]);
            if reported_end > start_addr
                && reported_end.saturating_sub(1) <= end_addr.saturating_add(END_PTR_SLACK)
            {
                end_addr = reported_end.saturating_sub(1);
                end_source = BoundSource::PointerAfterRun { addr: ea_ptr };
            }
        }
        // 5. Packer entry_point hint override.
        if let Some(ep) = info.entry_point {
            entry_point = ep;
            entry_source = BoundSource::PackerSpec { id: spec_id };
        }
        // 6. Packer dep_addr hint override.
        if let Some(da) = info.dep_addr {
            dep_addr = da;
        }
    }

    // 7. Post-emulation refinement (`PointerGuard::McCrackenEndOfProgram` + `PackerHooks::refine`).
    if system.is_c64()
        && let Some(raw_hints) = raw_hints
        && let Some(
            src @ AddrSource::PointerAfterRun {
                ptr_addr,
                guard: PointerGuard::McCrackenEndOfProgram,
                ..
            },
        ) = raw_hints.end_addr
        && let Some(resolved_end) = src.resolve_post_run(
            mem,
            Registers {
                y: y_reg,
                ..Registers::default()
            },
            start_addr,
            end_addr,
            &PointerMonitor::new(),
        )
    {
        end_addr = resolved_end;
        end_source = BoundSource::PointerAfterRun { addr: ptr_addr };
    }
    if let Some(h) = hooks {
        let refine_ctx = RefineContext {
            mem,
            snapshot,
            written,
            system,
            registers: Registers {
                y: y_reg,
                ..Registers::default()
            },
        };
        let mut bounds = Bounds {
            start_addr,
            end_addr,
            entry_point,
        };
        h.refine(&refine_ctx, &mut bounds);
        // Ruling R6: only update the bound's source to `PackerRefine` when `refine`
        // actually changed that bound; unchanged bounds keep their previous source.
        if bounds.start_addr != start_addr {
            start_addr = bounds.start_addr;
            start_source = BoundSource::PackerRefine { id: spec_id };
        }
        if bounds.end_addr != end_addr {
            end_addr = bounds.end_addr;
            end_source = BoundSource::PackerRefine { id: spec_id };
        }
        if bounds.entry_point != entry_point {
            entry_point = bounds.entry_point;
            entry_source = BoundSource::PackerRefine { id: spec_id };
        }
    }

    // 8. BASIC `SYS` line clamp.
    let basic_mapped = is_basic_rom_mapped(mem, system);
    let kernal_mapped = is_kernal_rom_mapped(mem, system);
    let is_rom_entry = (is_in_basic_rom(entry_point, system) && basic_mapped)
        || (is_in_kernal_rom(entry_point, system) && kernal_mapped);

    if let Some(sys_ep) = find_sys_address(mem, basic_start) {
        if is_rom_entry || entry_point == basic_start {
            entry_point = sys_ep;
            entry_source = BoundSource::SysLineLookup;
        }
        if start_addr > basic_start {
            start_addr = basic_start;
            start_source = BoundSource::SysLineClamp;
        }
    }

    // 9. Clamp `start_addr` down to `entry_point` (if >= $0200) and `end_addr` up to `entry_point`.
    if entry_point < start_addr && entry_point >= MIN_VALID_ENTRY_ADDR {
        start_addr = entry_point;
        start_source = BoundSource::EntryClamp;
    }
    if entry_point > end_addr && usize::from(entry_point) < mem.len() {
        end_addr = entry_point;
        end_source = BoundSource::EntryClamp;
    }

    // 10. Validate `entry_point` is within `start_addr..=end_addr`.
    if entry_point < start_addr || entry_point > end_addr {
        return Err(UnpackError::InvalidAddressRange {
            start_addr,
            end_addr,
            entry_point,
        });
    }

    let data = mem[usize::from(start_addr)..=usize::from(end_addr)].to_vec();
    let packer = packer_id.or_else(|| hints.map(|info| info.name));
    let packer_name = hints.map(|info| info.name.to_string());
    let report = UnpackReport {
        exit_reason,
        find_depacker_instructions,
        decompress_instructions,
        start_source,
        end_source,
        entry_source,
        trace_tail,
    };

    Ok(UnpackResult {
        data,
        start_addr,
        end_addr,
        entry_point,
        dep_addr,
        instructions_executed,
        packer,
        packer_name,
        report,
    })
}

// ===========================================================================
// Tests
// ===========================================================================
#[cfg(test)]
mod tests {
    //! Characterization tests (Test Plan T2.0.9–T2.0.16, Orchestrator ruling
    //! R18).
    //!
    //! They were written and green against the engine *before* the ROM traps
    //! moved into the machine profile, and pin the engine's observable
    //! behavior so that move is provably behavior-preserving on every
    //! `TargetSystem`, not only on the C64 files the corpus baseline covers.
    //! The single adaptation point to the trap API is `invoke_trap`.

    use super::*;
    use crate::state::types::TargetSystem;
    use std::cell::RefCell;

    // -----------------------------------------------------------------------
    // Synthetic programs
    // -----------------------------------------------------------------------

    /// Load address of the synthetic BASIC programs (`10 SYS2061`).
    const PRG_LOAD: u16 = 0x0801;
    /// Target of the `SYS2061` line: the first byte after the BASIC stub.
    const SYS_ENTRY: u16 = 0x080D;
    /// Below-RAM address where stage 1 installs the phase-2 routine. Jumping
    /// here (below `ret_addr = $0800`) ends phase 1.
    const STAGE2_ADDR: u16 = 0x0340;
    /// Address the phase-2 routine writes to and then jumps to.
    const OUTPUT_ADDR: u16 = 0x2000;
    /// Budget for programs that finish within a few hundred thousand steps.
    const SMALL_BUDGET: u64 = 1_000_000;
    /// Budget for programs that are expected to time out.
    const TIMEOUT_BUDGET: u64 = 10_000;
    /// Instructions of [`stage1_installing`] for an 8-byte routine:
    /// 8 × (`LDA #imm`, `STA abs`) + `JMP`.
    const STAGE1_INSTRUCTIONS: u64 = 17;
    /// Instructions of [`store_and_jump`]: `LDA`, `STA`, `JMP`.
    const STAGE2_INSTRUCTIONS: u64 = 3;
    /// Instructions of [`unpacker_program`] without a prefix.
    const BASE_INSTRUCTIONS: u64 = STAGE1_INSTRUCTIONS + STAGE2_INSTRUCTIONS;

    /// `10 SYS2061` followed by `code` at [`SYS_ENTRY`], as raw PRG data
    /// (without the load address) for a load at [`PRG_LOAD`].
    fn sys_program(code: &[u8]) -> Vec<u8> {
        let mut raw = vec![
            0x0B, 0x08, // link to the end-of-program marker at $080B
            0x0A, 0x00, // line 10
            0x9E, b'2', b'0', b'6', b'1', // SYS2061
            0x00, // end of line
            0x00, 0x00, // end of program
        ];
        raw.extend_from_slice(code);
        raw
    }

    /// Position-independent stage 1: stores `routine` at [`STAGE2_ADDR`]
    /// with one `LDA #imm` / `STA abs` pair per byte, then jumps to it.
    fn stage1_installing(routine: &[u8]) -> Vec<u8> {
        let mut code = Vec::new();
        for (offset, &byte) in (0u16..).zip(routine) {
            let [lo, hi] = (STAGE2_ADDR + offset).to_le_bytes();
            code.extend_from_slice(&[0xA9, byte, 0x8D, lo, hi]);
        }
        let [lo, hi] = STAGE2_ADDR.to_le_bytes();
        code.extend_from_slice(&[0x4C, lo, hi]);
        code
    }

    /// Phase-2 routine: `LDA #$EA; STA target; JMP target`. Jumping to the
    /// freshly written `NOP` is the "written code reached" exit.
    fn store_and_jump(target: u16) -> Vec<u8> {
        let [lo, hi] = target.to_le_bytes();
        vec![0xA9, 0xEA, 0x8D, lo, hi, 0x4C, lo, hi]
    }

    /// `LDA #$01; STA OUTPUT_ADDR; JMP target`: writes the output byte, then
    /// jumps to `target` (a ROM address in the tests).
    fn store_and_jump_elsewhere(target: u16) -> Vec<u8> {
        let [out_lo, out_hi] = OUTPUT_ADDR.to_le_bytes();
        let [lo, hi] = target.to_le_bytes();
        vec![0xA9, 0x01, 0x8D, out_lo, out_hi, 0x4C, lo, hi]
    }

    /// A complete two-stage "unpacker": `prefix`, then stage 1 installing
    /// [`store_and_jump`]`(OUTPUT_ADDR)`. `prefix` must be position
    /// independent; it shifts stage 1 but not its behavior.
    fn unpacker_program(prefix: &[u8]) -> Vec<u8> {
        let mut code = prefix.to_vec();
        code.extend(stage1_installing(&store_and_jump(OUTPUT_ADDR)));
        sys_program(&code)
    }

    fn budget(max_instructions: u64) -> UnpackConfig {
        UnpackConfig {
            max_instructions,
            ..UnpackConfig::default()
        }
    }

    fn run(raw: &[u8], load_addr: u16, config: &UnpackConfig) -> Result<UnpackResult, UnpackError> {
        UnpackEngine::new(config, None).run(raw, load_addr)
    }

    /// `(start, end, entry, dep, instructions)` of a successful run.
    fn summary(result: &UnpackResult) -> (u16, u16, u16, u16, u64) {
        (
            result.start_addr,
            result.end_addr,
            result.entry_point,
            result.dep_addr,
            result.instructions_executed,
        )
    }

    /// The reference run that the other engine tests are measured against.
    #[test]
    fn engine_reference_program_outputs() {
        let result = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(
            summary(&result),
            (
                PRG_LOAD,
                OUTPUT_ADDR,
                OUTPUT_ADDR,
                STAGE2_ADDR,
                BASE_INSTRUCTIONS
            )
        );
        assert_eq!(result.packer_name, None);
    }

    /// T2.0.9 (Invariant 4): a ROM-trapped step counts as one instruction.
    /// `JSR $FFD2` adds the `JSR` plus the trapped CHROUT step.
    #[test]
    fn engine_rom_trapped_steps_count_as_instructions() {
        let base = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        // A is $00 after reset, so CHROUT does not take the clear-screen path.
        let with_chrout = run(
            &unpacker_program(&[0x20, 0xD2, 0xFF]),
            PRG_LOAD,
            &budget(SMALL_BUDGET),
        )
        .unwrap();
        assert_eq!(
            with_chrout.instructions_executed,
            base.instructions_executed + 2
        );
        assert_eq!(
            (with_chrout.entry_point, with_chrout.dep_addr),
            (base.entry_point, base.dep_addr)
        );
    }

    /// T2.0.10: an emulated illegal opcode (`$AB`, LAX #imm) counts as one
    /// instruction.
    #[test]
    fn engine_illegal_opcode_steps_count_as_instructions() {
        let base = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        let with_illegal = run(
            &unpacker_program(&[0xAB, 0x00]),
            PRG_LOAD,
            &budget(SMALL_BUDGET),
        )
        .unwrap();
        assert_eq!(
            with_illegal.instructions_executed,
            base.instructions_executed + 1
        );
        assert_eq!(
            (with_illegal.entry_point, with_illegal.dep_addr),
            (base.entry_point, base.dep_addr)
        );
    }

    /// T2.0.11: the progress callback fires after a CPU step whenever the
    /// global count is a multiple of 30 000.
    #[test]
    fn engine_progress_callback_cadence() {
        // LDX #0; LDY #0; inner: DEX; BNE inner; DEY; BNE inner
        // = 2 + 256 × (256 × 2 + 2) = 131 586 instructions.
        const DELAY_LOOP: [u8; 10] = [0xA2, 0x00, 0xA0, 0x00, 0xCA, 0xD0, 0xFD, 0x88, 0xD0, 0xFA];
        const DELAY_INSTRUCTIONS: u64 = 131_586;

        let calls = RefCell::new(Vec::new());
        let record = |count: u64| calls.borrow_mut().push(count);
        let config = budget(SMALL_BUDGET);
        let result = UnpackEngine::new(&config, Some(&record))
            .run(&unpacker_program(&DELAY_LOOP), PRG_LOAD)
            .unwrap();

        assert_eq!(
            result.instructions_executed,
            DELAY_INSTRUCTIONS + BASE_INSTRUCTIONS
        );
        assert_eq!(*calls.borrow(), vec![30_000, 60_000, 90_000, 120_000]);
    }

    /// T2.0.12: a program that never leaves stage 1 times out in phase 1.
    #[test]
    fn engine_jmp_self_returns_phase1_timeout() {
        let [lo, hi] = SYS_ENTRY.to_le_bytes();
        let raw = sys_program(&[0x4C, lo, hi]);
        assert_eq!(
            run(&raw, PRG_LOAD, &budget(TIMEOUT_BUDGET)).unwrap_err(),
            UnpackError::Phase1Timeout
        );
    }

    /// T2.0.13: one instruction counter spans both stages. A budget that only
    /// covers stage 1 plus one step times out in phase 2 right away.
    #[test]
    fn engine_budget_is_global_across_stages() {
        let raw = unpacker_program(&[]);
        let cases = [
            (STAGE1_INSTRUCTIONS, Err(UnpackError::Phase1Timeout)),
            (STAGE1_INSTRUCTIONS + 1, Err(UnpackError::Phase2Timeout)),
            (BASE_INSTRUCTIONS, Err(UnpackError::Phase2Timeout)),
            (BASE_INSTRUCTIONS + 1, Ok(BASE_INSTRUCTIONS)),
        ];
        for (max_instructions, expected) in cases {
            let actual = run(&raw, PRG_LOAD, &budget(max_instructions))
                .map(|result| result.instructions_executed);
            assert_eq!(actual, expected, "budget {max_instructions}");
        }
    }

    /// T2.0.14: the two early errors.
    #[test]
    fn engine_empty_data_and_no_entry_errors() {
        let config = budget(TIMEOUT_BUDGET);
        assert_eq!(
            run(&[], PRG_LOAD, &config).unwrap_err(),
            UnpackError::EmptyData
        );
        // A zero link pointer is an empty BASIC program: no SYS line.
        assert_eq!(
            run(&[0x00; 16], PRG_LOAD, &config).unwrap_err(),
            UnpackError::NoEntryPoint
        );
    }

    /// T2.0.15: for a file that reaches `$FFFF`, the phase-2 `load_end` wraps
    /// to `$0000`, so *every* written address at or above RAM start counts as
    /// "outside the loaded region". The same program one byte shorter keeps
    /// running inside its region and times out.
    #[test]
    fn engine_load_end_wraps_to_zero_quirk() {
        const HIGH_LOAD: u16 = 0xC000;
        const HIGH_TARGET: u16 = 0xC100;

        let high_program = |len: usize| {
            let mut raw = vec![0u8; len];
            let stage1 = stage1_installing(&store_and_jump(HIGH_TARGET));
            raw[..stage1.len()].copy_from_slice(&stage1);
            // After the written NOP at HIGH_TARGET: JMP to itself (unwritten).
            let self_loop = usize::from(HIGH_TARGET - HIGH_LOAD) + 1;
            let [lo, hi] = (HIGH_TARGET + 1).to_le_bytes();
            raw[self_loop..self_loop + 3].copy_from_slice(&[0x4C, lo, hi]);
            raw
        };
        let config = UnpackConfig {
            forced_entry: Some(HIGH_LOAD),
            ..budget(TIMEOUT_BUDGET)
        };
        let to_top = 0x1_0000 - usize::from(HIGH_LOAD);

        let reaching_ffff = run(&high_program(to_top), HIGH_LOAD, &config).unwrap();
        assert_eq!(reaching_ffff.entry_point, HIGH_TARGET);
        assert_eq!(reaching_ffff.dep_addr, STAGE2_ADDR);

        assert_eq!(
            run(&high_program(to_top - 1), HIGH_LOAD, &config).unwrap_err(),
            UnpackError::Phase2Timeout
        );
    }

    /// T2.0.16: the engine's handling of a ROM exit vector and a BASIC RUN
    /// trap in each stage. ROM entries are replaced by the SYS address.
    #[test]
    fn engine_rom_exit_vector_and_basic_run_paths() {
        // `LDA`, `STA`, `JMP`; the trapped step that exits is not counted.
        const PHASE1_INSTRUCTIONS: u64 = 3;
        // Stage 1 itself jumps to the ROM address.
        let phase1_jump = |target: u16| sys_program(&store_and_jump_elsewhere(target));
        // The phase-2 routine jumps to the ROM address.
        let phase2_jump =
            |target: u16| sys_program(&stage1_installing(&store_and_jump_elsewhere(target)));

        let cases = [
            (
                "phase 1 LOAD",
                phase1_jump(0xFFD5),
                0xFFD5,
                PHASE1_INSTRUCTIONS,
            ),
            (
                "phase 1 BASIC RUN",
                phase1_jump(0xA7AE),
                0xA7AE,
                PHASE1_INSTRUCTIONS,
            ),
            (
                "phase 2 warm start",
                phase2_jump(0xFCE2),
                STAGE2_ADDR,
                BASE_INSTRUCTIONS,
            ),
            (
                "phase 2 BASIC RUN",
                phase2_jump(0xA7AE),
                STAGE2_ADDR,
                BASE_INSTRUCTIONS,
            ),
            (
                "phase 2 BASIC exit $A660",
                phase2_jump(0xA660),
                STAGE2_ADDR,
                BASE_INSTRUCTIONS,
            ),
            (
                "phase 2 BASIC exit $A57C",
                phase2_jump(0xA57C),
                STAGE2_ADDR,
                BASE_INSTRUCTIONS,
            ),
            (
                "phase 2 IRQ handler",
                phase2_jump(0xEA31),
                STAGE2_ADDR,
                BASE_INSTRUCTIONS,
            ),
        ];
        for (label, raw, dep_addr, instructions) in cases {
            let result = run(&raw, PRG_LOAD, &budget(SMALL_BUDGET))
                .unwrap_or_else(|e| panic!("{label}: {e}"));
            assert_eq!(
                summary(&result),
                (PRG_LOAD, OUTPUT_ADDR, SYS_ENTRY, dep_addr, instructions),
                "{label}"
            );
        }
    }

    /// R18.3: on every system the engine looks for the SYS line at
    /// `load + 1` for loads at `$0800` and `$07C0`, and at the load address
    /// otherwise. With a zero budget a found entry surfaces as
    /// `Phase1Timeout`, a missing one as `NoEntryPoint`.
    ///
    /// The negative control is only observable for the padded loads: for a
    /// line one byte *past* a non-padded load, the SYS parser's lenient
    /// fallback (`parser::basic::find_sys_address`) finds it anyway.
    #[test]
    fn engine_basic_start_is_system_independent() {
        // (load address, BASIC start the engine uses)
        const LOADS: [(u16, u16); 5] = [
            (0x07C0, 0x07C1),
            (0x0800, 0x0801),
            (0x0801, 0x0801),
            (0x1000, 0x1000),
            (0x1C00, 0x1C00),
        ];
        // A one-line BASIC program placed at `line_addr` for a load at `load`.
        let program_with_line_at = |load: u16, line_addr: u16| {
            let mut raw = vec![0u8; usize::from(line_addr - load)];
            let [lo, hi] = (line_addr + 10).to_le_bytes();
            raw.extend_from_slice(&[lo, hi]);
            raw.extend_from_slice(&sys_program(&[])[2..]);
            raw
        };

        let mut failures = Vec::new();
        for system in all_systems() {
            let config = UnpackConfig {
                target_system: Some(system.clone()),
                ..budget(0)
            };
            for (load, basic_start) in LOADS {
                let footprint = load..=basic_start + 12;
                // The engine fills screen RAM after loading, which would erase
                // a line placed there.
                let on_screen = system.screen_ram().is_some_and(|screen| {
                    screen.start() <= footprint.end() && footprint.start() <= screen.end()
                });
                if on_screen {
                    continue;
                }
                let found = run(&program_with_line_at(load, basic_start), load, &config);
                if found.as_ref().err() != Some(&UnpackError::Phase1Timeout) {
                    failures.push(format!(
                        "{system} load ${load:04X}: line at ${basic_start:04X} gave {found:?}"
                    ));
                }
                // Padded load: a line at the load address itself is missed
                // (unless the default BASIC start fallback lands on it).
                if basic_start == load || load == system.default_basic_start() {
                    continue;
                }
                let missed = run(&program_with_line_at(load, load), load, &config);
                if missed.as_ref().err() != Some(&UnpackError::NoEntryPoint) {
                    failures.push(format!(
                        "{system} load ${load:04X}: line at ${load:04X} gave {missed:?}"
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    // -----------------------------------------------------------------------
    // ROM trap matrix (ruling R18): every system, representative addresses
    // -----------------------------------------------------------------------

    /// Where the forced `RTS` lands: one past the return address that the
    /// fake `JSR` pushed.
    const RTS_TARGET: u16 = 0x1234;
    const START_SP: u8 = 0xF0;
    /// PETSCII clear screen, so CHROUT takes its clear path where it has one.
    const START_A: u8 = 0x93;
    const START_X: u8 = 0x5A;
    const START_Y: u8 = 0xA5;
    /// C64 processor port direction register default (`$00`).
    const PORT_DDR: u8 = 0x2F;
    /// `$01` default: BASIC, KERNAL and I/O mapped. Other systems ignore the
    /// port (their ROMs are always mapped).
    const PORT_ALL_ROMS: u8 = 0x37;
    /// `$01` value that maps out BASIC but keeps the KERNAL.
    const PORT_KERNAL_ONLY: u8 = 0x36;
    /// `$01` value that maps out both ROMs.
    const PORT_ALL_RAM: u8 = 0x34;

    /// The single adaptation point between these tests and the trap API.
    fn invoke_trap(
        cpu: &mut CPU<C64Bus, Nmos6502>,
        getin_index: &mut usize,
        phase: u8,
    ) -> RomAction {
        let phase = match phase {
            1 => Phase::FindDepacker,
            2 => Phase::Decompress,
            other => panic!("phase must be 1 or 2, got {other}"),
        };
        let traps = RomTrapTable::for_system(&cpu.memory.system);
        handle_rom_entry(cpu, traps, getin_index, phase)
    }

    fn trap_cpu(system: TargetSystem, pc: u16, port: u8) -> CPU<C64Bus, Nmos6502> {
        let mut bus = C64Bus::new(system, None, None, None);
        bus.mem[0x00] = PORT_DDR;
        bus.mem[0x01] = port;
        let [lo, hi] = (RTS_TARGET - 1).to_le_bytes();
        bus.mem[0x0100 | usize::from(START_SP + 1)] = lo;
        bus.mem[0x0100 | usize::from(START_SP + 2)] = hi;
        let mut cpu = CPU::new(bus, Nmos6502);
        cpu.registers.program_counter = pc;
        cpu.registers.stack_pointer = mos6502::registers::StackPointer(START_SP);
        cpu.registers.accumulator = START_A;
        cpu.registers.index_x = START_X;
        cpu.registers.index_y = START_Y;
        cpu
    }

    /// Everything a trap may touch.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TrapState {
        action: RomAction,
        pc: u16,
        sp: u8,
        a: u8,
        x: u8,
        y: u8,
        getin_index: usize,
        port: u8,
        /// `$B7`, `$BB`, `$BC` (SETNAM).
        file_name: [u8; 3],
        /// `$0400`, `$07E7`, `$07E8`: first and last C64 screen byte, and the
        /// byte after the screen.
        screen: [u8; 3],
    }

    impl TrapState {
        fn capture(cpu: &CPU<C64Bus, Nmos6502>, action: RomAction, getin_index: usize) -> Self {
            let mem = &cpu.memory.mem;
            Self {
                action,
                pc: cpu.registers.program_counter,
                sp: cpu.registers.stack_pointer.0,
                a: cpu.registers.accumulator,
                x: cpu.registers.index_x,
                y: cpu.registers.index_y,
                getin_index,
                port: mem[0x01],
                file_name: [mem[0xB7], mem[0xBB], mem[0xBC]],
                screen: [mem[0x0400], mem[0x07E7], mem[0x07E8]],
            }
        }
    }

    /// Expected trap outcome class; [`expected_state`] spells out its literal
    /// effects.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Expect {
        /// Not trapped: the RAM code runs.
        Continue,
        BasicRun,
        Exit,
        /// Generic fallback: force `RTS`, nothing else.
        Rts,
        Getin,
        Cint,
        Chrout,
        Setnam,
        Ioinit,
        Restor,
    }

    fn expected_state(system: &TargetSystem, pc: u16, port: u8, expect: Expect) -> TrapState {
        let untouched = TrapState {
            action: RomAction::Continue,
            pc,
            sp: START_SP,
            a: START_A,
            x: START_X,
            y: START_Y,
            getin_index: 0,
            port,
            file_name: [0; 3],
            screen: [0; 3],
        };
        let rts = TrapState {
            action: RomAction::Handled,
            pc: RTS_TARGET,
            sp: START_SP + 2,
            ..untouched.clone()
        };
        // Ruling R5: trapped screen clears fill only on the C64.
        let cleared = if system.is_c64() {
            [0x20, 0x20, 0x00]
        } else {
            [0; 3]
        };
        match expect {
            Expect::Continue => untouched,
            Expect::BasicRun => TrapState {
                action: RomAction::BasicRun,
                ..untouched
            },
            Expect::Exit => TrapState {
                action: RomAction::Exit,
                ..untouched
            },
            Expect::Rts => rts,
            Expect::Getin => TrapState {
                a: 0x20,
                getin_index: 1,
                ..rts
            },
            Expect::Cint => TrapState {
                a: 0x00,
                x: 0x00,
                y: 0x00,
                screen: cleared,
                ..rts
            },
            Expect::Chrout => TrapState {
                screen: cleared,
                ..rts
            },
            Expect::Setnam => TrapState {
                file_name: [START_A, START_X, START_Y],
                ..rts
            },
            Expect::Ioinit => TrapState {
                port: 0xE7,
                a: 0xD7,
                x: 0xFF,
                ..rts
            },
            Expect::Restor => TrapState {
                a: 0x31,
                x: 0x30,
                y: 0xFF,
                ..rts
            },
        }
    }

    fn actual_state(
        system: &TargetSystem,
        pc: u16,
        phase: u8,
        port: u8,
        written: bool,
    ) -> TrapState {
        let mut cpu = trap_cpu(system.clone(), pc, port);
        cpu.memory.written[usize::from(pc)] = written;
        let mut getin_index = 0;
        let action = invoke_trap(&mut cpu, &mut getin_index, phase);
        TrapState::capture(&cpu, action, getin_index)
    }

    /// Matrix column order.
    fn all_systems() -> [TargetSystem; 11] {
        [
            TargetSystem::C64,
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Pet20,
            TargetSystem::Pet40,
            TargetSystem::Plus4,
            TargetSystem::C16,
            TargetSystem::C1541,
            TargetSystem::C1571,
            TargetSystem::C1581,
            // Equal to the C64 under `PartialEq`, but `is_c64()` is false, so
            // it takes the non-C64 path.
            TargetSystem::Custom("Commodore 64".into()),
        ]
    }

    /// R18.3: trap outcome per representative address, phase and system,
    /// with the ROMs mapped and nothing written at `pc`.
    #[test]
    fn engine_rom_trap_outcomes_per_system() {
        use Expect::{
            BasicRun as Run, Chrout, Cint, Continue as Ram, Exit, Getin, Ioinit, Restor, Rts,
            Setnam,
        };
        // Columns: C64, C128, VIC-20, PET 2001, PET 4000, Plus/4, C16, 1541,
        // 1571, 1581, Custom("Commodore 64").
        #[rustfmt::skip]
        let rows: [(u16, u8, [Expect; 11]); 39] = [
            (0x1000, 1, [Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram]),
            (0x4000, 1, [Ram, Rts, Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram, Ram]),
            (0xA000, 2, [Rts, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA7AE, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA7AE, 2, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA7B1, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA7EA, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA474, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA483, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA533, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA871, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA888, 1, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA8BC, 2, [Run, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA57C, 1, [Rts, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA57C, 2, [Exit, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA659, 2, [Exit, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA65A, 2, [Rts, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA660, 2, [Exit, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xA68E, 2, [Exit, Ram, Ram, Ram, Ram, Rts, Rts, Ram, Ram, Ram, Ram]),
            (0xC000, 1, [Ram, Rts, Rts, Rts, Rts, Ram, Ram, Rts, Rts, Rts, Ram]),
            (0xD000, 1, [Ram, Rts, Rts, Rts, Rts, Ram, Ram, Rts, Rts, Rts, Ram]),
            (0xE000, 1, [Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xFFE4, 1, [Getin, Getin, Getin, Getin, Getin, Getin, Getin, Getin, Getin, Getin, Getin]),
            (0xF13E, 1, [Getin, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xE536, 1, [Cint, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xE544, 2, [Cint, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xFF5B, 1, [Cint, Cint, Cint, Cint, Cint, Cint, Cint, Cint, Cint, Cint, Cint]),
            (0xFFD2, 2, [Chrout, Chrout, Chrout, Chrout, Chrout, Chrout, Chrout, Chrout, Chrout, Chrout, Chrout]),
            (0xFFBD, 1, [Setnam, Setnam, Setnam, Setnam, Setnam, Setnam, Setnam, Setnam, Setnam, Setnam, Setnam]),
            (0xFDA3, 1, [Ioinit, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xFD15, 2, [Restor, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xFFD5, 1, [Exit, Exit, Exit, Exit, Exit, Exit, Exit, Exit, Exit, Exit, Exit]),
            (0xF4A2, 2, [Exit, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xFCE2, 1, [Exit, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xEA31, 1, [Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xEA31, 2, [Exit, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xEB76, 2, [Exit, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xEA30, 2, [Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
            (0xEB77, 2, [Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts, Rts]),
        ];

        let mut failures = Vec::new();
        for (pc, phase, expects) in rows {
            for (system, expect) in all_systems().iter().zip(expects) {
                let expected = expected_state(system, pc, PORT_ALL_ROMS, expect);
                let actual = actual_state(system, pc, phase, PORT_ALL_ROMS, false);
                if actual != expected {
                    failures.push(format!(
                        "{system} ${pc:04X} phase {phase}: expected {expect:?} = {expected:?}, got {actual:?}"
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The C64 banking and `written` rules around the traps: BASIC RUN fires
    /// even when unmapped or written; written code under a mapped ROM is still
    /// trapped; an unmapped ROM runs the RAM underneath.
    #[test]
    fn engine_rom_trap_c64_banking_and_written_rules() {
        let c64 = TargetSystem::C64;
        // (label, pc, phase, $01, written, expected)
        #[rustfmt::skip]
        let rows = [
            ("RUN, all RAM", 0xA7AE, 1, PORT_ALL_RAM, false, Expect::BasicRun),
            ("RUN, written, all RAM", 0xA7AE, 2, PORT_ALL_RAM, true, Expect::BasicRun),
            ("BASIC fallback, BASIC unmapped", 0xA000, 1, PORT_KERNAL_ONLY, false, Expect::Continue),
            ("BASIC exit, BASIC unmapped", 0xA660, 2, PORT_KERNAL_ONLY, false, Expect::Continue),
            ("CHROUT, BASIC unmapped", 0xFFD2, 1, PORT_KERNAL_ONLY, false, Expect::Chrout),
            ("CHROUT, all RAM", 0xFFD2, 1, PORT_ALL_RAM, false, Expect::Continue),
            ("CHROUT, written, ROM mapped", 0xFFD2, 1, PORT_ALL_ROMS, true, Expect::Chrout),
            ("CHROUT, written, all RAM", 0xFFD2, 1, PORT_ALL_RAM, true, Expect::Continue),
            ("BASIC exit, written, mapped", 0xA660, 2, PORT_ALL_ROMS, true, Expect::Exit),
            ("IRQ, written, all RAM", 0xEA31, 2, PORT_ALL_RAM, true, Expect::Continue),
        ];
        let mut failures = Vec::new();
        for (label, pc, phase, port, written, expect) in rows {
            let expected = expected_state(&c64, pc, port, expect);
            let actual = actual_state(&c64, pc, phase, port, written);
            if actual != expected {
                failures.push(format!("{label}: expected {expected:?}, got {actual:?}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// GETIN cycles through the same 14 simulated key responses on every
    /// system.
    #[test]
    fn engine_getin_response_cycle() {
        const EXPECTED: [u8; 15] = [
            0x20, 0x00, 0x4E, 0x00, 0x03, 0x00, 0x5F, 0x00, 0x11, 0x00, 0x0D, 0x00, 0x31, 0x00,
            0x20,
        ];
        for system in all_systems() {
            let mut getin_index = 0;
            let responses: Vec<u8> = EXPECTED
                .iter()
                .map(|_| {
                    let mut cpu = trap_cpu(system.clone(), 0xFFE4, PORT_ALL_ROMS);
                    invoke_trap(&mut cpu, &mut getin_index, 1);
                    cpu.registers.accumulator
                })
                .collect();
            assert_eq!(responses, EXPECTED, "{system}");
            assert_eq!(getin_index, EXPECTED.len(), "{system}");
        }
    }

    // -----------------------------------------------------------------------
    // M2.2 state-machine & `finish` unit tests (Test Plan T2.3.2–T2.3.15)
    // -----------------------------------------------------------------------

    use super::super::machine::CARD_CRUNCHER_LOAD_ADDR;

    use crate::packers::{
        AddrSource, Anchor, Bounds, Hints, LoadedImage, MemoryPatch, PackerHooks, PackerSpec,
        PatchAddr, RefineContext, Signature, SignatureMatch, StepContext, sig,
    };

    type StepHookFn = Box<dyn FnMut(&mut StepContext<'_>) + Send>;
    type RefineHookFn = Box<dyn Fn(&RefineContext<'_>, &mut Bounds) + Send>;

    /// Configurable test [`PackerHooks`] for verifying hook ordering and `finish` overrides.
    struct MockHooks {
        step_hook: Option<StepHookFn>,
        refine_hook: Option<RefineHookFn>,
    }

    impl std::fmt::Debug for MockHooks {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockHooks").finish_non_exhaustive()
        }
    }

    impl PackerHooks for MockHooks {
        fn on_step(&mut self, ctx: &mut StepContext<'_>) {
            if let Some(ref mut hook) = self.step_hook {
                hook(ctx);
            }
        }

        fn refine(&self, ctx: &RefineContext<'_>, bounds: &mut Bounds) {
            if let Some(ref hook) = self.refine_hook {
                hook(ctx, bounds);
            }
        }
    }

    fn empty_packer_info() -> PackerInfo {
        PackerInfo {
            name: "MockPacker",
            dep_addr: None,
            start_addr: None,
            end_addr: None,
            entry_point: None,
            end_addr_ptr: None,
        }
    }

    /// Creates a minimal 64 KB `(mem, snapshot, written)` triple with `[0x1000..=0x2000]`
    /// marked written and differing from `snapshot`, so `detect_output_range` yields
    /// `(0x1000, 0x2000)`.
    fn finish_fixture() -> (Vec<u8>, Vec<u8>, Vec<bool>) {
        let mut mem = vec![0u8; ADDRESS_SPACE_SIZE];
        mem[0x00] = PORT_DDR;
        mem[0x01] = PORT_ALL_ROMS;
        let snapshot = mem.clone();
        let mut written = vec![false; ADDRESS_SPACE_SIZE];
        for addr in 0x1000..=0x2000 {
            mem[addr] = 0xEA;
            written[addr] = true;
        }
        (mem, snapshot, written)
    }

    fn test_finish_ctx<'a>(
        mem: &'a [u8],
        snapshot: &'a [u8],
        written: &'a [bool],
    ) -> FinishContext<'a> {
        FinishContext {
            mem,
            snapshot,
            written,
            entry_point: 0x1000,
            entry_source: BoundSource::ExitPc,
            dep_addr: STAGE2_ADDR,
            ret_addr: PRG_LOAD,
            instructions_executed: 10,
            find_depacker_instructions: 5,
            decompress_instructions: 5,
            exit_reason: ExitReason::WrittenCodeReached { pc: 0x1000 },
            trace_tail: Vec::new(),
            packer_id: None,
            basic_start: PRG_LOAD,
            hooks: None,
            hints: None,
            raw_hints: None,
            system: &TargetSystem::C64,
            y_reg: 0,
        }
    }

    /// T2.3.2 (Invariant 1): `step_find_depacker` reads `cpu.memory.in_phase2` to transition
    /// into [`Phase::Decompress`] even when the PC condition alone (`pc == known_dep` or
    /// `pc < ret_addr`) would not trigger.
    #[test]
    fn state_machine_reads_bus_phase_flag() {
        // Part A: A low-PC instruction writes at/above `ram_start`, causing `C64Bus::set_byte`
        // to latch `in_phase2 = true`. The packer's `dep_addr` is `Some(0x0900)` (`>= ret_addr`),
        // so the PC check is `pc == 0x0900` (not `pc < ret_addr`). On the step immediately
        // after the write, `step_find_depacker` sees `in_phase2 == true` and transitions.
        let config = UnpackConfig {
            forced_entry: Some(STAGE2_ADDR),
            ..budget(SMALL_BUDGET)
        };
        let mut state = EmulationState::new(&config, None, &[0xEA], PRG_LOAD).unwrap();
        let routine = store_and_jump(OUTPUT_ADDR);
        state.cpu.memory.mem[usize::from(STAGE2_ADDR)..usize::from(STAGE2_ADDR) + routine.len()]
            .copy_from_slice(&routine);
        state.hints = Some(PackerInfo {
            dep_addr: Some(0x0900),
            ..empty_packer_info()
        });

        // Step 1: `LDA #$EA` at `$0340` — no write yet, `in_phase2` stays false.
        assert_eq!(state.step_find_depacker(), Step::Continue);
        assert!(!state.cpu.memory.in_phase2);
        assert_eq!(state.total_instructions, 1);

        // Step 2: `STA $2000` at `$0342` — bus sees `current_pc < ram_start` writing `>= ram_start`
        // and flips `in_phase2 = true` during `single_step()`.
        assert_eq!(state.step_find_depacker(), Step::Continue);
        assert!(state.cpu.memory.in_phase2);
        assert_eq!(state.total_instructions, 2);

        // Step 3: PC is `$0345` (`!= 0x0900`), yet `step_find_depacker` transitions via `in_phase2`
        // without incrementing `total_instructions`.
        assert_eq!(
            state.step_find_depacker(),
            Step::ToPhase2 {
                dep_addr: STAGE2_ADDR + 5
            }
        );
        assert_eq!(state.total_instructions, 2);

        // Part B: A Phase 1 `on_step` hook sets `in_phase2 = true` while `pc >= ret_addr`.
        let raw = unpacker_program(&[]);
        let cfg = budget(SMALL_BUDGET);
        let mut hook_state = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        hook_state.hints = Some(empty_packer_info());
        hook_state.hooks = Some(Box::new(MockHooks {
            step_hook: Some(Box::new(|ctx| {
                assert_eq!(ctx.phase, Phase::FindDepacker);
                ctx.bus_mut().in_phase2 = true;
            })),
            refine_hook: None,
        }));

        assert_eq!(
            hook_state.step_find_depacker(),
            Step::ToPhase2 {
                dep_addr: SYS_ENTRY
            }
        );
        assert_eq!(hook_state.total_instructions, 0);
    }

    /// T2.3.3 (Invariant 2): in [`Phase::FindDepacker`], `pc` and `cpu.memory.current_pc` are
    /// captured *before* `hooks.on_step` runs, and the depacker check uses that
    /// pre-hook `pc`.
    #[test]
    fn hook_order_find_depacker_reads_pc_before_hook() {
        let raw = unpacker_program(&[]);
        let cfg = budget(SMALL_BUDGET);

        // Case A: `current_pc` starts at 0 while `program_counter` is `STAGE2_ADDR` (`< ret_addr`).
        // The hook verifies `current_pc` was already updated to `STAGE2_ADDR` before
        // `on_step` ran, then mutates `current_pc` to `SYS_ENTRY`.
        // `step_find_depacker` still transitions using the pre-hook `STAGE2_ADDR`.
        let mut state_a = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        state_a.cpu.memory.current_pc = 0;
        state_a.cpu.registers.program_counter = STAGE2_ADDR;
        state_a.hints = Some(empty_packer_info());
        state_a.hooks = Some(Box::new(MockHooks {
            step_hook: Some(Box::new(|ctx| {
                assert_eq!(ctx.phase, Phase::FindDepacker);
                assert_eq!(ctx.pc, STAGE2_ADDR);
                assert_eq!(ctx.bus_mut().current_pc, STAGE2_ADDR);
                ctx.bus_mut().current_pc = SYS_ENTRY;
            })),
            refine_hook: None,
        }));
        assert_eq!(
            state_a.step_find_depacker(),
            Step::ToPhase2 {
                dep_addr: STAGE2_ADDR
            }
        );

        // Case B: PC starts at `SYS_ENTRY` (`>= ret_addr`). The hook mutates `current_pc`
        // down to `STAGE2_ADDR` (`< ret_addr`), while a `NOP` (`$EA`) is at `SYS_ENTRY`. Because
        // `step_find_depacker` captured `pc = SYS_ENTRY` before the hook, it does NOT enter
        // Phase 2 on this step; it executes the `NOP` at `SYS_ENTRY` and returns `Continue`.
        let mut state_b = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        state_b.cpu.memory.mem[usize::from(SYS_ENTRY)] = 0xEA;
        state_b.hints = Some(empty_packer_info());
        state_b.hooks = Some(Box::new(MockHooks {
            step_hook: Some(Box::new(|ctx| {
                assert_eq!(ctx.phase, Phase::FindDepacker);
                assert_eq!(ctx.bus_mut().current_pc, SYS_ENTRY);
                ctx.bus_mut().current_pc = STAGE2_ADDR;
            })),
            refine_hook: None,
        }));
        assert_eq!(state_b.step_find_depacker(), Step::Continue);
        assert_eq!(state_b.total_instructions, 1);
        assert_eq!(state_b.cpu.registers.program_counter, SYS_ENTRY + 1);
    }

    /// T2.3.4 (Invariant 2): in [`Phase::Decompress`], `hooks.on_step` runs
    /// *before* `cpu.memory.current_pc` is updated and before exit checks inspect
    /// `written`.
    #[test]
    fn hook_order_decompress_hook_before_pc_read() {
        let raw = unpacker_program(&[]);
        let cfg = budget(SMALL_BUDGET);

        // Case A (TSCrunch relocation-clear pattern): PC is at `OUTPUT_ADDR`, which is marked
        // in `written` (from Phase 1 relocation) with a `NOP` (`$EA`) in memory. The Phase 2
        // hook clears `written[OUTPUT_ADDR]` via `forget_writes` before `step_decompress` checks
        // `is_written_code` or `LeftLoadedRegion`. As a result, `step_decompress` does NOT exit
        // and executes the `NOP`.
        let mut state_a = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        state_a.phase = Phase::Decompress;
        state_a.cpu.memory.in_phase2 = true;
        state_a.cpu.registers.program_counter = OUTPUT_ADDR;
        state_a.cpu.memory.mem[usize::from(OUTPUT_ADDR)] = 0xEA;
        state_a.cpu.memory.written[usize::from(OUTPUT_ADDR)] = true;
        state_a.hints = Some(empty_packer_info());
        state_a.hooks = Some(Box::new(MockHooks {
            step_hook: Some(Box::new(|ctx| {
                assert_eq!(ctx.phase, Phase::Decompress);
                ctx.forget_writes(OUTPUT_ADDR..=OUTPUT_ADDR);
            })),
            refine_hook: None,
        }));
        assert_eq!(state_a.step_decompress(), Step::Continue);
        assert_eq!(state_a.total_instructions, 1);

        // Case B: `current_pc` holds the previous instruction (`STAGE2_ADDR`) while
        // `program_counter` is `OUTPUT_ADDR` and `written[OUTPUT_ADDR]` is initially `false`.
        // The Phase 2 hook verifies that `current_pc` was NOT yet updated to
        // `OUTPUT_ADDR`, then sets `written[OUTPUT_ADDR] = true`. Because `step_decompress`
        // updates `current_pc` and evaluates `is_written_code` *after* the hook, it immediately
        // exits with `WrittenCodeReached`.
        let mut state_b = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        state_b.phase = Phase::Decompress;
        state_b.cpu.memory.in_phase2 = true;
        state_b.cpu.memory.current_pc = STAGE2_ADDR;
        state_b.cpu.registers.program_counter = OUTPUT_ADDR;
        state_b.cpu.memory.written[usize::from(OUTPUT_ADDR)] = false;
        state_b.hints = Some(empty_packer_info());
        state_b.hooks = Some(Box::new(MockHooks {
            step_hook: Some(Box::new(|ctx| {
                assert_eq!(ctx.phase, Phase::Decompress);
                assert_eq!(ctx.bus_mut().current_pc, STAGE2_ADDR);
                ctx.bus_mut().written[usize::from(OUTPUT_ADDR)] = true;
            })),
            refine_hook: None,
        }));
        assert_eq!(
            state_b.step_decompress(),
            Step::Done(ExitReason::WrittenCodeReached { pc: OUTPUT_ADDR })
        );
        assert_eq!(state_b.cpu.memory.current_pc, OUTPUT_ADDR);
        assert_eq!(state_b.total_instructions, 0);
    }

    /// T2.3.5 (Invariant 3): a packer's `known_entry` triggers an exit in [`Phase::Decompress`]
    /// only when `total_instructions > MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY` (`10`), whereas
    /// `is_written_code` exits regardless of the instruction count.
    #[test]
    fn known_entry_guard_requires_more_than_ten_instructions() {
        const KNOWN_ENTRY: u16 = 0x1800;
        let raw = unpacker_program(&[]);
        let cfg = budget(SMALL_BUDGET);
        let mut state = EmulationState::new(&cfg, None, &raw, PRG_LOAD).unwrap();
        state.phase = Phase::Decompress;
        state.cpu.memory.in_phase2 = true;
        state.hints = Some(PackerInfo {
            entry_point: Some(KNOWN_ENTRY),
            ..empty_packer_info()
        });
        state.cpu.memory.mem[usize::from(KNOWN_ENTRY)] = 0xEA; // NOP (unwritten)

        // For `total_instructions` 0..=10, hitting `KNOWN_ENTRY` (unwritten) does NOT exit.
        for count in 0..=MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY {
            state.total_instructions = count;
            state.cpu.registers.program_counter = KNOWN_ENTRY;
            assert_eq!(
                state.step_decompress(),
                Step::Continue,
                "count={count} must not trigger KnownEntryReached"
            );
            assert_eq!(state.total_instructions, count + 1);
        }

        // At `total_instructions == 11` (`> 10`), hitting `KNOWN_ENTRY` exits immediately
        // without executing the instruction or incrementing `total_instructions`.
        state.total_instructions = MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY + 1;
        state.cpu.registers.program_counter = KNOWN_ENTRY;
        assert_eq!(
            state.step_decompress(),
            Step::Done(ExitReason::KnownEntryReached { pc: KNOWN_ENTRY })
        );
        assert_eq!(
            state.total_instructions,
            MIN_INSTRUCTIONS_BEFORE_KNOWN_ENTRY + 1
        );

        // When `is_written_code` holds, `step_decompress` exits even at `total_instructions == 0`.
        state.total_instructions = 0;
        state.cpu.registers.program_counter = OUTPUT_ADDR;
        state.cpu.memory.written[usize::from(OUTPUT_ADDR)] = true;
        assert_eq!(
            state.step_decompress(),
            Step::Done(ExitReason::WrittenCodeReached { pc: OUTPUT_ADDR })
        );
    }

    /// T2.3.6 (Invariant 4): ROM `Handled` steps increment `total_instructions` by 1 in both
    /// [`Phase::FindDepacker`] and [`Phase::Decompress`], and do not fire the progress callback.
    #[test]
    fn rom_handled_steps_count_in_both_stages() {
        // Part A: End-to-end run with `JSR $FFD2` (CHROUT) in Phase 1 AND in Phase 2.
        // Each `JSR $FFD2` adds 2 instructions: 1 for `JSR` + 1 for the trapped `Handled` step.
        let mut stage2_with_chrout = vec![0x20, 0xD2, 0xFF];
        stage2_with_chrout.extend_from_slice(&store_and_jump(OUTPUT_ADDR));
        let mut code = vec![0x20, 0xD2, 0xFF];
        code.extend(stage1_installing(&stage2_with_chrout));
        let raw = sys_program(&code);

        // Stage 1 installs 3 extra bytes (3 * 2 = 6 extra install instructions) + 2 for Phase 1
        // JSR/CHROUT + 2 for Phase 2 JSR/CHROUT = BASE_INSTRUCTIONS + 10.
        let without_stage2_chrout = {
            let mut c = vec![0x20, 0xD2, 0xFF];
            c.extend(stage1_installing(&format_stage2_nops_then_store()));
            sys_program(&c)
        };
        let res_chrout = run(&raw, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        let res_nops = run(&without_stage2_chrout, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        // `JSR $FFD2` (2 steps: JSR + trapped Handled) vs three `NOP`s (3 steps) in Stage 2:
        assert_eq!(res_chrout.instructions_executed, BASE_INSTRUCTIONS + 10);
        assert_eq!(res_nops.instructions_executed, BASE_INSTRUCTIONS + 11);

        // Part B: Direct step verification in both phases at `PROGRESS_CALLBACK_INTERVAL - 1`.
        // The trapped step increments `total_instructions` to `30_000` without invoking `cb`.
        for phase in [Phase::FindDepacker, Phase::Decompress] {
            let calls = RefCell::new(Vec::new());
            let record = |count: u64| calls.borrow_mut().push(count);
            let cfg = budget(SMALL_BUDGET);
            let mut state =
                EmulationState::new(&cfg, Some(&record), &unpacker_program(&[]), PRG_LOAD).unwrap();
            state.phase = phase;
            state.total_instructions = PROGRESS_CALLBACK_INTERVAL - 1;
            state.cpu.registers.program_counter = 0xFFD2; // CHROUT
            state.cpu.registers.accumulator = 0x00;

            let step = match phase {
                Phase::FindDepacker => state.step_find_depacker(),
                Phase::Decompress => state.step_decompress(),
            };
            assert_eq!(step, Step::Continue, "{phase:?}");
            assert_eq!(
                state.total_instructions, PROGRESS_CALLBACK_INTERVAL,
                "{phase:?}"
            );
            assert!(
                calls.borrow().is_empty(),
                "{phase:?}: ROM Handled step must not fire progress_callback"
            );
        }
    }

    fn format_stage2_nops_then_store() -> Vec<u8> {
        let mut v = vec![0xEA, 0xEA, 0xEA];
        v.extend_from_slice(&store_and_jump(OUTPUT_ADDR));
        v
    }

    /// T2.3.7 (Invariant 5): `hints.end_addr` overrides `end_addr` *before* `hints.end_addr_ptr`
    /// evaluates the `<= end_addr.saturating_add(END_PTR_SLACK)` guard.
    #[test]
    fn finish_override_order_end_ptr_uses_overridden_end() {
        let (mut mem, snapshot, written) = finish_fixture();
        // Pointer at $00AE reports `$3101` (`reported_end - 1 == $3100`).
        // Detected `end_addr` from `finish_fixture` is `$2000`.
        let [lo, hi] = 0x3101u16.to_le_bytes();
        mem[0x00AE] = lo;
        mem[0x00AF] = hi;

        // With `hints.end_addr = Some(0x3000)`, `$3100 <= $3000 + 512 ($3200)` holds, so
        // `end_addr_ptr` wins and sets `end_addr = $3100`.
        let hints_with_end = PackerInfo {
            end_addr: Some(0x3000),
            end_addr_ptr: Some(0x00AE),
            ..empty_packer_info()
        };
        let res_accepted = finish(FinishContext {
            hints: Some(&hints_with_end),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_accepted.end_addr, 0x3100);

        // Without `hints.end_addr`, `$3100 <= $2000 + 512 ($2200)` fails, so `end_addr` stays `$2000`.
        let hints_without_end = PackerInfo {
            end_addr: None,
            end_addr_ptr: Some(0x00AE),
            ..empty_packer_info()
        };
        let res_rejected = finish(FinishContext {
            hints: Some(&hints_without_end),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_rejected.end_addr, 0x2000);
    }

    /// T2.3.8: `end_addr_ptr` is accepted iff `reported_end > start_addr` and
    /// `reported_end - 1 <= end_addr + END_PTR_SLACK` (`512`).
    #[test]
    fn finish_end_ptr_guard_rejects_out_of_slack_or_below_start() {
        // Detected range is `$1000..=$2000`, so `end_addr + END_PTR_SLACK == $2200`.
        let cases: &[(u16, u16, &str)] = &[
            (0x0000, 0x2000, "zero (below start)"),
            (0x0FFF, 0x2000, "below start"),
            (0x1000, 0x2000, "equal to start (not strictly greater)"),
            (0x1001, 0x1000, "start + 1 (minimum accepted, end = $1000)"),
            (0x2201, 0x2200, "exact slack boundary (end = $2200)"),
            (0x2202, 0x2000, "one byte past slack (rejected)"),
            (0xFFFF, 0x2000, "far past slack (rejected)"),
        ];

        let hints = PackerInfo {
            end_addr_ptr: Some(0x00AE),
            ..empty_packer_info()
        };
        let mut failures = Vec::new();
        for &(reported_end, expected_end, label) in cases {
            let (mut mem, snapshot, written) = finish_fixture();
            let [lo, hi] = reported_end.to_le_bytes();
            mem[0x00AE] = lo;
            mem[0x00AF] = hi;
            let res = finish(FinishContext {
                hints: Some(&hints),
                ..test_finish_ctx(&mem, &snapshot, &written)
            })
            .unwrap();
            if res.end_addr != expected_end {
                failures.push(format!(
                    "{label} (reported=${reported_end:04X}): expected ${expected_end:04X}, got ${:04X}",
                    res.end_addr
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// T2.3.9 (Ruling R19): when `end_addr_ptr == Some(0xFFFF)`, `finish` reads the high byte
    /// from `0xFFFFu16.wrapping_add(1)` (`$0000`) without panicking on integer overflow.
    #[test]
    fn finish_end_ptr_at_ffff_does_not_panic() {
        let (mut mem, snapshot, written) = finish_fixture();
        // Low byte at `$FFFF` = `$51`, high byte wrapped to `$0000` = `$20` -> `$2051` (`end = $2050`).
        mem[0xFFFF] = 0x51;
        mem[0x0000] = 0x20;
        let hints = PackerInfo {
            end_addr_ptr: Some(0xFFFF),
            ..empty_packer_info()
        };
        let res = finish(FinishContext {
            hints: Some(&hints),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res.end_addr, 0x2050);
    }

    /// T2.3.10: `hints.dep_addr` overrides `FinishContext::dep_addr` (including a
    /// `forced_dep_addr` captured at phase transition).
    #[test]
    fn finish_packer_dep_addr_wins() {
        let (mem, snapshot, written) = finish_fixture();
        let hints_with_dep = PackerInfo {
            dep_addr: Some(0x0150),
            ..empty_packer_info()
        };
        let res_overridden = finish(FinishContext {
            dep_addr: 0x0333,
            hints: Some(&hints_with_dep),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_overridden.dep_addr, 0x0150);

        let hints_without_dep = empty_packer_info();
        let res_kept = finish(FinishContext {
            dep_addr: 0x0333,
            hints: Some(&hints_without_dep),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_kept.dep_addr, 0x0333);
    }

    /// T2.3.11 (Invariant 5): `hooks.refine` runs *after* `start_addr`, `end_addr`,
    /// `end_addr_ptr`, and `entry_point` hints are applied, and *before* the BASIC `SYS`
    /// line clamp.
    #[test]
    fn finish_refine_runs_after_pointer_and_before_sys_clamp() {
        let (mut mem, snapshot, written) = finish_fixture();
        // Place `10 SYS2061` at `PRG_LOAD` (`$0801`).
        let basic_stub = sys_program(&[]);
        mem[usize::from(PRG_LOAD)..usize::from(PRG_LOAD) + basic_stub.len()]
            .copy_from_slice(&basic_stub);
        // Pointer at `$00AE` reports `$2101` (`end = $2100`).
        let [lo, hi] = 0x2101u16.to_le_bytes();
        mem[0x00AE] = lo;
        mem[0x00AF] = hi;

        let hints = PackerInfo {
            start_addr: Some(0x1100),
            end_addr: Some(0x2000),
            end_addr_ptr: Some(0x00AE),
            entry_point: Some(0x1500),
            ..empty_packer_info()
        };
        let hooks = MockHooks {
            step_hook: None,
            refine_hook: Some(Box::new(|ctx, bounds| {
                // Verify Steps 2, 3, 4, and 5 already ran before `refine`.
                assert_eq!((bounds.start_addr, bounds.end_addr), (0x1100, 0x2100));
                assert_eq!(bounds.entry_point, 0x1500);
                assert_eq!(ctx.registers.y, 0x77);
                // Refine range and set entry to `PRG_LOAD` so Step 8 (SYS clamp)
                // subsequently lowers `start_addr` to `PRG_LOAD` and redirects `entry_point`
                // to `SYS_ENTRY` (`$080D`).
                bounds.start_addr = 0x1200;
                bounds.end_addr = 0x2080;
                bounds.entry_point = PRG_LOAD;
            })),
        };
        let res = finish(FinishContext {
            hooks: Some(&hooks),
            hints: Some(&hints),
            y_reg: 0x77,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();

        assert_eq!(res.start_addr, PRG_LOAD);
        assert_eq!(res.end_addr, 0x2080);
        assert_eq!(res.entry_point, SYS_ENTRY);
    }

    /// T2.3.12 (Invariant 5): when a BASIC `SYS` line exists at `basic_start`, `finish`
    /// lowers `start_addr` to `basic_start` if `start_addr > basic_start`, leaves a lower
    /// `start_addr` untouched, and does not clamp when no `SYS` line is present.
    #[test]
    fn finish_sys_clamp_lowers_start_to_basic_start() {
        let (mut mem_with_sys, snapshot, written) = finish_fixture();
        let basic_stub = sys_program(&[]);
        mem_with_sys[usize::from(PRG_LOAD)..usize::from(PRG_LOAD) + basic_stub.len()]
            .copy_from_slice(&basic_stub);

        // 1. `start_addr` (`$1000`) > `basic_start` (`$0801`) with a `SYS` line -> lowered to `$0801`.
        let res_lowered = finish(FinishContext {
            entry_point: 0x1500,
            ..test_finish_ctx(&mem_with_sys, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_lowered.start_addr, PRG_LOAD);
        assert_eq!(res_lowered.entry_point, 0x1500);

        // 2. `start_addr` (`$07F0`) < `basic_start` (`$0801`) with a `SYS` line -> stays `$07F0`.
        let hints_low = PackerInfo {
            start_addr: Some(0x07F0),
            ..empty_packer_info()
        };
        let res_kept_low = finish(FinishContext {
            entry_point: 0x1500,
            hints: Some(&hints_low),
            ..test_finish_ctx(&mem_with_sys, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_kept_low.start_addr, 0x07F0);

        // 3. No `SYS` line at `basic_start` -> `start_addr` stays `$1000`.
        let (mem_no_sys, snapshot_no_sys, written_no_sys) = finish_fixture();
        let res_no_sys = finish(FinishContext {
            entry_point: 0x1500,
            ..test_finish_ctx(&mem_no_sys, &snapshot_no_sys, &written_no_sys)
        })
        .unwrap();
        assert_eq!(res_no_sys.start_addr, 0x1000);
    }

    /// T2.3.13 (Invariant 5): `finish` clamps `start_addr` down to `entry_point` when
    /// `entry_point >= MIN_VALID_ENTRY_ADDR` (`$0200`), clamps `end_addr` up to `entry_point`
    /// when `entry_point > end_addr`, and returns [`UnpackError::InvalidAddressRange`] when
    /// `entry_point < $0200` lies below `start_addr`.
    #[test]
    fn finish_entry_clamps() {
        let (mem, snapshot, written) = finish_fixture();

        // 1. `entry_point == $0200` (`< start_addr == $1000`, `>= $0200`) -> clamps `start_addr` to `$0200`.
        let res_low_clamp = finish(FinishContext {
            entry_point: MIN_VALID_ENTRY_ADDR,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_low_clamp.start_addr, MIN_VALID_ENTRY_ADDR);
        assert_eq!(res_low_clamp.end_addr, 0x2000);

        // 2. `entry_point == $01FF` (`< $0200`) -> not clamped; fails validation with `InvalidAddressRange`.
        let err_below_min = finish(FinishContext {
            entry_point: MIN_VALID_ENTRY_ADDR - 1,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap_err();
        assert_eq!(
            err_below_min,
            UnpackError::InvalidAddressRange {
                start_addr: 0x1000,
                end_addr: 0x2000,
                entry_point: MIN_VALID_ENTRY_ADDR - 1,
            }
        );

        // 3. `entry_point == $2500` (`> end_addr == $2000`) -> clamps `end_addr` up to `$2500`.
        let res_high_clamp = finish(FinishContext {
            entry_point: 0x2500,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_high_clamp.start_addr, 0x1000);
        assert_eq!(res_high_clamp.end_addr, 0x2500);
    }

    /// T2.3.14 (Invariant 5): in `finish`, an `entry_point` inside a currently mapped BASIC
    /// or KERNAL ROM (or equal to `basic_start`) is replaced by the `SYS` target address from
    /// `basic_start`, whereas an unmapped ROM address is kept as RAM code.
    #[test]
    fn finish_rom_entry_replaced_by_sys_address() {
        let (mut mem, snapshot, written) = finish_fixture();
        let basic_stub = sys_program(&[]);
        mem[usize::from(PRG_LOAD)..usize::from(PRG_LOAD) + basic_stub.len()]
            .copy_from_slice(&basic_stub);

        // Mapped BASIC ROM (`$A000`), mapped KERNAL ROM (`$FCE2`), and `basic_start` (`$0801`)
        // all resolve to `SYS_ENTRY` (`$080D`).
        for rom_or_basic_entry in [0xA000, 0xFCE2, PRG_LOAD] {
            let res = finish(FinishContext {
                entry_point: rom_or_basic_entry,
                ..test_finish_ctx(&mem, &snapshot, &written)
            })
            .unwrap();
            assert_eq!(
                res.entry_point, SYS_ENTRY,
                "entry ${rom_or_basic_entry:04X} with ROMs mapped"
            );
        }

        // With all RAM banked in (`$01 = $34`), `$A000` and `$FCE2` are RAM addresses, so
        // `is_rom_entry` is false and `entry_point` is preserved (clamping `end_addr` up).
        mem[0x01] = PORT_ALL_RAM;
        for ram_entry in [0xA000, 0xFCE2] {
            let res = finish(FinishContext {
                entry_point: ram_entry,
                ..test_finish_ctx(&mem, &snapshot, &written)
            })
            .unwrap();
            assert_eq!(
                res.entry_point, ram_entry,
                "entry ${ram_entry:04X} with ROMs unmapped"
            );
            assert_eq!(res.end_addr, ram_entry);
        }
    }

    /// T2.3.15: [`FinishContext`] names the BASIC start field `basic_start` (not `load_addr`),
    /// and a padded load (`CARD_CRUNCHER_LOAD_ADDR`, where `basic_start == load_addr + 1`)
    /// resolves its `SYS` line and start clamp against `basic_start`.
    #[test]
    fn finish_context_basic_start_field_is_named_correctly() {
        let load_addr = CARD_CRUNCHER_LOAD_ADDR;
        let basic_start = get_basic_start(load_addr);
        assert_eq!(basic_start, load_addr + 1);

        let (mut mem, snapshot, written) = finish_fixture();
        // Place a valid `SYS 2061` stub at `basic_start` (`$07C1`), with a `$00` padding byte
        // at `load_addr` (`$07C0`).
        let [link_lo, link_hi] = (basic_start + 10).to_le_bytes();
        mem[usize::from(basic_start)] = link_lo;
        mem[usize::from(basic_start) + 1] = link_hi;
        let stub_tail = &sys_program(&[])[2..];
        mem[usize::from(basic_start) + 2..usize::from(basic_start) + 2 + stub_tail.len()]
            .copy_from_slice(stub_tail);

        let ctx = FinishContext {
            entry_point: basic_start,
            ret_addr: load_addr,
            basic_start,
            ..test_finish_ctx(&mem, &snapshot, &written)
        };
        let res = finish(ctx).unwrap();
        assert_eq!(res.start_addr, basic_start);
        assert_eq!(res.entry_point, SYS_ENTRY);
    }

    /// Pins the phase-specific fallback differences in `resolve_exit` (`FindDepacker`
    /// `BasicRun` falls back to `basic_start`; `Decompress` `BasicRun` falls back to `pc`)
    /// and `LeftLoadedRegion` checking `written` only (not `written_phase2`).
    #[test]
    fn phase_exit_fallbacks_and_left_loaded_region_written_only() {
        let cfg = UnpackConfig {
            forced_entry: Some(SYS_ENTRY),
            ..budget(SMALL_BUDGET)
        };
        // Memory with NO `SYS` line at `basic_start` (`$0801`).
        let mut state = EmulationState::new(&cfg, None, &[0xEA; 16], PRG_LOAD).unwrap();
        let basic_run_pc = super::super::machine::C64_BASIC_RUN_ENTRIES[0];
        state.cpu.memory.current_pc = basic_run_pc;

        // In `FindDepacker`, `BasicRun` without a `SYS` line falls back to `basic_start`.
        state.phase = Phase::FindDepacker;
        assert_eq!(
            state.resolve_exit(ExitReason::BasicRun),
            (PRG_LOAD, basic_run_pc)
        );

        // In `Decompress`, `BasicRun` without a `SYS` line falls back to `pc`.
        state.phase = Phase::Decompress;
        state.dep_addr = STAGE2_ADDR;
        assert_eq!(
            state.resolve_exit(ExitReason::BasicRun),
            (basic_run_pc, STAGE2_ADDR)
        );

        // `LeftLoadedRegion`: when `ret_addr` is forced above `pc` (`$3000 > $2000`) so
        // `is_written_code` is false, jumping outside the loaded region to `$2000` exits iff
        // `written[$2000]` is true (not `written_phase2[$2000]` alone).
        let cfg_high_ret = UnpackConfig {
            forced_entry: Some(SYS_ENTRY),
            forced_ret_addr: Some(0x3000),
            ..budget(SMALL_BUDGET)
        };
        let mut lr_state = EmulationState::new(&cfg_high_ret, None, &[0xEA; 16], PRG_LOAD).unwrap();
        lr_state.phase = Phase::Decompress;
        lr_state.cpu.memory.in_phase2 = true;
        lr_state.cpu.registers.program_counter = OUTPUT_ADDR;
        lr_state.cpu.memory.mem[usize::from(OUTPUT_ADDR)] = 0xEA;

        // Only `written_phase2` set (`written` cleared, e.g. by TSCrunch): does NOT exit.
        lr_state.cpu.memory.written[usize::from(OUTPUT_ADDR)] = false;
        lr_state.cpu.memory.written_phase2[usize::from(OUTPUT_ADDR)] = true;
        assert_eq!(lr_state.step_decompress(), Step::Continue);

        // `written` set: triggers `LeftLoadedRegion`.
        lr_state.cpu.registers.program_counter = OUTPUT_ADDR;
        lr_state.cpu.memory.written[usize::from(OUTPUT_ADDR)] = true;
        lr_state.cpu.memory.written_phase2[usize::from(OUTPUT_ADDR)] = false;
        assert_eq!(
            lr_state.step_decompress(),
            Step::Done(ExitReason::LeftLoadedRegion { pc: OUTPUT_ADDR })
        );
    }

    // ═══════════════════════ M3.6a tests (T3.6a.4, T3.6.12) ═══════════════════════

    /// T3.6a.4: a test-only [`PackerSpec`] whose signature requires the zero-page template
    /// bytes `$00 = $2F, $01 = $37` matches during [`EmulationState::new_with_specs`],
    /// proving that packer detection runs *after* `init_c64_zero_page`.
    #[test]
    fn engine_detection_sees_zero_page_initialized_image() {
        static PATCHES: [MemoryPatch; 1] = [MemoryPatch {
            addr: PatchAddr::Absolute(SYS_ENTRY),
            expected: Some(0xEA),
            bytes: &[0x60],
        }];

        fn verify_zp_and_resolve(
            image: &LoadedImage<'_>,
            sig_match: &SignatureMatch,
        ) -> Option<crate::packers::ResolvedSpec> {
            // Zero-page pointers must already be populated when `resolve` runs.
            let txttab = u16::from_le_bytes([image.mem[0x002B], image.mem[0x002C]]);
            let vartab = u16::from_le_bytes([image.mem[0x002D], image.mem[0x002E]]);
            let eal = u16::from_le_bytes([image.mem[0x00AE], image.mem[0x00AF]]);
            if txttab != PRG_LOAD || vartab != image.load_end || eal != image.load_end {
                return None;
            }
            Some(crate::packers::ResolvedSpec {
                name: Some(sig_match.spec_name),
                hints: Hints {
                    dep_addr: Some(AddrSource::Const(STAGE2_ADDR)),
                    ..Hints::NONE
                },
                patches: &PATCHES,
            })
        }

        static ZP_SPEC: PackerSpec = PackerSpec {
            id: "test_zp_initialized",
            name: "TestZPInitialized",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[
                // Requires `$00 = $2F` and `$01 = $37` set by `init_c64_zero_page`.
                Signature {
                    anchor: Anchor::Absolute(0x0000),
                    pattern: sig![0x2F, 0x37],
                },
                Signature {
                    anchor: Anchor::BasicStart { offset: 0x0C },
                    pattern: sig![0xEA],
                },
            ],
            hints: Hints::NONE,
            patches: &[],
            resolve: Some(verify_zp_and_resolve),
            hooks: None,
            fixture: &[0xEA],
            fixture_load_addr: SYS_ENTRY,
        };
        static SPECS: &[&PackerSpec] = &[&ZP_SPEC];

        let raw = sys_program(&[0xEA]);
        let config = budget(SMALL_BUDGET);
        let state = EmulationState::new_with_specs(&config, None, &raw, PRG_LOAD, SPECS).unwrap();

        let info = state
            .hints
            .as_ref()
            .expect("ZP_SPEC must match after zero-page init");
        assert_eq!(info.name, "TestZPInitialized");
        assert_eq!(info.dep_addr, Some(STAGE2_ADDR));

        // The spec's pre-emulation patch (`$080D: $EA -> $60`) is applied before snapshot
        // and does not mark `$080D` as runtime-written.
        assert_eq!(state.cpu.memory.mem[usize::from(SYS_ENTRY)], 0x60);
        assert_eq!(state.snapshot[usize::from(SYS_ENTRY)], 0x60);
        assert!(!state.cpu.memory.written[usize::from(SYS_ENTRY)]);
    }

    /// T3.6.12: entry-point precedence in [`EmulationState::new_with_specs`] is strictly:
    /// user [`UnpackConfig::forced_entry`] > spec [`Hints::forced_entry`] > BASIC `SYS` line.
    #[test]
    fn engine_entry_precedence_user_over_spec_over_sys() {
        const USER_FORCED_ENTRY: u16 = 0x3000;
        const SPEC_FORCED_ENTRY: u16 = 0x2000;

        static FORCED_ENTRY_SPEC: PackerSpec = PackerSpec {
            id: "test_forced_entry",
            name: "TestForcedEntry",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xEA],
            }],
            hints: Hints {
                forced_entry: Some(AddrSource::Const(SPEC_FORCED_ENTRY)),
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xEA],
            fixture_load_addr: SYS_ENTRY,
        };

        static NO_FORCED_ENTRY_SPEC: PackerSpec = PackerSpec {
            id: "test_no_forced_entry",
            name: "TestNoForcedEntry",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xEA],
            }],
            hints: Hints::NONE,
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xEA],
            fixture_load_addr: SYS_ENTRY,
        };

        static SPECS_WITH_FORCED: &[&PackerSpec] = &[&FORCED_ENTRY_SPEC];
        static SPECS_WITHOUT_FORCED: &[&PackerSpec] = &[&NO_FORCED_ENTRY_SPEC];

        // Program with a valid `10 SYS 2061` (`SYS_ENTRY == $080D`) stub and `$EA` at `$080D`.
        let raw_with_sys = sys_program(&[0xEA]);

        // 1. User `forced_entry` ($3000) beats spec `forced_entry` ($2000) and SYS ($080D).
        let cfg_user = UnpackConfig {
            forced_entry: Some(USER_FORCED_ENTRY),
            ..budget(SMALL_BUDGET)
        };
        let state_user = EmulationState::new_with_specs(
            &cfg_user,
            None,
            &raw_with_sys,
            PRG_LOAD,
            SPECS_WITH_FORCED,
        )
        .unwrap();
        assert_eq!(state_user.cpu.registers.program_counter, USER_FORCED_ENTRY);

        // 2. With user `forced_entry = None`, spec `forced_entry` ($2000) beats SYS ($080D).
        let cfg_none = budget(SMALL_BUDGET);
        let state_spec = EmulationState::new_with_specs(
            &cfg_none,
            None,
            &raw_with_sys,
            PRG_LOAD,
            SPECS_WITH_FORCED,
        )
        .unwrap();
        assert_eq!(state_spec.cpu.registers.program_counter, SPEC_FORCED_ENTRY);

        // 2b. Spec `forced_entry` ($2000) also succeeds when no BASIC `SYS` line is present.
        let mut raw_without_sys = vec![0u8; 16];
        raw_without_sys[0x0C] = 0xEA;
        let state_spec_no_sys = EmulationState::new_with_specs(
            &cfg_none,
            None,
            &raw_without_sys,
            PRG_LOAD,
            SPECS_WITH_FORCED,
        )
        .unwrap();
        assert_eq!(
            state_spec_no_sys.cpu.registers.program_counter,
            SPEC_FORCED_ENTRY
        );

        // 3. With neither user nor spec `forced_entry`, the BASIC `SYS` line ($080D) is used.
        let state_sys = EmulationState::new_with_specs(
            &cfg_none,
            None,
            &raw_with_sys,
            PRG_LOAD,
            SPECS_WITHOUT_FORCED,
        )
        .unwrap();
        assert_eq!(state_sys.cpu.registers.program_counter, SYS_ENTRY);
    }

    /// T3.6.4: pre-emulation [`MemoryPatch`] writes directly to `memory.mem` and never marks
    /// any byte in `memory.written`.
    #[test]
    fn patch_does_not_mark_written() {
        static PATCHES: [MemoryPatch; 1] = [MemoryPatch {
            addr: PatchAddr::Absolute(SYS_ENTRY),
            expected: None,
            bytes: &[0xA9, 0x00, 0x60],
        }];
        static PATCH_SPEC: PackerSpec = PackerSpec {
            id: "test_patch_not_written",
            name: "TestPatchNotWritten",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xEA, 0xEA, 0xEA],
            }],
            hints: Hints::NONE,
            patches: &PATCHES,
            resolve: None,
            hooks: None,
            fixture: &[0xEA, 0xEA, 0xEA],
            fixture_load_addr: SYS_ENTRY,
        };
        static SPECS: &[&PackerSpec] = &[&PATCH_SPEC];

        let raw = sys_program(&[0xEA, 0xEA, 0xEA]);
        let cfg = budget(SMALL_BUDGET);
        let state = EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, SPECS).unwrap();

        let start = usize::from(SYS_ENTRY);
        assert_eq!(&state.cpu.memory.mem[start..start + 3], &[0xA9, 0x00, 0x60]);
        assert!(
            state.cpu.memory.written.iter().all(|&w| !w),
            "pre-emulation patches must not set any `written` flag"
        );
    }

    /// T3.6.5: pre-emulation patches are applied *before* `snapshot = memory.mem.clone()`,
    /// so `snapshot` records the post-patch bytes and `detect_output_range` compares `end_addr`
    /// against the patched snapshot.
    #[test]
    fn patch_applied_before_snapshot() {
        const PATCH_TARGET: u16 = 0x3000;
        static PATCHES: [MemoryPatch; 1] = [MemoryPatch {
            addr: PatchAddr::Absolute(PATCH_TARGET),
            expected: Some(0x00),
            bytes: &[0x99],
        }];
        static SNAPSHOT_PATCH_SPEC: PackerSpec = PackerSpec {
            id: "test_patch_before_snapshot",
            name: "TestPatchBeforeSnapshot",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                dep_addr: Some(AddrSource::Const(STAGE2_ADDR)),
                ..Hints::NONE
            },
            patches: &PATCHES,
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };
        static SPECS: &[&PackerSpec] = &[&SNAPSHOT_PATCH_SPEC];

        // Stage 2 writes `$99` to `PATCH_TARGET` (`$3000`, which was already patched to `$99`
        // before `snapshot` was taken) and writes `$EA` to `OUTPUT_ADDR` (`$2000`), then jumps
        // to `OUTPUT_ADDR`. Because `snapshot[$3000] == $99`, `detect_output_range` sees
        // `mem[$3000] == snapshot[$3000]` and excludes `$3000` from the snapshot-diff `end_addr`.
        let mut stage2 = vec![
            0xA9, 0x99, // LDA #$99
            0x8D, 0x00, 0x30, // STA $3000
        ];
        stage2.extend_from_slice(&store_and_jump(OUTPUT_ADDR));
        let mut raw = vec![0u8; 12]; // no BASIC SYS line at $0801 so SYS clamp does not lower start_addr
        raw.extend_from_slice(&stage1_installing(&stage2));

        let cfg = UnpackConfig {
            forced_entry: Some(SYS_ENTRY),
            ..budget(SMALL_BUDGET)
        };
        let mut state = EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, SPECS).unwrap();
        assert_eq!(state.snapshot[usize::from(PATCH_TARGET)], 0x99);
        assert_eq!(state.cpu.memory.mem[usize::from(PATCH_TARGET)], 0x99);

        let result = state.run().expect("unpack succeeds");
        assert!(
            state.cpu.memory.written[usize::from(PATCH_TARGET)],
            "stage 2 wrote to $3000 at runtime"
        );
        assert_eq!(result.start_addr, OUTPUT_ADDR);
        assert_eq!(
            result.end_addr, OUTPUT_ADDR,
            "because snapshot[$3000] already held the patched byte $99, $3000 must not extend end_addr"
        );
        assert_eq!(result.entry_point, OUTPUT_ADDR);
    }

    /// T3.6.6: BASIC `SYS` entry-point parsing runs *before* spec patches are applied (`P3.6.1`),
    /// so a patch that overwrites the BASIC `SYS` token (`$9E`) or stub header does not break
    /// `SYS` entry resolution.
    #[test]
    fn sys_entry_parsed_before_patch() {
        // In `sys_program`, the BASIC `SYS` token `$9E` resides at `$0805` (`0x0801 + 4`).
        const SYS_TOKEN_ADDR: u16 = 0x0805;
        static PATCHES: [MemoryPatch; 1] = [MemoryPatch {
            addr: PatchAddr::Absolute(SYS_TOKEN_ADDR),
            expected: Some(0x9E),
            bytes: &[0x00],
        }];
        static CLOBBER_SYS_SPEC: PackerSpec = PackerSpec {
            id: "test_sys_parsed_before_patch",
            name: "TestSysParsedBeforePatch",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xEA],
            }],
            hints: Hints::NONE,
            patches: &PATCHES,
            resolve: None,
            hooks: None,
            fixture: &[0xEA],
            fixture_load_addr: SYS_ENTRY,
        };
        static SPECS: &[&PackerSpec] = &[&CLOBBER_SYS_SPEC];

        let raw = sys_program(&[0xEA]);
        let cfg = budget(SMALL_BUDGET);
        let state = EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, SPECS)
            .expect("SYS entry must be resolved before patch clobbers $0805");
        assert_eq!(state.cpu.registers.program_counter, SYS_ENTRY);
        assert_eq!(state.cpu.memory.mem[usize::from(SYS_TOKEN_ADDR)], 0x00);
        assert_eq!(state.snapshot[usize::from(SYS_TOKEN_ADDR)], 0x00);
    }

    /// T3.6.11: when [`Hints::exact_return`] is `true`, [`EmulationState::step_decompress`]
    /// ignores intermediate jumps to written or out-of-region addresses (`$3000`) and stops
    /// only when `pc` equals the exact target (`known_entry.unwrap_or(ret_addr)`).
    #[test]
    fn engine_exact_return_stops_only_at_exact_address() {
        // Stage 2 at `STAGE2_ADDR` (`$0340`):
        // 1. Writes a post-processing subroutine at `$3000..=$3005`:
        //      LDA #$42 ; STA $2000 ; RTS
        // 2. Writes initial unpacked byte at `$2001`:
        //      60 (`RTS`)
        // 3. Calls `JSR $3000` (which is >= `ret_addr` and outside `[load_addr, load_end)`),
        //    then executes `JMP $2000`.
        let stage2: [u8; 39] = [
            0xA9, 0xA9, 0x8D, 0x00, 0x30, // mem[$3000] = $A9 (LDA #imm)
            0xA9, 0x42, 0x8D, 0x01, 0x30, // mem[$3001] = $42
            0xA9, 0x8D, 0x8D, 0x02, 0x30, // mem[$3002] = $8D (STA abs)
            0xA9, 0x00, 0x8D, 0x03, 0x30, // mem[$3003] = $00
            0xA9, 0x20, 0x8D, 0x04, 0x30, // mem[$3004] = $20
            0xA9, 0x60, 0x8D, 0x05, 0x30, // mem[$3005] = $60 (RTS)
            0x8D, 0x01, 0x20, //             mem[$2001] = $60 (RTS)
            0x20, 0x00, 0x30, //             JSR $3000
            0x4C, 0x00, 0x20, //             JMP $2000
        ];
        let raw = sys_program(&stage1_installing(&stage2));

        static NON_EXACT_SPEC: PackerSpec = PackerSpec {
            id: "test_non_exact_return",
            name: "TestNonExactReturn",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                dep_addr: Some(AddrSource::Const(STAGE2_ADDR)),
                entry_point: Some(AddrSource::Const(OUTPUT_ADDR)),
                exact_return: false,
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };

        static EXACT_KNOWN_ENTRY_SPEC: PackerSpec = PackerSpec {
            id: "test_exact_known_entry",
            name: "TestExactKnownEntry",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                dep_addr: Some(AddrSource::Const(STAGE2_ADDR)),
                entry_point: Some(AddrSource::Const(OUTPUT_ADDR)),
                exact_return: true,
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };

        static EXACT_RET_ADDR_SPEC: PackerSpec = PackerSpec {
            id: "test_exact_ret_addr",
            name: "TestExactRetAddr",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                dep_addr: Some(AddrSource::Const(STAGE2_ADDR)),
                entry_point: None,
                exact_return: true,
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };

        let cfg = budget(SMALL_BUDGET);

        // 1. Without `exact_return`, `JSR $3000` exits immediately on entering written code at `$3000`,
        //    so `$3000`'s `STA $2000` (`$42`) has NOT executed yet.
        let mut non_exact_state =
            EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, &[&NON_EXACT_SPEC]).unwrap();
        let non_exact_res = non_exact_state.run().unwrap();
        assert_eq!(non_exact_state.cpu.registers.program_counter, 0x3000);
        assert_eq!(
            non_exact_state.cpu.memory.mem[usize::from(OUTPUT_ADDR)],
            0x00,
            "without exact_return, execution must stop at $3000 before STA $2000 runs"
        );
        assert_eq!(non_exact_res.entry_point, OUTPUT_ADDR);

        // 2. With `exact_return: true` and `entry_point = Some($2000)`, `JSR $3000` executes
        //    through `RTS` and stops only when `pc == $2000`, so `mem[$2000] == $42`.
        let mut exact_known_state =
            EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, &[&EXACT_KNOWN_ENTRY_SPEC])
                .unwrap();
        let exact_known_res = exact_known_state.run().unwrap();
        assert_eq!(exact_known_state.cpu.registers.program_counter, OUTPUT_ADDR);
        assert_eq!(exact_known_res.entry_point, OUTPUT_ADDR);
        assert_eq!(
            exact_known_state.cpu.memory.mem[usize::from(OUTPUT_ADDR)],
            0x42
        );

        // 3. With `exact_return: true` and `entry_point = None`, `exact_target` falls back to
        //    `ret_addr` (`forced_ret_addr = Some(0x2000)`), stopping only when `pc == $2000`.
        let cfg_ret = UnpackConfig {
            forced_ret_addr: Some(OUTPUT_ADDR),
            ..budget(SMALL_BUDGET)
        };
        let mut exact_ret_state =
            EmulationState::new_with_specs(&cfg_ret, None, &raw, PRG_LOAD, &[&EXACT_RET_ADDR_SPEC])
                .unwrap();
        let exact_ret_res = exact_ret_state.run().unwrap();
        assert_eq!(exact_ret_state.cpu.registers.program_counter, OUTPUT_ADDR);
        assert_eq!(exact_ret_res.entry_point, OUTPUT_ADDR);
        assert_eq!(
            exact_ret_state.cpu.memory.mem[usize::from(OUTPUT_ADDR)],
            0x42
        );
    }

    // ═══════════════════════ M4.1 tests (T4.1.1–T4.1.11) ═══════════════════════

    /// T4.1.1: `UnpackReport::exit_reason` records the exact [`ExitReason`] variant (and `pc`
    /// where applicable) for all five exit conditions.
    #[test]
    fn report_records_exit_reason_for_every_variant() {
        let cfg = budget(SMALL_BUDGET);

        // 1. WrittenCodeReached { pc }
        let written_res = run(&unpacker_program(&[]), PRG_LOAD, &cfg).unwrap();
        assert_eq!(
            written_res.report.exit_reason,
            ExitReason::WrittenCodeReached { pc: OUTPUT_ADDR }
        );

        // 2. KnownEntryReached { pc }: Phase 2 jumps to an unwritten `known_entry` at `$1800`
        //    after > 10 instructions.
        const KNOWN_ENTRY: u16 = 0x1800;
        static KNOWN_ENTRY_SPEC: PackerSpec = PackerSpec {
            id: "test_known_entry_exit",
            name: "TestKnownEntryExit",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                entry_point: Some(AddrSource::Const(KNOWN_ENTRY)),
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };
        let raw_known = sys_program(&stage1_installing(&store_and_jump_elsewhere(KNOWN_ENTRY)));
        let mut known_state =
            EmulationState::new_with_specs(&cfg, None, &raw_known, PRG_LOAD, &[&KNOWN_ENTRY_SPEC])
                .unwrap();
        let known_res = known_state.run().unwrap();
        assert_eq!(
            known_res.report.exit_reason,
            ExitReason::KnownEntryReached { pc: KNOWN_ENTRY }
        );

        // 3. LeftLoadedRegion { pc }: force `ret_addr = $3000` (`> OUTPUT_ADDR == $2000`) so
        //    `is_written_code` (`pc >= ret_addr`) is false when Phase 2 jumps to written `$2000`
        //    outside the loaded region (`$0801..$084B`).
        let cfg_left = UnpackConfig {
            forced_ret_addr: Some(0x3000),
            ..budget(SMALL_BUDGET)
        };
        let left_res = run(&unpacker_program(&[]), PRG_LOAD, &cfg_left).unwrap();
        assert_eq!(
            left_res.report.exit_reason,
            ExitReason::LeftLoadedRegion { pc: OUTPUT_ADDR }
        );

        // 4. RomExitVector { pc }: Phase 2 jumps to C64 KERNAL reset vector `$FCE2`.
        let raw_rom_exit = sys_program(&stage1_installing(&store_and_jump_elsewhere(0xFCE2)));
        let rom_exit_res = run(&raw_rom_exit, PRG_LOAD, &cfg).unwrap();
        assert_eq!(
            rom_exit_res.report.exit_reason,
            ExitReason::RomExitVector { pc: 0xFCE2 }
        );

        // 5. BasicRun: Phase 2 jumps to C64 BASIC RUN entry `$A7AE`.
        let raw_basic_run = sys_program(&stage1_installing(&store_and_jump_elsewhere(0xA7AE)));
        let basic_run_res = run(&raw_basic_run, PRG_LOAD, &cfg).unwrap();
        assert_eq!(basic_run_res.report.exit_reason, ExitReason::BasicRun);
    }

    /// T4.1.2: `UnpackReport` splits `find_depacker_instructions` and `decompress_instructions`
    /// such that their sum equals `instructions_executed` while preserving the single global
    /// instruction budget (Invariant 4).
    #[test]
    fn report_splits_phase1_and_phase2_instructions() {
        let result = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(
            result.report.find_depacker_instructions,
            STAGE1_INSTRUCTIONS
        );
        assert_eq!(result.report.decompress_instructions, STAGE2_INSTRUCTIONS);
        assert_eq!(
            result.report.find_depacker_instructions + result.report.decompress_instructions,
            result.instructions_executed
        );
    }

    /// T4.1.3: `UnpackReport::trace_tail` is empty by default (`config.trace == false`) and
    /// bounded by [`TRACE_TAIL_CAPACITY`] (`32`) when `config.trace == true`, ending with the
    /// PC of the last executed instruction before exit.
    #[test]
    fn report_trace_tail_is_opt_in_and_bounded() {
        // 50 NOPs + 17 stage-1 instructions + 3 stage-2 instructions = 70 instructions (> 64).
        let raw = unpacker_program(&[0xEA; 50]);

        let default_res = run(&raw, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert!(default_res.instructions_executed > 64);
        assert!(default_res.report.trace_tail.is_empty());

        let trace_cfg = UnpackConfig {
            trace: true,
            ..budget(SMALL_BUDGET)
        };
        let trace_res = run(&raw, PRG_LOAD, &trace_cfg).unwrap();
        assert!(!trace_res.report.trace_tail.is_empty());
        assert_eq!(trace_res.report.trace_tail.len(), TRACE_TAIL_CAPACITY);
        assert!(trace_res.report.trace_tail.len() <= TRACE_TAIL_CAPACITY);
        // Stage 2 is `LDA #$EA` ($0340, 2B), `STA $2000` ($0342, 3B), `JMP $2000` ($0345, 3B).
        assert_eq!(
            trace_res.report.trace_tail.last().copied(),
            Some(STAGE2_ADDR + 5)
        );
    }

    /// T4.1.4: a generic unpack without packer hints or clamps reports [`BoundSource::WriteDiff`]
    /// for both `start_source` and `end_source`.
    #[test]
    fn bound_source_generic_is_write_diff() {
        // Synthetic program with no BASIC `SYS` line (`forced_entry` starts at `SYS_ENTRY`),
        // writing only to `OUTPUT_ADDR` (`$2000`) and jumping there.
        let mut raw = vec![0x00; usize::from(SYS_ENTRY - PRG_LOAD)];
        raw.extend(stage1_installing(&store_and_jump(OUTPUT_ADDR)));
        let cfg = UnpackConfig {
            forced_entry: Some(SYS_ENTRY),
            ..budget(SMALL_BUDGET)
        };
        let result = run(&raw, PRG_LOAD, &cfg).unwrap();
        assert_eq!(result.report.start_source, BoundSource::WriteDiff);
        assert_eq!(result.report.end_source, BoundSource::WriteDiff);
        assert_eq!(result.packer, None);
    }

    /// T4.1.5: static [`Hints`] (`start_addr`, `end_addr`, `entry_point`) from a [`PackerSpec`]
    /// report [`BoundSource::PackerSpec`] with that spec's `id`.
    #[test]
    fn bound_source_packer_spec_override() {
        static STATIC_SPEC: PackerSpec = PackerSpec {
            id: "test_static",
            name: "TestStatic",
            priority: 1,
            systems: &[TargetSystem::C64],
            load_addr_max: None,
            signatures: &[Signature {
                anchor: Anchor::BasicStart { offset: 0x0C },
                pattern: sig![0xA9],
            }],
            hints: Hints {
                start_addr: Some(AddrSource::Const(0x0800)),
                end_addr: Some(AddrSource::Const(0x2100)),
                entry_point: Some(AddrSource::Const(0x1800)),
                ..Hints::NONE
            },
            patches: &[],
            resolve: None,
            hooks: None,
            fixture: &[0xA9],
            fixture_load_addr: SYS_ENTRY,
        };

        let raw = unpacker_program(&[]);
        let cfg = budget(SMALL_BUDGET);
        let mut state =
            EmulationState::new_with_specs(&cfg, None, &raw, PRG_LOAD, &[&STATIC_SPEC]).unwrap();
        let res = state.run().unwrap();

        assert_eq!(res.packer, Some("test_static"));
        assert_eq!(res.packer_name.as_deref(), Some("TestStatic"));
        assert_eq!(
            res.report.start_source,
            BoundSource::PackerSpec { id: "test_static" }
        );
        assert_eq!(
            res.report.end_source,
            BoundSource::PackerSpec { id: "test_static" }
        );
        assert_eq!(
            res.report.entry_source,
            BoundSource::PackerSpec { id: "test_static" }
        );
    }

    /// T4.1.6: [`AddrSource::PointerAfterRun`] sets `end_source = BoundSource::PointerAfterRun`
    /// when the pointer guard accepts and preserves the prior `end_source` when the guard rejects.
    #[test]
    fn bound_source_pointer_after_run_accepted_and_rejected() {
        let (mut mem, snapshot, written) = finish_fixture();
        let hints = PackerInfo {
            name: "TestPtr",
            end_addr_ptr: Some(0x00AE),
            ..empty_packer_info()
        };

        // 1. Accepted (`$2101` -> `$2100`, within `$2000 + 512`):
        let [lo, hi] = 0x2101u16.to_le_bytes();
        mem[0x00AE] = lo;
        mem[0x00AF] = hi;
        let accepted = finish(FinishContext {
            packer_id: Some("test_ptr"),
            hints: Some(&hints),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(accepted.end_addr, 0x2100);
        assert_eq!(
            accepted.report.end_source,
            BoundSource::PointerAfterRun { addr: 0x00AE }
        );

        // 2. Rejected (`$4000` -> `$3FFF`, outside `$2000 + 512`): keeps `WriteDiff` (or `PackerSpec`).
        let [rej_lo, rej_hi] = 0x4000u16.to_le_bytes();
        mem[0x00AE] = rej_lo;
        mem[0x00AF] = rej_hi;
        let rejected_write_diff = finish(FinishContext {
            packer_id: Some("test_ptr"),
            hints: Some(&hints),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(rejected_write_diff.end_addr, 0x2000);
        assert_eq!(
            rejected_write_diff.report.end_source,
            BoundSource::WriteDiff
        );

        let hints_with_spec_end = PackerInfo {
            end_addr: Some(0x2080),
            ..hints
        };
        let rejected_spec_end = finish(FinishContext {
            packer_id: Some("test_ptr"),
            hints: Some(&hints_with_spec_end),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(rejected_spec_end.end_addr, 0x2080);
        assert_eq!(
            rejected_spec_end.report.end_source,
            BoundSource::PackerSpec { id: "test_ptr" }
        );
    }

    /// T4.1.7 (Ruling R6): [`PackerHooks::refine`] sets `BoundSource::PackerRefine` only for
    /// bounds it actually modifies and preserves the previous [`BoundSource`] for unchanged bounds.
    #[test]
    fn bound_source_refine() {
        let (mem, snapshot, written) = finish_fixture();
        let hints = PackerInfo {
            start_addr: Some(0x1000),
            ..empty_packer_info()
        };

        // Case 1: `refine` modifies `end_addr` only; `start_source` stays `PackerSpec` and
        // `entry_source` stays `ExitPc`.
        let hooks_modify_end = MockHooks {
            step_hook: None,
            refine_hook: Some(Box::new(|_ctx, bounds| {
                bounds.end_addr = 0x2080;
            })),
        };
        let res_modified = finish(FinishContext {
            packer_id: Some("test_refine"),
            hints: Some(&hints),
            hooks: Some(&hooks_modify_end),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(
            res_modified.report.start_source,
            BoundSource::PackerSpec { id: "test_refine" }
        );
        assert_eq!(
            res_modified.report.end_source,
            BoundSource::PackerRefine { id: "test_refine" }
        );
        assert_eq!(res_modified.report.entry_source, BoundSource::ExitPc);

        // Case 2: `refine` leaves all bounds unchanged; `end_source` stays `WriteDiff`.
        let hooks_noop = MockHooks {
            step_hook: None,
            refine_hook: Some(Box::new(|_ctx, _bounds| {})),
        };
        let res_unchanged = finish(FinishContext {
            packer_id: Some("test_refine"),
            hints: Some(&hints),
            hooks: Some(&hooks_noop),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(
            res_unchanged.report.start_source,
            BoundSource::PackerSpec { id: "test_refine" }
        );
        assert_eq!(res_unchanged.report.end_source, BoundSource::WriteDiff);
        assert_eq!(res_unchanged.report.entry_source, BoundSource::ExitPc);
    }

    /// T4.1.8: BASIC `SYS` line clamping sets `start_source = BoundSource::SysLineClamp` when
    /// `start_addr > basic_start`, and `BasicRun` / `RomExitVector` exits that resolve the entry
    /// from the `SYS` line set `entry_source = BoundSource::SysLineLookup`.
    #[test]
    fn bound_source_sys_line() {
        // 1. `unpacker_program(&[])` has a BASIC `SYS 2061` stub at `PRG_LOAD` (`$0801`) in the
        //    loaded snapshot and writes only to `OUTPUT_ADDR` (`$2000`), so `detect_output_range`
        //    returns `($2000, $2000)` and Step 8 lowers `start_addr` to `$0801` (`SysLineClamp`).
        let clamped = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(clamped.start_addr, PRG_LOAD);
        assert_eq!(clamped.report.start_source, BoundSource::SysLineClamp);
        assert_eq!(clamped.report.end_source, BoundSource::WriteDiff);
        assert_eq!(clamped.report.entry_source, BoundSource::ExitPc);

        // 2. Emulation exits via `BasicRun` (`$A7AE`) and resolves `entry_point` from `10 SYS2061`.
        let raw_basic_run = sys_program(&stage1_installing(&store_and_jump_elsewhere(0xA7AE)));
        let basic_run_res = run(&raw_basic_run, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(basic_run_res.entry_point, SYS_ENTRY);
        assert_eq!(
            basic_run_res.report.entry_source,
            BoundSource::SysLineLookup
        );

        // 3. Emulation exits via `RomExitVector` (`$FCE2`) so `finish` step 8 replaces the
        //    mapped KERNAL ROM entry with the `SYS` target address.
        let raw_rom_exit = sys_program(&stage1_installing(&store_and_jump_elsewhere(0xFCE2)));
        let rom_exit_res = run(&raw_rom_exit, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(rom_exit_res.entry_point, SYS_ENTRY);
        assert_eq!(rom_exit_res.report.entry_source, BoundSource::SysLineLookup);
    }

    /// T4.1.9: Step 9 entry clamps set `start_source = BoundSource::EntryClamp` when
    /// `entry_point < start_addr` (`>= $0200`) and `end_source = BoundSource::EntryClamp`
    /// when `entry_point > end_addr`.
    #[test]
    fn bound_source_entry_clamp() {
        let (mem, snapshot, written) = finish_fixture();

        let low_clamp = finish(FinishContext {
            entry_point: 0x0400,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(low_clamp.start_addr, 0x0400);
        assert_eq!(low_clamp.report.start_source, BoundSource::EntryClamp);
        assert_eq!(low_clamp.report.end_source, BoundSource::WriteDiff);

        let high_clamp = finish(FinishContext {
            entry_point: 0x2800,
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(high_clamp.end_addr, 0x2800);
        assert_eq!(high_clamp.report.start_source, BoundSource::WriteDiff);
        assert_eq!(high_clamp.report.end_source, BoundSource::EntryClamp);
    }

    /// T4.1.10 (Ruling R6): forced entry reports [`BoundSource::Forced`], a plain exit PC
    /// reports [`BoundSource::ExitPc`], and an entry recovered from `find_entry_in_snapshot`
    /// reports [`BoundSource::SnapshotEntry`].
    #[test]
    fn bound_source_forced_and_exit_entries() {
        // 1. Forced entry via `UnpackConfig::forced_entry`
        let forced_cfg = UnpackConfig {
            forced_entry: Some(SYS_ENTRY),
            ..budget(SMALL_BUDGET)
        };
        let forced_res = run(&unpacker_program(&[]), PRG_LOAD, &forced_cfg).unwrap();
        assert_eq!(forced_res.report.entry_source, BoundSource::Forced);

        // 2. Plain exit PC (`config.forced_entry == None`)
        let exit_pc_res = run(&unpacker_program(&[]), PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(exit_pc_res.report.entry_source, BoundSource::ExitPc);

        // 3. `SnapshotEntry`: Phase 2 exits at `pc == ret_addr` (`$0800`, written) and
        //    `find_entry_in_snapshot` scans `snapshot` from `load_addr` (`$0801`) for a `JMP`
        //    (`$4C`) to `$1500` (`>= ret_addr`).
        let mut raw_snap = sys_program(&stage1_installing(&store_and_jump(0x0800)));
        // Append a `JMP $1500` (`4C 00 15`) inside the loaded snapshot so `find_entry_in_snapshot`
        // (which scans backward from the end of the loaded data) finds `$1500`.
        raw_snap.extend_from_slice(&[0x4C, 0x00, 0x15]);
        let snap_res = run(&raw_snap, PRG_LOAD, &budget(SMALL_BUDGET)).unwrap();
        assert_eq!(snap_res.entry_point, 0x1500);
        assert_eq!(snap_res.report.entry_source, BoundSource::SnapshotEntry);
    }

    /// T4.1.11: when multiple steps in [`finish`] write to the same bound (`WriteDiff` →
    /// `PackerSpec` → `PointerAfterRun` → `PackerRefine` → `SysLineClamp` → `EntryClamp`),
    /// each bound's [`BoundSource`] records the **last** step that wrote it.
    #[test]
    fn bound_source_last_writer_wins() {
        let (mut mem, snapshot, written) = finish_fixture();
        // Put a BASIC `SYS 2061` stub at `PRG_LOAD` (`$0801`).
        let basic_stub = sys_program(&[]);
        mem[usize::from(PRG_LOAD)..usize::from(PRG_LOAD) + basic_stub.len()]
            .copy_from_slice(&basic_stub);
        // Pointer at `$00AE` reports `$2101` (`$2100`).
        let [lo, hi] = 0x2101u16.to_le_bytes();
        mem[0x00AE] = lo;
        mem[0x00AF] = hi;

        // Chain A:
        // - Step 1 (`WriteDiff`): `start = $1000`, `end = $2000`, `entry = $1000` (`ExitPc`)
        // - Step 2 (`PackerSpec`): `start = $1100`
        // - Step 3 (`PackerSpec`): `end = $2050`
        // - Step 4 (`PointerAfterRun`): `end = $2100`
        // - Step 5 (`PackerSpec`): `entry = $1500`
        // - Step 7 (`PackerRefine`): sets `start = $1200` (`> basic_start`), leaves `end = $2100`,
        //   sets `entry = $0801` (`== basic_start`)
        // - Step 8 (`SysLineClamp` / `SysLineLookup`): lowers `start` to `$0801` (`SysLineClamp`)
        //   and replaces `entry` with `$080D` (`SysLineLookup`); `end` stays `$2100` (`PointerAfterRun`).
        let hints = PackerInfo {
            start_addr: Some(0x1100),
            end_addr: Some(0x2050),
            end_addr_ptr: Some(0x00AE),
            entry_point: Some(0x1500),
            ..empty_packer_info()
        };
        let hooks_a = MockHooks {
            step_hook: None,
            refine_hook: Some(Box::new(|_ctx, bounds| {
                bounds.start_addr = 0x1200;
                bounds.entry_point = PRG_LOAD;
            })),
        };
        let res_a = finish(FinishContext {
            packer_id: Some("chain_a"),
            hints: Some(&hints),
            hooks: Some(&hooks_a),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_a.start_addr, PRG_LOAD);
        assert_eq!(res_a.end_addr, 0x2100);
        assert_eq!(res_a.entry_point, SYS_ENTRY);
        assert_eq!(res_a.report.start_source, BoundSource::SysLineClamp);
        assert_eq!(
            res_a.report.end_source,
            BoundSource::PointerAfterRun { addr: 0x00AE }
        );
        assert_eq!(res_a.report.entry_source, BoundSource::SysLineLookup);

        // Chain B:
        // Same as Chain A through Step 8 (`start` lowered to `$0801` by `SysLineClamp`), except
        // `refine` sets `entry = $0400` (`< $0801`) and `end = $2200`, so Step 9 (`EntryClamp`)
        // lowers `start` further from `$0801` down to `$0400` (`EntryClamp`), while `end` keeps
        // `PackerRefine` and `entry` keeps `PackerRefine`.
        let hooks_b = MockHooks {
            step_hook: None,
            refine_hook: Some(Box::new(|_ctx, bounds| {
                bounds.start_addr = 0x1200;
                bounds.end_addr = 0x2200;
                bounds.entry_point = 0x0400;
            })),
        };
        let res_b = finish(FinishContext {
            packer_id: Some("chain_b"),
            hints: Some(&hints),
            hooks: Some(&hooks_b),
            ..test_finish_ctx(&mem, &snapshot, &written)
        })
        .unwrap();
        assert_eq!(res_b.start_addr, 0x0400);
        assert_eq!(res_b.end_addr, 0x2200);
        assert_eq!(res_b.entry_point, 0x0400);
        assert_eq!(res_b.report.start_source, BoundSource::EntryClamp);
        assert_eq!(
            res_b.report.end_source,
            BoundSource::PackerRefine { id: "chain_b" }
        );
        assert_eq!(
            res_b.report.entry_source,
            BoundSource::PackerRefine { id: "chain_b" }
        );
    }
}
