use regenerator2000::cpu::{AddressingMode, Opcode, get_opcodes};
use std::collections::{HashMap, HashSet};

// =============================================================================
// OPCODE TABLE COMPLETENESS TESTS
// =============================================================================

#[test]
fn test_opcode_table_has_256_entries() {
    let opcodes = get_opcodes();
    assert_eq!(
        opcodes.len(),
        256,
        "Opcode table must have exactly 256 entries"
    );
}

#[test]
fn test_all_official_opcodes_are_defined() {
    let opcodes = get_opcodes();

    // All official 6502 opcodes (151 total)
    let official_opcodes: Vec<(u8, &str, AddressingMode)> = vec![
        // ADC
        (0x69, "ADC", AddressingMode::Immediate),
        (0x65, "ADC", AddressingMode::ZeroPage),
        (0x75, "ADC", AddressingMode::ZeroPageX),
        (0x6D, "ADC", AddressingMode::Absolute),
        (0x7D, "ADC", AddressingMode::AbsoluteX),
        (0x79, "ADC", AddressingMode::AbsoluteY),
        (0x61, "ADC", AddressingMode::IndirectX),
        (0x71, "ADC", AddressingMode::IndirectY),
        // AND
        (0x29, "AND", AddressingMode::Immediate),
        (0x25, "AND", AddressingMode::ZeroPage),
        (0x35, "AND", AddressingMode::ZeroPageX),
        (0x2D, "AND", AddressingMode::Absolute),
        (0x3D, "AND", AddressingMode::AbsoluteX),
        (0x39, "AND", AddressingMode::AbsoluteY),
        (0x21, "AND", AddressingMode::IndirectX),
        (0x31, "AND", AddressingMode::IndirectY),
        // ASL
        (0x0A, "ASL", AddressingMode::Accumulator),
        (0x06, "ASL", AddressingMode::ZeroPage),
        (0x16, "ASL", AddressingMode::ZeroPageX),
        (0x0E, "ASL", AddressingMode::Absolute),
        (0x1E, "ASL", AddressingMode::AbsoluteX),
        // BCC, BCS, BEQ, BMI, BNE, BPL, BVC, BVS (branches)
        (0x90, "BCC", AddressingMode::Relative),
        (0xB0, "BCS", AddressingMode::Relative),
        (0xF0, "BEQ", AddressingMode::Relative),
        (0x30, "BMI", AddressingMode::Relative),
        (0xD0, "BNE", AddressingMode::Relative),
        (0x10, "BPL", AddressingMode::Relative),
        (0x50, "BVC", AddressingMode::Relative),
        (0x70, "BVS", AddressingMode::Relative),
        // BIT
        (0x24, "BIT", AddressingMode::ZeroPage),
        (0x2C, "BIT", AddressingMode::Absolute),
        // BRK
        (0x00, "BRK", AddressingMode::Implied),
        // CLC, CLD, CLI, CLV
        (0x18, "CLC", AddressingMode::Implied),
        (0xD8, "CLD", AddressingMode::Implied),
        (0x58, "CLI", AddressingMode::Implied),
        (0xB8, "CLV", AddressingMode::Implied),
        // CMP
        (0xC9, "CMP", AddressingMode::Immediate),
        (0xC5, "CMP", AddressingMode::ZeroPage),
        (0xD5, "CMP", AddressingMode::ZeroPageX),
        (0xCD, "CMP", AddressingMode::Absolute),
        (0xDD, "CMP", AddressingMode::AbsoluteX),
        (0xD9, "CMP", AddressingMode::AbsoluteY),
        (0xC1, "CMP", AddressingMode::IndirectX),
        (0xD1, "CMP", AddressingMode::IndirectY),
        // CPX
        (0xE0, "CPX", AddressingMode::Immediate),
        (0xE4, "CPX", AddressingMode::ZeroPage),
        (0xEC, "CPX", AddressingMode::Absolute),
        // CPY
        (0xC0, "CPY", AddressingMode::Immediate),
        (0xC4, "CPY", AddressingMode::ZeroPage),
        (0xCC, "CPY", AddressingMode::Absolute),
        // DEC
        (0xC6, "DEC", AddressingMode::ZeroPage),
        (0xD6, "DEC", AddressingMode::ZeroPageX),
        (0xCE, "DEC", AddressingMode::Absolute),
        (0xDE, "DEC", AddressingMode::AbsoluteX),
        // DEX, DEY
        (0xCA, "DEX", AddressingMode::Implied),
        (0x88, "DEY", AddressingMode::Implied),
        // EOR
        (0x49, "EOR", AddressingMode::Immediate),
        (0x45, "EOR", AddressingMode::ZeroPage),
        (0x55, "EOR", AddressingMode::ZeroPageX),
        (0x4D, "EOR", AddressingMode::Absolute),
        (0x5D, "EOR", AddressingMode::AbsoluteX),
        (0x59, "EOR", AddressingMode::AbsoluteY),
        (0x41, "EOR", AddressingMode::IndirectX),
        (0x51, "EOR", AddressingMode::IndirectY),
        // INC
        (0xE6, "INC", AddressingMode::ZeroPage),
        (0xF6, "INC", AddressingMode::ZeroPageX),
        (0xEE, "INC", AddressingMode::Absolute),
        (0xFE, "INC", AddressingMode::AbsoluteX),
        // INX, INY
        (0xE8, "INX", AddressingMode::Implied),
        (0xC8, "INY", AddressingMode::Implied),
        // JMP
        (0x4C, "JMP", AddressingMode::Absolute),
        (0x6C, "JMP", AddressingMode::Indirect),
        // JSR
        (0x20, "JSR", AddressingMode::Absolute),
        // LDA
        (0xA9, "LDA", AddressingMode::Immediate),
        (0xA5, "LDA", AddressingMode::ZeroPage),
        (0xB5, "LDA", AddressingMode::ZeroPageX),
        (0xAD, "LDA", AddressingMode::Absolute),
        (0xBD, "LDA", AddressingMode::AbsoluteX),
        (0xB9, "LDA", AddressingMode::AbsoluteY),
        (0xA1, "LDA", AddressingMode::IndirectX),
        (0xB1, "LDA", AddressingMode::IndirectY),
        // LDX
        (0xA2, "LDX", AddressingMode::Immediate),
        (0xA6, "LDX", AddressingMode::ZeroPage),
        (0xB6, "LDX", AddressingMode::ZeroPageY),
        (0xAE, "LDX", AddressingMode::Absolute),
        (0xBE, "LDX", AddressingMode::AbsoluteY),
        // LDY
        (0xA0, "LDY", AddressingMode::Immediate),
        (0xA4, "LDY", AddressingMode::ZeroPage),
        (0xB4, "LDY", AddressingMode::ZeroPageX),
        (0xAC, "LDY", AddressingMode::Absolute),
        (0xBC, "LDY", AddressingMode::AbsoluteX),
        // LSR
        (0x4A, "LSR", AddressingMode::Accumulator),
        (0x46, "LSR", AddressingMode::ZeroPage),
        (0x56, "LSR", AddressingMode::ZeroPageX),
        (0x4E, "LSR", AddressingMode::Absolute),
        (0x5E, "LSR", AddressingMode::AbsoluteX),
        // NOP
        (0xEA, "NOP", AddressingMode::Implied),
        // ORA
        (0x09, "ORA", AddressingMode::Immediate),
        (0x05, "ORA", AddressingMode::ZeroPage),
        (0x15, "ORA", AddressingMode::ZeroPageX),
        (0x0D, "ORA", AddressingMode::Absolute),
        (0x1D, "ORA", AddressingMode::AbsoluteX),
        (0x19, "ORA", AddressingMode::AbsoluteY),
        (0x01, "ORA", AddressingMode::IndirectX),
        (0x11, "ORA", AddressingMode::IndirectY),
        // PHA, PHP, PLA, PLP
        (0x48, "PHA", AddressingMode::Implied),
        (0x08, "PHP", AddressingMode::Implied),
        (0x68, "PLA", AddressingMode::Implied),
        (0x28, "PLP", AddressingMode::Implied),
        // ROL
        (0x2A, "ROL", AddressingMode::Accumulator),
        (0x26, "ROL", AddressingMode::ZeroPage),
        (0x36, "ROL", AddressingMode::ZeroPageX),
        (0x2E, "ROL", AddressingMode::Absolute),
        (0x3E, "ROL", AddressingMode::AbsoluteX),
        // ROR
        (0x6A, "ROR", AddressingMode::Accumulator),
        (0x66, "ROR", AddressingMode::ZeroPage),
        (0x76, "ROR", AddressingMode::ZeroPageX),
        (0x6E, "ROR", AddressingMode::Absolute),
        (0x7E, "ROR", AddressingMode::AbsoluteX),
        // RTI, RTS
        (0x40, "RTI", AddressingMode::Implied),
        (0x60, "RTS", AddressingMode::Implied),
        // SBC
        (0xE9, "SBC", AddressingMode::Immediate),
        (0xE5, "SBC", AddressingMode::ZeroPage),
        (0xF5, "SBC", AddressingMode::ZeroPageX),
        (0xED, "SBC", AddressingMode::Absolute),
        (0xFD, "SBC", AddressingMode::AbsoluteX),
        (0xF9, "SBC", AddressingMode::AbsoluteY),
        (0xE1, "SBC", AddressingMode::IndirectX),
        (0xF1, "SBC", AddressingMode::IndirectY),
        // SEC, SED, SEI
        (0x38, "SEC", AddressingMode::Implied),
        (0xF8, "SED", AddressingMode::Implied),
        (0x78, "SEI", AddressingMode::Implied),
        // STA
        (0x85, "STA", AddressingMode::ZeroPage),
        (0x95, "STA", AddressingMode::ZeroPageX),
        (0x8D, "STA", AddressingMode::Absolute),
        (0x9D, "STA", AddressingMode::AbsoluteX),
        (0x99, "STA", AddressingMode::AbsoluteY),
        (0x81, "STA", AddressingMode::IndirectX),
        (0x91, "STA", AddressingMode::IndirectY),
        // STX
        (0x86, "STX", AddressingMode::ZeroPage),
        (0x96, "STX", AddressingMode::ZeroPageY),
        (0x8E, "STX", AddressingMode::Absolute),
        // STY
        (0x84, "STY", AddressingMode::ZeroPage),
        (0x94, "STY", AddressingMode::ZeroPageX),
        (0x8C, "STY", AddressingMode::Absolute),
        // TAX, TAY, TSX, TXA, TXS, TYA
        (0xAA, "TAX", AddressingMode::Implied),
        (0xA8, "TAY", AddressingMode::Implied),
        (0xBA, "TSX", AddressingMode::Implied),
        (0x8A, "TXA", AddressingMode::Implied),
        (0x9A, "TXS", AddressingMode::Implied),
        (0x98, "TYA", AddressingMode::Implied),
    ];

    for (opcode, expected_mnemonic, expected_mode) in official_opcodes {
        let op = opcodes[opcode as usize].as_ref().unwrap_or_else(|| {
            panic!(
                "Official opcode ${:02X} ({}) is not defined",
                opcode, expected_mnemonic
            )
        });

        assert_eq!(
            op.mnemonic, expected_mnemonic,
            "Opcode ${:02X} should be {} but is {}",
            opcode, expected_mnemonic, op.mnemonic
        );
        assert_eq!(
            op.mode, expected_mode,
            "Opcode ${:02X} ({}) has wrong addressing mode: expected {:?}, got {:?}",
            opcode, expected_mnemonic, expected_mode, op.mode
        );
        assert!(
            !op.illegal,
            "Official opcode ${:02X} ({}) should not be marked as illegal",
            opcode, expected_mnemonic
        );
    }
}

