#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// ACME formatter tests — instruction formatting, directives, text/screencode encoding
///
/// Mirrors the test structure of `formatter_ca65_tests.rs` and `formatter_kickasm_tests.rs`,
/// providing equivalent coverage for the ACME assembler output.
use regenerator_core::disassembler::Disassembler;
use regenerator_core::state::{Assembler, DocumentSettings};
use std::collections::BTreeMap;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator_core::cpu::get_opcodes();

    // LDA #$00  (A9 00)
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA9].as_ref().unwrap(),
        operands: &[0x00],
        address: regenerator_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "#$00".to_string())
    );

    // STA $D020  (8D 20 D0)
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x8D].as_ref().unwrap(),
        operands: &[0x20, 0xD0],
        address: regenerator_core::state::Addr(0x1002),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("sta".to_string(), "$d020".to_string())
    );

    // NOP  (EA) — Implied
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xEA].as_ref().unwrap(),
        operands: &[],
        address: regenerator_core::state::Addr(0x1005),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("nop".to_string(), String::new())
    );

    // ASL (Accumulator) — ACME uses empty operand (not "a")
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x0A].as_ref().unwrap(),
        operands: &[],
        address: regenerator_core::state::Addr(0x1006),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    let (mnemonic, operand) = formatter.format_instruction(&ctx);
    assert_eq!(mnemonic, "asl");
    assert_eq!(operand, ""); // ACME uses implicit accumulator
}

#[test]
fn test_origin() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(
        formatter.format_header_origin(regenerator_core::state::Addr(0x0801)),
        "* = $0801"
    );
    assert_eq!(
        formatter.format_header_origin(regenerator_core::state::Addr(0xC000)),
        "* = $c000"
    );
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    // ACME uses label without colon for both definition and reference
    assert_eq!(formatter.format_label("MyLabel"), "MyLabel");
    assert_eq!(formatter.format_label_definition("MyLabel"), "MyLabel");
}

#[test]
fn test_directives() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.byte_directive(), "!byte");
    assert_eq!(formatter.word_directive(), "!word");
    assert_eq!(formatter.comment_prefix(), ";");
}

#[test]
fn test_forced_absolute_plus2() {
    let mut settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator_core::cpu::get_opcodes();

    // ACME uses "+2" suffix for forced 16-bit addressing
    settings.preserve_long_bytes = true;
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: regenerator_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda+2".to_string(), "$0002".to_string())
    );

    // With preserve_long_bytes=false, no +2
    let mut settings_false = settings.clone();
    settings_false.preserve_long_bytes = false;
    let ctx_false = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x02, 0x00],
        address: regenerator_core::state::Addr(0x1000),
        target_context: None,
        labels: &labels,
        settings: &settings_false,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_false),
        ("lda".to_string(), "$0002".to_string())
    );

    // Absolute,X with forced absolute: STA $0010,X
    let ctx_x = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0x9D].as_ref().unwrap(), // STA abs,X
        operands: &[0x10, 0x00],
        address: regenerator_core::state::Addr(0x1003),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_x),
        ("sta+2".to_string(), "$0010,x".to_string())
    );

    // Non-ZP absolute address: should NOT get +2
    let ctx_normal = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xAD].as_ref().unwrap(),
        operands: &[0x20, 0xD0],
        address: regenerator_core::state::Addr(0x1006),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx_normal),
        ("lda".to_string(), "$d020".to_string())
    );
}

#[test]
fn test_addressing_modes() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator_core::cpu::get_opcodes();

    // LDA ($20,X) — Indirect X
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xA1].as_ref().unwrap(),
        operands: &[0x20],
        address: regenerator_core::state::Addr(0x2000),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "($20,x)".to_string())
    );

    // LDA ($20),Y — Indirect Y
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xB1].as_ref().unwrap(),
        operands: &[0x20],
        address: regenerator_core::state::Addr(0x2002),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("lda".to_string(), "($20),y".to_string())
    );

    // BNE $2007 — Relative forward
    let ctx = regenerator_core::disassembler::formatter::FormatContext {
        opcode: opcodes[0xD0].as_ref().unwrap(),
        operands: &[0x03],
        address: regenerator_core::state::Addr(0x2004),
        target_context: None,
        labels: &labels,
        settings: &settings,
        immediate_value_formats: &immediate_value_formats,
    };
    assert_eq!(
        formatter.format_instruction(&ctx),
        ("bne".to_string(), "$2009".to_string())
    );
}

