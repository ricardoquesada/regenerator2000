use crate::cpu::{AddressingMode, Opcode};
use crate::state::{AddressType, AppState};
use std::collections::HashMap;

use crate::state::LabelType;

pub fn analyze(state: &AppState) -> HashMap<u16, crate::state::Label> {
    // We want to track ALL usages, illegal or not, and then pick the best ones.
    // Map: Address -> Set of used LabelTypes
    // We also need ref counts.
    // Map: Address -> (HashMap<LabelType, usize>, Vec<u16>, LabelType)
    // storing (counts, refs, first_seen_type)
    let mut usage_map: HashMap<u16, (HashMap<LabelType, usize>, Vec<u16>, LabelType)> =
        HashMap::new();
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
                        LabelType::AbsoluteAddress,
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
    for (addr, (types_map, refs, first_type)) in usage_map {
        // Check if there is an existing User label
        if let Some(existing) = state.labels.get(&addr) {
            if existing.kind == crate::state::LabelKind::User {
                labels.insert(
                    addr,
                    crate::state::Label {
                        name: existing.name.clone(),
                        names: existing.names.clone(), // Preserve existing context names if any (or should we overwrite auto ones?)
                        // If it is user label, we trust the user. But ideally we might want to Add new auto-detected context names if they are missing?
                        // For now, let's just preserve.
                        kind: crate::state::LabelKind::User,
                        refs: existing.refs.clone(),
                    },
                );
                continue;
            }
        }

        // Generate Auto label
        let is_ext = state.is_external(addr);

        // Generate names for all discovered types
        let mut names = HashMap::new();
        for (l_type, _) in &types_map {
            // Check for external jump case
            let effective_type = if is_ext
                && (*l_type == LabelType::Jump
                    || *l_type == LabelType::Subroutine
                    || *l_type == LabelType::Branch)
            {
                LabelType::ExternalJump
            } else {
                *l_type
            };

            let prefix = effective_type.prefix();

            let name = if effective_type == LabelType::ZeroPageAbsoluteAddress
                || effective_type == LabelType::ZeroPageField
                || effective_type == LabelType::ZeroPagePointer
            {
                format!("{}{:02X}", prefix, addr)
            } else {
                format!("{}{:04X}", prefix, addr)
            };
            names.insert(effective_type, name);
        }

        // Determine default name (highest priority)
        // We can sort types or just pick one. `LabelType` derives Ord. Higher enum value = higher priority?
        // In the enum definition:
        // ZeroPageField=0, Field=1, ZeroPageAbsoluteAddress=2, AbsoluteAddress=3, Pointer=4, ZeroPagePointer=5,
        // ExternalJump=6, Jump=7, Subroutine=8, Branch=9, Predefined=10, UserDefined=11

        // Determine default name (highest priority)
        // If INTERNAL, we use the `first_type` (the type of the first instruction that referenced it).
        // If EXTERNAL, we stick to the priority logic (usually ExternalJump or Absolute).
        let best_type = if !is_ext {
            first_type
        } else {
            types_map
                .keys()
                .map(|t| {
                    if is_ext
                        && (*t == LabelType::Jump
                            || *t == LabelType::Subroutine
                            || *t == LabelType::Branch)
                    {
                        LabelType::ExternalJump
                    } else {
                        *t
                    }
                })
                .max()
                .unwrap_or(LabelType::AbsoluteAddress)
        };

        let default_name = names
            .get(&best_type)
            .cloned()
            .unwrap_or_else(|| format!("a{:04X}", addr));

        labels.insert(
            addr,
            crate::state::Label {
                name: default_name,
                names,
                kind: crate::state::LabelKind::Auto,
                refs, // We use all refs
            },
        );
    }

    // 2. Preserve User labels that have 0 references (or weren't found in this pass)
    for (addr, label) in &state.labels {
        if label.kind == crate::state::LabelKind::User && !labels.contains_key(addr) {
            labels.insert(
                *addr,
                crate::state::Label {
                    name: label.name.clone(),
                    names: label.names.clone(),
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
    usage_map: &mut HashMap<u16, (HashMap<LabelType, usize>, Vec<u16>, LabelType)>,
) {
    match opcode.mode {
        AddressingMode::Implied | AddressingMode::Accumulator | AddressingMode::Immediate => {}
        AddressingMode::ZeroPage => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // "a: Zero Page Address"
                update_usage(usage_map, addr, LabelType::ZeroPageAbsoluteAddress, address);
            }
        }
        AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
            if !operands.is_empty() {
                let addr = operands[0] as u16;
                // Indexed zero page often used for arrays/fields
                update_usage(usage_map, addr, LabelType::ZeroPageField, address);
            }
        }
        AddressingMode::Relative => {
            if !operands.is_empty() {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                // "b: ... branch opcodes"
                update_usage(usage_map, target, LabelType::Branch, address);
            }
        }
        AddressingMode::Absolute => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);

                if opcode.mnemonic == "JSR" {
                    update_usage(usage_map, target, LabelType::Subroutine, address);
                } else if opcode.mnemonic == "JMP" {
                    update_usage(usage_map, target, LabelType::Jump, address);
                } else {
                    // "a: ... absolute address"
                    update_usage(usage_map, target, LabelType::AbsoluteAddress, address);
                }
            }
        }
        AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
            if operands.len() >= 2 {
                let target = (operands[1] as u16) << 8 | (operands[0] as u16);
                // Indexed absolute is also "absolute address" usage
                update_usage(usage_map, target, LabelType::Field, address);
            }
        }
        AddressingMode::Indirect => {
            if operands.len() >= 2 {
                let pointer_addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                // "p: if this is a pointer"
                // The address `pointer_addr` is BEING USED a pointer.
                update_usage(usage_map, pointer_addr, LabelType::Pointer, address);
            }
        }
        AddressingMode::IndirectX => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base, X) -> points to a table of pointers in ZP? Or just ZP pointer?
                // It is "Indirect" X. The address `base` (and base+1) holds the address.
                // So `base` is a pointer.
                update_usage(usage_map, base, LabelType::ZeroPagePointer, address);
            }
        }
        AddressingMode::IndirectY => {
            if !operands.is_empty() {
                let base = operands[0] as u16;
                // (base), Y -> base is a ZP pointer.
                update_usage(usage_map, base, LabelType::ZeroPagePointer, address);
            }
        }
        AddressingMode::Unknown => {}
    }
}

