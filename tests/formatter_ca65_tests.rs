use regenerator2000::disassembler::Disassembler;
use regenerator2000::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000::cpu::get_opcodes();

    // LDA #$00
    let ctx = regenerator2000::disassembler::formatter::FormatContext {
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
    let ctx = regenerator2000::disassembler::formatter::FormatContext {
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
    let mut settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000::cpu::get_opcodes();

    // LDA $0002 (Absolute) -> AD 02 00
    // Should be formatted as "lda a:$0002" because value <= $FF
    settings.preserve_long_bytes = true;
    let ctx = regenerator2000::disassembler::formatter::FormatContext {
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
        ("lda".to_string(), "a:$0002".to_string())
    );

    // False functionality: should NOT output a: prefix
    let mut settings_false = settings;
    settings_false.preserve_long_bytes = false;
    let ctx_false = regenerator2000::disassembler::formatter::FormatContext {
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
    let ctx_zp = regenerator2000::disassembler::formatter::FormatContext {
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
fn test_format_screencode() {
    use regenerator2000::disassembler::formatter::TextFragment;
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Case 1: Pure text
    let fragments = vec![TextFragment::Text("HELLO WORLD".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "scrcode");
    assert_eq!(lines[0].1, "\"hello world\""); // Swapped case

    // Case 2: Text mixed with bytes
    let fragments = vec![
        TextFragment::Text("HELLO".to_string()),
        TextFragment::Byte(0x00),
        TextFragment::Text("WORLD".to_string()),
    ];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].0, "scrcode");
    assert_eq!(lines[0].1, "\"hello\""); // Swapped case
    assert_eq!(lines[1].0, ".byte");
    assert_eq!(lines[1].1, "$00");
    assert_eq!(lines[2].0, "scrcode");
    assert_eq!(lines[2].1, "\"world\""); // Swapped case

    // Case 3: Quote escaping
    let fragments = vec![TextFragment::Text("FOO\"BAR".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "scrcode");
    assert_eq!(lines[0].1, "\"foo\", $22, \"bar\""); // Swapped case

    // Case 4: Case swapping
    // "Hello World" -> "hELLO wORLD"
    let fragments = vec![TextFragment::Text("Hello World".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines[0].1, "\"hELLO wORLD\"");
    // Case 5: Multiple bytes
    let fragments = vec![
        TextFragment::Byte(0xA9),
        TextFragment::Byte(0xA9),
        TextFragment::Byte(0xA9),
    ];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, ".byte");
    assert_eq!(lines[0].1, "$a9, $a9, $a9");
}

#[test]
fn test_format_text_escaping() {
    use regenerator2000::disassembler::formatter::TextFragment;
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Case 1: Quotes
    // Expected: "he said ", $22, "hi", $22
    let fragments = vec![TextFragment::Text("HE SAID \"HI\"".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    // format_text returns Vec<(String, String, bool)> where .1 is the operand
    // We expect: .byte "he said ", $22, "hi", $22
    assert_eq!(lines[0].1, "\"he said \", $22, \"hi\", $22");

    // Case 2: Backslash
    // Expected: "C:\DOS" (no escaping for backslash)
    let fragments = vec![TextFragment::Text("C:\\DOS".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines[0].1, "\"c:\\\\dos\"");
}
