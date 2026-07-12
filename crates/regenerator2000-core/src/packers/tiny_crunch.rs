//! TinyCrunch strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects TinyCrunch signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x815 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA2
            && mem[q + 2] == 0xBB
            && mem[q + 3] == 0xBD
            && mem[q + 4] == 0x1B
            && mem[q + 5] == 0x08
            && mem[q + 6] == 0x9D
            && mem[q + 7] == 0xFF
        {
            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "TinyCrunch",
                dep_addr: Some(0x010E),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[0x836], mem[0x837]])),
                end_addr_ptr: None,
            })));
        }
    }
    None
}
