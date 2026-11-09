//! Custom memory bus and ROM banking logic for 6502 unpacker emulation.

use super::cia::CiaState;
use mos6502::memory::Bus;

/// Custom memory bus for the unpacker.
///
/// Provides flat 64 KB RAM with per-byte write tracking and I/O suppression.
#[derive(Clone)]
pub struct C64Bus {
    /// Flat 64 KB memory.
    pub(crate) mem: Vec<u8>,
    /// Per-byte write tracking across all phases.
    pub(crate) written: Vec<bool>,
    /// Per-byte write tracking during Phase 2.
    pub(crate) written_phase2: Vec<bool>,
    /// Target system.
    pub(crate) system: crate::state::types::System,
    /// Total CPU cycles executed (for VIC-II raster simulation).
    pub(crate) total_cycles: u64,
    /// Shadow byte for VIC-II $D011 register.
    pub(crate) shadow_d011: u8,
    /// Whether emulation is in Phase 2.
    pub(crate) in_phase2: bool,
    /// Optional 8 KB BASIC ROM image (`$A000`–`$BFFF`).
    pub(crate) basic_rom: Option<Vec<u8>>,
    /// Optional 8 KB KERNAL ROM image (`$E000`–`$FFFF`).
    pub(crate) kernal_rom: Option<Vec<u8>>,
    /// Optional 4 KB Character ROM image (`$D000`–`$DFFF`).
    pub(crate) char_rom: Option<Vec<u8>>,
    /// Current program counter for Phase 2 write-tracking trigger.
    pub(crate) current_pc: u16,
    /// CIA 1 chip state ($DC00–$DC0F).
    pub(crate) cia1: CiaState,
    /// CIA 2 chip state ($DD00–$DD0F).
    pub(crate) cia2: CiaState,
    /// Cached `system.is_c64()` flag to avoid matching `TargetSystem` on every byte access.
    pub is_c64: bool,
    /// Cached `system.ram_start()` to avoid matching `TargetSystem` on every byte write.
    pub ram_start: u16,
    /// Whether any CIA 1 or CIA 2 timer is currently active.
    pub cia_timers_active: bool,
    /// 16-bit mask indexed by `addr >> 12` indicating 4 KB pages that require C64 ROM/IO read dispatch.
    pub c64_read_mask: u16,
    /// Whether C64 I/O space (`$D000..=$DFFF`) is currently mapped for writes.
    pub c64_io_mapped: bool,
    /// 16-bit mask indexed by `pc >> 12` indicating 4 KB pages where ROM interception can fire.
    pub rom_exec_mask: u16,
}

/// Alias for backward compatibility and API clarity.
pub type UnpackerMemory = C64Bus;

impl C64Bus {
    /// Creates a new zeroed 64 KB memory bus.
    #[must_use]
    pub fn new(
        system: crate::state::types::System,
        basic_rom: Option<Vec<u8>>,
        kernal_rom: Option<Vec<u8>>,
        char_rom: Option<Vec<u8>>,
    ) -> Self {
        let is_c64 = system.is_c64();
        let ram_start = system.ram_start();
        let mut bus = Self {
            mem: vec![0u8; 0x1_0000],
            written: vec![false; 0x1_0000],
            written_phase2: vec![false; 0x1_0000],
            system,
            total_cycles: 0,
            shadow_d011: 0x1B,
            in_phase2: false,
            basic_rom,
            kernal_rom,
            char_rom,
            current_pc: 0,
            cia1: CiaState::default(),
            cia2: CiaState::default(),
            is_c64,
            ram_start,
            cia_timers_active: false,
            c64_read_mask: 0,
            c64_io_mapped: false,
            rom_exec_mask: 0,
        };
        bus.sync_c64_banking();
        bus
    }

