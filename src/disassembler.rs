use crate::cpu::{get_opcodes, AddressingMode, Opcode};
use crate::state::AddressType;
use crate::state::LabelType;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    pub label: Option<String>,
    pub opcode: Option<Opcode>,
}

pub struct Disassembler {
    pub opcodes: [Option<Opcode>; 256],
}

impl Disassembler {
    pub fn new() -> Self {
        Self {
            opcodes: get_opcodes(),
        }
    }

    pub fn disassemble(
        &self,
        data: &[u8],
        address_types: &[AddressType],
        labels: &std::collections::HashMap<u16, crate::state::Label>,
        origin: u16,
    ) -> Vec<DisassemblyLine> {
        let mut lines = Vec::new();
        let mut pc = 0;

        while pc < data.len() {
            let address = origin.wrapping_add(pc as u16);
            let label_name = labels.get(&address).map(|l| l.name.clone());

            // Check for Label (User or Auto) -> explicit line
            if let Some(name) = &label_name {
                let mut comment = String::new();
                if let Some(label) = labels.get(&address) {
                    if !label.refs.is_empty() {
                        let mut refs = label.refs.clone();
                        refs.sort_unstable();
                        refs.dedup(); // Optional: remove duplicates if any

                        let refs_str: Vec<String> =
                            refs.iter().take(5).map(|r| format!("{:04X}", r)).collect();
                        comment = format!("x-ref: {}", refs_str.join(", "));
                    }
                }

                lines.push(DisassemblyLine {
                    address,
                    bytes: Vec::new(),
                    mnemonic: format!("{}:", name),
                    operand: String::new(),
                    comment,
                    label: Some(name.clone()),
                    opcode: None,
                });
            }

            let current_type = address_types.get(pc).copied().unwrap_or(AddressType::Code);

            match current_type {
                AddressType::Code => {
                    let opcode_byte = data[pc];
                    let opcode_opt = &self.opcodes[opcode_byte as usize];

                    if let Some(opcode) = opcode_opt {
                        let mut bytes = vec![opcode_byte];

                        // Check if we have enough bytes
                        if pc + opcode.size as usize <= data.len() {
                            let mut collision = false;
                            for i in 1..opcode.size {
                                if let Some(t) = address_types.get(pc + i as usize) {
                                    if *t != AddressType::Code {
                                        collision = true;
                                        break;
                                    }
                                }
                            }

                            if !collision {
                                for i in 1..opcode.size {
                                    bytes.push(data[pc + i as usize]);
                                }

                                let operand_str =
                                    self.format_operand(opcode, &bytes[1..], address, labels);
                                pc += opcode.size as usize;

                                lines.push(DisassemblyLine {
                                    address,
                                    bytes,
                                    mnemonic: opcode.mnemonic.to_string(),
                                    operand: operand_str,
                                    comment: String::new(),
                                    label: label_name.clone(),
                                    opcode: Some(opcode.clone()),
                                });
                                continue;
                            }
                        }
                    }

                    // Fallthrough
                    lines.push(DisassemblyLine {
                        address,
                        bytes: vec![opcode_byte],
                        mnemonic: ".BYTE".to_string(),
                        operand: format!("${:02X}", opcode_byte),
                        comment: "Invalid or partial instruction".to_string(),
                        label: label_name.clone(),
                        opcode: None,
                    });
                    pc += 1;
                }
                AddressType::DataByte => {
                    let b = data[pc];
                    lines.push(DisassemblyLine {
                        address,
                        bytes: vec![b],
                        mnemonic: ".BYTE".to_string(),
                        operand: format!("${:02X}", b),
                        comment: String::new(),
                        label: label_name.clone(),
                        opcode: None,
                    });
                    pc += 1;
                }
                AddressType::DataWord => {
                    if pc + 2 <= data.len() {
                        let low = data[pc];
                        let high = data[pc + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![low, high],
                            mnemonic: ".WORD".to_string(),
                            operand: format!("${:04X}", val),
                            comment: String::new(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += 2;
                    } else {
                        // Not enough data
                        let b = data[pc];
                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![b],
                            mnemonic: ".BYTE".to_string(),
                            operand: format!("${:02X}", b),
                            comment: "Partial Word".to_string(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += 1;
                    }
                }
                AddressType::Address => {
                    if pc + 2 <= data.len() {
                        let low = data[pc];
                        let high = data[pc + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![low, high],
                            mnemonic: ".WORD".to_string(),
                            operand: if let Some(label) = labels.get(&val) {
                                label.name.clone()
                            } else {
                                format!("${:04X}", val)
                            },
                            comment: String::new(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += 2;
                    } else {
                        let b = data[pc];
                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![b],
                            mnemonic: ".BYTE".to_string(),
                            operand: format!("${:02X}", b),
                            comment: "Partial Address".to_string(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += 1;
                    }
                }
            }
        }

        lines
    }

    fn format_operand(
        &self,
        opcode: &Opcode,
        operands: &[u8],
        address: u16,
        labels: &HashMap<u16, crate::state::Label>,
    ) -> String {
        let get_label = |addr: u16, l_type: LabelType| -> Option<String> {
            labels.get(&addr).map(|l| {
                l.names
                    .get(&l_type)
                    .cloned()
                    .unwrap_or_else(|| l.name.clone())
            })
        };

        match opcode.mode {
            AddressingMode::Implied | AddressingMode::Accumulator => String::new(),
            AddressingMode::Immediate => format!("#${:02X}", operands[0]),
            AddressingMode::ZeroPage => {
                let addr = operands[0] as u16; // Zero page address
                if let Some(name) = get_label(addr, LabelType::ZeroPageAbsoluteAddress) {
                    name
                } else {
                    format!("${:02X}", addr)
                }
            }
            AddressingMode::ZeroPageX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},X", name)
                } else {
                    format!("${:02X},X", addr)
                }
            }
            AddressingMode::ZeroPageY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPageField) {
                    format!("{},Y", name)
                } else {
                    format!("${:02X},Y", addr)
                }
            }
            AddressingMode::Relative => {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                if let Some(name) = get_label(target, LabelType::Branch) {
                    name
                } else {
                    format!("${:04X}", target)
                }
            }
            AddressingMode::Absolute => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                let l_type = if opcode.mnemonic == "JSR" {
                    LabelType::Subroutine
                } else if opcode.mnemonic == "JMP" {
                    LabelType::Jump
                } else {
                    LabelType::AbsoluteAddress
                };

                if let Some(name) = get_label(addr, l_type) {
                    name
                } else {
                    format!("${:04X}", addr)
                }
            }
            AddressingMode::AbsoluteX => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},X", name)
                } else {
                    format!("${:04X},X", addr)
                }
            }
            AddressingMode::AbsoluteY => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Field) {
                    format!("{},Y", name)
                } else {
                    format!("${:04X},Y", addr)
                }
            }

            AddressingMode::Indirect => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                if let Some(name) = get_label(addr, LabelType::Pointer) {
                    format!("({})", name)
                } else {
                    format!("(${:04X})", addr)
                }
            }
            AddressingMode::IndirectX => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({},X)", name)
                } else {
                    format!("(${:02X},X)", addr)
                }
            }
            AddressingMode::IndirectY => {
                let addr = operands[0] as u16;
                if let Some(name) = get_label(addr, LabelType::ZeroPagePointer) {
                    format!("({}),Y", name)
                } else {
                    format!("(${:02X}),Y", addr)
                }
            }

            AddressingMode::Unknown => "???".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassembly_simple() {
        let disassembler = Disassembler::new();
        // A9 01 (LDA #$01), 8D 00 10 (STA $1000), 4C 00 10 (JMP $1000)
        let data = vec![0xA9, 0x01, 0x8D, 0x00, 0x10, 0x4C, 0x00, 0x10];
        let address_types = vec![AddressType::Code; data.len()];
        let labels = HashMap::new();
        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        assert_eq!(lines.len(), 3);

        assert_eq!(lines[0].address, 0x1000);
        assert_eq!(lines[0].mnemonic, "LDA");
        assert_eq!(lines[0].operand, "#$01");

        assert_eq!(lines[1].address, 0x1002);
        assert_eq!(lines[1].mnemonic, "STA");
        assert_eq!(lines[1].operand, "$1000");

        assert_eq!(lines[2].address, 0x1005);
        assert_eq!(lines[2].mnemonic, "JMP");
        assert_eq!(lines[2].operand, "$1000");
    }

    #[test]
    fn test_disassembly_with_data_types() {
        let disassembler = Disassembler::new();
        // A9 01 (LDA #$01), 02 (DataByte), 03 04 (DataWord)
        let data = vec![0xA9, 0x01, 0x02, 0x03, 0x04];
        let mut address_types = vec![AddressType::Code; data.len()];

        // Force byte at index 2
        address_types[2] = AddressType::DataByte;
        // Force word at index 3
        address_types[3] = AddressType::DataWord;
        address_types[4] = AddressType::DataWord;

        let labels = HashMap::new();
        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        assert_eq!(lines.len(), 3);

        // Line 0: Code
        assert_eq!(lines[0].address, 0x1000);
        assert_eq!(lines[0].mnemonic, "LDA");

        // Line 1: Byte
        assert_eq!(lines[1].address, 0x1002);
        assert_eq!(lines[1].mnemonic, ".BYTE");
        assert_eq!(lines[1].operand, "$02");

        // Line 2: Word
        assert_eq!(lines[2].address, 0x1003);
        assert_eq!(lines[2].mnemonic, ".WORD");
        assert_eq!(lines[2].operand, "$0403"); // Little Endian
    }

    #[test]
    fn test_disassembly_with_labels() {
        let disassembler = Disassembler::new();
        // 4C 03 10 (JMP $1003)
        // 00 (Byte) $1003
        let data = vec![0x4C, 0x03, 0x10, 0x00];
        let address_types = vec![AddressType::Code; data.len()];

        let mut labels = HashMap::new();
        labels.insert(
            0x1003,
            crate::state::Label {
                name: "MyLabel".to_string(),
                kind: crate::state::LabelKind::User,
                names: HashMap::new(),
                refs: Vec::new(),
            },
        );

        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        // Expected:
        // JMP MyLabel
        // MyLabel:
        // .BYTE $00 (actually JMP is 3 bytes, so next is at 1003) Wait...
        // 1000: JMP $1003
        // 1003: 00 -> treated as BRK by default if Code, or something else.
        // Let's assume default Code.
        // But we added a label at 0x1003.

        // Output lines:
        // 1. JMP MyLabel
        // 2. MyLabel:  (Label line)
        // 3. BRK / .BYTE depending on opcode (00 is BRK)

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].mnemonic, "JMP");
        assert_eq!(lines[0].operand, "MyLabel");

        assert_eq!(lines[1].mnemonic, "MyLabel:");
        assert_eq!(lines[1].bytes.len(), 0);

        assert_eq!(lines[2].mnemonic, "BRK");
    }

    #[test]
    fn test_disassembly_with_xrefs() {
        let disassembler = Disassembler::new();
        // Just JMP $1000
        let data = vec![0x4C, 0x00, 0x10];
        let address_types = vec![AddressType::Code; data.len()];

        let mut labels = HashMap::new();
        labels.insert(
            0x1000,
            crate::state::Label {
                name: "MyLabel".to_string(),
                kind: crate::state::LabelKind::User,
                names: HashMap::new(),
                refs: vec![0x2000, 0x3000, 0x4000],
            },
        );

        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        // Accessing the line with the label
        // Output lines:
        // 1. JMP MyLabel (Address 1000)
        // 2. MyLabel: (Address 1000)
        // 3. BRK / .BYTE (Address 1003)
        // Note: The disassembler is state machine.
        // It processed 4C 00 10. PC=3. Address=1003.

        let label_line = lines
            .iter()
            .find(|l| l.mnemonic == "MyLabel:")
            .expect("Label line not found");

        assert!(label_line.comment.contains("x-ref: 2000, 3000, 4000"));
    }

    #[test]
    fn test_context_aware_labels() {
        let disassembler = Disassembler::new();
        // Setup:
        // $1000: JMP $3000 (4C 00 30)
        // $1003: JSR $3000 (20 00 30)
        let data = vec![0x4C, 0x00, 0x30, 0x20, 0x00, 0x30];
        let address_types = vec![AddressType::Code; data.len()];

        // Define label at $3000 with multiple names
        let mut labels = HashMap::new();
        let mut names = HashMap::new();
        names.insert(LabelType::Jump, "j3000".to_string());
        names.insert(LabelType::Subroutine, "s3000".to_string());
        names.insert(LabelType::AbsoluteAddress, "a3000".to_string());

        labels.insert(
            0x3000,
            crate::state::Label {
                name: "a3000".to_string(), // Default
                kind: crate::state::LabelKind::Auto,
                names,
                refs: Vec::new(),
            },
        );

        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        assert_eq!(lines.len(), 2);

        // JMP $3000 -> Should use j3000
        assert_eq!(lines[0].mnemonic, "JMP");
        assert_eq!(lines[0].operand, "j3000");

        // JSR $3000 -> Should use s3000
        assert_eq!(lines[1].mnemonic, "JSR");
        assert_eq!(lines[1].operand, "s3000");
    }

    #[test]
    fn test_indirect_y_label_bug() {
        let disassembler = Disassembler::new();
        // 31 00: AND ($00), Y
        // 4C 00 00: JMP $0000
        let data = vec![0x31, 0x00, 0x4C, 0x00, 0x00];
        let address_types = vec![AddressType::Code; data.len()];

        // Manually construct labels as if Analyzer produced them
        let mut labels = HashMap::new();
        let mut names = HashMap::new();
        names.insert(LabelType::ZeroPagePointer, "p00".to_string());
        names.insert(LabelType::Jump, "e0000".to_string());

        labels.insert(
            0x0000,
            crate::state::Label {
                name: "e0000".to_string(), // Default name
                kind: crate::state::LabelKind::Auto,
                names,
                refs: Vec::new(),
            },
        );

        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        // Line 0: AND (p00), Y
        assert_eq!(lines[0].mnemonic, "AND");
        assert_eq!(lines[0].operand, "(p00),Y");
    }

    #[test]
    fn test_absolute_x_label_bug() {
        let disassembler = Disassembler::new();
        // 9D 4B 00: STA $004B, X (Absolute X)
        // 31 4B:    AND ($4B), Y (Indirect Y - valid ZP pointer usage)
        let data = vec![0x9D, 0x4B, 0x00, 0x31, 0x4B];
        let address_types = vec![AddressType::Code; data.len()];

        // Manually construct labels
        // Address $004B is used as:
        // 1. AbsoluteField (from STA $004B,X) -> f004B
        // 2. ZeroPagePointer (from AND ($4B),Y) -> p4B

        let mut labels = HashMap::new();
        let mut names = HashMap::new();
        names.insert(LabelType::Field, "f004B".to_string());
        names.insert(LabelType::ZeroPagePointer, "p4B".to_string());

        // ZeroPagePointer (5) > AbsoluteField (3), so default name is p4B
        labels.insert(
            0x004B,
            crate::state::Label {
                name: "p4B".to_string(),
                kind: crate::state::LabelKind::Auto,
                names,
                refs: Vec::new(),
            },
        );

        let lines = disassembler.disassemble(&data, &address_types, &labels, 0x1000);

        // Line 0: STA f004B, X
        assert_eq!(lines[0].mnemonic, "STA");
        // BUG: Currently falls back to default label "p4B" because it looks for Absolute, not AbsoluteField
        // Expected: "f004B,X"
        // Actual (bug): "p4B,X"
        assert_eq!(lines[0].operand, "f004B,X");
    }
}
