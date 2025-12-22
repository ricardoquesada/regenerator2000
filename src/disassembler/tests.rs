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