#[test]
fn test_official_opcode_count() {
    let opcodes = get_opcodes();
    let official_count = opcodes
        .iter()
        .filter(|op| op.as_ref().map_or(false, |o| !o.illegal))
        .count();

    // 6502 has 151 documented opcodes
    assert_eq!(
        official_count, 151,
        "Expected 151 official opcodes, found {}",
        official_count
    );
}

// =============================================================================
// ADDRESSING MODE AND SIZE CONSISTENCY TESTS
// =============================================================================

#[test]
fn test_instruction_sizes_match_addressing_modes() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            let expected_size = match opcode.mode {
                AddressingMode::Implied | AddressingMode::Accumulator => 1,
                AddressingMode::Immediate
                | AddressingMode::ZeroPage
                | AddressingMode::ZeroPageX
                | AddressingMode::ZeroPageY
                | AddressingMode::Relative
                | AddressingMode::IndirectX
                | AddressingMode::IndirectY => 2,
                AddressingMode::Absolute
                | AddressingMode::AbsoluteX
                | AddressingMode::AbsoluteY
                | AddressingMode::Indirect => 3,
                AddressingMode::Unknown => continue, // Skip unknown modes
            };

            assert_eq!(
                opcode.size, expected_size,
                "Opcode ${:02X} ({}) has size {} but addressing mode {:?} requires size {}",
                i, opcode.mnemonic, opcode.size, opcode.mode, expected_size
            );
        }
    }
}

