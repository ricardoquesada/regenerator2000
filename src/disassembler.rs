use crate::cpu::{get_opcodes, Opcode};
use crate::state::{AddressType, Assembler, DocumentSettings, Label};
use std::collections::HashMap;

mod acme;
mod formatter;
mod tass;

use acme::AcmeFormatter;
use formatter::Formatter;
use tass::TassFormatter;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    #[allow(dead_code)]
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
        labels: &HashMap<u16, Label>,
        origin: u16,
        settings: &DocumentSettings,
    ) -> Vec<DisassemblyLine> {
        let formatter: Box<dyn Formatter> = match settings.assembler {
            Assembler::Tass64 => Box::new(TassFormatter),
            Assembler::Acme => Box::new(AcmeFormatter),
        };

        let mut lines = Vec::new();
        let mut pc = 0;

        while pc < data.len() {
            let address = origin.wrapping_add(pc as u16);
            let label_name = labels.get(&address).map(|l| l.name.clone());

            let mut comment = String::new();
            if let Some(label) = labels.get(&address) {
                if !label.refs.is_empty() {
                    let mut refs = label.refs.clone();
                    refs.sort_unstable();
                    refs.dedup();

                    let refs_str: Vec<String> =
                        refs.iter().take(5).map(|r| format!("{:04X}", r)).collect();
                    comment = format!("x-ref: {}", refs_str.join(", "));
                }
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

                                let operand_str = formatter.format_operand(
                                    opcode,
                                    &bytes[1..],
                                    address,
                                    labels,
                                    settings,
                                );
                                pc += opcode.size as usize;

                                lines.push(DisassemblyLine {
                                    address,
                                    bytes,
                                    mnemonic: opcode.mnemonic.to_string(),
                                    operand: operand_str,
                                    comment: comment.clone(),
                                    label: label_name.clone(),
                                    opcode: Some(opcode.clone()),
                                });
                                continue;
                            }
                        }
                    }

                    // Fallthrough
                    let mut line_comment = "Invalid or partial instruction".to_string();
                    if !comment.is_empty() {
                        line_comment = format!("{}; {}", comment, line_comment);
                    }
                    lines.push(DisassemblyLine {
                        address,
                        bytes: vec![opcode_byte],
                        mnemonic: formatter.byte_directive().to_string(),
                        operand: format!("${:02X}", opcode_byte),
                        comment: line_comment,
                        label: label_name.clone(),
                        opcode: None,
                    });
                    pc += 1;
                }
                AddressType::DataByte => {
                    let mut bytes = Vec::new();
                    let mut operands = Vec::new();
                    let mut count = 0;

                    while pc + count < data.len() && count < 8 {
                        let current_pc = pc + count;
                        let current_address = origin.wrapping_add(current_pc as u16);

                        // Stop if type changes
                        if address_types.get(current_pc) != Some(&AddressType::DataByte) {
                            break;
                        }

                        // Stop if label exists (except for the first byte, which is handled by outer loop logic)
                        if count > 0 && labels.contains_key(&current_address) {
                            break;
                        }

                        let b = data[current_pc];
                        bytes.push(b);
                        operands.push(format!("${:02X}", b));
                        count += 1;
                    }

                    lines.push(DisassemblyLine {
                        address,
                        bytes: Vec::new(),
                        mnemonic: formatter.byte_directive().to_string(),
                        operand: operands.join(", "),
                        comment: comment.clone(),
                        label: label_name.clone(),
                        opcode: None,
                    });
                    pc += count;
                }
                AddressType::DataWord => {
                    let mut bytes = Vec::new();
                    let mut operands = Vec::new();
                    let mut count = 0; // Number of words

                    while pc + (count * 2) + 1 < data.len() && count < 4 {
                        let current_pc_start = pc + (count * 2);
                        let current_address = origin.wrapping_add(current_pc_start as u16);

                        // Stop if type changes for the first byte of word
                        if address_types.get(current_pc_start) != Some(&AddressType::DataWord) {
                            break;
                        }
                        // Stop if type changes for the second byte of word (should be consistent, but check)
                        if address_types.get(current_pc_start + 1) != Some(&AddressType::DataWord) {
                            break;
                        }

                        // Stop if label exists at word start (except for first one)
                        if count > 0 && labels.contains_key(&current_address) {
                            break;
                        }

                        let low = data[current_pc_start];
                        let high = data[current_pc_start + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        bytes.push(low);
                        bytes.push(high);
                        operands.push(format!("${:04X}", val));
                        count += 1;
                    }

                    if count > 0 {
                        lines.push(DisassemblyLine {
                            address,
                            bytes: Vec::new(),
                            mnemonic: formatter.word_directive().to_string(),
                            operand: operands.join(", "),
                            comment: comment.clone(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += count * 2;
                    } else {
                        // Fallback for partial word at end of data or mismatched types
                        if pc < data.len() {
                            let b = data[pc];
                            let mut line_comment = "Partial Word".to_string();
                            if !comment.is_empty() {
                                line_comment = format!("{}; {}", comment, line_comment);
                            }
                            lines.push(DisassemblyLine {
                                address,
                                bytes: Vec::new(),
                                mnemonic: formatter.byte_directive().to_string(),
                                operand: format!("${:02X}", b),
                                comment: line_comment,
                                label: label_name.clone(),
                                opcode: None,
                            });
                            pc += 1;
                        }
                    }
                }
                AddressType::Address => {
                    let mut bytes = Vec::new();
                    let mut operands = Vec::new();
                    let mut count = 0;

                    while pc + (count * 2) + 1 < data.len() && count < 4 {
                        let current_pc_start = pc + (count * 2);
                        let current_address = origin.wrapping_add(current_pc_start as u16);

                        // Stop if type changes for the first byte of word
                        if address_types.get(current_pc_start) != Some(&AddressType::Address) {
                            break;
                        }
                        // Stop if type changes for the second byte
                        if address_types.get(current_pc_start + 1) != Some(&AddressType::Address) {
                            break;
                        }

                        // Stop if label exists at word start (except for first one)
                        if count > 0 && labels.contains_key(&current_address) {
                            break;
                        }

                        let low = data[current_pc_start];
                        let high = data[current_pc_start + 1];
                        let val = (high as u16) << 8 | (low as u16);

                        bytes.push(low);
                        bytes.push(high);

                        let operand = if let Some(label) = labels.get(&val) {
                            label.name.clone()
                        } else {
                            format!("${:04X}", val)
                        };
                        operands.push(operand);

                        count += 1;
                    }

                    if count > 0 {
                        lines.push(DisassemblyLine {
                            address,
                            bytes: Vec::new(),
                            mnemonic: formatter.word_directive().to_string(),
                            operand: operands.join(", "),
                            comment: comment.clone(),
                            label: label_name.clone(),
                            opcode: None,
                        });
                        pc += count * 2;
                    } else {
                        // Fallback for partial word
                        let b = data[pc];
                        let mut line_comment = "Partial Address".to_string();
                        if !comment.is_empty() {
                            line_comment = format!("{}; {}", comment, line_comment);
                        }
                        lines.push(DisassemblyLine {
                            address,
                            bytes: Vec::new(),
                            mnemonic: formatter.byte_directive().to_string(),
                            operand: format!("${:02X}", b),
                            comment: line_comment,
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
}
