use super::*;
use crate::state::{Assembler, DocumentSettings, Label, LabelKind, LabelType};
use std::collections::BTreeMap;

#[test]
fn test_tass_label_placement_on_text() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let disassembler = Disassembler::new();
    let mut labels = BTreeMap::new();
    let origin = 0x1000;
    let mut cross_refs = BTreeMap::new();
    cross_refs.insert(0x1000, vec![0x2000]);

    // Label at start of text block
    labels.insert(
        0x1000,
        vec![Label {
            name: "TextLabel".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::AbsoluteAddress,
        }],
    );

    // "ABC"
    let code = vec![0x41, 0x42, 0x43];
    let block_types = vec![BlockType::Text; 3];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(), // user_side_comments
        &BTreeMap::new(), // user_line_comments
        &BTreeMap::new(), // immediate_value_formats
        &cross_refs,      // cross_refs
        &[],
    );

    // Expected Tass output:
    // .encode
    // .enc "none"
    // TextLabel .text "ABC"
    // .endencode

    // Find the line with the label
    let labeled_line = lines
        .iter()
        .find(|l| l.label.is_some())
        .expect("Should have a labeled line");

    // VALIDATE: Check that the labeled line is ".text", NOT ".encode"
    assert_eq!(
        labeled_line.mnemonic, ".text",
        "Label should be on .text line, found on {}",
        labeled_line.mnemonic
    );
    assert_eq!(labeled_line.label.as_deref(), Some("TextLabel"));

    // VALIDATE: Check that the line with label ALSO has the comment
    assert!(
        labeled_line.comment.contains("x-ref: $2000"),
        "Comment should be on line with label (.text), found: '{}'",
        labeled_line.comment
    );
}

#[test]
fn test_tass_label_placement_on_screencode() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let disassembler = Disassembler::new();
    let mut labels = BTreeMap::new();
    let origin = 0x1000;

    // Add side comment
    let mut user_side_comments = BTreeMap::new();
    user_side_comments.insert(0x1000, "My Side Comment".to_string());

    // Label at start of screencode block
    labels.insert(
        0x1000,
        vec![Label {
            name: "ScreenLabel".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::AbsoluteAddress,
        }],
    );

    // "ABC" (sc 1,2,3)
    let code = vec![1, 2, 3];
    let block_types = vec![BlockType::Screencode; 3];

    let lines = disassembler.disassemble(
        &code,
        &block_types,
        &labels,
        origin,
        &settings,
        &BTreeMap::new(),
        &user_side_comments,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
    );

    // Expected Tass output:
    // .encode
    // .enc "screen"
    // ScreenLabel .text "ABC"
    // .endencode

    // Find the line with the label
    let labeled_line = lines
        .iter()
        .find(|l| l.label.is_some())
        .expect("Should have a labeled line");

    // VALIDATE: Check that the labeled line is ".text", NOT ".encode"
    assert_eq!(
        labeled_line.mnemonic, ".text",
        "Label should be on .text line, found on {}",
        labeled_line.mnemonic
    );
    assert_eq!(labeled_line.label.as_deref(), Some("ScreenLabel"));

    // VALIDATE: Check that the line with label ALSO has the comment
    assert!(
        labeled_line.comment.contains("My Side Comment"),
        "Comment should be on line with label (.text), found: '{}'",
        labeled_line.comment
    );
}