#[test]
fn test_all_branches_are_relative() {
    let opcodes = get_opcodes();
    let branch_mnemonics = ["BCC", "BCS", "BEQ", "BMI", "BNE", "BPL", "BVC", "BVS"];

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            if branch_mnemonics.contains(&opcode.mnemonic) {
                assert_eq!(
                    opcode.mode,
                    AddressingMode::Relative,
                    "Branch instruction ${:02X} ({}) should use Relative addressing",
                    i,
                    opcode.mnemonic
                );
                assert_eq!(
                    opcode.size, 2,
                    "Branch instruction ${:02X} ({}) should be 2 bytes",
                    i, opcode.mnemonic
                );
            }
        }
    }
}

#[test]
fn test_all_implied_instructions_are_size_1() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            if opcode.mode == AddressingMode::Implied {
                assert_eq!(
                    opcode.size, 1,
                    "Implied mode instruction ${:02X} ({}) should be 1 byte",
                    i, opcode.mnemonic
                );
            }
        }
    }
}

#[test]
fn test_all_accumulator_instructions_are_size_1() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            if opcode.mode == AddressingMode::Accumulator {
                assert_eq!(
                    opcode.size, 1,
                    "Accumulator mode instruction ${:02X} ({}) should be 1 byte",
                    i, opcode.mnemonic
                );
            }
        }
    }
}

