use crate::disassembler::{BlockType, Disassembler};
use crate::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_user_comments_override_system_comments() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    let mut system_comments = BTreeMap::new();
    system_comments.insert(0x1000, "System Comment".to_string());

    let mut user_comments = BTreeMap::new();
    user_comments.insert(0x1000, "User Comment".to_string());

    let code = vec![0xEA]; // NOP
    let block_types = vec![BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &system_comments,
        &user_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].comment, "User Comment");
}

#[test]
fn test_user_comments_fallthrough() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    let mut system_comments = BTreeMap::new();
    system_comments.insert(0x1000, "System Comment".to_string());

    let user_comments = BTreeMap::new();
    // No user comment for 0x1000

    let code = vec![0xEA]; // NOP
    let block_types = vec![BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &system_comments,
        &user_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].comment, "System Comment");
}

#[test]
fn test_user_comments_referenced_address() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    // 0x2000 is referenced
    let mut system_comments = BTreeMap::new();
    system_comments.insert(0x2000, "System Ref Comment".to_string());

    let mut user_comments = BTreeMap::new();
    user_comments.insert(0x2000, "User Ref Comment".to_string());

    // JMP $2000
    let code = vec![0x4C, 0x00, 0x20];
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &system_comments,
        &user_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
    );

    assert_eq!(lines.len(), 1);
    // Should contain User Ref Comment
    assert!(lines[0].comment.contains("User Ref Comment"));
    assert!(!lines[0].comment.contains("System Ref Comment"));
}
