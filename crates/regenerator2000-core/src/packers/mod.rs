//! 6502 binary packer detection and unpacker lifecycle strategies.

pub mod action_replay;
pub mod alz64;
pub mod antiram;
pub mod byteboozer;
pub mod ccs;
pub mod cruel_cruncher;
pub mod dali;
pub mod eagle;
pub mod eca;
pub mod exomizer;
pub mod final_cartridge;
pub mod mc_cracken;
pub mod pucrunch;
pub mod simple;
pub mod super_cruncher;
pub mod tbc;
pub mod time_cruncher;
pub mod tiny_crunch;
pub mod triad;
pub mod turbo_cruncher;

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use std::fmt::Debug;

use crate::state::types::System;
use crate::unpacker::UnpackerMemory;

/// Information about a detected packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackerInfo {
    /// Human-readable name of the packer.
    pub name: &'static str,
    /// Depacker start address.
    pub dep_addr: Option<u16>,
    /// Unpacked output start address override.
    pub start_addr: Option<u16>,
    /// Unpacked output end address override.
    pub end_addr: Option<u16>,
    /// Entry point address override.
    pub entry_point: Option<u16>,
    /// Zero-page or memory pointer address containing end address.
    pub end_addr_ptr: Option<u16>,
}

/// A byte modification to apply to RAM prior to 6502 emulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryPatch {
    /// Target address to modify.
    pub addr: u16,
    /// Expected byte value before patching (if any).
    pub expected: Option<u8>,
    /// Replacement byte value.
    pub patch: u8,
}

/// Interface for packer detection and lifecycle hooks.
pub trait Packer: Debug + Send + Sync {
    /// Returns the metadata description of this packer.
    fn info(&self) -> PackerInfo;

    /// Pre-emulation hook: applies patches to memory before 6502 emulation starts.
    fn pre_emulate(&self, _mem: &mut [u8], _system: &System) {}

    /// Instruction step hook: invoked during 6502 emulation.
    fn on_step(&mut self, _cpu: &mut CPU<UnpackerMemory, Nmos6502>, _phase: u8) {}

    /// Post-emulation hook: refines/overrides the unpacked memory bounds and entry point.
    #[allow(clippy::too_many_arguments)]
    fn post_emulate(
        &self,
        _mem: &[u8],
        _snapshot: &[u8],
        _written: &[bool],
        _range: &mut (u16, u16),
        _entry_point: &mut u16,
        _system: &System,
        _y_reg: u8,
    ) {
    }
}

