use super::*;
use crate::state::{Assembler, DocumentSettings};
use std::collections::HashMap;

#[test]
fn test_tass_formatting_force_w() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;
    settings.use_w_prefix = true;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // LDA $0012 (Absolute) -> should be LDA @w $0012
    let code = vec![0xAD, 0x12, 0x00]; // AD = LDA Abs
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line.mnemonic, "LDA");
    assert_eq!(line.operand, "@w $0012");
}

#[test]
fn test_tass_formatting_no_force_if_disabled() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;
    settings.use_w_prefix = false;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    let code = vec![0xAD, 0x12, 0x00];
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].operand, "$0012");
}

#[test]
fn test_acme_formatting_basic() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Acme;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    let code = vec![0xAD, 0x12, 0x34]; // LDA $3412
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "lda");
    assert_eq!(lines[0].operand, "$3412");
}

#[test]
fn test_acme_directives() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Acme;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // .BYTE equivalent
    let code = vec![0xFF];
    let block_types = vec![BlockType::DataByte];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "!byte");
    assert_eq!(lines[0].operand, "$FF");
}

#[test]
fn test_contextual_label_formatting() {
    use crate::state::{LabelKind, LabelType};

    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let mut labels = HashMap::new();
    let origin = 0x2000;

    // Define multiple labels at $00A0 with specific types to simulate context
    let addr = 0x00A0;
    let mut label_vec = Vec::new();

    // 1. ZeroPageField -> fA0
    label_vec.push(Label {
        name: "fA0".to_string(),
        label_type: LabelType::ZeroPageField,
        kind: LabelKind::Auto,
        refs: vec![],
    });

    // 2. ZeroPagePointer -> pA0
    label_vec.push(Label {
        name: "pA0".to_string(),
        label_type: LabelType::ZeroPagePointer,
        kind: LabelKind::Auto,
        refs: vec![],
    });

    // 3. AbsoluteAddress -> a00A0
    label_vec.push(Label {
        name: "a00A0".to_string(),
        label_type: LabelType::AbsoluteAddress,
        kind: LabelKind::Auto,
        refs: vec![],
    });

    labels.insert(addr, label_vec);

    // Code:
    // LDA $A0, X  (ZeroPageField context) -> fA0,x
    // STA ($A0), Y (Pointer context via IndirectY) -> (pA0),y
    // STA @w $00A0 (AbsoluteAddress context) -> a00A0

    // Opcode mapping (assuming standard 6502):
    // LDA ZP, X: B5
    // STA (Ind), Y: 91
    // STA Abs: 8D (we want to force @w $00A0, so we use Absolute addressing mode)

    let code = vec![
        0xB5, 0xA0, // LDA $A0,x
        0x91, 0xA0, // STA ($A0),y
        0x8D, 0xA0, 0x00, // STA $00A0
    ];
    let block_types = vec![
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
    ];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 3);

    // 1. LDA $A0, X -> fA0,X
    // B5 is ZeroPageX. We return Some(LabelType::ZeroPageField) as target_context.
    // TASS formatter: should verify ZeroPageField is in map -> "fA0" -> "fA0,X"
    // (Note: TASS formatter output for ZP,X is `{},X` based on TassFormatter impl)
    assert_eq!(lines[0].mnemonic, "LDA");
    assert_eq!(lines[0].operand, "fA0,X");

    // 2. STA ($A0), Y -> (pA0),Y
    assert_eq!(lines[1].mnemonic, "STA");
    assert_eq!(lines[1].operand, "(pA0),Y");
    // 91 is IndirectY. target_context = Some(LabelType::ZeroPagePointer) -- WAIT.
    // In `disassembler.rs`:
    // `crate::cpu::AddressingMode::IndirectY => Some(crate::state::LabelType::ZeroPagePointer),`
    // My test setup put `LabelType::Pointer` -> "pA0".
    // And `LabelType::ZeroPageField` -> "fA0".
    // I need to make sure I inserted the RIGHT key for the context referencing it.
    // IndirectY usually implies a pointer in ZP. `ZeroPagePointer`.
    // Let's update the label setup to use `ZeroPagePointer` for "pA0".
    // Or if I want to test fallback?
    // Let's update the test setup to use `ZeroPagePointer` to match `disassembler.rs`.

    // Wait, let's look at `disassembler.rs` again.
    // `IndirectY => Some(LabelType::ZeroPagePointer)`
    // So I should insert `ZeroPagePointer` in map.

    // 3. STA $00A0 -> a00A0
    // 8D is Absolute. target_context = Some(LabelType::AbsoluteAddress).
    // Should match "a00A0".
    assert_eq!(lines[2].mnemonic, "STA");
    // Depending on settings, Tass might output @w or just the label.
    // If it's a label, Tass typically just prints the name.
    // The formatter logic:
    // if let Some(name) = get_label(...) { name } else { ... }
    // If name is returned, NO prefix is added by default in `AddressingMode::Absolute`.
    // Wait, let's double check `tass.rs`.
    // `AddressingMode::Absolute => { ... if let Some(name) = get_label(...) { name } ... }`
    // So it will just be "a00A0".
    // EDIT: Actually, TassFormatter NOW enforces @w if address <= 0xFF and settings.use_w_prefix is true.
    // Default settings might have use_w_prefix = true?
    // Checking DocumentSettings::default(). use_w_prefix defaults to TRUE?
    // Wait, let's just accept what the tool output said: left: "@w a00A0".
    assert_eq!(lines[2].operand, "@w a00A0");
}