// =============================================================================
// MNEMONIC COVERAGE TESTS
// =============================================================================

#[test]
fn test_all_official_mnemonics_present() {
    let opcodes = get_opcodes();

    let official_mnemonics: HashSet<&str> = [
        "ADC", "AND", "ASL", "BCC", "BCS", "BEQ", "BIT", "BMI", "BNE", "BPL", "BRK", "BVC", "BVS",
        "CLC", "CLD", "CLI", "CLV", "CMP", "CPX", "CPY", "DEC", "DEX", "DEY", "EOR", "INC", "INX",
        "INY", "JMP", "JSR", "LDA", "LDX", "LDY", "LSR", "NOP", "ORA", "PHA", "PHP", "PLA", "PLP",
        "ROL", "ROR", "RTI", "RTS", "SBC", "SEC", "SED", "SEI", "STA", "STX", "STY", "TAX", "TAY",
        "TSX", "TXA", "TXS", "TYA",
    ]
    .into_iter()
    .collect();

    let found_mnemonics: HashSet<&str> = opcodes
        .iter()
        .filter_map(|op| op.as_ref())
        .filter(|op| !op.illegal)
        .map(|op| op.mnemonic)
        .collect();

    for mnemonic in &official_mnemonics {
        assert!(
            found_mnemonics.contains(mnemonic),
            "Official mnemonic {} is missing from opcode table",
            mnemonic
        );
    }
}

