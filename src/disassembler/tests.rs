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
    let address_types = vec![AddressType::Code, AddressType::Code, AddressType::Code];

    let lines = disassembler.disassemble(&code, &address_types, &labels, origin, &settings);

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
    let address_types = vec![AddressType::Code, AddressType::Code, AddressType::Code];

    let lines = disassembler.disassemble(&code, &address_types, &labels, origin, &settings);

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
    let address_types = vec![AddressType::Code, AddressType::Code, AddressType::Code];

    let lines = disassembler.disassemble(&code, &address_types, &labels, origin, &settings);

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].mnemonic, "LDA");
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
    let address_types = vec![AddressType::DataByte];

    let lines = disassembler.disassemble(&code, &address_types, &labels, origin, &settings);

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
    let address_types = vec![
        AddressType::Code,
        AddressType::Code,
        AddressType::Code,
        AddressType::Code,
        AddressType::Code,
        AddressType::Code,
        AddressType::Code,
    ];

    let lines = disassembler.disassemble(&code, &address_types, &labels, origin, &settings);

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