    /// Synchronizes cached 4 KB page bitmasks (`c64_read_mask`, `c64_io_mapped`,
    /// and `rom_exec_mask`) with the current processor port (`$0000`/`$0001`) state.
    #[inline(always)]
    pub fn sync_c64_banking(&mut self) {
        if self.is_c64 {
            let bank = get_c64_bank(&self.mem);
            let basic = (bank & 0x03) == 0x03;
            let kernal = (bank & 0x02) != 0;
            let d000 = (bank & 0x03) != 0;
            let io = (bank & 0x04) != 0 && d000;
            let mut read_mask = 0u16;
            // Always include 0xA on C64 so BASIC RUN entries ($A474..=$A8BC) are intercepted
            // even when BASIC ROM is unmapped.
            let mut exec_mask = 1u16 << 0xA;
            if basic {
                read_mask |= (1 << 0xA) | (1 << 0xB);
                exec_mask |= (1 << 0xA) | (1 << 0xB);
            }
            if d000 {
                read_mask |= 1 << 0xD;
            }
            if kernal {
                read_mask |= (1 << 0xE) | (1 << 0xF);
                exec_mask |= (1 << 0xE) | (1 << 0xF);
            }
            self.c64_read_mask = read_mask;
            self.c64_io_mapped = io;
            self.rom_exec_mask = exec_mask;
        } else {
            self.c64_read_mask = 0;
            self.c64_io_mapped = false;
            let mut exec_mask = 0u16;
            for nibble in 0u16..16 {
                let addr = nibble << 12;
                if self.system.is_in_basic_rom(addr) || self.system.is_in_kernal_rom(addr) {
                    exec_mask |= 1u16 << nibble;
                }
            }
            self.rom_exec_mask = exec_mask;
        }
    }

    /// Returns `true` if `pc` lies in a 4 KB page where ROM interception can fire.
    #[must_use]
    #[inline(always)]
    pub fn is_rom_exec_candidate(&self, pc: u16) -> bool {
        (self.rom_exec_mask & (1u16 << (pc >> 12))) != 0
    }

    /// Synchronizes `cia_timers_active` with the current CIA 1 and CIA 2 control registers.
    #[inline(always)]
    pub fn sync_cia_timers_active(&mut self) {
        self.cia_timers_active = self.cia1.has_active_timer() || self.cia2.has_active_timer();
    }

    /// Advances `total_cycles` and active C64 CIA timers by `cycles` CPU clock cycles.
    #[inline(always)]
    pub fn step_cycles(&mut self, cycles: u32) {
        self.total_cycles = self.total_cycles.wrapping_add(u64::from(cycles));
        if self.cia_timers_active {
            self.step_cia_timers(cycles);
        }
    }

    /// Steps CIA 1 and CIA 2 timers on the cold path when `cia_timers_active` is `true`,
    /// then refreshes `cia_timers_active` in case a one-shot timer underflowed.
    #[cold]
    #[inline(never)]
    fn step_cia_timers(&mut self, cycles: u32) {
        self.cia1.step_cycles(cycles);
        self.cia2.step_cycles(cycles);
        self.sync_cia_timers_active();
    }

    /// Cold-path read handler for C64 banked regions (`$A000`–`$BFFF` BASIC ROM,
    /// `$D000`–`$DFFF` I/O registers or Character ROM, and `$E000`–`$FFFF` KERNAL ROM).
    #[cold]
    #[inline(never)]
    fn get_byte_c64_high(&mut self, addr: u16) -> u8 {
        let bank = get_c64_bank(&self.mem);
        let a = addr as usize;
        if addr < 0xC000 {
            if (bank & 0x03) == 0x03
                && let Some(ref rom) = self.basic_rom
                && let Some(&val) = rom.get(a - 0xA000)
            {
                return val;
            }
            return self.mem[a];
        }
        if addr >= 0xE000 {
            if (bank & 0x02) != 0
                && let Some(ref rom) = self.kernal_rom
                && let Some(&val) = rom.get(a - 0xE000)
            {
                return val;
            }
            return self.mem[a];
        }
        if (0xD000..=0xDFFF).contains(&addr) {
            if (bank & 0x04) != 0 {
                if (0xDC00..=0xDC0F).contains(&addr) {
                    return self.cia1.read_reg((addr & 0x0F) as u8);
                }
                if (0xDD00..=0xDD0F).contains(&addr) {
                    return self.cia2.read_reg((addr & 0x0F) as u8);
                }
                // PAL C64 VIC-II timing: 63 CPU cycles per raster line, 312 lines per frame.
                // $D012 returns the lower 8 bits of the current raster line.
                // $D011 bit 7 returns the 9th bit (MSB, 1 when line >= 256) and
                // preserves control bits [6:0] from shadow_d011.
                if addr == 0xD012 {
                    let current_line = (self.total_cycles / 63) % 312;
                    return (current_line & 0xFF) as u8;
                }
                if addr == 0xD011 {
                    let current_line = (self.total_cycles / 63) % 312;
                    let msb = if current_line >= 256 { 0x80 } else { 0x00 };
                    return (self.shadow_d011 & 0x7F) | msb;
                }
                return 0;
            }
            if let Some(ref rom) = self.char_rom
                && let Some(&val) = rom.get(a - 0xD000)
            {
                return val;
            }
        }
        self.mem[a]
    }

