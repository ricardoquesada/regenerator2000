use crate::disassembler::Disassembler;
use crate::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = crate::cpu::get_opcodes();

    // LDA #$00
    let ctx = crate::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA9].as_ref().unwrap(),
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
        opcode: opcodes[0x8D].as_ref().unwrap(),
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
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // KickAssembler usually uses *=$xxxx
    assert_eq!(formatter.format_header_origin(0x1000), "*=$1000");
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // Label references: "Label"
    // Label definitions: "Label:"
    assert_eq!(formatter.format_label("MyLabel"), "MyLabel");
    assert_eq!(formatter.format_label_definition("MyLabel"), "MyLabel:");
}

#[test]
fn test_relative_label() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // .label myLabel = * + 10
    assert_eq!(
        formatter.format_relative_label("myLabel", 10),
        ".label myLabel = * + 10"
    );
}

#[test]
fn test_forced_absolute() {
    let mut settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = crate::cpu::get_opcodes();

    // True functionality: should output .abs
    settings.preserve_long_bytes = true;
    let ctx = crate::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: 0x1000,
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda.abs".to_string(), "$0002".to_string())
    );

    // False functionality: should NOT output .abs
    // Note: If preserve_long_bytes is false, the disassembler usually tries to reduce to ZP if possible
    // but here we are testing the formatter output given a specific instruction context (Absolute).
    // If the instruction IS Absolute in the context (0xAD) but the settings say don't preserve long bytes,
    // technically the re-assembler might optimize it back to ZP if we don't force it.
    // But the request is to control the .abs suffix.
    // If we omit .abs, KickAssembler will likely assemble it as ZP ($A5).
    // This matches the behavior of "not preserving" the long form.
    let mut settings_false = settings;
    settings_false.preserve_long_bytes = false;
    let ctx_false = crate::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
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
        opcode: opcodes[0xA5].as_ref().unwrap(),
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

#[test]
fn test_text_encoding() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Test implicitly creates formatter
    use crate::disassembler::formatter::TextFragment;

    // 1. Text Start -> .encoding "ascii" + .text
    let fragments = vec![TextFragment::Text("hello".to_string())];
    let lines = formatter.format_text(&fragments, true, false);
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        (
            ".encoding".to_string(),
            "\"petscii_upper\"".to_string(),
            false
        )
    );
    assert_eq!(
        lines[1],
        (".text".to_string(), "@\"hello\"".to_string(), true)
    );

    // 2. Text Continuation -> .text only
    let lines_cont = formatter.format_text(&fragments, false, false);
    assert_eq!(lines_cont.len(), 1);
    assert_eq!(
        lines_cont[0],
        (".text".to_string(), "@\"hello\"".to_string(), true)
    );
}

#[test]
fn test_screencode_encoding() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use crate::disassembler::formatter::TextFragment;

    // 1. Screencode Pre -> .encoding "screencode_upper"
    let pre_lines = formatter.format_screencode_pre();
    assert_eq!(pre_lines.len(), 1);
    assert_eq!(
        pre_lines[0],
        (".encoding".to_string(), "\"screencode_mixed\"".to_string())
    );

    // 2. Screencode Body -> .text (no manual inversion)
    // "Hello" should stay "Hello" because "screencode_upper" handles the mapping/inversion
    let fragments = vec![TextFragment::Text("Hello".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        (".text".to_string(), "@\"Hello\"".to_string(), true)
    );
}

#[test]
fn test_mixed_encoding() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use crate::disassembler::formatter::TextFragment;

    // Mixed text and bytes
    let fragments = vec![
        TextFragment::Text("hello".to_string()),
        TextFragment::Byte(0xFF),
        TextFragment::Text("world".to_string()),
    ];

    let lines = formatter.format_text(&fragments, true, false);
    // Expected:
    // .encoding "ascii"
    // .text "hello"
    // .byte $ff
    // .text "world"
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines[0],
        (
            ".encoding".to_string(),
            "\"petscii_upper\"".to_string(),
            false
        )
    );
    assert_eq!(
        lines[1],
        (".text".to_string(), "@\"hello\"".to_string(), true)
    );
    assert_eq!(lines[2], (".byte".to_string(), "$ff".to_string(), true));
    assert_eq!(
        lines[3],
        (".text".to_string(), "@\"world\"".to_string(), true)
    );
}

#[test]
fn test_quote_escaping() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use crate::disassembler::formatter::TextFragment;

    // String with quotes: He said "Hi"
    let fragments = vec![TextFragment::Text("He said \"Hi\"".to_string())];
    let lines = formatter.format_text(&fragments, true, false);

    // Expected: .text @"He said \"Hi\""
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        (
            ".encoding".to_string(),
            "\"petscii_upper\"".to_string(),
            false
        )
    );
    // Verify double-quote escaping
    assert_eq!(
        lines[1],
        (
            ".text".to_string(),
            "@\"He said \\\"Hi\\\"\"".to_string(),
            true
        )
    );
}
