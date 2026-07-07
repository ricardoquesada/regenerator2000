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
            // Looking for Exomizer 3.0.2 pattern:
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
                    if mem[k] == 0x20 && mem[k + 1] == 0x00 && mem[k + 2] == 0x01 {
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

                return Some(PackerInfo {
                    name: "Exomizer v3.02+",
                    dep_addr: Some(0x0100 | (mem[p - 5] as u16)),
                    start_addr: Some(0x0801),
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
            // Look for C8 C0 34 D0 (INY, CPY #$34, BNE)
            if mem[p] == 0xC8 && mem[p + 1] == 0xC0 && mem[p + 3] == 0xD0 {
                let dep_low = mem[p + 2]; // 0x34
                // Typical Exomizer pattern follows
                if mem[p + 7] == 0x4C {
                    return Some(PackerInfo {
                        name: "Exomizer",
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
    if mem.len() > 0x817 && load_addr <= 0x0801 {
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
                start_addr: Some(0x0801),
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
    fn test_detect_exomizer_v3() {
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
        assert_eq!(info.name, "Exomizer v3.02+");
        assert_eq!(info.dep_addr, Some(0x0134));
        assert_eq!(info.start_addr, Some(0x0801));
        assert_eq!(info.entry_point, Some(0x0820));
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
        assert_eq!(info.name, "Exomizer");
        assert_eq!(info.dep_addr, Some(0x0150));
        assert_eq!(info.start_addr, Some(0x0801));
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
}
