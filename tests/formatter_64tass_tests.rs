#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// 64tass formatter tests — instruction formatting, directives, text/screencode encoding
///
/// Mirrors the test structure of `formatter_ca65_tests.rs` and `formatter_kickasm_tests.rs`,
/// providing equivalent coverage for the 64tass assembler output.
use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000_core::cpu::get_opcodes();

    // LDA #$00  (A9 00)
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA9].as_ref().unwrap(),
        operands: &[0x00],
        address: regenerator2000_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "#$00".to_string())
    );

    // STA $D020  (8D 20 D0)
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x8D].as_ref().unwrap(),
        operands: &[0x20, 0xD0],
        address: regenerator2000_core::state::Addr(0x1002),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("sta".to_string(), "$d020".to_string())
    );

    // LDA $02  (A5 02) — Zero Page
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA5].as_ref().unwrap(),
        operands: &[0x02],
        address: regenerator2000_core::state::Addr(0x1004),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "$02".to_string())
    );

    // JMP ($1234)  (6C 34 12) — Indirect
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x6C].as_ref().unwrap(),
        operands: &[0x34, 0x12],
        address: regenerator2000_core::state::Addr(0x1006),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("jmp".to_string(), "($1234)".to_string())
    );

    // RTS  (60) — Implied
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x60].as_ref().unwrap(),
        operands: &[],
        address: regenerator2000_core::state::Addr(0x1009),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("rts".to_string(), String::new())
    );

    // LSR A  (4A) — Accumulator
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x4A].as_ref().unwrap(),
        operands: &[],
        address: regenerator2000_core::state::Addr(0x100A),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lsr".to_string(), "a".to_string())
    );
}

#[test]
fn test_origin() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(
        formatter.format_header_origin(regenerator2000_core::state::Addr(0x0801)),
        "* = $0801"
    );
    assert_eq!(
        formatter.format_header_origin(regenerator2000_core::state::Addr(0xC000)),
        "* = $c000"
    );
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // 64tass uses label (no colon) for both reference and definition
    assert_eq!(formatter.format_label("MyLabel"), "MyLabel");
    assert_eq!(formatter.format_label_definition("MyLabel"), "MyLabel");
}

#[test]
fn test_directives() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.byte_directive(), ".byte");
    assert_eq!(formatter.word_directive(), ".word");
    assert_eq!(formatter.comment_prefix(), ";");
}

#[test]
fn test_format_byte_and_address() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.format_byte(0x00), "$00");
    assert_eq!(formatter.format_byte(0xFF), "$ff");
    assert_eq!(
        formatter.format_address(regenerator2000_core::state::Addr(0x0000)),
        "$0000"
    );
    assert_eq!(
        formatter.format_address(regenerator2000_core::state::Addr(0xFFFF)),
        "$ffff"
    );
}

#[test]
fn test_forced_absolute() {
    let mut settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000_core::cpu::get_opcodes();

    // LDA $0002 (Absolute) — 64tass uses "@w" prefix for forced absolute
    settings.preserve_long_bytes = true;
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: regenerator2000_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "@w $0002".to_string())
    );

    // With preserve_long_bytes=false, should NOT use @w prefix
    let mut settings_false = settings.clone();
    settings_false.preserve_long_bytes = false;
    let ctx_false = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: regenerator2000_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings_false,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_false),
        ("lda".to_string(), "$0002".to_string())
    );
}

#[test]
fn test_addressing_modes() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000_core::cpu::get_opcodes();

    // STA $02,X (Zero Page,X) — 95 02
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x95].as_ref().unwrap(),
        operands: &[0x02],
        address: regenerator2000_core::state::Addr(0x2000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("sta".to_string(), "$02,x".to_string())
    );

    // LDX $02,Y (Zero Page,Y) — B6 02
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xB6].as_ref().unwrap(),
        operands: &[0x02],
        address: regenerator2000_core::state::Addr(0x2002),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("ldx".to_string(), "$02,y".to_string())
    );

    // LDA $1234,X (Absolute,X) — BD 34 12
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xBD].as_ref().unwrap(),
        operands: &[0x34, 0x12],
        address: regenerator2000_core::state::Addr(0x2004),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "$1234,x".to_string())
    );

    // LDA $1234,Y (Absolute,Y) — B9 34 12
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xB9].as_ref().unwrap(),
        operands: &[0x34, 0x12],
        address: regenerator2000_core::state::Addr(0x2007),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "$1234,y".to_string())
    );

    // LDA ($20,X) (Indirect,X) — A1 20
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA1].as_ref().unwrap(),
        operands: &[0x20],
        address: regenerator2000_core::state::Addr(0x200A),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "($20,x)".to_string())
    );

    // LDA ($20),Y (Indirect,Y) — B1 20
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xB1].as_ref().unwrap(),
        operands: &[0x20],
        address: regenerator2000_core::state::Addr(0x200C),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "($20),y".to_string())
    );
}