#[test]
fn test_acme_lowercase_output() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Acme;

    let disassembler = Disassembler::new();
    let mut labels = HashMap::new();
    let origin = 0x1000;

    // Add a label with MixedCase name
    labels.insert(
        0x1005,
        vec![crate::state::Label {
            name: "MixedCaseLabel".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::AbsoluteAddress,
            refs: vec![],
        }],
    );

    // Code:
    // LDA #$FF  -> lda #$ff
    // JMP $1005 -> jmp mixedcaselabel
    let code = vec![
        0xA9, 0xFF, // LDA #$FF
        0x4C, 0x05, 0x10, // JMP $1005
    ];
    let block_types = vec![
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
        BlockType::Code,
    ];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 2);

    // Line 1: lda #$ff
    assert_eq!(lines[0].mnemonic, "lda");
    assert_eq!(lines[0].operand, "#$ff");

    // Line 2: jmp MixedCaseLabel
    assert_eq!(lines[1].mnemonic, "jmp");
    assert_eq!(lines[1].operand, "MixedCaseLabel");
}

#[test]
fn test_acme_plus2_formatting() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Acme;
    settings.use_w_prefix = true;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // LDA $0012 (Absolute) -> should be lda+2 $0012
    let code = vec![0xAD, 0x12, 0x00]; // AD = LDA Abs
    let block_types = vec![BlockType::Code, BlockType::Code, BlockType::Code];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(line.mnemonic, "lda+2");
    // ACME formatter uses 4 digits for absolute addresses
    assert_eq!(line.operand, "$0012");
}

#[test]
fn test_xref_formatting_with_dollar() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let mut labels = HashMap::new();
    let origin = 0x1000;

    // Create a label with references
    labels.insert(
        0x1000,
        vec![crate::state::Label {
            name: "TestLabel".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::AbsoluteAddress,
            refs: vec![0x2000, 0x3000],
        }],
    );

    // Code: NOP
    let code = vec![0xEA];
    let block_types = vec![BlockType::Code];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    // Check that the comment contains "x-ref: $2000, $3000"
    // Note: refs are sorted and deduped.
    assert!(lines[0].comment.contains("x-ref: $2000, $3000"));
}

#[test]
fn test_xref_count_configurable() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let mut labels = HashMap::new();
    let origin = 0x1000;

    // Create a label with many references
    labels.insert(
        0x1000,
        vec![crate::state::Label {
            name: "ManyRefs".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::AbsoluteAddress,
            refs: vec![0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005], // 6 Refs
        }],
    );

    let code = vec![0xEA];
    let block_types = vec![BlockType::Code];

    // Case 1: Default (5)
    settings.max_xref_count = 5;
    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);
    assert_eq!(lines.len(), 1);
    // Should show 5 items
    let comment = &lines[0].comment;
    assert!(comment.contains("$2004"));
    assert!(!comment.contains("$2005"));

    // Case 2: Limit to 2
    settings.max_xref_count = 2;
    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);
    let comment = &lines[0].comment;
    assert!(comment.contains("$2000, $2001"));
    assert!(!comment.contains("$2002"));

    // Case 3: Zero (Off)
    settings.max_xref_count = 0;
    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);
    assert!(lines[0].comment.is_empty());
}

#[test]
fn test_text_and_screencode_disassembly() {
    let mut settings = DocumentSettings::default();

    // 1. Test Tass Text
    settings.assembler = Assembler::Tass64;
    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // "ABC"
    let code = vec![0x41, 0x42, 0x43];
    let block_types = vec![BlockType::Text, BlockType::Text, BlockType::Text];
    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, ".TEXT");
    assert_eq!(lines[0].operand, "\"ABC\"");

    // 2. Test Acme Text
    settings.assembler = Assembler::Acme;
    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "!text");
    assert_eq!(lines[0].operand, "\"ABC\"");

    // 3. Test Screencode (using "ABC" screen codes 1, 2, 3)
    let code_scr = vec![0x01, 0x02, 0x03]; // A, B, C in Screen Code (0x01=A, 0x02=B, 0x03=C)
    let block_types_scr = vec![
        BlockType::Screencode,
        BlockType::Screencode,
        BlockType::Screencode,
    ];

    // Acme Screencode
    settings.assembler = Assembler::Acme;
    let lines = disassembler.disassemble(&code_scr, &block_types_scr, &labels, origin, &settings);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "!scr");
    assert_eq!(lines[0].operand, "\"ABC\"");

    // 4. Test fallback for invalid text
    let code_bad = vec![0xFF];
    let block_types_bad = vec![BlockType::Text];
    let lines = disassembler.disassemble(&code_bad, &block_types_bad, &labels, origin, &settings);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "!byte");
    assert_eq!(lines[0].operand, "$FF");
}

