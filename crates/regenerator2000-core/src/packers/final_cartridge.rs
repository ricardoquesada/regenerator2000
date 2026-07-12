//! Final Cartridge III strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects Final Cartridge III signature.
#[must_use]
pub fn detect(mem: &[u8], load_addr: u16) -> Option<Box<dyn Packer>> {
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x37
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0x8D
            && mem[q + 6] == 0x00
            && mem[q + 7] == 0xDD
        {
            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "Final Cartridge III",
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