#[test]
fn test_relative_branch() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator2000_core::cpu::get_opcodes();

    // BNE $1005 — branch forward 3 bytes from PC+2 => offset = 3
    // At address $1000, BNE with offset 3 => target = $1000 + 2 + 3 = $1005
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xD0].as_ref().unwrap(),
        operands: &[0x03],
        address: regenerator2000_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("bne".to_string(), "$1005".to_string())
    );

    // BEQ backwards — offset = -4 (0xFC) from $1010+2 = $100E
    let ctx = regenerator2000_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xF0].as_ref().unwrap(),
        operands: &[0xFC], // -4
        address: regenerator2000_core::state::Addr(0x1010),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
        local_label_names: None,
        label_scope_names: None,
        current_scope_name: None,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("beq".to_string(), "$100e".to_string())
    );
}

#[test]
fn test_definition() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // Zero page definition (is_zp=true, value <= 0xFF)
    assert_eq!(
        formatter.format_definition("zpVar", 0x00C5, true),
        "zpVar = $c5"
    );
    // Non-ZP definition
    assert_eq!(
        formatter.format_definition("myLabel", 0xD020, false),
        "myLabel = $d020"
    );
    // ZP flag but value > 0xFF
    assert_eq!(
        formatter.format_definition("bigZp", 0x1234, true),
        "bigZp = $1234"
    );
}

#[test]
fn test_file_header() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let header = formatter.format_file_header("my_program", false);
    assert!(header.contains("Regenerator 2000"));
    assert!(header.contains("64tass"));
    assert!(header.contains("-o my_program.prg my_program.asm"));
    assert!(!header.contains("-i")); // no illegal opcodes flag

    let header_illegal = formatter.format_file_header("game", true);
    assert!(header_illegal.contains("-i ")); // illegal opcodes flag
}

#[test]
fn test_text_encoding() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Start of text block: should emit .encode / .enc "none" / .text
    let fragments = vec![TextFragment::Text("HELLO".to_string())];
    let lines = formatter.format_text(&fragments, true, false);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].0, ".encode");
    assert_eq!(lines[1].0, ".enc");
    assert_eq!(lines[1].1, "\"none\"");
    assert_eq!(lines[2].0, ".text");
    assert_eq!(lines[2].1, "\"HELLO\"");

    // End of text block: should add .endencode
    let lines = formatter.format_text(&fragments, false, true);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, ".text");
    assert_eq!(lines[1].0, ".endencode");

    // Start+end: everything
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].0, ".encode");
    assert_eq!(lines[3].0, ".endencode");
}

#[test]
fn test_text_mixed_with_bytes() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let fragments = vec![
        TextFragment::Text("AB".to_string()),
        TextFragment::Byte(0xFF),
        TextFragment::Text("CD".to_string()),
    ];
    let lines = formatter.format_text(&fragments, false, false);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, ".text");
    assert_eq!(lines[0].1, "\"AB\", $ff, \"CD\"");
}

#[test]
fn test_screencode_encoding() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Pre: .encode + .enc "screen"
    let pre = formatter.format_screencode_pre();
    assert_eq!(pre.len(), 2);
    assert_eq!(pre[0].0, ".encode");
    assert_eq!(pre[1].0, ".enc");
    assert_eq!(pre[1].1, "\"screen\"");

    // Body: .text with content
    let fragments = vec![TextFragment::Text("HELLO".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, ".text");
    assert_eq!(lines[0].1, "\"HELLO\"");

    // Post: .endencode
    let post = formatter.format_screencode_post();
    assert_eq!(post.len(), 1);
    assert_eq!(post[0].0, ".endencode");
}

#[test]
fn test_text_quote_escaping() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // 64tass escapes quotes by doubling them: "say ""hi"""
    let fragments = vec![TextFragment::Text("SAY \"HI\"".to_string())];
    let lines = formatter.format_text(&fragments, false, false);
    assert_eq!(lines[0].1, "\"SAY \"\"HI\"\"\"");
}
