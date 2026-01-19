use regenerator2000::disassembler::Disassembler;
use regenerator2000::state::{BlockType, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_illegal_opcodes_disabled_by_default() {
    let disassembler = Disassembler::new();
    // SLO $xxxx (0F) is an illegal opcode
    // 0F 00 10 -> SLO $1000
    let data = vec![0x0F, 0x00, 0x10];
    let block_types = vec![BlockType::Code; 3];
    let settings = DocumentSettings {
        use_illegal_opcodes: false,
        ..Default::default()
    };

    let lines = disassembler.disassemble(
        &data,
        &block_types,
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );

    // Should NOT disassemble as SLO, but as Invalid/Byte
    // With current logic, invalid bytes are output as .byte 0F
    // Then next 2 bytes are BRK #$10 (default behavior)
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].mnemonic, ".byte");
    assert_eq!(lines[0].operand, "$0f"); // lowercase hex

    assert_eq!(lines[1].mnemonic, "brk");
    assert_eq!(lines[1].operand, "#$10");
}

#[test]
fn test_illegal_opcodes_enabled() {
    let disassembler = Disassembler::new();
    // SLO $xxxx (0F) is an illegal opcode
    // 0F 00 10 -> SLO $1000
    let data = vec![0x0F, 0x00, 0x10];
    let block_types = vec![BlockType::Code; 3];
    let settings = DocumentSettings {
        use_illegal_opcodes: true,
        ..Default::default()
    };

    let lines = disassembler.disassemble(
        &data,
        &block_types,
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );

    // Should disassemble as SLO
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "slo"); // lowercase mnemonic
    assert_eq!(lines[0].operand, "$1000"); // Standard formatting for now (auto label would be a1000 if analyzed)
}

#[test]
fn test_new_illegal_opcodes() {
    let disassembler = Disassembler::new();
    let settings = DocumentSettings {
        use_illegal_opcodes: true,
        ..Default::default()
    };

    // ANC #$10 ($0B $10)
    let data_anc = vec![0x0B, 0x10];
    let lines = disassembler.disassemble(
        &data_anc,
        &[BlockType::Code; 2],
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(lines[0].mnemonic, "anc");
    assert_eq!(lines[0].operand, "#$10");

    // ASR #$20 ($4B $20)
    let data_asr = vec![0x4B, 0x20];
    let lines = disassembler.disassemble(
        &data_asr,
        &[BlockType::Code; 2],
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(lines[0].mnemonic, "asr");
    assert_eq!(lines[0].operand, "#$20");

    // ARR #$30 ($6B $30)
    let data_arr = vec![0x6B, 0x30];
    let lines = disassembler.disassemble(
        &data_arr,
        &[BlockType::Code; 2],
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(lines[0].mnemonic, "arr");
    assert_eq!(lines[0].operand, "#$30");

    // SBX #$40 ($CB $40)
    let data_sbx = vec![0xCB, 0x40];
    let lines = disassembler.disassemble(
        &data_sbx,
        &[BlockType::Code; 2],
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(lines[0].mnemonic, "sbx");
    assert_eq!(lines[0].operand, "#$40");

    // LAX #$00 ($AB $00)
    let data_lax = vec![0xAB, 0x00];
    let lines = disassembler.disassemble(
        &data_lax,
        &[BlockType::Code; 2],
        &BTreeMap::new(),
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );
    assert_eq!(lines[0].mnemonic, "lax");
    assert_eq!(lines[0].operand, "#$00");
}