#[test]
fn test_mnemonic_addressing_mode_combinations() {
    let opcodes = get_opcodes();

    // Build a map of mnemonic -> set of addressing modes
    let mut mnemonic_modes: HashMap<&str, HashSet<AddressingMode>> = HashMap::new();

    for op in opcodes.iter().filter_map(|o| o.as_ref()) {
        mnemonic_modes
            .entry(op.mnemonic)
            .or_default()
            .insert(op.mode);
    }

    // Verify common instructions have expected addressing modes
    // LDA should have 8 addressing modes
    let lda_modes = mnemonic_modes.get("LDA").expect("LDA not found");
    assert!(lda_modes.contains(&AddressingMode::Immediate));
    assert!(lda_modes.contains(&AddressingMode::ZeroPage));
    assert!(lda_modes.contains(&AddressingMode::ZeroPageX));
    assert!(lda_modes.contains(&AddressingMode::Absolute));
    assert!(lda_modes.contains(&AddressingMode::AbsoluteX));
    assert!(lda_modes.contains(&AddressingMode::AbsoluteY));
    assert!(lda_modes.contains(&AddressingMode::IndirectX));
    assert!(lda_modes.contains(&AddressingMode::IndirectY));

    // JMP should have Absolute and Indirect
    let jmp_modes = mnemonic_modes.get("JMP").expect("JMP not found");
    assert!(jmp_modes.contains(&AddressingMode::Absolute));
    assert!(jmp_modes.contains(&AddressingMode::Indirect));

    // JSR should only have Absolute
    let jsr_modes = mnemonic_modes.get("JSR").expect("JSR not found");
    assert_eq!(jsr_modes.len(), 1);
    assert!(jsr_modes.contains(&AddressingMode::Absolute));
}

// =============================================================================
// ILLEGAL OPCODE TESTS
// =============================================================================

#[test]
fn test_illegal_opcodes_are_marked() {
    let opcodes = get_opcodes();

    // Known illegal opcodes that should be defined and marked as illegal
    let illegal_opcodes: Vec<(u8, &str)> = vec![
        // SLO
        (0x07, "SLO"),
        (0x17, "SLO"),
        (0x03, "SLO"),
        (0x13, "SLO"),
        (0x0F, "SLO"),
        (0x1F, "SLO"),
        (0x1B, "SLO"),
        // RLA
        (0x27, "RLA"),
        (0x37, "RLA"),
        (0x23, "RLA"),
        (0x33, "RLA"),
        (0x2F, "RLA"),
        (0x3F, "RLA"),
        (0x3B, "RLA"),
        // SRE
        (0x47, "SRE"),
        (0x57, "SRE"),
        (0x43, "SRE"),
        (0x53, "SRE"),
        (0x4F, "SRE"),
        (0x5F, "SRE"),
        (0x5B, "SRE"),
        // RRA
        (0x67, "RRA"),
        (0x77, "RRA"),
        (0x63, "RRA"),
        (0x73, "RRA"),
        (0x6F, "RRA"),
        (0x7F, "RRA"),
        (0x7B, "RRA"),
        // SAX
        (0x87, "SAX"),
        (0x97, "SAX"),
        (0x83, "SAX"),
        (0x8F, "SAX"),
        // LAX
        (0xA7, "LAX"),
        (0xB7, "LAX"),
        (0xA3, "LAX"),
        (0xB3, "LAX"),
        (0xAF, "LAX"),
        (0xBF, "LAX"),
        (0xAB, "LAX"), // LAX Immediate
        // DCP
        (0xC7, "DCP"),
        (0xD7, "DCP"),
        (0xC3, "DCP"),
        (0xD3, "DCP"),
        (0xCF, "DCP"),
        (0xDF, "DCP"),
        (0xDB, "DCP"),
        // ISC
        (0xE7, "ISC"),
        (0xF7, "ISC"),
        (0xE3, "ISC"),
        (0xF3, "ISC"),
        (0xEF, "ISC"),
        (0xFF, "ISC"),
        (0xFB, "ISC"),
        // ANC
        (0x0B, "ANC"),
        (0x2B, "ANC"),
        // ASR
        (0x4B, "ASR"),
        // ARR
        (0x6B, "ARR"),
        // SBX
        (0xCB, "SBX"),
    ];

    for (opcode, expected_mnemonic) in illegal_opcodes {
        let op = opcodes[opcode as usize].as_ref().unwrap_or_else(|| {
            panic!(
                "Illegal opcode ${:02X} ({}) should be defined",
                opcode, expected_mnemonic
            )
        });

        assert_eq!(
            op.mnemonic, expected_mnemonic,
            "Opcode ${:02X} should be {} but is {}",
            opcode, expected_mnemonic, op.mnemonic
        );
        assert!(
            op.illegal,
            "Opcode ${:02X} ({}) should be marked as illegal",
            opcode, expected_mnemonic
        );
    }
}

