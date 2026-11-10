#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// 64tass formatter tests — instruction formatting, directives, text/screencode encoding
///
/// Mirrors the test structure of `formatter_ca65_tests.rs` and `formatter_kickasm_tests.rs`,
/// providing equivalent coverage for the 64tass assembler output.
use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{Addr, Assembler, DocumentSettings};
use symbolic_offset_formatter::format_raw_instruction;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    for (opcode, operands, addr, expected) in [
        (0xA9, &[0x00][..], 0x1000, ("lda", "#$00")),
        (0x8D, &[0x20, 0xD0][..], 0x1002, ("sta", "$d020")),
        (0xA5, &[0x02][..], 0x1004, ("lda", "$02")),
        (0x6C, &[0x34, 0x12][..], 0x1006, ("jmp", "($1234)")),
        (0x60, &[][..], 0x1009, ("rts", "")),
        (0x4A, &[][..], 0x100A, ("lsr", "a")),
    ] {
        assert_eq!(
            format_raw_instruction(&settings, opcode, operands, addr),
            (expected.0.to_string(), expected.1.to_string())
        );
    }
}

#[test]
fn test_origin() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.format_header_origin(Addr(0x0801)), "* = $0801");
    assert_eq!(formatter.format_header_origin(Addr(0xC000)), "* = $c000");
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
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
    assert_eq!(formatter.format_address(Addr(0x0000)), "$0000");
    assert_eq!(formatter.format_address(Addr(0xFFFF)), "$ffff");
}

#[test]
fn test_forced_absolute() {
    let mut settings = DocumentSettings {
        assembler: Assembler::Tass64,
        preserve_long_bytes: true,
        ..Default::default()
    };

    // LDA $0002 (Absolute) — 64tass uses "@w" prefix for forced absolute
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda".to_string(), "@w $0002".to_string())
    );

    // With preserve_long_bytes=false, should NOT use @w prefix
    settings.preserve_long_bytes = false;
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda".to_string(), "$0002".to_string())
    );
}

#[test]
fn test_addressing_modes() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    for (opcode, operands, addr, expected) in [
        (0x95, &[0x02][..], 0x2000, ("sta", "$02,x")),
        (0xB6, &[0x02][..], 0x2002, ("ldx", "$02,y")),
        (0xBD, &[0x34, 0x12][..], 0x2004, ("lda", "$1234,x")),
        (0xB9, &[0x34, 0x12][..], 0x2007, ("lda", "$1234,y")),
        (0xA1, &[0x20][..], 0x200A, ("lda", "($20,x)")),
        (0xB1, &[0x20][..], 0x200C, ("lda", "($20),y")),
    ] {
        assert_eq!(
            format_raw_instruction(&settings, opcode, operands, addr),
            (expected.0.to_string(), expected.1.to_string())
        );
    }
}

#[test]
fn test_relative_branch() {
    let settings = DocumentSettings {
        assembler: Assembler::Tass64,
        ..Default::default()
    };

    assert_eq!(
        format_raw_instruction(&settings, 0xD0, &[0x03], 0x1000),
        ("bne".to_string(), "$1005".to_string())
    );
    assert_eq!(
        format_raw_instruction(&settings, 0xF0, &[0xFC], 0x1010),
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

#[path = "common/symbolic_offset_formatter.rs"]
mod symbolic_offset_formatter;

#[test]
fn test_symbolic_offset_formatting_all_modes_64tass() {
    symbolic_offset_formatter::assert_symbolic_offset_formatting_all_modes(
        Assembler::Tass64,
        "lda",
        "@w table_0100 - 1,x",
    );
}