    /// Cold-path write handler for C64 mapped I/O space (`$D000`–`$DFFF`).
    ///
    /// Routes register writes to CIA 1 (`$DC00`–`$DC0F`), CIA 2 (`$DD00`–`$DD0F`),
    /// or VIC-II `$D011` (`shadow_d011`), and refreshes `cia_timers_active` when
    /// a CIA control register (`$0E` or `$0F`) is modified.
    #[cold]
    #[inline(never)]
    fn set_byte_c64_io(&mut self, addr: u16, value: u8) {
        if (0xDC00..=0xDC0F).contains(&addr) {
            let reg = (addr & 0x0F) as u8;
            self.cia1.write_reg(reg, value);
            if reg >= 0x0E {
                self.sync_cia_timers_active();
            }
        } else if (0xDD00..=0xDD0F).contains(&addr) {
            let reg = (addr & 0x0F) as u8;
            self.cia2.write_reg(reg, value);
            if reg >= 0x0E {
                self.sync_cia_timers_active();
            }
        } else if addr == 0xD011 {
            self.shadow_d011 = value;
        }
    }
}

impl Bus for C64Bus {
    #[inline(always)]
    fn get_byte(&mut self, addr: u16) -> u8 {
        self.step_cycles(1);
        if (self.c64_read_mask & (1u16 << (addr >> 12))) != 0 {
            return self.get_byte_c64_high(addr);
        }
        self.mem[addr as usize]
    }

    #[inline(always)]
    fn set_byte(&mut self, addr: u16, val: u8) {
        self.step_cycles(1);
        // When C64 I/O space is mapped (CHAREN=1 and LORAM=1 or HIRAM=1), writes to $D000–$DFFF
        // target I/O chip registers (VIC-II, SID, CIA 1, CIA 2) instead of RAM.
        // Do not mutate `self.mem[a]` here. If I/O writes update RAM, border flashes
        // (such as `INC $D020`) corrupt the RAM under I/O space. Decompression then fails
        // when the depacker maps RAM in and reads those addresses.
        if self.c64_io_mapped && (addr & 0xF000) == 0xD000 {
            self.set_byte_c64_io(addr, val);
            return;
        }

        let a = addr as usize;
        self.mem[a] = val;
        self.written[a] = true;
        if addr <= 0x0001 && self.is_c64 {
            self.sync_c64_banking();
        }

        // Bus-driven stage flip: when code below `ram_start` (such as a zero-page,
        // stack, or low-memory depacker stub) writes at or above `ram_start`, the bus
        // latches `in_phase2 = true` and records the write in `written_phase2`.
        // `Phase::FindDepacker` reads `in_phase2` to enter `Phase::Decompress`.
        let ram_start = self.ram_start;
        if !self.in_phase2 && self.current_pc < ram_start && addr >= ram_start {
            self.in_phase2 = true;
        }

        if self.in_phase2 {
            self.written_phase2[a] = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Memory banking helpers
// ---------------------------------------------------------------------------

/// Computes the effective 3-bit C64 PLA bank configuration (`CHAREN`, `HIRAM`, `LORAM`)
/// from the 6510 data direction register (`$0000`) and processor port (`$0001`).
/// Input bits (`DDR == 0`) float high to `1` via internal pull-ups.
#[must_use]
pub(crate) fn get_c64_bank(mem: &[u8]) -> u8 {
    (mem[0x01] & mem[0x00]) | (!mem[0x00] & 0x07)
}

/// Returns `true` if BASIC ROM is currently mapped into memory on `system`.
#[must_use]
pub(crate) fn is_basic_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        let bank = get_c64_bank(mem);
        (bank & 0x01) != 0 && (bank & 0x02) != 0
    } else {
        true
    }
}

/// Returns `true` if KERNAL ROM is currently mapped into memory on `system`.
#[must_use]
pub(crate) fn is_kernal_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        let bank = get_c64_bank(mem);
        (bank & 0x02) != 0
    } else {
        true
    }
}