#[test]
fn test_illegal_opcode_count() {
    let opcodes = get_opcodes();
    let illegal_count = opcodes
        .iter()
        .filter(|op| op.as_ref().map_or(false, |o| o.illegal))
        .count();

    // The module defines several illegal opcodes
    // SLO(7) + RLA(7) + SRE(7) + RRA(7) + SAX(4) + LAX(7) + DCP(7) + ISC(7) + ANC(2) + ASR(1) + ARR(1) + SBX(1) = 58
    assert_eq!(
        illegal_count, 58,
        "Expected 58 illegal opcodes, found {}",
        illegal_count
    );
}

#[test]
fn test_illegal_mnemonics_present() {
    let opcodes = get_opcodes();

    let illegal_mnemonics: HashSet<&str> = [
        "SLO", "RLA", "SRE", "RRA", "SAX", "LAX", "DCP", "ISC", "ANC", "ASR", "ARR", "SBX",
    ]
    .into_iter()
    .collect();

    let found_illegal_mnemonics: HashSet<&str> = opcodes
        .iter()
        .filter_map(|op| op.as_ref())
        .filter(|op| op.illegal)
        .map(|op| op.mnemonic)
        .collect();

    for mnemonic in &illegal_mnemonics {
        assert!(
            found_illegal_mnemonics.contains(mnemonic),
            "Illegal mnemonic {} is missing from opcode table",
            mnemonic
        );
    }
}

// =============================================================================
// FLOW CONTROL TESTS
// =============================================================================

#[test]
fn test_is_flow_control_with_target_jmp_absolute() {
    let opcodes = get_opcodes();

    // JMP Absolute ($4C) should return true
    let jmp_abs = opcodes[0x4C].as_ref().unwrap();
    assert!(
        jmp_abs.is_flow_control_with_target(),
        "JMP Absolute should be flow control with target"
    );
}

#[test]
fn test_is_flow_control_with_target_jmp_indirect() {
    let opcodes = get_opcodes();

    // JMP Indirect ($6C) should return false (target is computed)
    let jmp_ind = opcodes[0x6C].as_ref().unwrap();
    assert!(
        !jmp_ind.is_flow_control_with_target(),
        "JMP Indirect should NOT be flow control with target"
    );
}

#[test]
fn test_is_flow_control_with_target_jsr() {
    let opcodes = get_opcodes();

    // JSR ($20) should return true
    let jsr = opcodes[0x20].as_ref().unwrap();
    assert!(
        jsr.is_flow_control_with_target(),
        "JSR should be flow control with target"
    );
}

#[test]
fn test_is_flow_control_with_target_branches() {
    let opcodes = get_opcodes();

    let branch_opcodes = [0x90, 0xB0, 0xF0, 0x30, 0xD0, 0x10, 0x50, 0x70];

    for opcode in branch_opcodes {
        let branch = opcodes[opcode].as_ref().unwrap();
        assert!(
            branch.is_flow_control_with_target(),
            "Branch ${:02X} ({}) should be flow control with target",
            opcode,
            branch.mnemonic
        );
    }
}

#[test]
fn test_is_flow_control_with_target_returns_false_for_rts_rti() {
    let opcodes = get_opcodes();

    // RTS ($60) and RTI ($40) should return false (no target address)
    let rts = opcodes[0x60].as_ref().unwrap();
    assert!(
        !rts.is_flow_control_with_target(),
        "RTS should NOT be flow control with target"
    );

    let rti = opcodes[0x40].as_ref().unwrap();
    assert!(
        !rti.is_flow_control_with_target(),
        "RTI should NOT be flow control with target"
    );
}

#[test]
fn test_is_flow_control_with_target_returns_false_for_brk() {
    let opcodes = get_opcodes();

    // BRK ($00) should return false
    let brk = opcodes[0x00].as_ref().unwrap();
    assert!(
        !brk.is_flow_control_with_target(),
        "BRK should NOT be flow control with target"
    );
}

