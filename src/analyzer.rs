use crate::cpu::{AddressingMode, Opcode};
use crate::state::{AddressType, AppState};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum LabelPriority {
    ZeroPage = 0,
    Field = 1,
    Absolute = 2,
    Pointer = 3,
    Branch = 4,
    Jump = 5,
    Subroutine = 6,
}

impl LabelPriority {
    fn prefix(&self) -> char {
        match self {
            LabelPriority::ZeroPage => 'a',
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
            if current_type == AddressType::Address {
                if pc + 2 <= data_len {
                    let low = state.raw_data[pc];
                    let high = state.raw_data[pc + 1];
                    let val = (high as u16) << 8 | (low as u16);
                    update_usage(
                        &mut usage_map,
                        val,
                        LabelPriority::Absolute,
                        origin.wrapping_add(pc as u16),
                    );
                    pc += 2;
                } else {
                    pc += 1;
                }
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
        let is_ext = is_external(addr, origin, data_len);

        let prefix = if is_ext
            && (priority == LabelPriority::Jump || priority == LabelPriority::Subroutine)
        {
            'e'
        } else {
            priority.prefix()
        };

        // Format name based on type
        // ZeroPage -> 2 digit hex
        // Field (in ZP) -> 2 digit hex
        // Others -> 4 digit hex
        let name = if priority == LabelPriority::ZeroPage
            || (priority == LabelPriority::Field && addr <= 0xFF)
        {
            format!("{}{:02X}", prefix, addr)
        } else {
            format!("{}{:04X}", prefix, addr)
        };
        labels.insert(
            addr,
            crate::state::Label {
                name,
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
                // "a: Zero Page Address"
                update_usage(usage_map, addr, LabelPriority::ZeroPage, address);
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
        // JMP $2000 (4C 00 20) -> usage j2000 -> external e2000
        // JSR $2000 (20 00 20) -> usage s2000 -> external e2000
        // Since both are external, and both allow external prefix, result is e2000.
        let data = vec![0x4C, 0x00, 0x20, 0x20, 0x00, 0x20];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);
        assert_eq!(labels.get(&0x2000).map(|l| l.name.as_str()), Some("e2000"));
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
        // ZP access -> ZeroPage Priority -> a10
        assert_eq!(labels.get(&0x0010).map(|l| l.name.as_str()), Some("a10"));
    }

    #[test]
    fn test_zp_field() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // LDA $50, X (B5 50) -> Field usage, ZP address
        let data = vec![0xB5, 0x50];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; 2];

        let labels = analyze(&state);
        // Field usage in ZP -> f50
        assert_eq!(labels.get(&0x0050).map(|l| l.name.as_str()), Some("f50"));
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
        // External Jump -> e0010
        // (Note: Jumps use 4 digits usually, unless we want e10?
        // User said "Only for external jumps... not for data".
        // Standard external handling logic still uses 4 digits for 'e' prefix in my code:
        // format!("{}{:04X}", prefix, addr)
        // ZeroPage special formatting is only for ZP priority or ZP Field.
        // Jump is Jump priority. So e0010 is correct.)
        assert_eq!(labels.get(&0x0010).map(|l| l.name.as_str()), Some("e0010"));
    }

    #[test]
    fn test_data_word_vs_address() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // $1000: DataWord ($2000) -> 00 20
        // $1002: Address ($1000) -> 00 10 (Internal)
        let data = vec![0x00, 0x20, 0x00, 0x10];
        state.raw_data = data;
        state.address_types = vec![
            AddressType::DataWord,
            AddressType::DataWord,
            AddressType::Address,
            AddressType::Address,
        ];

        let labels = analyze(&state);

        // DataWord at $1000 should NOT generate label for ITSELF ($1000)
        // BUT $1002 IS Reference to $1000. So $1000 SHOULD have a label now.
        // Address type usage at 1002 -> Absolute priority -> a1000
        assert_eq!(labels.get(&0x1000).map(|l| l.name.as_str()), Some("a1000"));

        // And content of DataWord ($2000) should still be None (assuming it's external/ignored)
        assert_eq!(labels.get(&0x2000), None);
    }
    #[test]
    fn test_branch_offsets() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // BNE instructions
        let data = vec![0xD0, 0x00, 0xD0, 0x7F, 0xD0, 0x80, 0xD0, 0xFF, 0xD0, 0xFE];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Case 1: $1000 -> jump to $1002. Usage: b1002 (Internal)
        assert_eq!(labels.get(&0x1002).map(|l| l.name.as_str()), Some("b1002"));

        // Case 2: $1002 -> jump to $1083. Usage: b1083 (External logic only applies to JMP/JSR)
        assert_eq!(labels.get(&0x1083).map(|l| l.name.as_str()), Some("b1083"));

        // Case 3: $1004 -> jump to $0F86. Usage: b0F86
        assert_eq!(labels.get(&0x0F86).map(|l| l.name.as_str()), Some("b0F86"));

        // Case 4: $1006 -> jump to $1007. Usage: b1007
        assert_eq!(labels.get(&0x1007).map(|l| l.name.as_str()), Some("b1007"));

        // Case 5: $1008 -> jump to $1008. Usage: b1008
        assert_eq!(labels.get(&0x1008).map(|l| l.name.as_str()), Some("b1008"));
    }
}
