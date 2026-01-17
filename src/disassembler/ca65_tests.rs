use crate::disassembler::Disassembler;
use crate::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_format_instructions() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Ca65;
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = crate::cpu::get_opcodes();

    // LDA #$00
    let ctx = crate::disassembler::formatter::FormatContext {
        opcode: &opcodes[0xA9].as_ref().unwrap(),
        operands: &[0x00],
        address: 0x1000,
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "#$00".to_string())
    );

    // STA $D020
    let ctx = crate::disassembler::formatter::FormatContext {
        opcode: &opcodes[0x8D].as_ref().unwrap(),
        operands: &[0x20, 0xD0],
        address: 0x1002,
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("sta".to_string(), "$d020".to_string())
    );
}

#[test]
fn test_origin() {
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.format_header_origin(0x1000), ".org $1000");
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.format_label("MyLabel"), "MyLabel");
    assert_eq!(formatter.format_label_definition("MyLabel"), "MyLabel:");
}
