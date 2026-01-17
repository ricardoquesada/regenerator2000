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

#[test]
fn test_forced_absolute() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Ca65;
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = crate::cpu::get_opcodes();

    // LDA $0002 (Absolute) -> AD 02 00
    // Should be formatted as "lda a:$0002" because value <= $FF
    settings.preserve_long_bytes = true;
    let ctx = crate::disassembler::formatter::FormatContext {
        opcode: &opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: 0x1000,
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "a:$0002".to_string())
    );

    // False functionality: should NOT output a: prefix
    let mut settings_false = settings.clone();
    settings_false.preserve_long_bytes = false;
    let ctx_false = crate::disassembler::formatter::FormatContext {
        opcode: &opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: 0x1000,
        target_context: None,
        labels: &labels,
        settings: &settings_false,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_false),
        ("lda".to_string(), "$0002".to_string())
    );

    // LDA $02 (ZeroPage) -> A5 02
    // Should be formatted as "lda $02"
    let ctx_zp = crate::disassembler::formatter::FormatContext {
        opcode: &opcodes[0xA5].as_ref().unwrap(),
        operands: &[0x02],
        address: 0x1000,
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_zp),
        ("lda".to_string(), "$02".to_string())
    );
}
