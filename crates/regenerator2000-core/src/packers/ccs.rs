//! Commodore Cruncher System (CCS) strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects Commodore Cruncher System (CCS) signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x81C && load_addr <= 0x0801 {
        // CCS signatures:
        // Variant 1: SEI at $0817 followed by INC $01, LDA ... ($78, $E6, $01, $B9)
        let is_v1 = mem.len() > 0x81B
            && mem[0x817 - load_addr as usize..].starts_with(&[0x78, 0xE6, 0x01, 0xB9]);
        // Variant 2: LDY #$00, SEI, INC $01 at $0812 ($A0, $00, $78, $E6)
        let is_v2 = mem.len() > 0x816
            && mem[0x812 - load_addr as usize..].starts_with(&[0xA0, 0x00, 0x78, 0xE6]);
        // Variant 3: SEI, INC $01, LDA at $0814 ($78, $E6, $01, $B9)
        let is_v3 = mem.len() > 0x818
            && mem[0x814 - load_addr as usize..].starts_with(&[0x78, 0xE6, 0x01, 0xB9]);
        // Variant 4: LDY #$00, SEI, STY at $080B ($A0, $00, $78, $8C)
        let is_v4 = mem.len() > 0x80F
            && mem[0x80B - load_addr as usize..].starts_with(&[0xA0, 0x00, 0x78, 0x8C]);

        if is_v1 || is_v2 || is_v3 || is_v4 {
            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "Commodore Cruncher System",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            })));
        }
    }
    None
}