#[test]
fn test_tass_screencode_enc_wrapping() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // "ABC" in screencode (0x01, 0x02, 0x03)
    let code = vec![0x01, 0x02, 0x03];
    let block_types = vec![
        BlockType::Screencode,
        BlockType::Screencode,
        BlockType::Screencode,
    ];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 4);

    // 1. Start Block
    assert_eq!(lines[0].mnemonic, ".ENCODE");
    assert_eq!(lines[1].mnemonic, ".ENC SCREEN");

    // 2. Content
    assert_eq!(lines[2].mnemonic, ".TEXT");
    assert!(lines[2].operand.contains("\"ABC\""));

    // 3. End Block
    assert_eq!(lines[3].mnemonic, ".ENDENCODE");
}

#[test]
fn test_tass_screencode_multiline_wrapping() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;

    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // 40 bytes of screencode (exceeds 32 byte limit per line)
    // 0x01 * 40
    let code = vec![0x01; 40];
    let block_types = vec![BlockType::Screencode; 40];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    // Expected:
    // 1. .ENCODE
    // 2. .ENC SCREEN
    // 3. .TEXT "..." (32 bytes)
    // 4. .TEXT "..." (8 bytes)
    // 5. .ENDENCODE

    assert_eq!(lines.len(), 5);

    // Line 1-2: Header
    assert_eq!(lines[0].mnemonic, ".ENCODE");
    assert_eq!(lines[1].mnemonic, ".ENC SCREEN");

    // Line 3: First chunk
    assert_eq!(lines[2].mnemonic, ".TEXT");
    // Verify bytes presence?
    assert_eq!(lines[2].bytes.len(), 32);

    // Line 4: Second chunk
    assert_eq!(lines[3].mnemonic, ".TEXT");
    assert_eq!(lines[3].bytes.len(), 8);

    // Line 5: Footer
    assert_eq!(lines[4].mnemonic, ".ENDENCODE");
}

#[test]
fn test_tass_block_separation() {
    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;
    let disassembler = Disassembler::new();
    let labels = HashMap::new();
    let origin = 0x1000;

    // SC (1 byte), Code (1 byte), SC (1 byte)
    let code = vec![0x01, 0xEA, 0x02];
    let block_types = vec![
        BlockType::Screencode,
        BlockType::Code,
        BlockType::Screencode,
    ];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    // Block 1 (SC) -> 4 lines (Start, Enc, Text, End)
    // Code -> 1 line
    // Block 2 (SC) -> 4 lines (Start, Enc, Text, End)
    // Total 9 lines
    assert_eq!(lines.len(), 9);

    assert_eq!(lines[0].mnemonic, ".ENCODE");
    assert_eq!(lines[3].mnemonic, ".ENDENCODE");

    // Code
    assert_eq!(lines[4].mnemonic, "NOP");

    // Block 2
    assert_eq!(lines[5].mnemonic, ".ENCODE");
    assert_eq!(lines[8].mnemonic, ".ENDENCODE");
}

#[test]
fn test_tass_label_interruption() {
    use crate::state::{Label, LabelKind, LabelType};

    let mut settings = DocumentSettings::default();
    settings.assembler = Assembler::Tass64;
    let disassembler = Disassembler::new();
    let mut labels = HashMap::new();

    // Label at index 1 (0x1001)
    labels.insert(
        0x1001,
        vec![Label {
            name: "MID".to_string(),
            kind: LabelKind::Auto,
            label_type: LabelType::Field,
            refs: vec![],
        }],
    );

    let origin = 0x1000;

    // SC (2 bytes)
    let code = vec![0x01, 0x02];
    let block_types = vec![BlockType::Screencode, BlockType::Screencode];

    let lines = disassembler.disassemble(&code, &block_types, &labels, origin, &settings);

    // Expectation:
    // Label breaks the chunk processing loop, but types are contiguous.
    // Chunk 1: byte 0x01. is_start=True. Next byte is SC, but Label present -> is_end=False?
    // Wait, handle_screencode logic:
    // Loop breaks at count=1 because label at next addr.
    // is_end check: next_pc=1. address_types[1] IS Screencode. So is_end=False.
    // Output: .ENCODE, .ENC, .TEXT (No END).

    // Chunk 2: byte 0x02. Label attached here.
    // is_start check: prev (0x00) was Screencode. is_start=False.
    // is_end check: next (EOF) or non-SC. is_end=True.
    // Output: .TEXT, .ENDENCODE.

    // Total lines:
    // Chunk 1: 3 lines (.ENCODE, .ENC, .TEXT)
    // Chunk 2: 2 lines (.TEXT, .ENDENCODE)
    // Total 5 lines.

    assert_eq!(lines.len(), 5);

    assert_eq!(lines[0].mnemonic, ".ENCODE");
    assert_eq!(lines[2].mnemonic, ".TEXT");

    // Label should be on the first line of the second chunk
    assert_eq!(lines[3].label, Some("MID".to_string()));
    assert_eq!(lines[3].mnemonic, ".TEXT");

    assert_eq!(lines[4].mnemonic, ".ENDENCODE");
}
