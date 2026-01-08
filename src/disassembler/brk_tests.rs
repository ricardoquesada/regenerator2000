use crate::disassembler::Disassembler;
use crate::state::{BlockType, DocumentSettings, Label, LabelKind, LabelType};
use std::collections::BTreeMap;

#[test]
fn test_brk_default_behavior() {
    // BRK single byte: DISABLED
    // Patch BRK: DISABLED
    // Expected: $00 $00 -> "BRK #$00"
    // $00 $10 -> "BRK #$10"

    let disassembler = Disassembler::new();
    let data = vec![0x00, 0x10]; // BRK, Signature $10
    let block_types = vec![BlockType::Code; 2];
    let settings = DocumentSettings {
        brk_single_byte: false,
        patch_brk: false,
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
    );

    assert_eq!(lines.len(), 1);
    // Tass formatter uses lowercase
    assert_eq!(lines[0].mnemonic, "brk");
    assert_eq!(lines[0].operand, "#$10");
    assert_eq!(lines[0].bytes, vec![0x00, 0x10]);
}

#[test]
fn test_brk_patch_brk_enabled() {
    // BRK single byte: DISABLED
    // Patch BRK: ENABLED
    // Expected: $00 $10 -> "BRK", ".byte $10"

    let disassembler = Disassembler::new();
    let data = vec![0x00, 0x10];
    let block_types = vec![BlockType::Code; 2];
    let settings = DocumentSettings {
        brk_single_byte: false,
        patch_brk: true,
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
    );

    assert_eq!(lines.len(), 2);

    assert_eq!(lines[0].mnemonic, "brk");
    assert_eq!(lines[0].operand, ""); // No operand for single-byte BRK
    assert_eq!(lines[0].bytes, vec![0x00]);

    assert_eq!(lines[1].mnemonic, ".byte");
    assert_eq!(lines[1].operand, "$10");
    assert_eq!(lines[1].bytes, vec![0x10]);
}

#[test]
fn test_brk_single_byte_enabled() {
    // BRK single byte: ENABLED
    // Patch BRK: ignored (assumed)
    // Expected: $00 $00 -> "BRK", "BRK"

    let disassembler = Disassembler::new();
    let data = vec![0x00, 0x00];
    let block_types = vec![BlockType::Code; 2];
    let settings = DocumentSettings {
        brk_single_byte: true,
        patch_brk: false, // Should default to single byte behavior regardless of patch_brk
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
    );

    assert_eq!(lines.len(), 2);

    // First BRK
    assert_eq!(lines[0].mnemonic, "brk");
    assert_eq!(lines[0].operand, "");
    assert_eq!(lines[0].bytes, vec![0x00]);

    // Second BRK
    assert_eq!(lines[1].mnemonic, "brk");
    assert_eq!(lines[1].operand, "");
    assert_eq!(lines[1].bytes, vec![0x00]);
}

#[test]
fn test_brk_patch_brk_with_label() {
    // BRK patch enabled, and there is a label on the second byte.
    // Address $1000: BRK
    // Address $1001: Signature byte (with label "b1001")

    let disassembler = Disassembler::new();
    let data = vec![0x00, 0x01];
    let block_types = vec![BlockType::Code; 2];
    let settings = DocumentSettings {
        brk_single_byte: false,
        patch_brk: true,
        ..Default::default()
    };

    let mut labels = BTreeMap::new();
    labels.insert(
        0x1001,
        vec![Label {
            name: "b1001".to_string(),
            label_type: LabelType::UserDefined,
            kind: LabelKind::User,
        }],
    );

    let lines = disassembler.disassemble(
        &data,
        &block_types,
        &labels,
        0x1000,
        &settings,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
    );

    assert_eq!(lines.len(), 2);

    assert_eq!(lines[0].mnemonic, "brk");
    // Check second byte
    assert_eq!(lines[1].label, Some("b1001".to_string()));
    assert_eq!(lines[1].mnemonic, ".byte");
    assert_eq!(lines[1].operand, "$01");
}
