#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// ACME formatter tests — instruction formatting, directives, text/screencode encoding
///
/// Mirrors the test structure of `formatter_ca65_tests.rs` and `formatter_kickasm_tests.rs`,
/// providing equivalent coverage for the ACME assembler output.
use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{Addr, Assembler, DocumentSettings};
use symbolic_offset_formatter::format_raw_instruction;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };

    for (opcode, operands, addr, expected) in [
        (0xA9, &[0x00][..], 0x1000, ("lda", "#$00")),
        (0x8D, &[0x20, 0xD0][..], 0x1002, ("sta", "$d020")),
        (0xEA, &[][..], 0x1005, ("nop", "")),
        (0x0A, &[][..], 0x1006, ("asl", "")),
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
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(formatter.format_header_origin(Addr(0x0801)), "* = $0801");
    assert_eq!(formatter.format_header_origin(Addr(0xC000)), "* = $c000");
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
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
        preserve_long_bytes: true,
        ..Default::default()
    };

    // ACME uses "+2" suffix for forced 16-bit addressing
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda+2".to_string(), "$0002".to_string())
    );
    assert_eq!(
        format_raw_instruction(&settings, 0x9D, &[0x10, 0x00], 0x1003),
        ("sta+2".to_string(), "$0010,x".to_string())
    );
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x20, 0xD0], 0x1006),
        ("lda".to_string(), "$d020".to_string())
    );

    // With preserve_long_bytes=false, no +2
    settings.preserve_long_bytes = false;
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda".to_string(), "$0002".to_string())
    );
}

#[test]
fn test_addressing_modes() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };

    for (opcode, operands, addr, expected) in [
        (0xA1, &[0x20][..], 0x2000, ("lda", "($20,x)")),
        (0xB1, &[0x20][..], 0x2002, ("lda", "($20),y")),
        (0xD0, &[0x03][..], 0x2004, ("bne", "$2009")),
    ] {
        assert_eq!(
            format_raw_instruction(&settings, opcode, operands, addr),
            (expected.0.to_string(), expected.1.to_string())
        );
    }
}

#[test]
fn test_definition() {
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    assert_eq!(
        formatter.format_definition("zpVar", 0x00C5, true),
        "zpVar = $c5"
    );
    assert_eq!(
        formatter.format_definition("kernal", 0xFFD2, false),
        "kernal = $ffd2"
    );
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
    assert!(header.contains("acme"));
    assert!(header.contains("--format cbm"));
    assert!(header.contains("-o my_game.prg my_game.asm"));
    assert!(!header.contains("--cpu 6510"));

    let header_illegal = formatter.format_file_header("my_game", true);
    assert!(header_illegal.contains("--cpu 6510"));
}

#[test]
fn test_text_formatting() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let fragments = vec![TextFragment::Text("hello".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!text");
    assert_eq!(lines[0].1, "\"hello\"");

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
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    let fragments = vec![TextFragment::Text("SAY \"HI\"".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines[0].1, "\"SAY \\\"HI\\\"\"");

    let fragments = vec![TextFragment::Text("C:\\DOS".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines[0].1, "\"C:\\\\DOS\"");
}

#[test]
fn test_screencode_formatting() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);

    assert!(formatter.format_screencode_pre().is_empty());
    assert!(formatter.format_screencode_post().is_empty());

    let fragments = vec![TextFragment::Text("Hello World".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!scr");
    assert_eq!(lines[0].1, "\"hELLO wORLD\"");

    let fragments = vec![TextFragment::Text("A{B|C}D~E".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, "!scr");
    assert_eq!(
        lines[0].1,
        "\"a\", $5b, \"b\", $5c, \"c\", $5d, \"d\", $5e, \"e\""
    );
}

#[test]
fn test_screencode_bytes() {
    use regenerator2000_core::disassembler::formatter::TextFragment;

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
    let settings = DocumentSettings {
        assembler: Assembler::Acme,
        use_illegal_opcodes: true,
        ..Default::default()
    };
    let (mnemonic, _) = format_raw_instruction(&settings, 0xAB, &[0x42], 0x1000);
    assert_eq!(mnemonic, "lxa", "ACME should use 'lxa' for opcode $AB");
}

#[path = "common/symbolic_offset_formatter.rs"]
mod symbolic_offset_formatter;

#[test]
fn test_symbolic_offset_formatting_all_modes_acme() {
    symbolic_offset_formatter::assert_symbolic_offset_formatting_all_modes(
        Assembler::Acme,
        "lda+2",
        "table_0100 - 1,x",
    );
}
