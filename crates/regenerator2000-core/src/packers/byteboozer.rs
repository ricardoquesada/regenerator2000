//! ByteBoozer 2.0 strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;

/// Strategy implementation for ByteBoozer 2.0 packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteBoozerPacker {
    /// Packer metadata.
    pub info: PackerInfo,
}

impl ByteBoozerPacker {
    /// Creates a new [`ByteBoozerPacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo) -> Self {
        Self { info }
    }
}

impl Packer for ByteBoozerPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn post_emulate(
        &self,
        mem: &[u8],
        snapshot: &[u8],
        _written: &[bool],
        range: &mut (u16, u16),
        _entry_point: &mut u16,
        system: &System,
        _y_reg: u8,
    ) {
        if !system.is_c64() || snapshot.len() < 0x8C4 {
            return;
        }

        let b0 = snapshot[0x80D..0x811] == [0x78, 0xA9, 0x34, 0x85];
        let b1 = snapshot[0x813..0x817] == [0xB7, 0xBD, 0x1E, 0x08];
        let b2 = snapshot[0x870..0x874] == [0xA8, 0x20, 0xAD, 0x00];
        let b3 = snapshot[0x8C0..0x8C4] == [0xAE, 0xD0, 0x02, 0xE6];

        if b0 && b1 && b2 && b3 {
            let reported_end = u16::from_le_bytes([mem[0x77], mem[0x78]]);
            if reported_end > range.0 {
                range.1 = reported_end.saturating_sub(1);
            }
        }
    }
}

/// Detects ByteBoozer 2.0 signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x887 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 6] == 0xB7
            && mem[q + 7] == 0xBD
            && mem[q + 8] == 0x1E
            && mem[q + 9] == 0x08
        {
            return Some(Box::new(ByteBoozerPacker::new(PackerInfo {
                name: "ByteBoozer",
                dep_addr: Some(0x0010),
                start_addr: Some(u16::from_le_bytes([mem[0x886], mem[0x887]])),
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[0x8CB], mem[0x8CC]])),
                end_addr_ptr: Some(0x0077),
            })));
        }
    }
    None
}
