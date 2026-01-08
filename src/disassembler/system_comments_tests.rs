use crate::disassembler::{BlockType, Disassembler};
use crate::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_system_comments_logic() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    let mut system_comments = BTreeMap::new();
    // Comment for target address
    system_comments.insert(0xFF81, "init VIC".to_string());
    // Comment for current address
    system_comments.insert(0x1000, "Start Routine".to_string());

    // 1000: JSR $FF81
    let code = vec![0x20, 0x81, 0xFF];
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &system_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
    );

    assert_eq!(lines.len(), 1);
    let line = &lines[0];

    // Mnemonic: JSR
    assert_eq!(line.mnemonic, "jsr");
    // Operand: $FF81
    assert_eq!(line.operand, "$ff81");
    // Comment should combine:
    // 1. System comment for current address (0x1000) -> "Start Routine"
    // 2. Referenced address (0xFF81) -> "init VIC"
    // Joined by "; "
    // Expected: "Start Routine; init VIC" or "init VIC; Start Routine"?
    // In code:
    // comment = get_comment(pc) -> "Start Routine"
    // then append referenced -> "Start Routine; init VIC"
    assert_eq!(line.comment, "Start Routine; init VIC");
}

#[test]
fn test_system_comment_on_sta() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x2000;

    let mut system_comments = BTreeMap::new();
    // D020: Border Color
    system_comments.insert(0xD020, "Border Color".to_string());

    // 2000: STA $D020 (8D 20 D0)
    let code = vec![0x8D, 0x20, 0xD0];
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &system_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
    );

    assert_eq!(lines.len(), 1);
    let line = &lines[0];

    assert_eq!(line.mnemonic, "sta");
    assert_eq!(line.operand, "$d020");
    // This assertion fails currently because handling of STA doesn't look up target address for comments
    // because get_target_address returns None for STA.
    assert_eq!(line.comment, "Border Color");
}
