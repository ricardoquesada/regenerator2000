//! PUCrunch strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects PUCrunch signature.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    if mem.len() >= 0x0944
        && mem[0x813] == 0x85
        && mem[0x814] == 0x01
        && mem[0x815] == 0xA2
        && mem[0x816] == 0x34
        && mem[0x817] == 0xBD
        && mem[0x818] == 0x42
        && mem[0x819] == 0x08
        && mem[0x81A] == 0x9D
        && mem[0x81B] == 0xFF
        && mem[0x81C] == 0x01
        && mem[0x81D] == 0xCA
        && mem[0x81E] == 0xD0
    {
        let mut entry_point = None;
        for p in 0x0912..0x0938 {
            if mem[p] == 0xA5
                && mem[p + 1] == 0xFA
                && mem[p + 2] == 0x85
                && mem[p + 3] == 0x2D
                && mem[p + 4] == 0xA5
                && mem[p + 5] == 0xFB
                && mem[p + 6] == 0x85
                && mem[p + 7] == 0x2E
            {
                entry_point = Some(u16::from_le_bytes([mem[p + 0x0A], mem[p + 0x0B]]));
                break;
            }
        }

        Some(Box::new(SimplePacker::new(PackerInfo {
            name: "PUCrunch",
            dep_addr: Some(u16::from_le_bytes([mem[0x841], mem[0x842]])),
            start_addr: Some(u16::from_le_bytes([mem[0x879], mem[0x87A]])),
            end_addr: None,
            entry_point,
            end_addr_ptr: Some(0x00FA),
        })))
    } else {
        None
    }
}