#[test]
fn test_definition() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // ACME: always uses shortest hex representation to avoid forcing absolute
    assert_eq!(
        formatter.format_definition("zpVar", 0x00C5, true),
        "zpVar = $c5"
    );
    assert_eq!(
        formatter.format_definition("kernal", 0xFFD2, false),
        "kernal = $ffd2"
    );
    // Even non-ZP, if value <= 0xFF, use $xx form
    assert_eq!(
        formatter.format_definition("lowAddr", 0x0010, false),
        "lowAddr = $10"
    );
}

#[test]
fn test_file_header() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let header = formatter.format_file_header("my_game", false);
    assert!(header.contains("Regenerator 2000"));
    assert!(header.contains("acme"));
    assert!(header.contains("--format cbm"));
    assert!(header.contains("-o my_game.prg my_game.asm"));
    assert!(!header.contains("--cpu 6510")); // no illegal opcodes

    let header_illegal = formatter.format_file_header("my_game", true);
    assert!(header_illegal.contains("--cpu 6510"));
}

#[test]
fn test_text_formatting() {
    use regenerator_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Pure text: !text "hello"
    let fragments = vec![TextFragment::Text("hello".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!text");
    assert_eq!(lines[0].1, "\"hello\"");

    // Mixed text and bytes
    let fragments = vec![
        TextFragment::Text("AB".to_string()),
        TextFragment::Byte(0x00),
        TextFragment::Text("CD".to_string()),
    ];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!text");
    assert_eq!(lines[0].1, "\"AB\", $00, \"CD\"");
}

#[test]
fn test_text_escaping() {
    use regenerator_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // Quotes: escaped with backslash in ACME
    let fragments = vec![TextFragment::Text("SAY \"HI\"".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    // ACME escapes with backslash
    assert_eq!(lines[0].1, "\"SAY \\\"HI\\\"\"");

    // Backslashes: escaped with double backslash
    let fragments = vec![TextFragment::Text("C:\\DOS".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines[0].1, "\"C:\\\\DOS\"");
}

#[test]
fn test_screencode_formatting() {
    use regenerator_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    // ACME: screencode pre/post are empty (no encoding wrapper)
    assert!(formatter.format_screencode_pre().is_empty());
    assert!(formatter.format_screencode_post().is_empty());

    // Screencode body: uses !scr with case-swapped text
    let fragments = vec![TextFragment::Text("Hello World".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!scr");
    assert_eq!(lines[0].1, "\"hELLO wORLD\"");

    // Screencode with special chars that need hex escaping
    let fragments = vec![TextFragment::Text("A{B|C}D~E".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!scr");
    // { -> $5b, | -> $5c, } -> $5d, ~ -> $5e, letters case-swapped
    assert_eq!(
        lines[0].1,
        "\"a\", $5b, \"b\", $5c, \"c\", $5d, \"d\", $5e, \"e\""
    );
}

#[test]
fn test_screencode_bytes() {
    use regenerator_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let fragments = vec![
        TextFragment::Text("AB".to_string()),
        TextFragment::Byte(0xFF),
    ];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!scr");
    assert_eq!(lines[0].1, "\"ab\", $ff");
}

#[test]
fn test_lax_immediate_mnemonic() {
    // ACME uses "lxa" for opcode $AB (LAX immediate), not "lax"
    let mut settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    settings.use_illegal_opcodes = true;
    let formatter = Disassembler::create_formatter(settings.assembler);
    let labels = BTreeMap::new();
    let immediate_value_formats = BTreeMap::new();
    let opcodes = regenerator_core::cpu::get_opcodes();

    if let Some(opcode) = opcodes[0xAB].as_ref() {
        let ctx = regenerator_core::disassembler::formatter::FormatContext {
            opcode,
            operands: &[0x42],
            address: regenerator_core::state::Addr(0x1000),
            target_context: None,
            labels: &labels,
            settings: &settings,
            immediate_value_formats: &immediate_value_formats,
        };
        let (mnemonic, _) = formatter.format_instruction(&ctx);
        assert_eq!(mnemonic, "lxa", "ACME should use 'lxa' for opcode $AB");
    }
}
