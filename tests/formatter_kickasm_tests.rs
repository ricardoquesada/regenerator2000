#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{Addr, Assembler, DocumentSettings};
use symbolic_offset_formatter::format_raw_instruction;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };

    // LDA #$00
    assert_eq!(
        format_raw_instruction(&settings, 0xA9, &[0x00], 0x1000),
        ("lda".to_string(), "#$00".to_string())
    );

    // STA $D020
    assert_eq!(
        format_raw_instruction(&settings, 0x8D, &[0x20, 0xD0], 0x1002),
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
    assert_eq!(formatter.format_header_origin(Addr(0x1000)), "*=$1000");
}

#[test]
fn test_labels() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
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
    assert_eq!(
        formatter.format_relative_label("myLabel", 10),
        ".label myLabel = * + 10"
    );
}

#[test]
fn test_forced_absolute() {
    let mut settings = DocumentSettings {
        assembler: Assembler::Kick,
        preserve_long_bytes: true,
        ..Default::default()
    };

    // True functionality: should output .abs
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda.abs".to_string(), "$0002".to_string())
    );

    // False functionality: should NOT output .abs
    settings.preserve_long_bytes = false;
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda".to_string(), "$0002".to_string())
    );

    // LDA $02 (ZeroPage) -> A5 02
    settings.preserve_long_bytes = true;
    assert_eq!(
        format_raw_instruction(&settings, 0xA5, &[0x02], 0x1000),
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
    use regenerator2000_core::disassembler::formatter::TextFragment;

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
        (".text".to_string(), "\"hello\"".to_string(), true)
    );

    // 2. Text Continuation -> .text only
    let lines_cont = formatter.format_text(&fragments, false, false);
    assert_eq!(lines_cont.len(), 1);
    assert_eq!(
        lines_cont[0],
        (".text".to_string(), "\"hello\"".to_string(), true)
    );
}

#[test]
fn test_screencode_encoding() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use regenerator2000_core::disassembler::formatter::TextFragment;

    // 1. Screencode Pre -> .encoding "screencode_upper"
    let pre_lines = formatter.format_screencode_pre();
    assert_eq!(pre_lines.len(), 1);
    assert_eq!(
        pre_lines[0],
        (".encoding".to_string(), "\"screencode_mixed\"".to_string())
    );

    // 2. Screencode Body -> .text (inverted case)
    // "Hello" should become "hELLO"
    let fragments = vec![TextFragment::Text("Hello".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        (".text".to_string(), "\"hELLO\"".to_string(), true)
    );
}

#[test]
fn test_screencode_case_swap() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use regenerator2000_core::disassembler::formatter::TextFragment;

    // "Hello World" -> "hELLO wORLD"
    let fragments = vec![TextFragment::Text("Hello World".to_string())];
    let lines = formatter.format_screencode(&fragments);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        (".text".to_string(), "\"hELLO wORLD\"".to_string(), true)
    );
}

/// Regression test: screen codes $00 and $1B-$1F map to ASCII $40-$5F
/// non-letter characters (@, [, \, ], ^, _). KickAssembler's
/// screencode_mixed encoding does NOT remap these back to their screen
/// code values, so they must be emitted as raw `.byte` values.
///
/// Specifically, screen code $1E maps to '^' (ASCII $5E). If emitted as
/// `.text "^"` under screencode_mixed the assembler produces $5E, not $1E.
#[test]
fn test_screencode_non_letter_chars_as_raw_bytes() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use regenerator2000_core::disassembler::formatter::TextFragment;

    // Screen codes $00, $1B, $1C, $1D, $1E, $1F are mapped in
    // handle_screencode_text to ASCII $40, $5B, $5C, $5D, $5E, $5F
    // (@, [, \, ], ^, _). Each must come back as its original screen code.
    let fragments = vec![TextFragment::Text("@[\\]^_".to_string())];
    let lines = formatter.format_screencode(&fragments);

    // Expect: .byte $00, $1b, $1c, $1d, $1e, $1f
    assert_eq!(lines.len(), 1, "expected single .byte line, got: {lines:?}");
    assert_eq!(lines[0].0, ".byte");
    assert_eq!(lines[0].1, "$00, $1b, $1c, $1d, $1e, $1f");

    // Letters interleaved with non-letters: "A^B" should yield:
    //   .text "a", .byte $1e, .text "b"
    let fragments2 = vec![TextFragment::Text("A^B".to_string())];
    let lines2 = formatter.format_screencode(&fragments2);
    assert_eq!(lines2.len(), 3, "expected 3 lines, got: {lines2:?}");
    assert_eq!(lines2[0], (".text".to_string(), "\"a\"".to_string(), true));
    assert_eq!(lines2[1], (".byte".to_string(), "$1e".to_string(), true));
    assert_eq!(lines2[2], (".text".to_string(), "\"b\"".to_string(), true));
}

#[test]
fn test_mixed_encoding() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use regenerator2000_core::disassembler::formatter::TextFragment;

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
        (".text".to_string(), "\"hello\"".to_string(), true)
    );
    assert_eq!(lines[2], (".byte".to_string(), "$ff".to_string(), true));
    assert_eq!(
        lines[3],
        (".text".to_string(), "\"world\"".to_string(), true)
    );
}

#[test]
fn test_quote_escaping() {
    let settings = DocumentSettings {
        assembler: Assembler::Kick,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    use regenerator2000_core::disassembler::formatter::TextFragment;

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

#[path = "common/symbolic_offset_formatter.rs"]
mod symbolic_offset_formatter;

#[test]
fn test_symbolic_offset_formatting_all_modes_kickasm() {
    symbolic_offset_formatter::assert_symbolic_offset_formatting_all_modes(
        Assembler::Kick,
        "lda.abs",
        "table_0100 - 1,x",
    );
}
