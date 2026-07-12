//! TSCrunch strategy implementation.

use super::{Packer, PackerInfo};
use crate::state::types::System;

/// TSCrunch packer implementation with post-emulation end address recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TScrunchPacker {
    /// Information describing the packer.
    pub info: PackerInfo,
    /// Zero-page address holding the destination end pointer (low byte).
    pub zp_end_ptr: u16,
}

impl Packer for TScrunchPacker {
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
        _system: &System,
        _y_reg: u8,
    ) {
        let ptr = self.zp_end_ptr as usize;
        if mem.len() >= ptr + 2 {
            let end_addr = u16::from_le_bytes([mem[ptr], mem[ptr + 1]]);
            if end_addr > range.0 {
                range.1 = end_addr.saturating_sub(1);
            }
        }
    }
}

/// Detects TSCrunch signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x820 && load_addr <= 0x0801 {
        let q = 0x080D_usize;
        if mem.len() > q + 18 {
            // Variant 1: TSCrunch v1.3+ (Zero-Page depacker at $0002)
            // Code at $080D: SEI; LDX #$CC; LDA $081A,X; STA $00,X; DEX; BNE; JMP $0002
            if mem[q] == 0x78
                && mem[q + 1] == 0xA2
                && mem[q + 3] == 0xBD
                && mem[q + 6] == 0x95
                && mem[q + 7] == 0x00
                && mem[q + 8] == 0xCA
                && mem[q + 9] == 0xD0
                && mem[q + 11] == 0x4C
                && mem[q + 12] == 0x02
                && mem[q + 13] == 0x00
            {
                let mut entry_point = None;
                if mem.len() >= q + 0x70 {
                    for p in q + 14..mem.len().min(q + 0x80) {
                        if mem.len() >= p + 8
                            && mem[p] == 0xA9
                            && mem[p + 1] == 0x37
                            && mem[p + 2] == 0x85
                            && mem[p + 3] == 0x01
                            && mem[p + 4] == 0x58
                            && mem[p + 5] == 0x4C
                        {
                            entry_point = Some(u16::from_le_bytes([mem[p + 6], mem[p + 7]]));
                            break;
                        }
                    }
                }

                return Some(Box::new(TScrunchPacker {
                    info: PackerInfo {
                        name: "TSCrunch v1.3+",
                        dep_addr: Some(0x0002),
                        start_addr: Some(0x0800),
                        end_addr: None,
                        entry_point,
                        end_addr_ptr: Some(0x0026),
                    },
                    zp_end_ptr: 0x0026,
                }));
            }

            // Variant 2: TSCrunch v1.3+-X2 (Stack depacker at $0100)
            // Code at $080D: SEI; LDA #$34; STA $01; LDX #$D0; LDA $081F,X; STA $00FB,X; DEX; BNE; JMP $0100
            if mem[q] == 0x78
                && mem[q + 1] == 0xA9
                && mem[q + 2] == 0x34
                && mem[q + 3] == 0x85
                && mem[q + 4] == 0x01
                && mem[q + 5] == 0xA2
                && mem[q + 7] == 0xBD
                && mem[q + 10] == 0x9D
                && mem[q + 13] == 0xCA
                && mem[q + 14] == 0xD0
                && mem[q + 16] == 0x4C
                && mem[q + 17] == 0x00
                && mem[q + 18] == 0x01
            {
                let mut entry_point = None;
                if mem.len() >= q + 0x70 {
                    for p in q + 19..mem.len().min(q + 0x80) {
                        if mem.len() >= p + 8
                            && mem[p] == 0xA9
                            && mem[p + 1] == 0x37
                            && mem[p + 2] == 0x85
                            && mem[p + 3] == 0x01
                            && mem[p + 4] == 0x58
                            && mem[p + 5] == 0x4C
                        {
                            entry_point = Some(u16::from_le_bytes([mem[p + 6], mem[p + 7]]));
                            break;
                        }
                    }
                }

                return Some(Box::new(TScrunchPacker {
                    info: PackerInfo {
                        name: "TSCrunch v1.3+-X2",
                        dep_addr: Some(0x0100),
                        start_addr: Some(0x0801),
                        end_addr: None,
                        entry_point,
                        end_addr_ptr: Some(0x00FD),
                    },
                    zp_end_ptr: 0x00FD,
                }));
            }
        }
    }
    None
}
