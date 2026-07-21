//! Decompression execution engine driving 2-phase MOS 6502 emulation.

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::memory::Bus;

use super::bus::{C64Bus, is_basic_rom_mapped, is_io_mapped, is_kernal_rom_mapped};
use super::detector::{detect_output_range, find_entry_in_snapshot};
use super::{UnpackConfig, UnpackError, UnpackResult, find_sys_address};

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
        if raw_data.is_empty() {
            return Err(UnpackError::EmptyData);
        }

        let system = self
            .config
            .target_system
            .clone()
            .unwrap_or_else(crate::state::types::default_system);

        let basic_start = get_basic_start(load_addr);

        let basic_rom = self
            .config
            .basic_rom
            .clone()
            .or_else(|| crate::assets::default_basic_rom(&system));
        let kernal_rom = self
            .config
            .kernal_rom
            .clone()
            .or_else(|| crate::assets::default_kernal_rom(&system));
        let char_rom = self
            .config
            .char_rom
            .clone()
            .or_else(|| crate::assets::default_char_rom(&system));

        // Set up memory
        let mut memory = C64Bus::new(system.clone(), basic_rom, kernal_rom, char_rom);

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
        let entry = if let Some(forced) = self.config.forced_entry {
            forced
        } else {
            find_sys_address(&memory.mem, basic_start)
                .or_else(|| find_sys_address(&memory.mem, system.default_basic_start()))
                .ok_or(UnpackError::NoEntryPoint)?
        };

        let ret_addr = self
            .config
            .forced_ret_addr
            .unwrap_or_else(|| load_addr.min(system.ram_start()));
        let load_end = (load_addr as usize + data_len).min(0x10000) as u16;
        let mut packer = crate::packers::detect_packer(&memory.mem, load_addr, load_end);

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
            if total_instructions >= self.config.max_instructions {
                return Err(UnpackError::Phase1Timeout);
            }

            let pc = cpu.registers.program_counter;
            cpu.memory.current_pc = pc;

            if let Some(ref mut p) = packer {
                p.on_step(&mut cpu, 1);
            }

            let is_dep_addr = cpu.memory.in_phase2
                || (if let Some(ref p) = packer
                    && let Some(known_dep) = p.info().dep_addr
                    && known_dep >= ret_addr
                {
                    pc == known_dep
                } else {
                    pc < ret_addr && pc != 0x0000 && pc != 0x0002
                });

            if is_dep_addr {
                dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
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
                    dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
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
                    let entry_point =
                        find_sys_address(&cpu.memory.mem, basic_start).unwrap_or(basic_start);
                    dep_addr = self.config.forced_dep_addr.unwrap_or(pc);
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
                && let Some(cb) = self.progress_callback
            {
                cb(total_instructions);
            }
        }

        // -----------------------------------------------------------------------
        // Phase 2: Run decompression
        // Continues from where Phase 1 left off.
        // -----------------------------------------------------------------------
        let info = packer.as_ref().map(|p| p.info());
        let known_entry = info.as_ref().and_then(|i| i.entry_point);
        cpu.memory.in_phase2 = true;

        loop {
            if total_instructions >= self.config.max_instructions {
                return Err(UnpackError::Phase2Timeout);
            }

            if let Some(ref mut p) = packer {
                p.on_step(&mut cpu, 2);
            }

            let pc = cpu.registers.program_counter;
            cpu.memory.current_pc = pc;
            let mut exit_triggered = false;
            let basic_mapped = is_basic_rom_mapped(&cpu.memory.mem, &system);
            let kernal_mapped = is_kernal_rom_mapped(&cpu.memory.mem, &system);
            let io_mapped = is_io_mapped(&cpu.memory.mem, &system);

            let in_rom_or_io = (is_in_basic_rom(pc, &system) && basic_mapped)
                || (system.is_in_io(pc) && io_mapped)
                || (is_in_kernal_rom(pc, &system) && kernal_mapped);

            let is_written_code = pc >= ret_addr
                && (pc as usize) < system.memory_boundaries()[0]
                && !in_rom_or_io
                && (cpu.memory.written[pc as usize] || cpu.memory.written_phase2[pc as usize]);

            if let Some(ke) = known_entry {
                let ke_hit = pc == ke;
                if (ke_hit && total_instructions > 10) || is_written_code {
                    exit_triggered = true;
                }
            } else if is_written_code {
                exit_triggered = true;
            }

            if exit_triggered {
                let entry_point = if (basic_start..=basic_start.saturating_add(0x10)).contains(&pc)
                    || system.is_basic_exec_entry(pc)
                {
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
                && let Some(cb) = self.progress_callback
            {
                cb(total_instructions);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// System & zero-page initialization helpers
// ---------------------------------------------------------------------------

fn get_basic_start(load_addr: u16) -> u16 {
    if matches!(load_addr, 0x0800 | 0x07C0) {
        load_addr.wrapping_add(1)
    } else {
        load_addr
    }
}

fn is_in_basic_rom(pc: u16, system: &crate::state::types::System) -> bool {
    system.is_in_basic_rom(pc)
}

fn is_in_kernal_rom(pc: u16, system: &crate::state::types::System) -> bool {
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
            mem.mem[addr as usize] = 0x20;
        }
    }
}

// ---------------------------------------------------------------------------
// ROM interception
// ---------------------------------------------------------------------------

fn handle_rom_entry(
    cpu: &mut CPU<C64Bus, Nmos6502>,
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
    let is_c64 = system.is_c64();

    // BASIC RUN detection (check even if written, as packers jump to $A7AE to execute BASIC RUN)
    if is_c64
        && matches!(
            pc,
            0xA7AE | 0xA7B1 | 0xA7EA | 0xA474 | 0xA483 | 0xA533 | 0xA871 | 0xA888 | 0xA8BC
        )
    {
        return RomAction::BasicRun;
    }

    // If user code was written here (depacker at $FF00+, etc.) AND the ROM
    // at this address is not currently mapped, let it run as RAM code.
    if cpu.memory.written[pc as usize] {
        let rom_mapped_here = (in_basic && basic_mapped) || (in_kernal && kernal_mapped);
        if !rom_mapped_here {
            return RomAction::Continue;
        }
    }

    // BASIC ROM region
    if in_basic {
        if !basic_mapped {
            return RomAction::Continue; // RAM is visible, not ROM
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
            if reported_end > start_addr
                && reported_end.saturating_sub(1) <= end_addr.saturating_add(512)
            {
                end_addr = reported_end.saturating_sub(1);
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

    let basic_mapped = is_basic_rom_mapped(mem, system);
    let kernal_mapped = is_kernal_rom_mapped(mem, system);
    let is_rom_entry = (is_in_basic_rom(entry_point, system) && basic_mapped)
        || (is_in_kernal_rom(entry_point, system) && kernal_mapped);

    if let Some(sys_ep) = find_sys_address(mem, load_addr) {
        if is_rom_entry || (load_addr..=load_addr.saturating_add(0x10)).contains(&entry_point) {
            entry_point = sys_ep;
        }
        if start_addr > load_addr {
            start_addr = load_addr;
        }
    }

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
