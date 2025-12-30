use crate::cpu::{AddressingMode, Opcode};
use crate::state::{AppState, BlockType};
use std::collections::BTreeMap;

use crate::state::LabelType;

type UsageData = (BTreeMap<LabelType, usize>, Vec<u16>, LabelType);

pub fn analyze(state: &AppState) -> BTreeMap<u16, Vec<crate::state::Label>> {
    // We want to track ALL usages, illegal or not, and then pick the best ones.
    // Map: Address -> Set of used LabelTypes
    // We also need ref counts.
    // Map: Address -> UsageData
    // storing (counts, refs, first_seen_type)
    let mut usage_map: BTreeMap<u16, UsageData> = BTreeMap::new();
    let mut pc = 0;
    let data_len = state.raw_data.len();
    let origin = state.origin;

    while pc < data_len {
        let current_type = state
            .block_types
            .get(pc)
            .copied()
            .unwrap_or(BlockType::Code);

        if current_type == BlockType::Code {
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
            if current_type == BlockType::Address {
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
            } else if current_type == BlockType::DataWord {
                pc += 2;
            } else {
                pc += 1;
            }
        }
    }

    // Generate labels
    let mut labels: BTreeMap<u16, Vec<crate::state::Label>> = BTreeMap::new();

    // 1. Process all used addresses
    for (addr, (types_map, refs, first_type)) in usage_map {
        let mut addr_labels = Vec::new();

        // Check for existing User or System labels (preserve them if used)
        if let Some(existing_vec) = state.labels.get(&addr) {
            for existing in existing_vec {
                let should_preserve = existing.kind == crate::state::LabelKind::User
                    || existing.kind == crate::state::LabelKind::System;

                if should_preserve {
                    addr_labels.push(crate::state::Label {
                        name: existing.name.clone(),
                        label_type: existing.label_type,
                        kind: existing.kind.clone(), // User or System
                        refs: existing.refs.clone(), // Use manual refs or existing ones
                    });
                    // Assign refs to the last pushed label
                    if let Some(l) = addr_labels.last_mut() {
                        l.refs = refs.clone();
                    }
                }
            }
        }

        // If we have User labels for *all* needed contexts, we might skip Auto generation?
        // But for now, let's just generate Auto labels if needed.

        let is_ext = state.is_external(addr);

        if is_ext {
            // External: Generate for EACH type found
            let mut types_to_generate: Vec<LabelType> = Vec::new();
            for t in types_map.keys() {
                let canonical = if *t == LabelType::Jump
                    || *t == LabelType::Subroutine
                    || *t == LabelType::Branch
                {
                    LabelType::ExternalJump
                } else {
                    *t
                };
                if !types_to_generate.contains(&canonical) {
                    types_to_generate.push(canonical);
                }
            }
            // Sort to ensure deterministic order and priority (e.g. ExternalJump 'e' before others)
            // LabelType doesn't implement Ord, so we sort by string repr or arbitrary priority.
            // Let's assume ExternalJump should be first.
            types_to_generate.sort_by(|a, b| {
                fn priority(t: &LabelType) -> i32 {
                    match t {
                        LabelType::ExternalJump => 0,
                        LabelType::Subroutine => 1,
                        LabelType::Jump => 2,
                        LabelType::Branch => 3,
                        LabelType::AbsoluteAddress => 4,
                        _ => 10,
                    }
                }
                priority(a).cmp(&priority(b))
            });

            for l_type in types_to_generate {
                // Check if we already have this type in addr_labels (User defined)
                if addr_labels.iter().any(|l| l.label_type == l_type) {
                    continue;
                }

                let prefix = l_type.prefix();
                let name = if addr <= 0xFF {
                    if l_type == LabelType::ExternalJump
                        || l_type == LabelType::AbsoluteAddress
                        || l_type == LabelType::Field
                        || l_type == LabelType::Pointer
                    {
                        format!("{}{:04X}", prefix, addr)
                    } else {
                        format!("{}{:02X}", prefix, addr)
                    }
                } else {
                    format!("{}{:04X}", prefix, addr)
                };

                // Add Auto Label
                addr_labels.push(crate::state::Label {
                    name,
                    label_type: l_type,
                    kind: crate::state::LabelKind::Auto,
                    refs: refs.clone(),
                });
            }
        } else {
            // Internal: Single canonical label (first_type or best?)
            // If User label exists, do we add Auto label? No, User label supersedes.
            if addr_labels.is_empty() {
                // Match ignored types logic from earlier or just rely on first found?
                // Logic: if usage contains Subroutine, promote to 's'.
                // Internal: Single canonical label (first_wins)
                let final_type = first_type;
                let prefix = final_type.prefix();

                let name = if addr <= 0xFF {
                    format!("{}{:02X}", prefix, addr)
                } else {
                    format!("{}{:04X}", prefix, addr)
                };

                addr_labels.push(crate::state::Label {
                    name,
                    label_type: final_type,
                    kind: crate::state::LabelKind::Auto,
                    refs: refs.clone(),
                });
            } else {
                // User label exists. Update its refs?
                for l in addr_labels.iter_mut() {
                    l.refs = refs.clone();
                }
            }
        }

        labels.insert(addr, addr_labels);
    }

    // 2. Preserve strictly unused User labels
    for (addr, label_vec) in &state.labels {
        if !labels.contains_key(addr) {
            let mut preserved = Vec::new();
            for label in label_vec {
                if label.kind == crate::state::LabelKind::User {
                    preserved.push(crate::state::Label {
                        name: label.name.clone(),
                        label_type: label.label_type,
                        kind: crate::state::LabelKind::User,
                        refs: Vec::new(), // No refs found in analysis
                    });
                }
            }
            if !preserved.is_empty() {
                labels.insert(*addr, preserved);
            }
        }
    }

    labels
}

