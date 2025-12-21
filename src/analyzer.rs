use crate::cpu::{AddressingMode, Opcode};
use crate::state::{AddressType, AppState};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum LabelPriority {
    Field = 0,
    Absolute = 1,
    Pointer = 2,
    Branch = 3,
    Jump = 4,
    Subroutine = 5,
}

impl LabelPriority {
    fn prefix(&self) -> char {
        match self {
            LabelPriority::Field => 'f',
            LabelPriority::Absolute => 'a',
            LabelPriority::Pointer => 'p',
            LabelPriority::Branch => 'b',
            LabelPriority::Jump => 'j',
            LabelPriority::Subroutine => 's',
        }
    }
}

pub fn analyze(state: &AppState) -> HashMap<u16, String> {
    let mut usage_map: HashMap<u16, LabelPriority> = HashMap::new();
    let mut pc = 0;
    let data_len = state.raw_data.len();
    let origin = state.origin;

    while pc < data_len {
        let current_type = state
            .address_types
            .get(pc)
            .copied()
            .unwrap_or(AddressType::Code);

        if current_type == AddressType::Code {
            let opcode_byte = state.raw_data[pc];
            if let Some(opcode) = &state.disassembler.opcodes[opcode_byte as usize] {
                // Check if we have enough bytes
                if pc + opcode.size as usize <= data_len {
                    // Extract operands
                    let mut operands = Vec::new();
                    for i in 1..opcode.size {
                        operands.push(state.raw_data[pc + i as usize]);
                    }

                    analyze_instruction(
                        state,
                        opcode,
                        &operands,
                        origin.wrapping_add(pc as u16),
                        &mut usage_map,
                    );

                    pc += opcode.size as usize;
                } else {
                    pc += 1;
                }
            } else {
                pc += 1;
            }
        } else {
            // Data skip
            // Note: If we have explicit pointers (DataPtr), we should mark them?
            // "p: if this is a pointer".
            // If the user marked it as DataPtr, it IS a pointer.
            // But usually we label the DESTINATION?
            // "The automatic label should have the name of the address ... with a single letter as prefix"
            // "if this address is being called from..."
            // "p: if this is a pointer" - this sounds like the address ITSELF is a pointer.
            // If I have `AddressType::DataPtr` at $1000, then $1000 *contains* a pointer.
            // But is $1000 a pointer? Or is the value inside?
            // Usually we label the address. If I label $1000 as `p1000`, it implies $1000 is a pointer.
            // I will stick to usage-based for now, as that covers "being called from".
            // If I see JMP ($1234), I label $1234 as `p`.

            // However, iterating data to find pointers (if type is DataPtr) might be useful?
            // If AddressType::DataPtr is set, we treat the VALUE as an address that is pointed to?
            // No, DataPtr at $1000 means $1000 holds 2 bytes which is an address.
            // Does that mean $1000 should be labeled 'p'? Yes.
            // "p: if this is a pointer" -> This address holds a pointer.

            if current_type == AddressType::DataPtr {
                pc += 2; // words
            } else if current_type == AddressType::DataWord {
                pc += 2;
            } else {
                pc += 1;
            }
        }
    }

    // Generate strings
    let mut labels = HashMap::new();
    for (addr, priority) in usage_map {
        let prefix = if is_external(addr, origin, data_len) {
            'e'
        } else {
            priority.prefix()
        };
        labels.insert(addr, format!("{}{:04X}", prefix, addr));
    }

    labels
}

fn analyze_instruction(
    _state: &AppState,
    opcode: &Opcode,
    operands: &[u8],
    address: u16,
    usage_map: &mut HashMap<u16, LabelPriority>,
) {
    match opcode.mode {
        AddressingMode::Implied | AddressingMode::Accumulator | AddressingMode::Immediate => {}
        AddressingMode::ZeroPage => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // "f: if this is a field"
                update_usage(usage_map, addr, LabelPriority::Field);
            }
        }
        AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // Indexed zero page often used for arrays/fields
                update_usage(usage_map, addr, LabelPriority::Field);
            }
        }
        AddressingMode::Relative => {
            if !operands.is_empty() {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                // "b: ... branch opcodes"
                update_usage(usage_map, target, LabelPriority::Branch);
            }
        }
        AddressingMode::Absolute => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);

                if opcode.mnemonic == "JSR" {
                    update_usage(usage_map, target, LabelPriority::Subroutine);
                } else if opcode.mnemonic == "JMP" {
                    update_usage(usage_map, target, LabelPriority::Jump);
                } else {
                    // "a: ... absolute address"
                    update_usage(usage_map, target, LabelPriority::Absolute);
                }
            }
        }
        AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);
                // Indexed absolute is also "absolute address" usage
                update_usage(usage_map, target, LabelPriority::Absolute);
            }
        }
        AddressingMode::Indirect => {
            if operands.len() >= 2 {
                let pointer_addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                // "p: if this is a pointer"
                // The address `pointer_addr` is BEING USED a pointer.
                update_usage(usage_map, pointer_addr, LabelPriority::Pointer);
            }
        }
        AddressingMode::IndirectX => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base, X) -> points to a table of pointers in ZP? Or just ZP pointer?
                // It is "Indirect" X. The address `base` (and base+1) holds the address.
                // So `base` is a pointer.
                update_usage(usage_map, base, LabelPriority::Pointer);
            }
        }
        AddressingMode::IndirectY => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base), Y -> base is a ZP pointer.
                update_usage(usage_map, base, LabelPriority::Pointer);
            }
        }
        AddressingMode::Unknown => {}
    }
}

