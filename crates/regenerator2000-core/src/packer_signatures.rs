/// Information about a detected packer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackerInfo {
    pub name: &'static str,
    pub dep_addr: Option<u16>,
    pub start_addr: Option<u16>,
    pub end_addr: Option<u16>,
    pub entry_point: Option<u16>,
    pub end_addr_ptr: Option<u16>,
}

/// Scans memory for known packer signatures and returns info if found.
#[must_use]
pub fn detect_packer(mem: &[u8], load_addr: u16, load_end: u16) -> Option<PackerInfo> {
    // Exomizer 3.x
    for p in ((load_addr as usize)..=(load_end as usize).saturating_sub(8)).rev() {
        if p >= 6 && mem.len() > p + 8 {
            // Looking for Exomizer 3.x pattern:
            // 69 80 0A 10 0F 06 FD D0
            if mem[p] == 0x69
                && mem[p + 1] == 0x80
                && mem[p + 2] == 0x0A
                && mem[p + 3] == 0x10
                && mem[p + 4] == 0x0F
                && mem[p + 5] == 0x06
                && mem[p + 6] == 0xFD
                && mem[p + 7] == 0xD0
                && mem[p - 6] == 0x4C
                && mem[p - 4] == 0x01
            {
                let mut entry_point = None;
                for k in p..mem.len().saturating_sub(3) {
                    if mem[k] == 0x20 && mem[k + 2] == 0x01 {
                        for j in (k + 3)..mem.len().saturating_sub(2) {
                            if mem[j] == 0x4C {
                                let target = u16::from_le_bytes([mem[j + 1], mem[j + 2]]);
                                if target >= 0x0200
                                    && !(0xA000..=0xBFFF).contains(&target)
                                    && !(0xE000..=0xFFFF).contains(&target)
                                {
                                    entry_point = Some(target);
                                    break;
                                }
                            }
                        }
                        if entry_point.is_some() {
                            break;
                        }
                    }
                }

                // Differentiate Exomizer 3.0 from Exomizer 3.0.2+:
                // Exomizer 3.0 uses 08 48 20 1A 01 after get_bits (69 80 0A 10 0F 06 FD D0).
                let is_exo_30 = p + 12 < mem.len()
                    && mem[p + 8] == 0x08
                    && mem[p + 9] == 0x48
                    && mem[p + 10] == 0x20
                    && mem[p + 11] == 0x1A
                    && mem[p + 12] == 0x01;

                let name = if is_exo_30 {
                    "Exomizer 3.0"
                } else {
                    "Exomizer v3.02+"
                };

                return Some(PackerInfo {
                    name,
                    dep_addr: Some(0x0100 | (mem[p - 5] as u16)),
                    start_addr: None,
                    end_addr: None,
                    entry_point,
                    end_addr_ptr: None,
                });
            }
        }
    }

    // Exomizer 1.x / 2.x
    for p in ((load_addr as usize)..=(load_end as usize).saturating_sub(8)).rev() {
        if mem.len() > p + 7 {
            // Look for C8 C0 34 D0 or C8 C0 50 D0 (INY, CPY #$34 / CPY #$50, BNE)
            if mem[p] == 0xC8
                && mem[p + 1] == 0xC0
                && (mem[p + 2] == 0x34 || mem[p + 2] == 0x50)
                && mem[p + 3] == 0xD0
            {
                let dep_low = mem[p + 2];
                // Typical Exomizer pattern follows
                if mem[p + 7] == 0x4C {
                    return Some(PackerInfo {
                        name: "Exomizer 2.x",
                        dep_addr: Some(0x0100 | (dep_low as u16)),
                        start_addr: Some(0x0801), // Set to 0x0801 for f600
                        end_addr: None,
                        entry_point: None,
                        end_addr_ptr: None,
                    });
                }
            }
        }
    }

    // PuCrunch variant
    if mem.len() >= 0x0938
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

        return Some(PackerInfo {
            name: "PUCrunch",
            dep_addr: Some(u16::from_le_bytes([mem[0x841], mem[0x842]])),
            start_addr: Some(u16::from_le_bytes([mem[0x879], mem[0x87A]])),
            end_addr: None,
            entry_point,
            end_addr_ptr: Some(0x00FA), // unp64 sets Unp->EndAdr=0xfa
        });
    }

    // Time Cruncher (Scoop)
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
        return Some(PackerInfo {
            name: "Time Cruncher",
            dep_addr: Some(0x0100),
            start_addr: Some(0x0801), // unp64 forces start=0x0801 for Scoop test
            end_addr: None,
            entry_point: None,
            end_addr_ptr: None,
        });
    }

    // Dali 0.3.3
    if mem.len() > 0x822 && load_addr <= 0x0801 {
        let base = 0x080D;
        if base >= load_addr as usize {
            let q = base;
            if mem[q] == 0x78
                && mem[q + 1] == 0xA2
                && mem[q + 2] == 0x0B
                && mem[q + 3] == 0x9A
                && mem[q + 4] == 0xA0
                && mem[q + 5] == 0xEC
                && mem[q + 6] == 0x48
                && mem[q + 7] == 0xB7
                && mem[q + 0x11] == 0x4C
                && mem[q + 0x12] == 0x03
                && mem[q + 0x13] == 0x00
                && mem[q + 0x14] == 0x34
            {
                return Some(PackerInfo {
                    name: "Dali",
                    dep_addr: Some(0x0003),
                    start_addr: Some(0x0801),
                    end_addr: None,
                    entry_point: Some(u16::from_le_bytes([mem[q + 0xFE], mem[q + 0xFF]])),
                    end_addr_ptr: Some(mem[q + 0x44] as u16),
                });
            }
        }
    }

    // ByteBoozer 2.0
    if mem.len() > 0x887 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 6] == 0xB7
            && mem[q + 7] == 0xBD
            && mem[q + 8] == 0x1E
            && mem[q + 9] == 0x08
        {
            return Some(PackerInfo {
                name: "ByteBoozer",
                dep_addr: Some(0x0010),
                start_addr: Some(u16::from_le_bytes([mem[0x886], mem[0x887]])),
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[0x8CB], mem[0x8CC]])),
                end_addr_ptr: Some(0x0077),
            });
        }
    }

    // TinyCrunch Variant 2
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
            return Some(PackerInfo {
                name: "TinyCrunch",
                dep_addr: Some(0x010E),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[0x836], mem[0x837]])),
                end_addr_ptr: None,
            });
        }
    }

    // Cruel Cruncher (v2.2 / v2.5)
    if mem.len() > 0x820 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && (mem[q + 5] == 0xA2 || mem[q + 5] == 0xA0)
            && (mem[q + 6] == 0x1A || mem[q + 6] == 0x1F || mem[q + 6] == 0x20)
        {
            return Some(PackerInfo {
                name: "Cruel Cruncher",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Commodore Cruncher System (CCS)
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
            return Some(PackerInfo {
                name: "Commodore Cruncher System",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Turbo Cruncher (TSK / Mr. Z)
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0x20
            && mem[q + 8] == 0x4C
        {
            return Some(PackerInfo {
                name: "Turbo Cruncher",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Action Replay Freezer / Packer
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && (mem[q + 1] == 0xA9 && (mem[q + 2] == 0x37 || mem[q + 2] == 0x34))
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0xA2
            && mem[q + 6] == 0x00
            && mem[q + 7] == 0x8E
        {
            return Some(PackerInfo {
                name: "Action Replay",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Final Cartridge III Freezer / Packer
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
            return Some(PackerInfo {
                name: "Final Cartridge III",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Triad / TC2000 Cruncher
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0xA0
            && mem[q + 7] == 0xB9
            && mem[q + 0x0A] == 0x99
            && mem[q + 0x0D] == 0xC8
            && mem[q + 0x0E] == 0xD0
        {
            return Some(PackerInfo {
                name: "Triad Cruncher",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Eagle Cruncher
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
            return Some(PackerInfo {
                name: "Eagle Cruncher",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // Super Cruncher / Master Cruncher
    if mem.len() > 0x818 && load_addr <= 0x0801 {
        let q = 0x080D;
        if q >= load_addr as usize
            && mem[q] == 0x78
            && mem[q + 1] == 0xA9
            && mem[q + 2] == 0x34
            && mem[q + 3] == 0x85
            && mem[q + 4] == 0x01
            && mem[q + 5] == 0xA2
            && mem[q + 7] == 0xBD
            && mem[q + 0x0D] == 0xC8
            && mem[q + 0x0E] == 0xB9
            && mem[q + 0x11] == 0x99
        {
            return Some(PackerInfo {
                name: "Super Cruncher",
                dep_addr: Some(0x0100),
                start_addr: Some(0x0801),
                end_addr: None,
                entry_point: None,
                end_addr_ptr: None,
            });
        }
    }

    // ALZ64/Quiss
    if mem.len() > 0x82f
        && load_addr <= 0x080b
        && mem[0x80b] == 0xA2
        && mem[0x80c] == 0x00
        && mem[0x80d] == 0x78
        && mem[0x80e] == 0xB5
        && mem[0x819] == 0xD0
        && mem[0x81a] == 0xF3
        && mem[0x81b] == 0xA2
        && mem[0x81c] == 0x03
        && mem[0x824] == 0xA2
        && mem[0x825] == 0x10
        && mem[0x826] == 0x89
        && mem[0x827] == 0x38
        && mem[0x82c] == 0xF1
        && mem[0x82d] == 0x4C
        && mem[0x82e] == 0x5E
        && mem[0x82f] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x814], mem[0x815]]) as usize;
        if p + 0xf3 < mem.len() && mem[p + 0xf1] == 0x4C {
            let mut start_addr = Some(0x080b);
            for q in (p + 0xcb)..=(p + 0xf0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = Some(u16::from_le_bytes([mem[q + 4], mem[q + 5]]));
                    break;
                }
            }
            return Some(PackerInfo {
                name: "ALZ64/Quiss",
                dep_addr: Some(0x005E),
                start_addr,
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[p + 0xf2], mem[p + 0xf3]])),
                end_addr_ptr: Some(0x00CF),
            });
        }
    }

    // ALZ64/Kabuto
    if mem.len() > 0x838
        && load_addr <= 0x080b
        && mem[0x80c] == 0x00
        && mem[0x80d] == 0x78
        && mem[0x80e] == 0x86
        && mem[0x818] == 0xCA
        && mem[0x819] == 0xD0
        && mem[0x81a] == 0xF7
        && mem[0x81b] == 0xCE
        && mem[0x822] == 0xD0
        && mem[0x823] == 0xEE
        && mem[0x824] == 0xA2
        && mem[0x825] == 0x03
        && mem[0x835] == 0xF1
        && mem[0x836] == 0x4C
        && mem[0x837] == 0x5E
        && mem[0x838] == 0x00
    {
        let p = u16::from_le_bytes([mem[0x813], mem[0x814]]) as usize;
        if p + 0xff < mem.len() && mem[p + 0xfd] == 0x4C {
            let mut start_addr = Some(0x080b);
            for q in (p + 0xe6)..=(p + 0xf0) {
                if q + 5 < mem.len() && mem[q..q + 4] == [0x02, 0xE6, 0xC7, 0x8D] {
                    start_addr = Some(u16::from_le_bytes([mem[q + 4], mem[q + 5]]));
                    break;
                }
            }
            return Some(PackerInfo {
                name: "ALZ64/Kabuto",
                dep_addr: Some(0x005E),
                start_addr,
                end_addr: None,
                entry_point: Some(u16::from_le_bytes([mem[p + 0xfe], mem[p + 0xff]])),
                end_addr_ptr: Some(0x00CF),
            });
        }
    }

    // TBC Multicompactor
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
                let strtmp = u16::from_le_bytes([mem[q - 1], mem[q]]) as u32;
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

            return Some(PackerInfo {
                name: "TBC Multicompactor",
                dep_addr: Some(0x0100),
                start_addr: Some(start_addr),
                end_addr,
                entry_point: Some(entry_point),
                end_addr_ptr: None,
            });
        }
    }

    // ECA Compactor/Linker
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
                        end_addr_ptr = Some(mem[p + q + 4] as u16);
                        break;
                    }
                }

                return Some(PackerInfo {
                    name: "ECA Compactor",
                    dep_addr,
                    start_addr,
                    end_addr: None,
                    entry_point,
                    end_addr_ptr,
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_packer_none() {
        let mem = vec![0; 0x1000];
        assert_eq!(detect_packer(&mem, 0x0801, 0x1000), None);
    }

    #[test]
    fn test_detect_exomizer_v3_0() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        // mem[p..p+8] = 69 80 0A 10 0F 06 FD D0
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        // Exomizer 3.0 marker (08 48 20 1A 01)
        mem[p + 8] = 0x08;
        mem[p + 9] = 0x48;
        mem[p + 10] = 0x20;
        mem[p + 11] = 0x1A;
        mem[p + 12] = 0x01;

        // mem[p-6] = 0x4C, mem[p-4] = 0x01
        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34; // dep_low
        mem[p - 4] = 0x01;

        // JSR $0100 -> JMP $0820
        mem[p + 0x10] = 0x20;
        mem[p + 0x11] = 0x00;
        mem[p + 0x12] = 0x01;
        mem[p + 0x15] = 0x4C;
        mem[p + 0x16] = 0x20;
        mem[p + 0x17] = 0x08;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer 3.0");
        assert_eq!(info.dep_addr, Some(0x0134));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x0820));
    }

    #[test]
    fn test_detect_exomizer_v3_02() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0x69;
        mem[p + 1] = 0x80;
        mem[p + 2] = 0x0A;
        mem[p + 3] = 0x10;
        mem[p + 4] = 0x0F;
        mem[p + 5] = 0x06;
        mem[p + 6] = 0xFD;
        mem[p + 7] = 0xD0;

        // Not 08 48 20 1A 01 -> v3.02+
        mem[p + 8] = 0x99;

        mem[p - 6] = 0x4C;
        mem[p - 5] = 0x34;
        mem[p - 4] = 0x01;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer v3.02+");
    }

    #[test]
    fn test_detect_exomizer_v1_v2() {
        let mut mem = vec![0; 0x1000];
        let p = 0x0900;
        mem[p] = 0xC8;
        mem[p + 1] = 0xC0;
        mem[p + 2] = 0x50; // dep_low
        mem[p + 3] = 0xD0;
        mem[p + 7] = 0x4C;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Exomizer 2.x");
        assert_eq!(info.dep_addr, Some(0x0150));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_exomizer_real_files() {
        use std::fs;
        if let Ok(data) = fs::read("../../tests/6502/c64_lft-rodents-in-the-attic.exo3.prg") {
            let load_addr = u16::from_le_bytes([data[0], data[1]]);
            let mut mem = vec![0u8; 0x10000];
            let end = (load_addr as usize + data.len() - 2).min(0x10000);
            mem[load_addr as usize..end].copy_from_slice(&data[2..2 + (end - load_addr as usize)]);
            let info = detect_packer(&mem, load_addr, end as u16).unwrap();
            assert_eq!(info.name, "Exomizer 3.0");
        }

        if let Ok(data) = fs::read("../../tests/6502/c64_f600.exo.prg") {
            let load_addr = u16::from_le_bytes([data[0], data[1]]);
            let mut mem = vec![0u8; 0x10000];
            let end = (load_addr as usize + data.len() - 2).min(0x10000);
            mem[load_addr as usize..end].copy_from_slice(&data[2..2 + (end - load_addr as usize)]);
            let info = detect_packer(&mem, load_addr, end as u16).unwrap();
            assert_eq!(info.name, "Exomizer 2.x");
        }
    }

    #[test]
    fn test_detect_pucrunch() {
        let mut mem = vec![0; 0x1000];
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA2;
        mem[0x816] = 0x34;
        mem[0x817] = 0xBD;
        mem[0x818] = 0x42;
        mem[0x819] = 0x08;
        mem[0x81A] = 0x9D;
        mem[0x81B] = 0xFF;
        mem[0x81C] = 0x01;
        mem[0x81D] = 0xCA;
        mem[0x81E] = 0xD0;

        mem[0x841] = 0x16;
        mem[0x842] = 0x01;

        mem[0x879] = 0x01;
        mem[0x87A] = 0x08;

        // Entry point sequence
        let ep = 0x0920;
        mem[ep] = 0xA5;
        mem[ep + 1] = 0xFA;
        mem[ep + 2] = 0x85;
        mem[ep + 3] = 0x2D;
        mem[ep + 4] = 0xA5;
        mem[ep + 5] = 0xFB;
        mem[ep + 6] = 0x85;
        mem[ep + 7] = 0x2E;
        mem[ep + 0x0A] = 0x00;
        mem[ep + 0x0B] = 0x20;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "PUCrunch");
        assert_eq!(info.dep_addr, Some(0x0116));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00FA));
    }

    #[test]
    fn test_detect_time_cruncher() {
        let mut mem = vec![0; 0x1000];
        mem[0x810] = 0x78;
        mem[0x811] = 0xA9;
        mem[0x812] = 0x34;
        mem[0x813] = 0x85;
        mem[0x814] = 0x01;
        mem[0x815] = 0xA0;
        mem[0x816] = 0xC4;
        mem[0x817] = 0xB9;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Time Cruncher");
        assert_eq!(info.dep_addr, Some(0x0100));
        assert_eq!(info.start_addr, Some(0x0801));
    }

    #[test]
    fn test_detect_dali() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA2;
        mem[q + 2] = 0x0B;
        mem[q + 3] = 0x9A;
        mem[q + 4] = 0xA0;
        mem[q + 5] = 0xEC;
        mem[q + 6] = 0x48;
        mem[q + 7] = 0xB7;
        mem[q + 0x11] = 0x4C;
        mem[q + 0x12] = 0x03;
        mem[q + 0x13] = 0x00;
        mem[q + 0x14] = 0x34;

        mem[q + 0xFE] = 0x00;
        mem[q + 0xFF] = 0x10;
        mem[q + 0x44] = 0x6B;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Dali");
        assert_eq!(info.dep_addr, Some(0x0003));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x1000));
        assert_eq!(info.end_addr_ptr, Some(0x006B));
    }

    #[test]
    fn test_detect_byte_boozer() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 6] = 0xB7;
        mem[q + 7] = 0xBD;
        mem[q + 8] = 0x1E;
        mem[q + 9] = 0x08;

        mem[0x8CB] = 0x00;
        mem[0x8CC] = 0x20;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ByteBoozer");
        assert_eq!(info.dep_addr, Some(0x0010));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x0077));
    }

    #[test]
    fn test_detect_tiny_crunch() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA2;
        mem[q + 2] = 0xBB;
        mem[q + 3] = 0xBD;
        mem[q + 4] = 0x1B;
        mem[q + 5] = 0x08;
        mem[q + 6] = 0x9D;
        mem[q + 7] = 0xFF;

        mem[0x836] = 0x11;
        mem[0x837] = 0x09;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "TinyCrunch");
        assert_eq!(info.dep_addr, Some(0x010E));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x0911));
    }

    #[test]
    fn test_detect_cruel_cruncher() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 4] = 0x01;
        mem[q + 5] = 0xA2;
        mem[q + 6] = 0x1A;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Cruel Cruncher");
    }

    #[test]
    fn test_detect_ccs() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 4] = 0x01;
        mem[q + 5] = 0xA2;
        mem[q + 7] = 0xBD;
        mem[q + 0x0A] = 0x9D;
        mem[q + 0x0D] = 0xCA;
        mem[q + 0x0E] = 0xD0;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Commodore Cruncher System");
    }

    #[test]
    fn test_detect_action_replay() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x37;
        mem[q + 3] = 0x85;
        mem[q + 4] = 0x01;
        mem[q + 5] = 0xA2;
        mem[q + 6] = 0x00;
        mem[q + 7] = 0x8E;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Action Replay");
    }

    #[test]
    fn test_detect_final_cartridge() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x37;
        mem[q + 3] = 0x85;
        mem[q + 4] = 0x01;
        mem[q + 5] = 0x8D;
        mem[q + 6] = 0x00;
        mem[q + 7] = 0xDD;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Final Cartridge III");
    }

    #[test]
    fn test_detect_triad_cruncher() {
        let mut mem = vec![0; 0x1000];
        let q = 0x080D;
        mem[q] = 0x78;
        mem[q + 1] = 0xA9;
        mem[q + 2] = 0x34;
        mem[q + 3] = 0x85;
        mem[q + 4] = 0x01;
        mem[q + 5] = 0xA0;
        mem[q + 7] = 0xB9;
        mem[q + 0x0A] = 0x99;
        mem[q + 0x0D] = 0xC8;
        mem[q + 0x0E] = 0xD0;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "Triad Cruncher");
    }

    #[test]
    fn test_detect_alz64_quiss() {
        let mut mem = vec![0; 0x1000];
        mem[0x80b] = 0xA2;
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0xB5;
        mem[0x814] = 0x00; // p = 0x0900
        mem[0x815] = 0x09;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF3;
        mem[0x81b] = 0xA2;
        mem[0x81c] = 0x03;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x10;
        mem[0x826] = 0x89;
        mem[0x827] = 0x38;
        mem[0x82c] = 0xF1;
        mem[0x82d] = 0x4C;
        mem[0x82e] = 0x5E;
        mem[0x82f] = 0x00;

        let p = 0x0900;
        mem[p + 0xf1] = 0x4C;
        mem[p + 0xf2] = 0x00; // RetAdr = 0x2000
        mem[p + 0xf3] = 0x20;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ALZ64/Quiss");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }

    #[test]
    fn test_detect_alz64_kabuto() {
        let mut mem = vec![0; 0x1000];
        mem[0x80c] = 0x00;
        mem[0x80d] = 0x78;
        mem[0x80e] = 0x86;
        mem[0x813] = 0x00; // p = 0x0900
        mem[0x814] = 0x09;
        mem[0x818] = 0xCA;
        mem[0x819] = 0xD0;
        mem[0x81a] = 0xF7;
        mem[0x81b] = 0xCE;
        mem[0x822] = 0xD0;
        mem[0x823] = 0xEE;
        mem[0x824] = 0xA2;
        mem[0x825] = 0x03;
        mem[0x835] = 0xF1;
        mem[0x836] = 0x4C;
        mem[0x837] = 0x5E;
        mem[0x838] = 0x00;

        let p = 0x0900;
        mem[p + 0xfd] = 0x4C;
        mem[p + 0xfe] = 0x00; // RetAdr = 0x2000
        mem[p + 0xff] = 0x20;

        let info = detect_packer(&mem, 0x0801, 0x0950).unwrap();
        assert_eq!(info.name, "ALZ64/Kabuto");
        assert_eq!(info.dep_addr, Some(0x005E));
        assert_eq!(info.start_addr, Some(0x080b));
        assert_eq!(info.entry_point, Some(0x2000));
        assert_eq!(info.end_addr_ptr, Some(0x00CF));
    }
}