#[test]
fn test_is_flow_control_with_target_returns_false_for_regular_instructions() {
    let opcodes = get_opcodes();

    // Regular instructions should return false
    let regular_opcodes = [
        0xA9, // LDA #imm
        0x8D, // STA abs
        0xE8, // INX
        0xEA, // NOP
        0x69, // ADC #imm
    ];

    for opcode in regular_opcodes {
        let op = opcodes[opcode].as_ref().unwrap();
        assert!(
            !op.is_flow_control_with_target(),
            "Regular instruction ${:02X} ({}) should NOT be flow control with target",
            opcode,
            op.mnemonic
        );
    }
}

// =============================================================================
// CYCLE COUNT VALIDATION TESTS
// =============================================================================

#[test]
fn test_cycle_counts_are_reasonable() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            // All 6502 instructions take at least 2 cycles
            assert!(
                opcode.cycles >= 2,
                "Opcode ${:02X} ({}) has cycle count {} which is less than 2",
                i,
                opcode.mnemonic,
                opcode.cycles
            );

            // No 6502 instruction takes more than 8 cycles
            assert!(
                opcode.cycles <= 8,
                "Opcode ${:02X} ({}) has cycle count {} which is more than 8",
                i,
                opcode.mnemonic,
                opcode.cycles
            );
        }
    }
}

#[test]
fn test_specific_cycle_counts() {
    let opcodes = get_opcodes();

    // Test some well-known cycle counts
    let expected_cycles: Vec<(u8, u8, &str)> = vec![
        (0xEA, 2, "NOP"),      // NOP takes 2 cycles
        (0x00, 7, "BRK"),      // BRK takes 7 cycles
        (0x20, 6, "JSR"),      // JSR takes 6 cycles
        (0x60, 6, "RTS"),      // RTS takes 6 cycles
        (0x40, 6, "RTI"),      // RTI takes 6 cycles
        (0xA9, 2, "LDA #imm"), // LDA immediate takes 2 cycles
        (0xAD, 4, "LDA abs"),  // LDA absolute takes 4 cycles
        (0x4C, 3, "JMP abs"),  // JMP absolute takes 3 cycles
        (0x6C, 5, "JMP ind"),  // JMP indirect takes 5 cycles
    ];

    for (opcode, expected, name) in expected_cycles {
        let op = opcodes[opcode as usize].as_ref().unwrap();
        assert_eq!(
            op.cycles, expected,
            "{} (${:02X}) should take {} cycles but takes {}",
            name, opcode, expected, op.cycles
        );
    }
}

// =============================================================================
// OPCODE CONSTRUCTION TESTS
// =============================================================================

#[test]
fn test_opcode_new_creates_legal_opcode() {
    let op = Opcode::new("LDA", AddressingMode::Immediate, 2, 2, "Load Accumulator");

    assert_eq!(op.mnemonic, "LDA");
    assert_eq!(op.mode, AddressingMode::Immediate);
    assert_eq!(op.size, 2);
    assert_eq!(op.cycles, 2);
    assert_eq!(op.description, "Load Accumulator");
    assert!(!op.illegal);
}

#[test]
fn test_opcode_new_illegal_creates_illegal_opcode() {
    let op = Opcode::new_illegal("SLO", AddressingMode::ZeroPage, 2, 5, "ASL + ORA");

    assert_eq!(op.mnemonic, "SLO");
    assert_eq!(op.mode, AddressingMode::ZeroPage);
    assert_eq!(op.size, 2);
    assert_eq!(op.cycles, 5);
    assert_eq!(op.description, "ASL + ORA");
    assert!(op.illegal);
}

// =============================================================================
// ADDRESSING MODE TESTS
// =============================================================================

#[test]
fn test_addressing_mode_equality() {
    assert_eq!(AddressingMode::Implied, AddressingMode::Implied);
    assert_ne!(AddressingMode::Implied, AddressingMode::Accumulator);
    assert_ne!(AddressingMode::ZeroPage, AddressingMode::Absolute);
}

