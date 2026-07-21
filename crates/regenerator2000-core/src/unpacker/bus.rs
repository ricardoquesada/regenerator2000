//! Custom memory bus and ROM banking logic for 6502 unpacker emulation.

use super::cia::CiaState;
use mos6502::memory::Bus;

/// Optional observer trait for listening to memory access events during emulation.
pub trait MemoryAccessHook: Send + Sync {
    /// Triggered on every memory write access.
    fn on_write(&mut self, addr: u16, val: u8, pc: u16);
}

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
        Self {
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
        }
    }

    /// Steps CIA timers by the specified CPU cycle count.
    pub fn step_cycles(&mut self, cycles: u32) {
        self.cia1.step_cycles(cycles);
        self.cia2.step_cycles(cycles);
    }
}

impl Bus for C64Bus {
    fn get_byte(&mut self, addr: u16) -> u8 {
        self.total_cycles = self.total_cycles.wrapping_add(1);
        self.step_cycles(1);
        let a = addr as usize;
        if self.system.is_c64() {
            // Compute active MOS 6510 processor port bits [2:0] (CHAREN, HIRAM, LORAM).
            // On a C64, $0000 (DDR6510) sets bit output direction. Bits set to output (1) take
            // their state from $0001 (R6510), while input bits (0) pull up to 1 (HIGH logic).
            // Reference: https://www.pagetable.com/c64ref/c64mem/
            let bank = get_c64_bank(&self.mem);
            let loram = (bank & 0x01) != 0;
            let hiram = (bank & 0x02) != 0;
            let charen = (bank & 0x04) != 0;

            if (0xA000..=0xBFFF).contains(&addr)
                && loram
                && hiram
                && let Some(ref rom) = self.basic_rom
                && let Some(offset) = (addr as usize).checked_sub(0xA000)
                && let Some(&val) = rom.get(offset)
            {
                return val;
            }

            if (0xE000..=0xFFFF).contains(&addr)
                && hiram
                && let Some(ref rom) = self.kernal_rom
                && let Some(offset) = (addr as usize).checked_sub(0xE000)
                && let Some(&val) = rom.get(offset)
            {
                return val;
            }

            if (0xD000..=0xDFFF).contains(&addr) {
                if charen && (loram || hiram) {
                    if (0xDC00..=0xDC0F).contains(&addr) {
                        return self.cia1.read_reg((addr & 0x0F) as u8);
                    }
                    if (0xDD00..=0xDD0F).contains(&addr) {
                        return self.cia2.read_reg((addr & 0x0F) as u8);
                    }
                    // PAL C64 VIC-II timing: 63 CPU cycles per raster line, 312 lines per frame.
                    // $D012 provides lower 8 bits of current raster line.
                    // $D011 bit 7 provides the 9th bit (MSB, set when line >= 256) while
                    // preserving lower VIC-II control bits [6:0] from shadow_d011.
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
                if !charen
                    && (loram || hiram)
                    && let Some(ref rom) = self.char_rom
                    && let Some(offset) = (addr as usize).checked_sub(0xD000)
                    && let Some(&val) = rom.get(offset)
                {
                    return val;
                }
            }
        }
        self.mem[a]
    }

    fn set_byte(&mut self, addr: u16, val: u8) {
        self.total_cycles = self.total_cycles.wrapping_add(1);
        self.step_cycles(1);
        let a = addr as usize;
        if self.system.is_c64() {
            let bank = get_c64_bank(&self.mem);
            let loram = (bank & 0x01) != 0;
            let hiram = (bank & 0x02) != 0;
            let charen = (bank & 0x04) != 0;

            // When I/O space is mapped (CHAREN=1 AND (LORAM=1 OR HIRAM=1)), writes to $D000–$DFFF
            // target I/O chip registers (VIC-II, SID, CIA 1, CIA 2), NOT underlying RAM.
            // Do NOT mutate `self.mem[a]` here: updating RAM during I/O writes causes depackers
            // that perform border color flashing (e.g. `INC $D020`) to corrupt RAM bytes beneath
            // I/O space, which breaks decompression when the depacker later reads those RAM
            // addresses after switching to RAM bank configuration.
            if (0xD000..=0xDFFF).contains(&addr) && charen && (loram || hiram) {
                if (0xDC00..=0xDC0F).contains(&addr) {
                    self.cia1.write_reg((addr & 0x0F) as u8, val);
                } else if (0xDD00..=0xDD0F).contains(&addr) {
                    self.cia2.write_reg((addr & 0x0F) as u8, val);
                } else if addr == 0xD011 {
                    self.shadow_d011 = val;
                }
                return;
            }
        }

        self.mem[a] = val;
        self.written[a] = true;

        let ram_start = self.system.ram_start();
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

#[must_use]
pub(crate) fn get_c64_bank(mem: &[u8]) -> u8 {
    (mem[0x01] & mem[0x00]) | (!mem[0x00] & 0x07)
}

#[must_use]
pub(crate) fn is_basic_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        let bank = get_c64_bank(mem);
        (bank & 0x01) != 0 && (bank & 0x02) != 0
    } else {
        true
    }
}

#[must_use]
pub(crate) fn is_kernal_rom_mapped(mem: &[u8], system: &crate::state::types::System) -> bool {
    if system.is_c64() {
        let bank = get_c64_bank(mem);
        (bank & 0x02) != 0
    } else {
        true
    }
}

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
        mem.set_byte(0xD020, 0xFF); // VIC border color
        assert_eq!(mem.get_byte(0xD020), 0x00); // Reads return 0
        assert!(!mem.written[0xD020]); // Write not tracked
    }
}