/// Scans memory for known packer signatures and returns a corresponding [`Packer`] strategy if found.
#[must_use]
pub fn detect_packer(mem: &[u8], load_addr: u16, load_end: u16) -> Option<Box<dyn Packer>> {
    if let Some(p) = exomizer::detect(mem, load_addr, load_end) {
        return Some(p);
    }
    if let Some(p) = pucrunch::detect(mem) {
        return Some(p);
    }
    if let Some(p) = time_cruncher::detect(mem) {
        return Some(p);
    }
    if let Some(p) = dali::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = byteboozer::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = tiny_crunch::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = cruel_cruncher::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = ccs::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = turbo_cruncher::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = action_replay::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = final_cartridge::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = triad::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = eagle::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = super_cruncher::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = alz64::detect(mem, load_addr) {
        return Some(p);
    }
    if let Some(p) = tbc::detect(mem) {
        return Some(p);
    }
    if let Some(p) = eca::detect(mem) {
        return Some(p);
    }
    if let Some(p) = antiram::detect(mem) {
        return Some(p);
    }
    if let Some(p) = mc_cracken::detect(mem) {
        return Some(p);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_packer_none() {
        let mem = vec![0; 0x1000];
        assert!(detect_packer(&mem, 0x0801, 0x1000).is_none());
    }

    #[test]
    fn test_detect_exomizer_v3_0() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        mem[p + 8] = 0x08;
        mem[p + 9] = 0x48;
        mem[p + 10] = 0x20;
        mem[p + 11] = 0x1A;
        mem[p + 12] = 0x01;

        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34;
        mem[p - 4] = 0x01;

        mem[p + 0x10] = 0x20;
        mem[p + 0x11] = 0x00;
        mem[p + 0x12] = 0x01;
        mem[p + 0x15] = 0x4C;
        mem[p + 0x16] = 0x20;
        mem[p + 0x17] = 0x08;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "Exomizer 3.0");
        assert_eq!(info.dep_addr, Some(0x0134));
        assert_eq!(info.start_addr, None);
        assert_eq!(info.entry_point, Some(0x0820));
    }

    #[test]
    fn test_detect_exomizer_v3_02() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        mem[p + 8] = 0x99;

        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34;
        mem[p - 4] = 0x01;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "Exomizer v3.02+");
    }

    #[test]
    fn test_detect_exomizer_v1_v2() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0xC8;
        mem[p + 1] = 0xC0;
        mem[p + 2] = 0x50;
        mem[p + 3] = 0xD0;
        mem[p + 7] = 0x4C;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "Exomizer 2.x");
        assert_eq!(info.dep_addr, Some(0x0150));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_pucrunch() {
        let mut mem = vec![0; 0x1000];
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA2;
        mem[0x816] = 0x34;
        mem[0x817] = 0xBD;
        mem[0x818] = 0x42;
        mem[0x819] = 0x08;
        mem[0x81A] = 0x9D;
        mem[0x81B] = 0xFF;
        mem[0x81C] = 0x01;
        mem[0x81D] = 0xCA;
        mem[0x81E] = 0xD0;

        mem[0x841] = 0x16;
        mem[0x842] = 0x01;

        mem[0x879] = 0x01;
        mem[0x87A] = 0x08;

        let ep = 0x0920;
        mem[ep] = 0xA5;
        mem[ep + 1] = 0xFA;
        mem[ep + 2] = 0x85;
        mem[ep + 3] = 0x2D;
        mem[ep + 4] = 0xA5;
        mem[ep + 5] = 0xFB;
        mem[ep + 6] = 0x85;
        mem[ep + 7] = 0x2E;
        mem[ep + 0x0A] = 0x00;
        mem[ep + 0x0B] = 0x20;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "PUCrunch");
        assert_eq!(info.dep_addr, Some(0x0116));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00FA));
    }

    #[test]
    fn test_detect_time_cruncher() {
        let mut mem = vec![0; 0x1000];
        mem[0x810] = 0x78;
        mem[0x811] = 0xA9;
        mem[0x812] = 0x34;
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA0;
        mem[0x816] = 0xC4;
        mem[0x817] = 0xB9;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "Time Cruncher");
        assert_eq!(info.dep_addr, Some(0x0100));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_dali() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA2;
        mem[q + 2] = 0x0B;
        mem[q + 3] = 0x9A;
        mem[q + 4] = 0xA0;
        mem[q + 5] = 0xEC;
        mem[q + 6] = 0x48;
        mem[q + 7] = 0xB7;
        mem[q + 0x11] = 0x4C;
        mem[q + 0x12] = 0x03;
        mem[q + 0x13] = 0x00;
        mem[q + 0x14] = 0x34;

        mem[q + 0xFE] = 0x00;
        mem[q + 0xFF] = 0x10;
        mem[q + 0x44] = 0x6B;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "Dali");
        assert_eq!(info.dep_addr, Some(0x0003));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x1000));
        assert_eq!(info.end_addr_ptr, Some(0x006B));
    }

    #[test]
    fn test_detect_byte_boozer() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 6] = 0xB7;
        mem[q + 7] = 0xBD;
        mem[q + 8] = 0x1E;
        mem[q + 9] = 0x08;

        mem[0x8CB] = 0x00;
        mem[0x8CC] = 0x20;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "ByteBoozer");
        assert_eq!(info.dep_addr, Some(0x0010));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x0077));
    }

    #[test]
    fn test_detect_alz64_quiss() {
        let mut mem = vec![0; 0x1000];
        mem[0x80b] = 0xA2;
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0xB5;
        mem[0x814] = 0x00;
        mem[0x815] = 0x09;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF3;
        mem[0x81b] = 0xA2;
        mem[0x81c] = 0x03;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x10;
        mem[0x826] = 0x89;
        mem[0x827] = 0x38;
        mem[0x82c] = 0xF1;
        mem[0x82d] = 0x4C;
        mem[0x82e] = 0x5E;
        mem[0x82f] = 0x00;

        let p = 0x0900;
        mem[p + 0xf1] = 0x4C;
        mem[p + 0xf2] = 0x00;
        mem[p + 0xf3] = 0x20;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "ALZ64/Quiss");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }

    #[test]
    fn test_detect_alz64_kabuto() {
        let mut mem = vec![0; 0x1000];
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0x86;
        mem[0x813] = 0x00;
        mem[0x814] = 0x09;
        mem[0x818] = 0xCA;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF7;
        mem[0x81b] = 0xCE;
        mem[0x822] = 0xD0;
        mem[0x823] = 0xEE;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x03;
        mem[0x835] = 0xF1;
        mem[0x836] = 0x4C;
        mem[0x837] = 0x5E;
        mem[0x838] = 0x00;

        let p = 0x0900;
        mem[p + 0xfd] = 0x4C;
        mem[p + 0xfe] = 0x00;
        mem[p + 0xff] = 0x20;

        let packer = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        let info = packer.info();
        assert_eq!(info.name, "ALZ64/Kabuto");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }
}
