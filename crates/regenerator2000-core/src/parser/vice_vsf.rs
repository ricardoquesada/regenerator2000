//! VICE Snapshot File (`.vsf`) parser.

use anyhow::{Result, anyhow};

const VSF_MIN_HEADER_LEN: usize = 37;
const VSF_MAGIC: &[u8; 19] = b"VICE Snapshot File\x1a";
const MODULE_HEADER_LEN: usize = 22;
const C64_RAM_SIZE: usize = 65_536;
const CART_ROM_BASE: usize = 0x8000;
const CART_MIN_ROM_SIZE: usize = 8_192;

/// Extracted memory image, CPU program counter, and machine identifier from a `.vsf` snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VsfData {
    /// 64 KiB system RAM image extracted from `C64MEM` (with optional `CARTGENERIC` overlay).
    pub memory: Vec<u8>,
    /// CPU `PC` register extracted from the `MAINCPU` module, if present.
    pub start_address: Option<u16>,
    /// Trimmed machine name from the VSF header (such as `"C64"` or `"C128"`).
    pub machine_name: String,
}

/// Parses a VICE Snapshot File (`.vsf`) byte slice into a [`VsfData`] record.
///
/// # Errors
///
/// Returns an error if `data` is shorter than 37 bytes, has an invalid magic signature,
/// contains a module with a size smaller than 22 bytes or extending past `data.len()`,
/// or lacks a valid `C64MEM` module.
pub fn parse_vsf(data: &[u8]) -> Result<VsfData> {
    if data.len() < VSF_MIN_HEADER_LEN {
        return Err(anyhow!("File too short to be a valid VSF file"));
    }

    if &data[0..19] != VSF_MAGIC {
        return Err(anyhow!("Invalid VSF signature"));
    }

    let machine_name = String::from_utf8_lossy(&data[21..37])
        .trim_matches(char::from(0))
        .to_string();

    let mut current_offset = VSF_MIN_HEADER_LEN;
    let mut c64mem_ram: Option<&[u8]> = None;
    let mut cart_rom: Option<&[u8]> = None;
    let mut pc: Option<u16> = None;

    while current_offset < data.len() {
        let Some(header_end) = current_offset.checked_add(MODULE_HEADER_LEN) else {
            break;
        };
        if header_end > data.len() {
            break;
        }

        let module_name = String::from_utf8_lossy(&data[current_offset..current_offset + 16]);
        let module_name_trimmed = module_name.trim_matches(char::from(0));

        if module_name.starts_with("VICE Version") {
            // Extended header record: 16-byte tag + 4-byte version + zero padding.
            current_offset += 16 + 4;
            while current_offset < data.len() && data[current_offset] == 0 {
                current_offset += 1;
            }
            continue;
        }

        let size = u32::from_le_bytes([
            data[current_offset + 18],
            data[current_offset + 19],
            data[current_offset + 20],
            data[current_offset + 21],
        ]) as usize;

        if size < MODULE_HEADER_LEN {
            return Err(anyhow!("Invalid module size"));
        }

        let Some(module_end) = current_offset.checked_add(size) else {
            return Err(anyhow!("Module {module_name_trimmed} truncated"));
        };
        if module_end > data.len() {
            return Err(anyhow!("Module {module_name_trimmed} truncated"));
        }

        let module_data = &data[header_end..module_end];

        match module_name_trimmed {
            "C64MEM" if module_data.len() >= 4 + C64_RAM_SIZE => {
                // Byte 0: CPUDATA ($01), Byte 1: CPUDIR ($00), Byte 2: EXROM, Byte 3: GAME, Bytes 4..: 64 KiB RAM.
                c64mem_ram = Some(&module_data[4..4 + C64_RAM_SIZE]);
            }
            "MAINCPU" if module_data.len() >= 14 => {
                // Offset 12..=13 holds the little-endian 16-bit program counter (`PC`).
                pc = Some(u16::from_le_bytes([module_data[12], module_data[13]]));
            }
            "CARTGENERIC" if module_data.len() >= CART_MIN_ROM_SIZE => {
                cart_rom = Some(module_data);
            }
            _ => {}
        }

        current_offset = module_end;
    }

    let Some(ram_slice) = c64mem_ram else {
        return Err(anyhow!("C64MEM module not found in VSF file"));
    };

    let mut memory = ram_slice.to_vec();
    if let Some(cart) = cart_rom {
        let max_cart_len = C64_RAM_SIZE - CART_ROM_BASE;
        if cart.len() <= max_cart_len {
            memory[CART_ROM_BASE..CART_ROM_BASE + cart.len()].copy_from_slice(cart);
        }
    }

    Ok(VsfData {
        memory,
        start_address: pc,
        machine_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vsf_header(machine: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(VSF_MAGIC);
        data.push(0); // Major
        data.push(0); // Minor
        let mut name_buf = [0u8; 16];
        let copy_len = machine.len().min(16);
        name_buf[..copy_len].copy_from_slice(&machine[..copy_len]);
        data.extend_from_slice(&name_buf);
        data
    }

    fn append_module(data: &mut Vec<u8>, name: &[u8], payload: &[u8]) {
        let mut name_buf = [0u8; 16];
        let copy_len = name.len().min(16);
        name_buf[..copy_len].copy_from_slice(&name[..copy_len]);
        data.extend_from_slice(&name_buf);
        data.push(0); // Major
        data.push(0); // Minor
        let total_size = (MODULE_HEADER_LEN + payload.len()) as u32;
        data.extend_from_slice(&total_size.to_le_bytes());
        data.extend_from_slice(payload);
    }

    fn make_c64mem_payload(fill: u8) -> Vec<u8> {
        let mut payload = vec![0x37, 0x2F, 0x00, 0x00];
        payload.extend(std::iter::repeat_n(fill, C64_RAM_SIZE));
        payload
    }

    #[test]
    fn test_parse_vsf_c64mem() {
        let mut data = make_vsf_header(b"C64");
        append_module(&mut data, b"C64MEM", &make_c64mem_payload(0xEA));

        let vsf = parse_vsf(&data).expect("Valid VSF must parse");
        assert_eq!(vsf.memory.len(), C64_RAM_SIZE);
        assert_eq!(vsf.memory[0], 0xEA);
        assert_eq!(vsf.memory[65535], 0xEA);
    }

    #[test]
    fn test_parse_vsf_with_pc() {
        let mut data = make_vsf_header(b"C64");
        append_module(&mut data, b"C64MEM", &make_c64mem_payload(0xEA));

        // Exact 14-byte MAINCPU payload with PC at bytes 12..=13.
        let mut cpu_data = vec![0u8; 14];
        cpu_data[12] = 0x34;
        cpu_data[13] = 0x12;
        append_module(&mut data, b"MAINCPU", &cpu_data);

        let vsf = parse_vsf(&data).expect("VSF with MAINCPU must parse");
        assert_eq!(vsf.start_address, Some(0x1234));
        assert_eq!(vsf.machine_name, "C64");
    }

    #[test]
    fn test_parse_vsf_cartgeneric_before_c64mem_preserves_cart_rom() {
        let mut data = make_vsf_header(b"C64");
        let cart_rom = vec![0x42u8; CART_MIN_ROM_SIZE];
        append_module(&mut data, b"CARTGENERIC", &cart_rom);
        append_module(&mut data, b"C64MEM", &make_c64mem_payload(0xEA));

        let vsf = parse_vsf(&data).expect("VSF with CARTGENERIC before C64MEM must parse");
        assert_eq!(vsf.memory[0x0000], 0xEA);
        assert_eq!(vsf.memory[0x8000], 0x42);
        assert_eq!(vsf.memory[0x9FFF], 0x42);
        assert_eq!(vsf.memory[0xA000], 0xEA);
    }
}
