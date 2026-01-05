use anyhow::{Result, anyhow};

pub struct VsfData {
    pub memory: Vec<u8>,
    pub start_address: Option<u16>,
}

pub fn parse_vsf(data: &[u8]) -> Result<VsfData> {
    if data.len() < 0x25 {
        // 19 + 2 + 16 minimal header
        return Err(anyhow!("File too short to be a valid VSF file"));
    }

    // 1. Check Magic "VICE Snapshot File\x1a"
    let magic = &data[0..19];
    let expected_magic = b"VICE Snapshot File\x1a";
    if magic != expected_magic {
        return Err(anyhow!("Invalid VSF signature"));
    }

    // 2. Parse Main Header
    // Offset 19: Major Version
    // Offset 20: Minor Version
    // Offset 21: Machine Name (16 bytes)

    let machine_name = &data[21..37];
    // We expect "C64" but let's be lenient or just check if it starts with C64
    if !machine_name.starts_with(b"C64") {
        // It might be valid to load other machines (like VIC20) if the user wants,
        // but we primarily support C64. Let's warn or fail?
    }

    let mut current_offset = 37; // 19 + 1 + 1 + 16

    let mut memory: Option<Vec<u8>> = None;
    let mut pc: Option<u16> = None;

    while current_offset < data.len() {
        // Module Header: Name (16), Major (1), Minor (1), Size (4) = 22 bytes
        // But need to check if we have at least 22 bytes.
        if current_offset + 22 > data.len() {
            println!(
                "DEBUG: Reached end of data or truncated header at offset {:x}",
                current_offset
            );
            break;
        }

        let module_name_bytes = &data[current_offset..current_offset + 16];
        let module_name = String::from_utf8_lossy(module_name_bytes);
        let module_name_trimmed = module_name.trim_matches(char::from(0));

        if module_name.starts_with("VICE Version") {
            // This is the extended header info, not a real module.
            // It consists of "VICE Version" (16 bytes) + 4 bytes version info.
            // Plus potential padding?
            current_offset += 16 + 4;

            // Skip any zero padding to find start of next module name (which should be ASCII)
            while current_offset < data.len() && data[current_offset] == 0 {
                current_offset += 1;
            }
            continue;
        }

        let _major = data[current_offset + 16];
        let _minor = data[current_offset + 17];

        // Size is Little Endian
        let size_bytes = [
            data[current_offset + 18],
            data[current_offset + 19],
            data[current_offset + 20],
            data[current_offset + 21],
        ];
        let size = u32::from_le_bytes(size_bytes) as usize;

        // Size includes the header (22 bytes) or not?
        // Spec: "SIZE: 4 bytes (DWORD, low-byte first), representing the total size of the module including this header"
        // Yes, includes header.

        if size < 22 {
            return Err(anyhow!("Invalid module size"));
        }

        // Ensure we don't go out of bounds
        if current_offset + size > data.len() {
            return Err(anyhow!("Module {} truncated", module_name_trimmed));
        }

        let module_data = &data[current_offset + 22..current_offset + size];

        if module_name_trimmed == "C64MEM" {
            // C64MEM Data:
            // Byte 0: CPU Data (RAM $01)
            // Byte 1: CPU Dir (RAM $00)
            // Byte 2: EXROM
            // Byte 3: GAME
            // Byte 4..: 64K RAM
            // Data length should be at least 4 + 65536
            if module_data.len() >= 4 + 65536 {
                if memory.is_none() {
                    let mut ram = vec![0u8; 65536];
                    ram.copy_from_slice(&module_data[4..4 + 65536]);
                    memory = Some(ram);
                } else {
                    // If we already have memory (e.g. allocated earlier?), unlikely for C64MEM.
                    // But if we did, we'd copy into it.
                    if let Some(mem) = &mut memory
                        && mem.len() >= 65536
                    {
                        mem[0..65536].copy_from_slice(&module_data[4..4 + 65536]);
                    }
                }
            }
        } else if module_name_trimmed == "MAINCPU" {
            // MAINCPU Data:
            // Regs at offset 0x0C: PC (Lo), PC (Hi)
            if module_data.len() > 14 {
                // Offset 12 seems to be PC (after 8 bytes clock? + ?)
                // From debug: 75 03 3B 00 00 00 00 00 00 01 00 FF [DC 8B]
                // 0-7: Clock?
                // 8: ?
                // 9: ?
                // 10: ?
                // 11: ?
                // 12: PC Lo
                // 13: PC Hi
                let pc_lo = module_data[12];
                let pc_hi = module_data[13];
                let pc_val = (pc_hi as u16) << 8 | (pc_lo as u16);
                pc = Some(pc_val);
            }
        } else if module_name_trimmed == "CARTGENERIC" {
            // Generic Cartridge Data usually mapped at $8000.
            // Can be 8KB or 16KB.
            let cart_len = module_data.len();
            if cart_len >= 8192 {
                // Ensure we have a memory buffer to write to.
                // C64MEM usually comes before or after? VSF order is not strictly guaranteed but usually C64MEM is early.
                // If memory is not yet allocated, we can allocate it (initialized to 0) and hope C64MEM fills the rest later.
                if memory.is_none() {
                    memory = Some(vec![0u8; 65536]);
                }

                if let Some(mem) = &mut memory
                    && mem.len() >= 0x8000 + cart_len
                {
                    mem[0x8000..0x8000 + cart_len].copy_from_slice(&module_data[0..cart_len]);
                    // Note: If 16KB, it fills $8000-$BFFF.
                }
            }
        }

        current_offset += size;
    }

    if let Some(mem) = memory {
        Ok(VsfData {
            memory: mem,
            start_address: pc,
        })
    } else {
        Err(anyhow!("C64MEM module not found in VSF file"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vsf_c64mem() {
        let mut data = Vec::new();
        // Magic
        data.extend_from_slice(b"VICE Snapshot File\x1a");
        // Major/Minor
        data.push(0);
        data.push(0);
        // Machine Name "C64" + padding
        data.extend_from_slice(b"C64");
        data.extend_from_slice(&[0u8; 13]);

        // Module "C64MEM"
        let mod_name = b"C64MEM";
        data.extend_from_slice(mod_name);
        data.extend_from_slice(&[0u8; 10]); // padding to 16
        data.push(0); // Major
        data.push(0); // Minor

        // Data size: 4 (header regs) + 65536 (RAM)
        let data_size = 4 + 65536;
        let total_size = 22 + data_size; // Header (22) + Data

        data.extend_from_slice(&(total_size as u32).to_le_bytes());

        // Module Data
        data.push(0x37); // CPUDATA
        data.push(0x2F); // CPUDIR
        data.push(0); // EXROM
        data.push(0); // GAME

        // RAM
        let ram = vec![0xEA; 65536]; // NOPs
        data.extend_from_slice(&ram);

        let result = parse_vsf(&data);
        assert!(result.is_ok());
        let vsf = result.expect("Result failed");
        assert_eq!(vsf.memory.len(), 65536);
        assert_eq!(vsf.memory[0], 0xEA);
        assert_eq!(vsf.memory[65535], 0xEA);
    }
    #[test]
    fn test_repro_frogger() {
        let path = std::path::Path::new("tests/frogger.vsf");
        if path.exists() {
            let data = std::fs::read(path).expect("Failed to read frogger.vsf");
            let res = parse_vsf(&data);
            assert!(res.is_ok());
            let vsf = res.expect("Failed to parse frogger.vsf");

            // Check potential PC at $8BDC
            let addr = 0x8BDC;
            if addr + 4 <= vsf.memory.len() {
                let actual = &vsf.memory[addr..addr + 4];
                println!("Content at PC {:04X}: {:02X?}", addr, actual);
            }
        } else {
            // Test is skipped if file not found
        }
    }
}
