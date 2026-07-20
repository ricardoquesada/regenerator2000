//! MC-Cracken Compressor strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;

/// Strategy implementation for MC-Cracken Compressor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McCrackenPacker {
    /// Packer metadata.
    pub info: PackerInfo,
}

impl McCrackenPacker {
    /// Creates a new [`McCrackenPacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo) -> Self {
        Self { info }
    }
}

impl Packer for McCrackenPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn post_emulate(
        &self,
        mem: &[u8],
        _snapshot: &[u8],
        _written: &[bool],
        range: &mut (u16, u16),
        _entry_point: &mut u16,
        system: &System,
        _y_reg: u8,
    ) {
        if !system.is_c64() {
            return;
        }

        // unp64 compatibility for MC-Cracken Compressor:
        if mem.len() >= 0xB0 {
            let reported_end = u16::from_le_bytes([mem[0xAE], mem[0xAF]]);
            if reported_end > range.0 && reported_end < 0xFFFF {
                range.1 = reported_end.saturating_sub(1);
            }
        }
    }
}

/// Detects MC-Cracken signature.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    for p in 0x0810..=0x0840 {
        if mem.len() > p + 0x40
            && mem[p] == 0xA9
            && mem[p + 2] == 0x85
            && mem[p + 4] == 0xA0
            && mem[p + 5] == 0x00
            && mem[p + 6] == 0xC6
            && mem[p + 7] == 0xAF
        {
            let entry_point = if mem.len() > p + 0x64 && mem[p + 0x61] == 0x4C {
                Some(u16::from_le_bytes([mem[p + 0x62], mem[p + 0x63]]))
            } else {
                Some(0x1100)
            };

            return Some(Box::new(McCrackenPacker::new(PackerInfo {
                name: "McCracken Compressor",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0800),
                end_addr: None,
                entry_point,
                end_addr_ptr: Some(0x00AE),
            })));
        }
    }
    None
}