/// Returns `true` if I/O register space (`$D000`–`$DFFF`) is currently mapped on `system`.
#[must_use]
pub(crate) fn is_io_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        let bank = get_c64_bank(mem);
        (bank & 0x04) != 0 && ((bank & 0x01) != 0 || (bank & 0x02) != 0)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::TargetSystem;

    #[test]
    fn test_memory_write_tracking() {
        let mut mem = C64Bus::new(crate::state::types::default_system(), None, None, None);
        assert!(!mem.written[0x1000]);
        mem.set_byte(0x1000, 0x42);
        assert!(mem.written[0x1000]);
        assert_eq!(mem.get_byte(0x1000), 0x42);
    }

    #[test]
    fn test_memory_io_suppression() {
        let mut mem = C64Bus::new(crate::state::types::default_system(), None, None, None);
        // Set PLA bank register to default C64 value ($37) where I/O is visible
        mem.mem[0x01] = 0x37;
        mem.sync_c64_banking();
        mem.set_byte(0xD020, 0xFF); // VIC border color
        assert_eq!(mem.get_byte(0xD020), 0x00); // Reads return 0
        assert!(!mem.written[0xD020]); // Write not tracked
    }

    /// Verifies that writing at or above `system.ram_start()` while
    /// `current_pc < system.ram_start()` flips `in_phase2` to `true` and marks
    /// `written_phase2`. Writes from high PC or to addresses below `ram_start`
    /// leave `in_phase2` false.
    #[test]
    fn bus_write_above_ram_start_from_low_pc_flips_phase2() {
        let mut bus = C64Bus::new(TargetSystem::C64, None, None, None);
        bus.mem[0x00] = 0x2F;
        bus.mem[0x01] = 0x37;
        bus.sync_c64_banking();
        let ram_start = TargetSystem::C64.ram_start();
        assert_eq!(ram_start, 0x0800);
        assert!(!bus.in_phase2);

        // High PC ($0800 >= ram_start) writing to high RAM ($0800): does NOT flip in_phase2.
        bus.current_pc = ram_start;
        bus.set_byte(ram_start, 0x11);
        assert!(!bus.in_phase2);
        assert!(bus.written[usize::from(ram_start)]);
        assert!(!bus.written_phase2[usize::from(ram_start)]);

        // Low PC ($07FF < ram_start) writing below ram_start ($07FF): does NOT flip in_phase2.
        bus.current_pc = ram_start - 1;
        bus.set_byte(ram_start - 1, 0x22);
        assert!(!bus.in_phase2);
        assert!(bus.written[usize::from(ram_start - 1)]);
        assert!(!bus.written_phase2[usize::from(ram_start - 1)]);

        // Low PC writing to mapped I/O ($D020): suppressed, so does NOT flip in_phase2.
        bus.set_byte(0xD020, 0x01);
        assert!(!bus.in_phase2);
        assert!(!bus.written[0xD020]);

        // Low PC ($07FF < ram_start) writing at ram_start ($0800): flips in_phase2 to true
        // and records the write in both `written` and `written_phase2`.
        bus.set_byte(ram_start, 0x33);
        assert!(bus.in_phase2);
        assert!(bus.written[usize::from(ram_start)]);
        assert!(bus.written_phase2[usize::from(ram_start)]);

        // Once latched, subsequent writes (even from high PC to low RAM) mark `written_phase2`.
        bus.current_pc = 0x1000;
        bus.set_byte(0x0300, 0x44);
        assert!(bus.in_phase2);
        assert!(bus.written_phase2[0x0300]);

        // System-aware boundary check on C128 (ram_start = $1C00).
        let mut c128_bus = C64Bus::new(TargetSystem::C128, None, None, None);
        let c128_ram_start = TargetSystem::C128.ram_start();
        assert_eq!(c128_ram_start, 0x1C00);
        c128_bus.current_pc = 0x0800;
        c128_bus.set_byte(c128_ram_start - 1, 0x55);
        assert!(!c128_bus.in_phase2);
        c128_bus.set_byte(c128_ram_start, 0x66);
        assert!(c128_bus.in_phase2);
        assert!(c128_bus.written_phase2[usize::from(c128_ram_start)]);
    }

    #[test]
    fn c64_banking_masks_track_processor_port_transitions() {
        let mut bus = C64Bus::new(
            TargetSystem::C64,
            Some(vec![0xAA; 0x2000]),
            Some(vec![0xBB; 0x2000]),
            Some(vec![0xCC; 0x1000]),
        );
        bus.mem[0xA000] = 0x11;
        bus.mem[0xD000] = 0x22;
        bus.mem[0xE000] = 0x33;
        bus.set_byte(0x0000, 0x2F);

        // 1. port = 0x37 (BASIC + KERNAL + I/O)
        bus.set_byte(0x0001, 0x37);
        assert_eq!(
            bus.c64_read_mask,
            (1 << 0xA) | (1 << 0xB) | (1 << 0xD) | (1 << 0xE) | (1 << 0xF)
        );
        assert!(bus.c64_io_mapped);
        assert_eq!(
            bus.rom_exec_mask,
            (1 << 0xA) | (1 << 0xB) | (1 << 0xE) | (1 << 0xF)
        );
        assert!(bus.is_rom_exec_candidate(0xA000));
        assert!(bus.is_rom_exec_candidate(0xE000));
        assert!(!bus.is_rom_exec_candidate(0x1000));
        assert_eq!(bus.get_byte(0xA000), 0xAA);
        assert_eq!(bus.get_byte(0xE000), 0xBB);
        assert_eq!(bus.get_byte(0xD020), 0x00);

        // 2. port = 0x36 (KERNAL + I/O, BASIC unmapped)
        bus.set_byte(0x0001, 0x36);
        assert_eq!(bus.c64_read_mask, (1 << 0xD) | (1 << 0xE) | (1 << 0xF));
        assert!(bus.c64_io_mapped);
        assert_eq!(bus.rom_exec_mask, (1 << 0xA) | (1 << 0xE) | (1 << 0xF));
        assert_eq!(bus.get_byte(0xA000), 0x11);
        assert_eq!(bus.get_byte(0xE000), 0xBB);

        // 3. port = 0x35 (I/O only, BASIC + KERNAL unmapped)
        bus.set_byte(0x0001, 0x35);
        assert_eq!(bus.c64_read_mask, 1 << 0xD);
        assert!(bus.c64_io_mapped);
        assert_eq!(bus.rom_exec_mask, 1 << 0xA);
        assert_eq!(bus.get_byte(0xA000), 0x11);
        assert_eq!(bus.get_byte(0xE000), 0x33);
        assert_eq!(bus.get_byte(0xD020), 0x00);

        // 4. port = 0x34 (All RAM)
        bus.set_byte(0x0001, 0x34);
        assert_eq!(bus.c64_read_mask, 0);
        assert!(!bus.c64_io_mapped);
        assert_eq!(bus.rom_exec_mask, 1 << 0xA);
        assert_eq!(bus.get_byte(0xA000), 0x11);
        assert_eq!(bus.get_byte(0xD000), 0x22);
        assert_eq!(bus.get_byte(0xE000), 0x33);
        bus.set_byte(0xD000, 0x77);
        assert_eq!(bus.mem[0xD000], 0x77);
        assert!(bus.written[0xD000]);

        // 5. port = 0x33 (BASIC + KERNAL + Character ROM at $D000..=$DFFF)
        bus.set_byte(0x0001, 0x33);
        assert_eq!(
            bus.c64_read_mask,
            (1 << 0xA) | (1 << 0xB) | (1 << 0xD) | (1 << 0xE) | (1 << 0xF)
        );
        assert!(!bus.c64_io_mapped);
        assert_eq!(
            bus.rom_exec_mask,
            (1 << 0xA) | (1 << 0xB) | (1 << 0xE) | (1 << 0xF)
        );
        assert_eq!(bus.get_byte(0xD000), 0xCC);
        bus.set_byte(0xD000, 0x88);
        assert_eq!(bus.mem[0xD000], 0x88);

        // 6. port = 0x30 (All RAM with CHAREN=0, HIRAM=0, LORAM=0)
        bus.set_byte(0x0001, 0x30);
        assert_eq!(bus.c64_read_mask, 0);
        assert!(!bus.c64_io_mapped);
        assert_eq!(bus.rom_exec_mask, 1 << 0xA);

        // 7. None ROM fallthrough verification
        let mut no_rom_bus = C64Bus::new(TargetSystem::C64, None, None, None);
        no_rom_bus.mem[0xA000] = 0x41;
        no_rom_bus.mem[0xD000] = 0x42;
        no_rom_bus.mem[0xE000] = 0x43;
        no_rom_bus.set_byte(0x0000, 0x2F);
        no_rom_bus.set_byte(0x0001, 0x33);
        assert_eq!(no_rom_bus.get_byte(0xA000), 0x41);
        assert_eq!(no_rom_bus.get_byte(0xD000), 0x42);
        assert_eq!(no_rom_bus.get_byte(0xE000), 0x43);

        // 8. Non-C64 systems verification
        let non_c64_systems = [
            TargetSystem::C128,
            TargetSystem::Vic20,
            TargetSystem::Plus4,
            TargetSystem::Pet20,
            TargetSystem::Custom("Commodore 64".into()),
        ];
        for system in non_c64_systems {
            let sys_bus = C64Bus::new(system.clone(), None, None, None);
            assert!(!sys_bus.is_c64, "{system}");
            assert_eq!(sys_bus.c64_read_mask, 0, "{system}");
            assert!(!sys_bus.c64_io_mapped, "{system}");
            assert_eq!(sys_bus.ram_start, system.ram_start(), "{system}");
            for nibble in 0u16..16 {
                let addr = nibble << 12;
                let expected = system.is_in_basic_rom(addr) || system.is_in_kernal_rom(addr);
                assert_eq!(
                    ((sys_bus.rom_exec_mask >> nibble) & 1) != 0,
                    expected,
                    "{system} nibble {nibble:X}"
                );
                assert_eq!(
                    sys_bus.is_rom_exec_candidate(addr),
                    expected,
                    "{system} addr ${addr:04X}"
                );
            }
        }
    }

    #[test]
    fn cia_timers_active_tracks_start_and_oneshot_underflow() {
        let mut bus = C64Bus::new(TargetSystem::C64, None, None, None);
        bus.set_byte(0x0000, 0x2F);
        bus.set_byte(0x0001, 0x37);

        assert!(!bus.cia_timers_active);
        assert!(!bus.cia1.has_active_timer());
        assert!(!bus.cia2.has_active_timer());

        // Write CIA 1 Timer A latch = 4 (still stopped)
        bus.set_byte(0xDC04, 4);
        bus.set_byte(0xDC05, 0);
        assert!(!bus.cia_timers_active);

        // Start CIA 1 Timer A in one-shot mode (START | LOAD | ONESHOT = 0x19)
        bus.set_byte(0xDC0E, 0x19);
        assert!(bus.cia_timers_active);
        assert!(bus.cia1.has_active_timer());

        // Step 3 cycles: counter decrements from 4 to 1, timer remains active
        bus.step_cycles(3);
        assert!(bus.cia_timers_active);
        assert_eq!(bus.cia1.read_reg(0x04), 1);

        // Step 2 more cycles: 1 cycle decrements 1 -> 0, next cycle underflows (reloads 4 and clears START in one-shot mode)
        // Note: `get_byte(0xDC0D)` steps 1 cycle before reading ICR, or we can step via `step_cycles`
        bus.step_cycles(2);
        assert!(!bus.cia1.has_active_timer());
        assert!(!bus.cia_timers_active);
        assert_eq!(bus.get_byte(0xDC0D) & 0x01, 0x01);

        // Start CIA 2 Timer B in continuous mode ($DD06/$DD07 = 10, $DD0F = 0x11)
        bus.set_byte(0xDD06, 10);
        bus.set_byte(0xDD07, 0);
        bus.set_byte(0xDD0F, 0x11);
        assert!(bus.cia_timers_active);
        assert!(bus.cia2.has_active_timer());

        // Stop CIA 2 Timer B via control register ($DD0F = 0x00)
        bus.set_byte(0xDD0F, 0x00);
        assert!(!bus.cia2.has_active_timer());
        assert!(!bus.cia_timers_active);
    }
}
