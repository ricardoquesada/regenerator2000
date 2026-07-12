//! ECA Compactor strategy implementation.

use super::simple::SimplePacker;
use super::{Packer, PackerInfo};

/// Detects ECA Compactor/Linker signature.
#[must_use]
pub fn detect(mem: &[u8]) -> Option<Box<dyn Packer>> {
    if mem.len() >= 0x0950 {
        for p in 0x080D..=0x0830 {
            if p + 0x3E <= mem.len() && mem[p + 0x3A..p + 0x3E] == [0x2A, 0x2A, 0x2A, 0x2A] {
                let mut matched = false;

                // Check 1: Standard ECA
                if p + 0x10 <= mem.len() {
                    let w08 =
                        u32::from_le_bytes([mem[p + 8], mem[p + 9], mem[p + 10], mem[p + 11]]);
                    let w0c =
                        u32::from_le_bytes([mem[p + 12], mem[p + 13], mem[p + 14], mem[p + 15]]);
                    if w08 == 0x2D9D0032_u32.wrapping_add(p as u32) && w0c == 0xF710CA00 {
                        let w00 = u32::from_le_bytes([mem[p], mem[p + 1], mem[p + 2], mem[p + 3]]);
                        let w04 =
                            u32::from_le_bytes([mem[p + 4], mem[p + 5], mem[p + 6], mem[p + 7]]);
                        if ((w00 & 0xF4FFF000) == 0x8434A000 && w04 == 0xBD05A201)
                            || ((w00 & 0xFFFFFF00) == 0x04A27800 && w04 == 0xBDE80186)
                        {
                            matched = true;
                        } else if p >= 3 {
                            let wm3 =
                                u32::from_le_bytes([mem[p - 3], mem[p - 2], mem[p - 1], mem[p]]);
                            if (wm3 & 0xFFFFFF00) == 0x04A27800 && w04 == 0xBDE80186 {
                                matched = true;
                            }
                        }
                        if !matched && p >= 3 {
                            let wm3 =
                                u32::from_le_bytes([mem[p - 3], mem[p - 2], mem[p - 1], mem[p]]);
                            if wm3 == 0x8D00A978 {
                                matched = true;
                            }
                        }
                    }
                }

                // Check 2: Variant 2
                if !matched && p >= 3 && p + 6 <= mem.len() {
                    let w02 = u32::from_le_bytes([mem[p + 2], mem[p + 3], mem[p + 4], mem[p + 5]]);
                    if w02 == 0x8534A978 && mem[p - 3] == 0xA0 {
                        matched = true;
                    }
                }

                // Check 3: FDT
                if !matched && p + 14 <= mem.len() {
                    let w03 = u32::from_le_bytes([mem[p + 3], mem[p + 4], mem[p + 5], mem[p + 6]]);
                    let w0a =
                        u32::from_le_bytes([mem[p + 10], mem[p + 11], mem[p + 12], mem[p + 13]]);
                    if w03 == 0x8604A278 && w0a == 0x2D950842 {
                        matched = true;
                    }
                }

                // Check 4: Decibel hacks
                if !matched && p >= 6 && p + 4 <= mem.len() {
                    let w00 = u32::from_le_bytes([mem[p], mem[p + 1], mem[p + 2], mem[p + 3]]);
                    let wm6 = u32::from_le_bytes([mem[p - 6], mem[p - 5], mem[p - 4], mem[p - 3]]);
                    if w00 == 0x9D085EBD && wm6 == 0x018534A9 {
                        matched = true;
                    }
                }

                if !matched {
                    continue;
                }

                let mut entry_point = None;
                for q in 0xD6..0xDE {
                    if p + q + 2 < mem.len() && (mem[p + q] == 0x20 || mem[p + q] == 0x4C) {
                        let target = u16::from_le_bytes([mem[p + q + 1], mem[p + q + 2]]);
                        if !matches!(target, 0xA659 | 0xFF81 | 0xE3BF | 0xE5A0 | 0xE518) {
                            entry_point = Some(target);
                            break;
                        }
                    }
                }

                let dep_addr = if p + 0x31 < mem.len() {
                    Some(u16::from_le_bytes([mem[p + 0x30], mem[p + 0x31]]))
                } else {
                    Some(0x0100)
                };

                let start_addr = if p + 0x33 < mem.len() {
                    Some(u16::from_le_bytes([mem[p + 0x32], mem[p + 0x33]]))
                } else {
                    Some(0x0800)
                };

                let mut end_addr_ptr = None;
                for q in 0xED..0x108 {
                    if p + q + 4 < mem.len() && mem[p + q..p + q + 4] == [0xD0, 0xF7, 0x18, 0xA5] {
                        end_addr_ptr = Some(u16::from(mem[p + q + 4]));
                        break;
                    }
                }

                return Some(Box::new(SimplePacker::new(PackerInfo {
                    name: "ECA Compactor",
                    dep_addr,
                    start_addr,
                    end_addr: None,
                    entry_point,
                    end_addr_ptr,
                })));
            }
        }
    }
    None
}