fn update_usage(
    map: &mut HashMap<u16, (HashMap<LabelType, usize>, Vec<u16>, LabelType)>,
    addr: u16,
    priority: LabelType,
    from_addr: u16,
) {
    map.entry(addr)
        .and_modify(|(types, refs, _)| {
            *types.entry(priority).or_insert(0) += 1;
            refs.push(from_addr);
        })
        .or_insert_with(|| {
            let mut types = HashMap::new();
            types.insert(priority, 1);
            let mut refs = Vec::new();
            refs.push(from_addr);
            (types, refs, priority)
        });
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
        // Changed expectations: explicit ExternalJump type logic
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

        // Case 2: $1002 -> jump to $1083. Usage: e1083 (External logic applies to Branch too now)
        assert_eq!(labels.get(&0x1083).map(|l| l.name.as_str()), Some("e1083"));

        // Case 3: $1004 -> jump to $0F86. Usage: e0F86
        assert_eq!(labels.get(&0x0F86).map(|l| l.name.as_str()), Some("e0F86"));

        // Case 4: $1006 -> jump to $1007. Usage: b1007
        assert_eq!(labels.get(&0x1007).map(|l| l.name.as_str()), Some("b1007"));

        // Case 5: $1008 -> jump to $1008. Usage: b1008
        assert_eq!(labels.get(&0x1008).map(|l| l.name.as_str()), Some("b1008"));
    }

    #[test]
    fn test_new_pointer_field_types() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // Indirect JMP (JMP ($1000)) -> 6C 00 10
        // LDA ($10, X) -> A1 10
        // LDA ($10), Y -> B1 10
        // LDA $1000, X -> BD 00 10
        // LDA $1000, Y -> B9 00 10
        // LDA $10, X -> B5 10
        let data = vec![
            0x6C, 0x00, 0x10, // JMP ($1000) -> Indirect -> p1000
            0xA1, 0x10, // LDA ($10, X) -> Indirect X -> p10
            0xB1, 0x20, // LDA ($20), Y -> Indirect Y -> p20
            0xBD, 0x50, 0x10, // LDA $1050, X -> Absolute X -> f1050
            0xB9, 0x60, 0x10, // LDA $1060, Y -> Absolute Y -> f1060
            0xB5, 0x30, // LDA $30, X -> ZeroPage X -> f30
        ];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Indirect JMP -> p1000
        assert_eq!(labels.get(&0x1000).map(|l| l.name.as_str()), Some("p1000"));

        // Indirect X -> p10
        assert_eq!(labels.get(&0x0010).map(|l| l.name.as_str()), Some("p10"));

        // Indirect Y -> p20
        assert_eq!(labels.get(&0x0020).map(|l| l.name.as_str()), Some("p20"));

        // Absolute X -> f1050
        assert_eq!(labels.get(&0x1050).map(|l| l.name.as_str()), Some("f1050"));

        // Absolute Y -> f1060
        assert_eq!(labels.get(&0x1060).map(|l| l.name.as_str()), Some("f1060"));

        // ZeroPage X -> f30
        assert_eq!(labels.get(&0x0030).map(|l| l.name.as_str()), Some("f30"));
    }

    #[test]
    fn test_internal_label_priority() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // Scenario: Internal address $1005 accessed via:
        // 1. Branch (BNE $1005) -> D0 03 (assuming PC is 1000 + 2 = 1002. 1002+3=1005)
        // 2. Jump (JMP $1005) -> 4C 05 10
        // 3. Subroutine (JSR $1005) -> 20 05 10
        //
        // Priority should be Subroutine ("s") > Jump ("j") > Branch ("b").
        // Result should be b1005 (First usage wins).

        // $1000: BNE $1005 (D0 03)
        // $1002: JMP $1005 (4C 05 10)
        // $1005: JSR $1005 (20 05 10) (recursive call just for usage)

        let data = vec![
            0xD0, 0x03, // 1000: BNE +3 -> 1005
            0x4C, 0x05, 0x10, // 1002: JMP $1005
            0x20, 0x05, 0x10, // 1005: JSR $1005
        ];

        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Check 1005
        // Expect 'b' because it was referenced by Branch FIRST.
        assert_eq!(labels.get(&0x1005).map(|l| l.name.as_str()), Some("b1005"));

        // Also verify usage map contains all types?
        // We can't easily check usage map here as it's internal to analyze, but we check the result name.
    }

    #[test]
    fn test_internal_label_first_wins() {
        let mut state = AppState::new();
        state.origin = 0x1000;

        // BEQ $1005 (D0 03) -> Branch type
        // JMP $1005 (4C 05 10) -> Jump type
        // Internal address $1005.
        // BEQ appears first, so it should define the label as 'b1005'.
        let data = vec![
            0xD0, 0x03, // 1000: BEQ $1005
            0x4C, 0x05, 0x10, // 1002: JMP $1005
            0xEA, // 1005: NOP (make it valid internal address)
        ];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Expectation: b1005 (Branch) because it was first.
        // (Before fix, this would likely be j1005 because Jump > Branch in priority)
        assert_eq!(labels.get(&0x1005).map(|l| l.name.as_str()), Some("b1005"));
    }

    #[test]
    fn test_external_label_still_prioritized() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        let len = 100;
        state.raw_data = vec![0; len]; // Just dummy data placeholder
        state.address_types = vec![AddressType::Code; len];

        // We construct a specific scenario:
        // LDA $E000 (AD 00 E0) -> Absolute usage
        // JMP $E000 (4C 00 E0) -> Jump usage
        // Address $E000 is external.
        // LDA appears first.
        // IF we applied "first wins" to external, we'd get aE000.
        // BUT for external, we want "best wins" (ExternalJump > Absolute).
        // So we expect eE000.

        let data = vec![
            0xAD, 0x00, 0xE0, // 1000: LDA $E000
            0x4C, 0x00, 0xE0, // 1003: JMP $E000
        ];
        // replace beginning of dummy data
        for (i, b) in data.iter().enumerate() {
            state.raw_data[i] = *b;
        }

        let labels = analyze(&state);

        assert_eq!(labels.get(&0xE000).map(|l| l.name.as_str()), Some("eE000"));
    }
    #[test]
    fn test_external_branch() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // BNE target that is out of range
        // Origin 1000. Data len 2. Range 1000..1002.
        // 1000: D0 7F (BNE +$7F) -> Target: 1002 + 7F = 1081.
        // 1081 is >= origin+len (1002), so it is external.

        let data = vec![0xD0, 0x7F];
        state.raw_data = data;
        state.address_types = vec![AddressType::Code; 2];

        let labels = analyze(&state);
        // Should be e1081 (ExternalJump type), NOT b1081 (Branch type).
        assert_eq!(labels.get(&0x1081).map(|l| l.name.as_str()), Some("e1081"));
    }
}
