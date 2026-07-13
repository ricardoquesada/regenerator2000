//! Exomizer 1.x / 2.x / 3.x packer strategy implementation.

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;

use super::{Packer, PackerInfo};
use crate::state::types::System;
use crate::unpacker::UnpackerMemory;

/// Strategy implementation for Exomizer packers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExomizerPacker {
    /// Packer metadata.
    pub info: PackerInfo,
    /// Major version (2 or 3).
    pub version: u8,
    /// Exomizer 3 sub-version (0x30 for 3.0, 0x32 for 3.02+).
    pub exo_ver: u8,
    /// Dynamic minimum start address tracked during emulation.
    pub min_start: Option<u16>,
}

impl ExomizerPacker {
    /// Creates a new [`ExomizerPacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo, version: u8, exo_ver: u8) -> Self {
        Self {
            info,
            version,
            exo_ver,
            min_start: None,
        }
    }
}

impl Packer for ExomizerPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn on_step(&mut self, cpu: &mut CPU<UnpackerMemory, Nmos6502>, phase: u8) {
        if phase != 2 || self.version != 3 {
            return;
        }
        let pc = cpu.registers.program_counter;
        if (0x0100..=0x01FF).contains(&pc) {
            let mut val = u16::from(cpu.memory.mem[0xFE]) + (u16::from(cpu.memory.mem[0xFF]) << 8);
            let y_val = u16::from(cpu.registers.index_y);
            if self.exo_ver == 0x30 {
                val = val.wrapping_add(y_val);
            } else {
                val = val.wrapping_add(y_val).wrapping_add(1);
            }
            if val > 0 {
                self.min_start = Some(self.min_start.map_or(val, |min| min.min(val)));
            }
        }
    }

    fn post_emulate(
        &self,
        mem: &[u8],
        snapshot: &[u8],
        _written: &[bool],
        range: &mut (u16, u16),
        _entry_point: &mut u16,
        system: &System,
        y_reg: u8,
    ) {
        if system.as_str() != System::C64 {
            return;
        }

        if let Some(min_s) = self.min_start
            && min_s < range.0
        {
            range.0 = min_s;
        }

        let mut exomizer_end_lo = None;
        let mut exomizer_end_hi = None;
        let mut exomizer_version = None;

        // Scan memory for the Exomizer 3 decruncher routine sequence:
        //   p - 6: 0x4C (JMP)
        //   p - 4: 0x01 (high byte of stack jump, e.g. $01xx)
        //   p    : 0x69 0x80 (ADC #$80)
        //   p + 2: 0x0A (ASL A)
        //   p + 3: 0x10 0x0F (BPL +15)
        //   p + 5: 0x06 0xFD (ASL $FD)
        //   p + 7: 0xD0 (BNE)
        for p in 0x0200..=0xFFF0 {
            if p >= 6
                && snapshot.len() > p + 8
                && snapshot[p] == 0x69
                && snapshot[p + 1] == 0x80
                && snapshot[p + 2] == 0x0A
                && snapshot[p + 3] == 0x10
                && snapshot[p + 4] == 0x0F
                && snapshot[p + 5] == 0x06
                && snapshot[p + 6] == 0xFD
                && snapshot[p + 7] == 0xD0
                && snapshot[p - 6] == 0x4C
                && snapshot[p - 4] == 0x01
            {
                let p_idx = p - 5;
                let mut q = 2;
                if snapshot[p_idx - q] == 0x8A {
                    q += 1;
                }
                let elo = snapshot[p_idx - q];
                let ehi = snapshot[p - 1];
                let is_exo_30 = snapshot[p_idx - q - 1] == snapshot[p_idx - q - 3]
                    && snapshot[p_idx - q - 2] == snapshot[p_idx - q];
                let ev = if is_exo_30 { 0x30 } else { 0x32 };
                exomizer_end_lo = Some(elo);
                exomizer_end_hi = Some(ehi);
                exomizer_version = Some(ev);
                break;
            }
        }

        if let Some(ver) = exomizer_version
            && let Some(end_lo) = exomizer_end_lo
        {
            let mut dyn_start = u16::from(mem[0xFE]) + (u16::from(mem[0xFF]) << 8);
            if ver == 0x30 {
                dyn_start = dyn_start.wrapping_add(u16::from(y_reg));
            } else {
                dyn_start = dyn_start.wrapping_add(u16::from(y_reg)).wrapping_add(1);
            }
            range.0 = dyn_start;

            let end_hi = exomizer_end_hi.unwrap_or_else(|| mem[0xFF]);
            let mut dyn_end = u16::from(end_lo) + (u16::from(end_hi) << 8);
            if ver == 0x32 {
                dyn_end = dyn_end.wrapping_add(1);
            }
            if dyn_end == 0 {
                dyn_end = 0xFF00;
            }
            if dyn_end > range.0 {
                range.1 = dyn_end.saturating_sub(1);
            }
        }
    }
}

/// Detects Exomizer 1.x, 2.x, 3.0, and 3.02+ signatures.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16, load_end: u16) -> Option<Box<dyn Packer>> {
    // Exomizer 3.x
    for p in ((load_addr as usize)..=(load_end as usize).saturating_sub(8)).rev() {
        if p >= 6
            && mem.len() > p + 8
            && mem[p] == 0x69
            && mem[p + 1] == 0x80
            && mem[p + 2] == 0x0A
            && mem[p + 3] == 0x10
            && mem[p + 4] == 0x0F
            && mem[p + 5] == 0x06
            && mem[p + 6] == 0xFD
            && mem[p + 7] == 0xD0
            && mem[p - 6] == 0x4C
            && mem[p - 4] == 0x01
        {
            let is_exo_30 = p + 12 < mem.len()
                && mem[p + 8] == 0x08
                && mem[p + 9] == 0x48
                && mem[p + 10] == 0x20
                && mem[p + 11] == 0x1A
                && mem[p + 12] == 0x01;

            let (name, exo_ver) = if is_exo_30 {
                ("Exomizer 3.0", 0x30)
            } else {
                ("Exomizer v3.02+", 0x32)
            };

            return Some(Box::new(ExomizerPacker::new(
                PackerInfo {
                    name,
                    dep_addr: Some(0x0100 | u16::from(mem[p - 5])),
                    start_addr: None,
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
                3,
                exo_ver,
            )));
        }
    }

    // Exomizer 1.x / 2.x
    for p in ((load_addr as usize)..=(load_end as usize).saturating_sub(8)).rev() {
        if mem.len() > p + 7
            && mem[p] == 0xC8
            && mem[p + 1] == 0xC0
            && (mem[p + 2] == 0x34 || mem[p + 2] == 0x50)
            && mem[p + 3] == 0xD0
            && mem[p + 7] == 0x4C
        {
            let dep_low = mem[p + 2];
            return Some(Box::new(ExomizerPacker::new(
                PackerInfo {
                    name: "Exomizer 2.x",
                    dep_addr: Some(0x0100 | u16::from(dep_low)),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: None,
                    end_addr_ptr: None,
                },
                2,
                0,
            )));
        }
    }

    None
}
