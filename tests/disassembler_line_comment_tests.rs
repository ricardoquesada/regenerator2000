use regenerator2000::disassembler::Disassembler;
use regenerator2000::state::{Assembler, BlockType, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_user_line_comments_basic() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    // Map for line comments
    let mut user_line_comments = BTreeMap::new();
    user_line_comments.insert(0x1000, "Start of routine".to_string());

    let code = vec![0xEA]; // NOP
    let block_types = vec![BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &BTreeMap::new(), // system side comments
        &BTreeMap::new(), // user side comments
        &user_line_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_comment, Some("Start of routine".to_string()));
    assert_eq!(lines[0].comment, ""); // No side comment
}

#[test]
fn test_user_line_and_side_comments_coexist() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    let disassembler = Disassembler::new();
    let labels = BTreeMap::new();
    let origin = 0x1000;

    let mut user_line_comments = BTreeMap::new();
    user_line_comments.insert(0x1000, "Header".to_string());

    let mut user_side_comments = BTreeMap::new();
    user_side_comments.insert(0x1000, "Inline note".to_string());

    let code = vec![0xEA];
    let block_types = vec![BlockType::Code];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &BTreeMap::new(),
        &user_side_comments,
        &user_line_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &std::collections::BTreeSet::new(),
    );

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_comment, Some("Header".to_string()));
    assert_eq!(lines[0].comment, "Inline note");
}