fn analyze_instruction(
    _state: &AppState,
    opcode: &Opcode,
    operands: &[u8],
    address: u16,
    usage_map: &mut BTreeMap<u16, UsageData>,
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
    map: &mut BTreeMap<u16, UsageData>,
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
            let mut types = BTreeMap::new();
            types.insert(priority, 1);
            let refs = vec![from_addr];
            (types, refs, priority)
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, BlockType};

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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // $1005 is JMP target -> j1005
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("j1005")
        );
        // $1008 is JSR target -> s1008
        assert_eq!(
            labels
                .get(&0x1008)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("s1008")
        );
        // $1000 is accessed via LDA (Absolute) -> a1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a1000")
        );
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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);
        // Changed expectations: explicit ExternalJump type logic
        assert_eq!(
            labels
                .get(&0x2000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e2000")
        );
    }

    #[test]
    fn test_access_types_zp() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // LDA $10 (ZP) -> A5 10
        let data = vec![0xA5, 0x10];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 2];

        let labels = analyze(&state);
        // ZP access -> ZeroPage Priority -> a10
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a10")
        );
    }

    #[test]
    fn test_zp_field() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // LDA $50, X (B5 50) -> Field usage, ZP address
        let data = vec![0xB5, 0x50];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 2];

        let labels = analyze(&state);
        // Field usage in ZP -> f50
        assert_eq!(
            labels
                .get(&0x0050)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f50")
        );
    }

    #[test]
    fn test_external() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // JMP $0010 (External, out of range [1000..1003])
        let data = vec![0x4C, 0x10, 0x00];
        state.raw_data = data;
        state.block_types = vec![BlockType::Code; 3];

        let labels = analyze(&state);
        // External Jump -> e0010
        // (Note: Jumps use 4 digits usually, unless we want e10?
        // User said "Only for external jumps... not for data".
        // Standard external handling logic still uses 4 digits for 'e' prefix in my code:
        // format!("{}{:04X}", prefix, addr)
        // ZeroPage special formatting is only for ZP priority or ZP Field.
        // Jump is Jump priority. So e0010 is correct.)
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e0010")
        );
    }

    #[test]
    fn test_data_word_vs_address() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        // $1000: DataWord ($2000) -> 00 20
        // $1002: Address ($1000) -> 00 10 (Internal)
        let data = vec![0x00, 0x20, 0x00, 0x10];
        state.raw_data = data;
        state.block_types = vec![
            BlockType::DataWord,
            BlockType::DataWord,
            BlockType::Address,
            BlockType::Address,
        ];

        // Re-analyze reference counts and labels
        let labels = analyze(&state);

        // DataWord at $1000 should NOT generate label for ITSELF ($1000)
        // BUT $1002 IS Reference to $1000. So $1000 SHOULD have a label now.
        // Address type usage at 1002 -> Absolute priority -> a1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("a1000")
        );

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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Case 1: $1000 -> jump to $1002. Usage: b1002 (Internal)
        assert_eq!(
            labels
                .get(&0x1002)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b1002")
        );

        // Case 2: $1002 -> jump to $1083. Usage: e1083 (External logic applies to Branch too now)
        assert_eq!(
            labels
                .get(&0x1083)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e1083")
        );

        // Case 3: $1004 -> jump to $0F86. Usage: e0F86
        assert_eq!(
            labels
                .get(&0x0F86)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e0F86")
        );

        // Case 4: $1006 -> jump to $1007. Usage: b1007
        assert_eq!(
            labels
                .get(&0x1007)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b1007")
        );

        // Case 5: $1008 -> jump to $1008. Usage: b1008
        assert_eq!(
            labels
                .get(&0x1008)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b1008")
        );
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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Indirect JMP -> p1000
        assert_eq!(
            labels
                .get(&0x1000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("p1000")
        );

        // Indirect X -> p10
        assert_eq!(
            labels
                .get(&0x0010)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("p10")
        );

        // Indirect Y -> p20
        assert_eq!(
            labels
                .get(&0x0020)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("p20")
        );

        // Absolute X -> f1050
        assert_eq!(
            labels
                .get(&0x1050)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f1050")
        );

        // Absolute Y -> f1060
        assert_eq!(
            labels
                .get(&0x1060)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f1060")
        );

        // ZeroPage X -> f30
        assert_eq!(
            labels
                .get(&0x0030)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("f30")
        );
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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Check 1005
        // Expect 'b' because it was referenced by Branch FIRST.
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b1005")
        );

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
        state.block_types = vec![BlockType::Code; state.raw_data.len()];

        let labels = analyze(&state);

        // Expectation: b1005 (Branch) because it was first.
        // (Before fix, this would likely be j1005 because Jump > Branch in priority)
        assert_eq!(
            labels
                .get(&0x1005)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("b1005")
        );
    }

    #[test]
    fn test_external_label_still_prioritized() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        let len = 100;
        state.raw_data = vec![0; len]; // Just dummy data placeholder
        state.block_types = vec![BlockType::Code; len];

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

        // Result should be eE000
        assert_eq!(
            labels
                .get(&0xE000)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("eE000")
        );
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
        state.block_types = vec![BlockType::Code; 2];

        let labels = analyze(&state);
        // Should be e1081 (ExternalJump type), NOT b1081 (Branch type).
        assert_eq!(
            labels
                .get(&0x1081)
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("e1081")
        );
    }

    #[test]
    fn test_absolute_addressing_on_zp_formatting() {
        let mut app_state = AppState::new();
        // 8D A0 00 -> STA $00A0 (Answer to Life, Universe, and Everything)
        // Absolute addressing mode targeting a ZP address.
        app_state.raw_data = vec![0x8D, 0xA0, 0x00];
        app_state.origin = 0x1000;
        app_state.block_types = vec![BlockType::Code; 3];
        // Fill opcodes
        app_state.disassembler.opcodes[0x8D] = Some(crate::cpu::Opcode {
            mnemonic: "STA",
            mode: AddressingMode::Absolute,
            size: 3,
            cycles: 4,
            description: "Store Accumulator",
        });

        let labels_map = analyze(&app_state);
        let labels = labels_map.get(&0x00A0);
        assert!(labels.is_some(), "Should have a label at $00A0");
        let label = labels.unwrap().first().unwrap();

        // User wants "a00A0" because it was forced absolute / accessed absolutely.
        // Current bug: "aA0"
        assert_eq!(label.name, "a00A0");
    }

    #[test]
    fn test_field_formatting_on_zp() {
        let mut app_state = AppState::new();
        // 9D A0 00 -> STA $00A0, X (Absolute, X)
        // Absolute indexed addressing mode targeting a ZP address.
        // Should generate "f00A0" (Field, 4 digits) NOT "fA0" (ZP Field, 2 digits).
        app_state.raw_data = vec![0x9D, 0xA0, 0x00];
        app_state.origin = 0x1000;
        app_state.block_types = vec![BlockType::Code; 3];
        // Fill opcodes
        app_state.disassembler.opcodes[0x9D] = Some(crate::cpu::Opcode {
            mnemonic: "STA",
            mode: AddressingMode::AbsoluteX,
            size: 3,
            cycles: 5,
            description: "Store Accumulator",
        });

        let labels_map = analyze(&app_state);
        let labels = labels_map.get(&0x00A0);
        assert!(labels.is_some(), "Should have a label at $00A0");
        let label = labels.unwrap().first().unwrap();

        assert_eq!(
            label.label_type,
            LabelType::Field,
            "Should be Field type (Absolute Indexed)"
        );
        assert_eq!(label.name, "f00A0");
    }

    #[test]
    fn test_dual_pointer_labels() {
        let mut app_state = AppState::new();
        // Scenario: Two instructions using address $00FB.
        // 1. Indirect JMP ($00FB) -> JMP ($00FB) -> 6C FB 00
        //    - This is AddressingMode::Indirect.
        //    - It targets $FB but the operands are $FB $00.
        //    - It should generate LabelType::Pointer ("p")
        //    - The USER wants this to be "p00FB" (4 digits) because it's used as a 16-bit pointer.

        // 2. LDA ($FB), Y -> B1 FB
        //    - This is AddressingMode::IndirectY.
        //    - It targets $FB.
        //    - It should generate LabelType::ZeroPagePointer ("p")
        //    - The USER wants this to be "pFB" (2 digits).

        app_state.origin = 0x1000;
        let data = vec![
            0x6C, 0xFB, 0x00, // JMP ($00FB)
            0xB1, 0xFB, // LDA ($FB), Y
        ];
        app_state.raw_data = data;
        app_state.block_types = vec![BlockType::Code; 5];

        // Mock Opcode info
        // JMP Indirect
        app_state.disassembler.opcodes[0x6C] = Some(crate::cpu::Opcode {
            mnemonic: "JMP",
            mode: AddressingMode::Indirect,
            size: 3,
            cycles: 5,
            description: "Jump Indirect",
        });
        // LDA IndirectY
        app_state.disassembler.opcodes[0xB1] = Some(crate::cpu::Opcode {
            mnemonic: "LDA",
            mode: AddressingMode::IndirectY,
            size: 2,
            cycles: 5,
            description: "Load Accumulator Indirect Y",
        });

        let labels_map = analyze(&app_state);
        let labels = labels_map.get(&0x00FB);
        assert!(labels.is_some(), "Should have labels at $00FB");

        let label_vec = labels.unwrap();
        // We expect TWO labels now: "p00FB" (Pointer) and "pFB" (ZeroPagePointer).

        let has_p00fb = label_vec
            .iter()
            .any(|l| l.name == "p00FB" && l.label_type == LabelType::Pointer);
        let has_pfb = label_vec
            .iter()
            .any(|l| l.name == "pFB" && l.label_type == LabelType::ZeroPagePointer);

        assert!(has_p00fb, "Should have p00FB label for JMP ($00FB)");
        assert!(has_pfb, "Should have pFB label for LDA ($FB),Y");
    }
}
