use crate::cpu::{get_opcodes, AddressingMode, Opcode};
use crate::state::AddressType;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    pub opcode: Option<Opcode>,
}

pub struct Disassembler {
    opcodes: [Option<Opcode>; 256],
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
        origin: u16,
    ) -> Vec<DisassemblyLine> {
        let mut lines = Vec::new();
        let mut pc = 0;

        while pc < data.len() {
            let address = origin.wrapping_add(pc as u16);
            let current_type = address_types.get(pc).copied().unwrap_or(AddressType::Code);

            match current_type {
                AddressType::Code => {
                    let opcode_byte = data[pc];
                    let opcode_opt = &self.opcodes[opcode_byte as usize];

                    if let Some(opcode) = opcode_opt {
                        let mut bytes = vec![opcode_byte];
                        let mut operand_str = String::new();

                        // Check if we have enough bytes
                        if pc + opcode.size as usize <= data.len() {
                            // Check if any of the subsequent bytes are NOT Code
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

                                operand_str = self.format_operand(opcode, &bytes[1..], address);
                                pc += opcode.size as usize;

                                lines.push(DisassemblyLine {
                                    address,
                                    bytes,
                                    mnemonic: opcode.mnemonic.to_string(),
                                    operand: operand_str,
                                    comment: String::new(),
                                    opcode: Some(opcode.clone()),
                                });
                                continue;
                            }
                        }
                    }

                    // Fallthrough to fallback byte handling if opcode is invalid or data collision
                    lines.push(DisassemblyLine {
                        address,
                        bytes: vec![opcode_byte],
                        mnemonic: ".BYTE".to_string(),
                        operand: format!("${:02X}", opcode_byte),
                        comment: "Invalid or partial instruction".to_string(),
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
                        opcode: None,
                    });
                    pc += 1;
                }
                AddressType::DataWord => {
                    if pc + 2 <= data.len() {
                        let low = data[pc];
                        let high = data[pc + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        // Check if next byte is also DataWord (it should be if we marked it consistently, but let's be safe)
                        // Actually, we process 2 bytes, so we skip the next one.
                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![low, high],
                            mnemonic: ".WORD".to_string(),
                            operand: format!("${:04X}", val),
                            comment: String::new(),
                            opcode: None,
                        });
                        pc += 2;
                    } else {
                        // Not enough data, just byte
                        let b = data[pc];
                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![b],
                            mnemonic: ".BYTE".to_string(),
                            operand: format!("${:02X}", b),
                            comment: "Partial Word".to_string(),
                            opcode: None,
                        });
                        pc += 1;
                    }
                }
                AddressType::DataPtr => {
                    if pc + 2 <= data.len() {
                        let low = data[pc];
                        let high = data[pc + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        lines.push(DisassemblyLine {
                            address,
                            bytes: vec![low, high],
                            mnemonic: ".WORD".to_string(),
                            operand: format!("${:04X}", val), // Could be formatted as a label later
                            comment: "Pointer".to_string(),
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
                            comment: "Partial Ptr".to_string(),
                            opcode: None,
                        });
                        pc += 1;
                    }
                }
            }
        }

        lines
    }

    fn format_operand(&self, opcode: &Opcode, operands: &[u8], address: u16) -> String {
        match opcode.mode {
            AddressingMode::Implied | AddressingMode::Accumulator => String::new(),
            AddressingMode::Immediate => format!("#${:02X}", operands[0]),
            AddressingMode::ZeroPage => format!("${:02X}", operands[0]),
            AddressingMode::ZeroPageX => format!("${:02X},X", operands[0]),
            AddressingMode::ZeroPageY => format!("${:02X},Y", operands[0]),
            AddressingMode::Relative => {
                let offset = operands[0] as i8;
                let target = address.wrapping_add(2).wrapping_add(offset as u16);
                format!("${:04X}", target)
            }
            AddressingMode::Absolute => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                format!("${:04X}", addr)
            }
            AddressingMode::AbsoluteX => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                format!("${:04X},X", addr)
            }
            AddressingMode::AbsoluteY => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                format!("${:04X},Y", addr)
            }
            AddressingMode::Indirect => {
                let addr = (operands[1] as u16) << 8 | (operands[0] as u16);
                format!("(${:04X})", addr)
            }
            AddressingMode::IndirectX => format!("(${:02X},X)", operands[0]),
            AddressingMode::IndirectY => format!("(${:02X}),Y", operands[0]),
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
        let lines = disassembler.disassemble(&data, &address_types, 0x1000);

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

        let lines = disassembler.disassemble(&data, &address_types, 0x1000);

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
}
