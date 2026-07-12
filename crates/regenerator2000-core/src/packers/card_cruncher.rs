//! 1001 Card Cruncher strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects 1001 Card Cruncher signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x840 && load_addr <= 0x0801 {
        // Variant 1: 1001 CardCruncher ACM
        // Code at $080D / $080F: SEI; LDX #$F0; STX $01; LDA $src,X; STA $dst,X; DEX; BNE; JMP $0100
        for q in [0x080D_usize, 0x080F_usize] {
            if mem.len() > q + 17
                && mem[q] == 0x78
                && mem[q + 1] == 0xA2
                && mem[q + 3] == 0x86
                && mem[q + 4] == 0x01
                && mem[q + 5] == 0xBD
                && mem[q + 8] == 0x9D
                && mem[q + 11] == 0xCA
                && mem[q + 12] == 0xD0
                && mem[q + 14] == 0x4C
                && mem[q + 15] == 0x00
                && mem[q + 16] == 0x01
            {
                let end_val = u16::from_le_bytes([mem[0x081E], mem[0x081F]]);
                let entry_val = u16::from_le_bytes([mem[0x083F], mem[0x0840]]);
                let end_addr = if end_val > load_addr {
                    Some(end_val.saturating_sub(1))
                } else {
                    None
                };

                return Some(Box::new(SimplePacker::new(PackerInfo {
                    name: "1001 CardCruncher ACM",
                    dep_addr: Some(0x0100),
                    start_addr: Some(0x0801),
                    end_addr,
                    entry_point: Some(entry_val),
                    end_addr_ptr: None,
                })));
            }
        }

        // Variant 2: 1001 CardCruncher v4
        // Code at $0815: STX $01; LDA $082A,Y; STA $FA99...
        if mem.len() > 0x941
            && mem[0x815] == 0x86
            && mem[0x816] == 0x01
            && mem[0x81A] == 0x2A
            && mem[0x81B] == 0x08
            && mem[0x81C] == 0x99
            && mem[0x81D] == 0xFA
        {
            let end_val = u16::from_le_bytes([mem[0x82E], mem[0x82F]]);
            let entry_val = u16::from_le_bytes([mem[0x940], mem[0x941]]);

            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "1001 CardCruncher v4",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: Some(end_val),
                entry_point: Some(entry_val),
                end_addr_ptr: None,
            })));
        }
    }
    None
}
