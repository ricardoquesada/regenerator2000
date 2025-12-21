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

pub fn analyze(state: &AppState) -> HashMap<u16, crate::state::Label> {
    let mut usage_map: HashMap<u16, (LabelPriority, Vec<u16>)> = HashMap::new();
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
            if current_type == AddressType::DataPtr {
                pc += 2; // words
            } else if current_type == AddressType::DataWord {
                pc += 2;
            } else {
                pc += 1;
            }
        }
    }

    // Generate labels
    let mut labels = HashMap::new();

    // 1. Process all used addresses
    for (addr, (priority, count)) in usage_map {
        // Check if there is an existing User label
        if let Some(existing) = state.labels.get(&addr) {
            if existing.kind == crate::state::LabelKind::User {
                labels.insert(
                    addr,
                    crate::state::Label {
                        name: existing.name.clone(),
                        kind: crate::state::LabelKind::User,
                        refs: existing.refs.clone(),
                    },
                );
                continue;
            }
        }

        // Otherwise generate Auto label
        let prefix = if priority == LabelPriority::Subroutine {
            's'
        } else if priority == LabelPriority::Field {
            'f'
        } else if is_external(addr, origin, data_len) {
            'e'
        } else {
            priority.prefix()
        };
        labels.insert(
            addr,
            crate::state::Label {
                name: format!("{}{:04X}", prefix, addr),
                kind: crate::state::LabelKind::Auto,
                refs: count,
            },
        );
    }

    // 2. Preserve User labels that have 0 references
    for (addr, label) in &state.labels {
        if label.kind == crate::state::LabelKind::User && !labels.contains_key(addr) {
            labels.insert(
                *addr,
                crate::state::Label {
                    name: label.name.clone(),
                    kind: crate::state::LabelKind::User,
                    refs: Vec::new(),
                },
            );
        }
    }

    labels
}

fn analyze_instruction(
    _state: &AppState,
    opcode: &Opcode,
    operands: &[u8],
    address: u16,
    usage_map: &mut HashMap<u16, (LabelPriority, Vec<u16>)>,
) {
    match opcode.mode {
        AddressingMode::Implied | AddressingMode::Accumulator | AddressingMode::Immediate => {}
        AddressingMode::ZeroPage => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // "f: if this is a field"
                update_usage(usage_map, addr, LabelPriority::Field, address);
            }
        }
        AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // Indexed zero page often used for arrays/fields
                update_usage(usage_map, addr, LabelPriority::Field, address);
            }
        }
        AddressingMode::Relative => {
            if !operands.is_empty() {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                // "b: ... branch opcodes"
                update_usage(usage_map, target, LabelPriority::Branch, address);
            }
        }
        AddressingMode::Absolute => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);

                if opcode.mnemonic == "JSR" {
                    update_usage(usage_map, target, LabelPriority::Subroutine, address);
                } else if opcode.mnemonic == "JMP" {
                    update_usage(usage_map, target, LabelPriority::Jump, address);
                } else {
                    // "a: ... absolute address"
                    update_usage(usage_map, target, LabelPriority::Absolute, address);
                }
            }
        }
        AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);
                // Indexed absolute is also "absolute address" usage
                update_usage(usage_map, target, LabelPriority::Absolute, address);
            }
        }
        AddressingMode::Indirect => {
            if operands.len() >= 2 {
                let pointer_addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                // "p: if this is a pointer"
                // The address `pointer_addr` is BEING USED a pointer.
                update_usage(usage_map, pointer_addr, LabelPriority::Pointer, address);
            }
        }
        AddressingMode::IndirectX => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base, X) -> points to a table of pointers in ZP? Or just ZP pointer?
                // It is "Indirect" X. The address `base` (and base+1) holds the address.
                // So `base` is a pointer.
                update_usage(usage_map, base, LabelPriority::Pointer, address);
            }
        }
        AddressingMode::IndirectY => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base), Y -> base is a ZP pointer.
                update_usage(usage_map, base, LabelPriority::Pointer, address);
            }
        }
        AddressingMode::Unknown => {}
    }
}

fn update_usage(
    map: &mut HashMap<u16, (LabelPriority, Vec<u16>)>,
    addr: u16,
    priority: LabelPriority,
    from_addr: u16,
) {
    map.entry(addr)
        .and_modify(|(p, refs)| {
            if priority > *p {
                *p = priority;
            }
            // Add reference if not already there (though duplications from same addr unlikely in single pass unless loop?)
            // Actually multiple refs from same instruction? No.
            // But we might want unique refs or all refs. Let's keep all for now, maybe sort/dedup later.
            refs.push(from_addr);
        })
        .or_insert_with(|| {
            let mut refs = Vec::new();
            refs.push(from_addr);
            (priority, refs)
        });
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
        assert_eq!(labels.get(&0x1005).map(|l| l.name.as_str()), Some("j1005"));
        // $1008 is JSR target -> s1008
        assert_eq!(labels.get(&0x1008).map(|l| l.name.as_str()), Some("s1008"));
        // $1000 is accessed via LDA (Absolute) -> a1000
        assert_eq!(labels.get(&0x1000).map(|l| l.name.as_str()), Some("a1000"));
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
        assert_eq!(labels.get(&0x2000).map(|l| l.name.as_str()), Some("s2000"));
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
        assert_eq!(labels.get(&0x0010).map(|l| l.name.as_str()), Some("f0010"));
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
        assert_eq!(labels.get(&0x0010).map(|l| l.name.as_str()), Some("e0010"));
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
        assert_eq!(labels.get(&0x1002).map(|l| l.name.as_str()), Some("b1002"));
        assert_eq!(
            labels.get(&0x1002).unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Case 2: $1002 -> jump to $1083. Usage: e1083 (External)
        assert_eq!(labels.get(&0x1083).map(|l| l.name.as_str()), Some("e1083"));
        assert_eq!(
            labels.get(&0x1083).unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Case 3: $1004 -> jump to $0F86. Usage: e0F86 (External)
        assert_eq!(labels.get(&0x0F86).map(|l| l.name.as_str()), Some("e0F86"));
        assert_eq!(
            labels.get(&0x0F86).unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Case 4: $1006 -> jump to $1007. Usage: b1007 (Internal)
        assert_eq!(labels.get(&0x1007).map(|l| l.name.as_str()), Some("b1007"));
        assert_eq!(
            labels.get(&0x1007).unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Case 5: $1008 -> jump to $1008. Usage: b1008 (Infinite loop)
        assert_eq!(labels.get(&0x1008).map(|l| l.name.as_str()), Some("b1008"));
        assert_eq!(
            labels.get(&0x1008).unwrap().kind,
            crate::state::LabelKind::Auto
        );
    }
}
