//! Eagle Cruncher strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects Eagle Cruncher signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA2
            && mem[q + 3] == 0xBD
            && mem[q + 6] == 0x9D
            && mem[q + 7] == 0x00
            && mem[q + 8] == 0x01
            && mem[q + 9] == 0xD0
            && mem[q + 0x0A] == 0xFC
            && mem[q + 0x0B] == 0x4C
            && mem[q + 0x0C] == 0x00
            && mem[q + 0x0D] == 0x01
        {
            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "Eagle Cruncher",
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
