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
        entry_point: &mut u16,
        system: &System,
        _y_reg: u8,
    ) {
        if system.as_str() != System::C64 {
            return;
        }

        // unp64 compatibility for MC-Cracken Compressor:
        // MC-Cracken's first pass depacker jumps to stack page ($0172) for second pass,
        // leaving JMP $0172 at $AB-$AD and exclusive end address at $AE-$AF.
        if *entry_point == 0x1100 && mem.len() >= 0xB0 && mem[0xAB..=0xAD] == [0x4C, 0x72, 0x01] {
            let reported_end = u16::from_le_bytes([mem[0xAE], mem[0xAF]]);
            if reported_end > range.0 {
                range.1 = reported_end.saturating_sub(1);
            }
        }
    }
}

/// Detects MC-Cracken signature.
#[must_use]
pub fn detect(_mem: &[u8]) -> Option<Box<dyn Packer>> {
    // Note: MC-Cracken signature is dynamically checked post-emulate,
    // but if detected initially, returns strategy instance.
    None
}
