//! Dali 0.3.3 strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;

/// Strategy implementation for Dali 0.3.3 packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaliPacker {
    /// Packer metadata.
    pub info: PackerInfo,
}

impl DaliPacker {
    /// Creates a new [`DaliPacker`] instance.
    #[must_use]
    pub fn new(info: PackerInfo) -> Self {
        Self { info }
    }
}

impl Packer for DaliPacker {
    fn info(&self) -> PackerInfo {
        self.info.clone()
    }

    fn post_emulate(
        &self,
        mem: &[u8],
        _snapshot: &[u8],
        written: &[bool],
        range: &mut (u16, u16),
        entry_point: &mut u16,
        system: &System,
        _y_reg: u8,
    ) {
        if system.as_str() != System::C64 {
            return;
        }

        let start_addr = range.0 as usize;

        // unp64 compatibility for Dali v0.3.3 / fast:
        // Dali copies its depacker to zero page and jumps to the entry point stored at $EB-$EC when done.
        // It leaves the compressed payload at the top of memory, which defeats standard gap trim.
        // We find the true end of decompressed data by finding the largest contiguous block of unwritten memory.
        if mem.len() >= 0xED
            && mem[0xEA] == 0x4C
            && *entry_point == u16::from_le_bytes([mem[0xEB], mem[0xEC]])
        {
            let mut max_gap_len = 0;
            let mut max_gap_start = 0;
            let mut current_gap_len = 0;
            let mut current_gap_start = 0;

            for (i, &is_written) in written.iter().enumerate().take(0x10000).skip(start_addr) {
                if !is_written {
                    if current_gap_len == 0 {
                        current_gap_start = i;
                    }
                    current_gap_len += 1;
                } else {
                    if current_gap_len > max_gap_len {
                        max_gap_len = current_gap_len;
                        max_gap_start = current_gap_start;
                    }
                    current_gap_len = 0;
                }
            }
            if current_gap_len > max_gap_len {
                max_gap_len = current_gap_len;
                max_gap_start = current_gap_start;
            }

            if max_gap_len > 256 {
                let e = max_gap_start.saturating_sub(1);
                if e >= start_addr {
                    range.1 = e as u16;
                }
            }
        }
    }
}

/// Detects Dali 0.3.3 signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x822 && load_addr <= 0x0801 {
        let base = 0x080D;
        if base >= load_addr as usize {
            let q = base;
            if mem[q] == 0x78
                && mem[q + 1] == 0xA2
                && mem[q + 2] == 0x0B
                && mem[q + 3] == 0x9A
                && mem[q + 4] == 0xA0
                && mem[q + 5] == 0xEC
                && mem[q + 6] == 0x48
                && mem[q + 7] == 0xB7
                && mem[q + 0x11] == 0x4C
                && mem[q + 0x12] == 0x03
                && mem[q + 0x13] == 0x00
                && mem[q + 0x14] == 0x34
            {
                return Some(Box::new(DaliPacker::new(PackerInfo {
                    name: "Dali",
                    dep_addr: Some(0x0003),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: Some(u16::from_le_bytes([mem[q + 0xFE], mem[q + 0xFF]])),
                    end_addr_ptr: Some(u16::from(mem[q + 0x44])),
                })));
            }
        }
    }
    None
}
