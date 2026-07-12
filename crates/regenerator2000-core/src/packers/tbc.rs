//! TBC Multicompactor strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects TBC Multicompactor signature.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    if mem.len() >= 0x08F0
        && (mem[0x82C] & 0xFD) == 0x84
        && mem[0x82D] == 0x01
        && mem[0x82E] == 0xCA
        && mem[0x82F] == 0x9A
        && mem[0x830] == 0x4C
        && mem[0x831] == 0x00
        && mem[0x832] == 0x01
        && mem[0x833] == 0xA0
        && mem[0x834] == 0x00
        && mem[0x835] == 0x84
        && mem[0x836] == 0xFD
        && mem[0x837] == 0x84
        && mem[0x8A2] == 0x01
        && mem[0x8A3] == 0x4C
        && mem[0x8A4] == 0x49
        && mem[0x8A5] == 0x01
    {
        let is_normal = mem.get(0x84A) == Some(&0x81);
        let is_firelord = mem.get(0x84A) == Some(&0x7B);

        if (is_normal && mem.len() >= 0x8B4 && mem[0x820..0x824] == [0xA2, 0xE9, 0xBD, 0x32])
            || (is_firelord && mem.len() >= 0x8AE && mem[0x81D..0x821] == [0xA2, 0xE9, 0xBD, 0x32])
        {
            let ret_ptr = if is_normal { 0x8B2 } else { 0x8AC };
            let entry_point = u16::from_le_bytes([mem[ret_ptr], mem[ret_ptr + 1]]);

            let p = 0x8EB;
            let start_addr = u16::from_le_bytes([mem[p + 1], mem[p + 2]]);

            let tbl_len = mem[p] as usize;
            let mut q = p + tbl_len;
            let mut max_end: u32 = 0;
            while q > p && q + 1 < mem.len() {
                let strtmp = u32::from(u16::from_le_bytes([mem[q - 1], mem[q]]));
                let val = if strtmp == 0 { 0x10000 } else { strtmp };
                if val > max_end {
                    max_end = val;
                }
                if q < 4 {
                    break;
                }
                q -= 4;
            }

            let end_addr = if max_end > 0 {
                Some((max_end as u16).wrapping_sub(1))
            } else {
                None
            };

            return Some(Box::new(SimplePacker::new(PackerInfo {
                name: "TBC Multicompactor",
                dep_addr: Some(0x0100),
                start_addr: Some(start_addr),
                end_addr,
                entry_point: Some(entry_point),
                end_addr_ptr: None,
            })));
        }
    }
    None
}
