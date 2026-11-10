#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regenerator2000_core::disassembler::Disassembler;
use regenerator2000_core::state::{Assembler, DocumentSettings};
use symbolic_offset_formatter::format_raw_instruction;

#[test]
fn test_format_instructions() {
    let settings = DocumentSettings {
        assembler: Assembler::Ca65,
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
        assembler: Assembler::Ca65,
        ..Default::default()
    };
    let formatter = Disassembler::create_formatter(settings.assembler);
    assert_eq!(
        formatter.format_header_origin(regenerator2000_core::state::Addr(0x1000)),
        ".org $1000"
    );
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
        preserve_long_bytes: true,
        ..Default::default()
    };

    // LDA $0002 (Absolute) -> AD 02 00 ("lda a:$0002" because value <= $FF)
    assert_eq!(
        format_raw_instruction(&settings, 0xAD, &[0x02, 0x00], 0x1000),
        ("lda".to_string(), "a:$0002".to_string())
    );

    // False functionality: should NOT output a: prefix
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
fn test_format_screencode() {
    use regenerator2000_core::disassembler::formatter::TextFragment;
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
    use regenerator2000_core::disassembler::formatter::TextFragment;
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

    // Case 2: Backslash ($5C) is outside the safe text range ($20-$5A),
    // so it's emitted as a raw byte. "C:\DOS" -> "c:", .byte $5c, "dos"
    let fragments = vec![TextFragment::Text("C:\\DOS".to_string())];
    let lines = formatter.format_text(&fragments, true, true);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].1, "\"c:\"");
    assert_eq!(lines[1].1, "$5c");
    assert_eq!(lines[2].1, "\"dos\"");
}

#[test]
fn test_ca65_format_file_directives_and_header() {
    use regenerator2000_core::disassembler::formatter::{FileDirective, Formatter};
    use regenerator2000_core::disassembler::formatter_ca65::Ca65Formatter;

    let formatter = Ca65Formatter;
    let directives = formatter.format_file_directives();
    assert_eq!(
        directives,
        vec![FileDirective {
            directive: ".macpack cbm".to_string(),
            comment: Some("adds support for scrcode".to_string()),
        }]
    );

    let header_std = formatter.format_file_header("test", false);
    assert!(header_std.contains("; Assemble with:"));
    assert!(header_std.contains(";   cl65 -t c64 -C c64-asm.cfg test.asm -o test.prg"));
    assert!(
        !header_std.contains(".macpack cbm"),
        "Header comment string must not contain .macpack cbm"
    );

    let header_illegal = formatter.format_file_header("test", true);
    assert!(header_illegal.contains("; Assemble with:"));
    assert!(
        header_illegal.contains(";   cl65 -t c64 --cpu 6502X -C c64-asm.cfg test.asm -o test.prg")
    );
    assert!(
        !header_illegal.contains(".macpack cbm"),
        "Header comment string must not contain .macpack cbm"
    );
}

#[test]
fn test_default_format_file_directives_empty() {
    for assembler in [Assembler::Tass64, Assembler::Acme, Assembler::Kick] {
        let formatter = Disassembler::create_formatter(assembler);
        assert!(
            formatter.format_file_directives().is_empty(),
            "Expected empty file directives for {assembler:?}"
        );
    }
}

#[path = "common/symbolic_offset_formatter.rs"]
mod symbolic_offset_formatter;

#[test]
fn test_symbolic_offset_formatting_all_modes_ca65() {
    symbolic_offset_formatter::assert_symbolic_offset_formatting_all_modes(
        Assembler::Ca65,
        "lda",
        "a:table_0100 - 1,x",
    );
}