fn update_usage(map: &mut HashMap<u16, LabelPriority>, addr: u16, priority: LabelPriority) {
    map.entry(addr)
        .and_modify(|p| {
            if priority > *p {
                *p = priority
            }
        })
        .or_insert(priority);
}

fn is_external(addr: u16, origin: u16, len: usize) -> bool {
    let end = origin.wrapping_add(len as u16);
    if origin < end {
        addr < origin || addr >= end
    } else {
        // Wrap around case (rare but possible in u16)
        // If origin=F000, len=2000 (0x11000) -> end=1000
        // range is [F000..FFFF] U [0000..0FFF]
        // addr is external if it is in [1000..EFFF]
        // logic: if addr >= origin || addr < end -> internal.
        !(addr >= origin || addr < end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AddressType, AppState};

    #[test]
    fn test_analyze_simple() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // JMP $1005 (4C 05 10)
        // JSR $1008 (20 08 10)
        // NOP (EA)
        // LDA $1000 (AD 00 10)
        let data = vec![0x4C, 0x05, 0x10, 0x20, 0x08, 0x10, 0xEA, 0xAD, 0x00, 0x10];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // $1005 is JMP target -> j1005
        assert_eq!(labels.get(&0x1005), Some(&"j1005".to_string()));
        // $1008 is JSR target -> s1008
        assert_eq!(labels.get(&0x1008), Some(&"s1008".to_string()));
        // $1000 is accessed via LDA (Absolute) -> a1000
        assert_eq!(labels.get(&0x1000), Some(&"a1000".to_string()));
    }

    #[test]
    fn test_priority_override() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // JMP $2000 (4C 00 20) -> usage j2000
        // JSR $2000 (20 00 20) -> usage s2000 (override)
        let data = vec![0x4C, 0x00, 0x20, 0x20, 0x00, 0x20];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);
        // s > j
        assert_eq!(labels.get(&0x2000), Some(&"s2000".to_string()));
    }

    #[test]
    fn test_access_types_zp() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // LDA $10 (ZP) -> A5 10
        let data = vec![0xA5, 0x10];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; 2];

        let labels = analyze(&state);
        // ZP access -> Field -> f0010
        assert_eq!(labels.get(&0x0010), Some(&"f0010".to_string()));
    }

    #[test]
    fn test_external() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // JMP $0010 (External, out of range [1000..1003])
        let data = vec![0x4C, 0x10, 0x00];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; 3];

        let labels = analyze(&state);
        // External -> e0010
        assert_eq!(labels.get(&0x0010), Some(&"e0010".to_string()));
    }

    #[test]
    fn test_data_word_vs_ptr() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // $1000: DataWord ($2000) -> 00 20
        // $1002: DataPtr ($3000) -> 00 30
        let data = vec![0x00, 0x20, 0x00, 0x30];
        state.raw_data = data;
        state.address_types = vec![
            AddressType::DataWord,
            AddressType::DataWord,
            AddressType::DataPtr,
            AddressType::DataPtr,
        ];

        let labels = analyze(&state);

        // DataWord at $1000 should NOT generate label for ITSELF ($1000)
        assert_eq!(labels.get(&0x1000), None);
        // And NOT for its content ($2000)
        assert_eq!(labels.get(&0x2000), None);

        // DataPtr at $1002 used to generate label p1002. Now it SHOULD NOT.
        assert_eq!(labels.get(&0x1002), None);
    }
    #[test]
    fn test_branch_offsets() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // We will test several BNE instructions (D0) with different offsets.
        // Opcode size is 2 bytes.
        // Target = Address + 2 + Offset

        // 1. Offset $00 (+0)
        // Address $1000: D0 00
        // Target = 1000 + 2 + 0 = 1002

        // 2. Offset $7F (+127)
        // Address $1002: D0 7F
        // Target = 1002 + 2 + 127 = 1004 + 7F = 1083 (hex) check: 1004 + 127 = 4100 + 127 = 4227? No.
        // 0x1000 + 2 + 0x00 = 0x1002
        // 0x1002 + 2 + 0x7F = 0x1004 + 0x7F = 0x1083

        // 3. Offset $80 (-128)
        // Address $1004: D0 80
        // Target = 1004 + 2 - 128 = 1006 - 128 = 1006 - 0x80 = 0x0F86

        // 4. Offset $FF (-1)
        // Address $1006: D0 FF
        // Target = 1006 + 2 - 1 = 1008 - 1 = 1007

        // 5. Offset $FE (-2)
        // Address $1008: D0 FE
        // Target = 1008 + 2 - 2 = 1008

        let data = vec![0xD0, 0x00, 0xD0, 0x7F, 0xD0, 0x80, 0xD0, 0xFF, 0xD0, 0xFE];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Case 1: $1000 -> jump to $1002. Usage: b1002 (Internal)
        assert_eq!(labels.get(&0x1002), Some(&"b1002".to_string()));

        // Case 2: $1002 -> jump to $1083. Usage: e1083 (External)
        assert_eq!(labels.get(&0x1083), Some(&"e1083".to_string()));

        // Case 3: $1004 -> jump to $0F86. Usage: e0F86 (External)
        assert_eq!(labels.get(&0x0F86), Some(&"e0F86".to_string()));

        // Case 4: $1006 -> jump to $1007. Usage: b1007 (Internal)
        assert_eq!(labels.get(&0x1007), Some(&"b1007".to_string()));

        // Case 5: $1008 -> jump to $1008. Usage: b1008 (Infinite loop)
        assert_eq!(labels.get(&0x1008), Some(&"b1008".to_string()));
    }
}
