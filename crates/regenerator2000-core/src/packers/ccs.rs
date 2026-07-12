//! Commodore Cruncher System (CCS) strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects Commodore Cruncher System (CCS) signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x81C && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0xA2
            && mem[q + 7] == 0xBD
            && mem[q + 0x0A] == 0x9D
            && mem[q + 0x0D] == 0xCA
            && mem[q + 0x0E] == 0xD0
        {
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