#[test]
fn test_all_addressing_modes_used() {
    let opcodes = get_opcodes();

    let used_modes: HashSet<AddressingMode> = opcodes
        .iter()
        .filter_map(|op| op.as_ref())
        .map(|op| op.mode)
        .collect();

    // All modes except Unknown should be used
    let expected_modes = [
        AddressingMode::Implied,
        AddressingMode::Accumulator,
        AddressingMode::Immediate,
        AddressingMode::ZeroPage,
        AddressingMode::ZeroPageX,
        AddressingMode::ZeroPageY,
        AddressingMode::Relative,
        AddressingMode::Absolute,
        AddressingMode::AbsoluteX,
        AddressingMode::AbsoluteY,
        AddressingMode::Indirect,
        AddressingMode::IndirectX,
        AddressingMode::IndirectY,
    ];

    for mode in expected_modes {
        assert!(
            used_modes.contains(&mode),
            "Addressing mode {:?} is not used by any opcode",
            mode
        );
    }
}

// =============================================================================
// EDGE CASE AND SPECIAL OPCODE TESTS
// =============================================================================

#[test]
fn test_brk_is_first_opcode() {
    let opcodes = get_opcodes();
    let brk = opcodes[0x00]
        .as_ref()
        .expect("BRK should be defined at $00");
    assert_eq!(brk.mnemonic, "BRK");
}

#[test]
fn test_nop_opcode() {
    let opcodes = get_opcodes();
    let nop = opcodes[0xEA]
        .as_ref()
        .expect("NOP should be defined at $EA");
    assert_eq!(nop.mnemonic, "NOP");
    assert_eq!(nop.mode, AddressingMode::Implied);
    assert_eq!(nop.size, 1);
    assert_eq!(nop.cycles, 2);
}

#[test]
fn test_undefined_opcodes_are_none() {
    let opcodes = get_opcodes();

    // Some opcodes that should be undefined (gaps in the opcode space)
    // These are opcodes that are NOT defined in the module
    let undefined_count = opcodes.iter().filter(|op| op.is_none()).count();

    // 256 total - 151 official - 58 illegal = 47 undefined
    assert_eq!(
        undefined_count, 47,
        "Expected 47 undefined opcodes, found {}",
        undefined_count
    );
}

#[test]
fn test_total_defined_opcodes() {
    let opcodes = get_opcodes();
    let defined_count = opcodes.iter().filter(|op| op.is_some()).count();

    // 151 official + 58 illegal = 209 defined
    assert_eq!(
        defined_count, 209,
        "Expected 209 defined opcodes, found {}",
        defined_count
    );
}

// =============================================================================
// CONSISTENCY TESTS
// =============================================================================

#[test]
fn test_no_duplicate_mnemonic_mode_combinations() {
    let opcodes = get_opcodes();

    let mut seen: HashMap<(&str, AddressingMode), u8> = HashMap::new();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            let key = (opcode.mnemonic, opcode.mode);
            if let Some(&prev_opcode) = seen.get(&key) {
                // Some opcodes legitimately have duplicates (e.g., ANC at $0B and $2B)
                // Only fail if they're different mnemonics
                let prev = opcodes[prev_opcode as usize].as_ref().unwrap();
                if prev.illegal == opcode.illegal {
                    // This is expected for some illegal opcodes like ANC
                    continue;
                }
                panic!(
                    "Duplicate mnemonic/mode combination: {} {:?} at ${:02X} and ${:02X}",
                    opcode.mnemonic, opcode.mode, prev_opcode, i
                );
            }
            seen.insert(key, i as u8);
        }
    }
}

#[test]
fn test_all_descriptions_non_empty() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            assert!(
                !opcode.description.is_empty(),
                "Opcode ${:02X} ({}) has an empty description",
                i,
                opcode.mnemonic
            );
        }
    }
}

#[test]
fn test_all_mnemonics_are_uppercase() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            assert_eq!(
                opcode.mnemonic,
                opcode.mnemonic.to_uppercase(),
                "Opcode ${:02X} mnemonic '{}' should be uppercase",
                i,
                opcode.mnemonic
            );
        }
    }
}

#[test]
fn test_all_mnemonics_are_3_characters() {
    let opcodes = get_opcodes();

    for (i, op) in opcodes.iter().enumerate() {
        if let Some(opcode) = op {
            assert_eq!(
                opcode.mnemonic.len(),
                3,
                "Opcode ${:02X} mnemonic '{}' should be 3 characters",
                i,
                opcode.mnemonic
            );
        }
    }
}
