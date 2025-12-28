use crate::disassembler::{BlockType, Disassembler};
use crate::state::{Assembler, DocumentSettings};
use std::collections::HashMap;

#[test]
fn test_system_comments_logic() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    let mut system_comments = HashMap::new();
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
        &HashMap::new(),
    );

    assert_eq!(lines.len(), 1);
    let line = &lines[0];

    // Mnemonic: JSR
    assert_eq!(line.mnemonic, "JSR");
    // Operand: $FF81
    assert_eq!(line.operand, "$FF81");
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
