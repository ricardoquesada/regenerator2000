//! Antiram Packer v1.0 and v2.0 strategy implementation.

use super::{Packer, PackerInfo};

/// Strategy implementation for Antiram Packers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AntiramPacker {
    /// Packer metadata.
    pub info: PackerInfo,
    /// Offset for ROM JSR patch.
    pub patch_offset: usize,
}

impl AntiramPacker {
    /// Creates a new [`AntiramPacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo, patch_offset: usize) -> Self {
        Self { info, patch_offset }
    }
}

impl Packer for AntiramPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }
}

/// Detects Antiram Packer v1.0 and v2.0 signatures.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    // Antiram Packer v1.0
    if mem.len() > 0x950
        && u32::from_le_bytes([mem[0x80d], mem[0x80e], mem[0x80f], mem[0x810]]) == 0xB900A078
        && u32::from_le_bytes([mem[0x81d], mem[0x81e], mem[0x81f], mem[0x820]]) == 0xC6FF004C
        && u32::from_le_bytes([mem[0x82d], mem[0x82e], mem[0x82f], mem[0x830]]) == 0x08C9AFA5
        && u32::from_le_bytes([mem[0x8ed], mem[0x8ee], mem[0x8ef], mem[0x8f0]]) == 0xFF354C03
    {
        let mut p = 0x904;
        if mem[p] == 0x20 {
            p += 3;
        }
        let ret_addr = u16::from_le_bytes([mem[p + 1], mem[p + 2]]);
        return Some(Box::new(AntiramPacker::new(
            PackerInfo {
                name: "Antiram Packer v1.0",
                dep_addr: Some(0xFF00),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(ret_addr),
                end_addr_ptr: Some(0x002D),
            },
            0x904,
        )));
    }

    // Antiram Packer v2.0
    if mem.len() > 0x950
        && u32::from_le_bytes([mem[0x80d], mem[0x80e], mem[0x80f], mem[0x810]]) == 0xB900A078
        && u32::from_le_bytes([mem[0x81d], mem[0x81e], mem[0x81f], mem[0x820]]) == 0xA9FF004C
        && u32::from_le_bytes([mem[0x82d], mem[0x82e], mem[0x82f], mem[0x830]]) == 0xC86291AE
        && u32::from_le_bytes([mem[0x8ed], mem[0x8ee], mem[0x8ef], mem[0x8f0]]) == 0xD0CA2EE6
    {
        let mut p = 0x911;
        if mem[p] == 0x20 {
            p += 3;
        }
        let ret_addr = u16::from_le_bytes([mem[p + 1], mem[p + 2]]);
        return Some(Box::new(AntiramPacker::new(
            PackerInfo {
                name: "Antiram Packer v2.0",
                dep_addr: Some(0xFF00),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(ret_addr),
                end_addr_ptr: Some(0x002D),
            },
            0x911,
        )));
    }

    None
}
