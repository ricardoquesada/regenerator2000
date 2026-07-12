//! ALZ64 (Quiss / Kabuto) strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;

/// Strategy implementation for ALZ64 packers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alz64Packer {
    /// Packer metadata.
    pub info: PackerInfo,
    /// Whether pre-emulation memory patching is needed (Kabuto variant).
    pub needs_patch: bool,
}

impl Alz64Packer {
    /// Creates a new [`Alz64Packer`] instance.
    #[must_use]
    pub fn new(info: PackerInfo, needs_patch: bool) -> Self {
        Self { info, needs_patch }
    }
}

impl Packer for Alz64Packer {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn pre_emulate(&self, mem: &mut [u8], system: &System) {
        if self.needs_patch && system.as_str() == System::C64 && mem.len() > 0x080B {
            // ALZ64/Kabuto bootstrap stub check: overwrite with LDX #imm (0xA2)
            mem[0x080B] = 0xA2;
        }
    }
}

/// Detects ALZ64/Quiss and ALZ64/Kabuto signatures.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    // ALZ64/Quiss
    if mem.len() > 0x82F
        && load_addr <= 0x080B
        && mem[0x80B] == 0xA2
        && mem[0x80C] == 0x00
        && mem[0x80D] == 0x78
        && mem[0x80E] == 0xB5
        && mem[0x819] == 0xD0
        && mem[0x81A] == 0xF3
        && mem[0x81B] == 0xA2
        && mem[0x81C] == 0x03
        && mem[0x824] == 0xA2
        && mem[0x825] == 0x10
        && mem[0x826] == 0x89
        && mem[0x827] == 0x38
        && mem[0x82C] == 0xF1
        && mem[0x82D] == 0x4C
        && mem[0x82E] == 0x5E
        && mem[0x82F] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x814], mem[0x815]]) as usize;
        if p + 0xF3 < mem.len() && mem[p + 0xF1] == 0x4C {
            let mut start_addr = Some(0x080B);
            for q in (p + 0xCB)..=(p + 0xF0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = Some(u16::from_le_bytes([mem[q + 4], mem[q + 5]]));
                    break;
                }
            }
            return Some(Box::new(Alz64Packer::new(
                PackerInfo {
                    name: "ALZ64/Quiss",
                    dep_addr: Some(0x005E),
                    start_addr,
                    end_addr: None,
                    entry_point: Some(u16::from_le_bytes([mem[p + 0xF2], mem[p + 0xF3]])),
                    end_addr_ptr: Some(0x00CF),
                },
                false,
            )));
        }
    }

    // ALZ64/Kabuto
    if mem.len() > 0x838
        && load_addr <= 0x080B
        && mem[0x80C] == 0x00
        && mem[0x80D] == 0x78
        && mem[0x80E] == 0x86
        && mem[0x818] == 0xCA
        && mem[0x819] == 0xD0
        && mem[0x81A] == 0xF7
        && mem[0x81B] == 0xCE
        && mem[0x822] == 0xD0
        && mem[0x823] == 0xEE
        && mem[0x824] == 0xA2
        && mem[0x825] == 0x03
        && mem[0x835] == 0xF1
        && mem[0x836] == 0x4C
        && mem[0x837] == 0x5E
        && mem[0x838] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x813], mem[0x814]]) as usize;
        if p + 0xFF < mem.len() && mem[p + 0xFD] == 0x4C {
            let mut start_addr = Some(0x080B);
            for q in (p + 0xE6)..=(p + 0xF0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = Some(u16::from_le_bytes([mem[q + 4], mem[q + 5]]));
                    break;
                }
            }
            return Some(Box::new(Alz64Packer::new(
                PackerInfo {
                    name: "ALZ64/Kabuto",
                    dep_addr: Some(0x005E),
                    start_addr,
                    end_addr: None,
                    entry_point: Some(u16::from_le_bytes([mem[p + 0xFE], mem[p + 0xFF]])),
                    end_addr_ptr: Some(0x00CF),
                },
                true,
            )));
        }
    }

    None
}
