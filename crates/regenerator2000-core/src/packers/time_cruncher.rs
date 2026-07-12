//! Time Cruncher strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects Time Cruncher signature.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    if mem.len() >= 0x0820
        && mem[0x810] == 0x78
        && mem[0x811] == 0xA9
        && mem[0x812] == 0x34
        && mem[0x813] == 0x85
        && mem[0x814] == 0x01
        && mem[0x815] == 0xA0
        && mem[0x816] == 0xC4
        && mem[0x817] == 0xB9
    {
        Some(Box::new(SimplePacker::new(PackerInfo {
            name: "Time Cruncher",
            dep_addr: Some(0x0100),
            start_addr: Some(0x0801),
            end_addr: None,
            entry_point: None,
            end_addr_ptr: None,
        })))
    } else {
        None
    }
}
