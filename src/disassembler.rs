use crate::cpu::{get_opcodes, Opcode};
use crate::state::{AddressType, Assembler, DocumentSettings, Label};
use std::collections::HashMap;

mod acme;
pub mod formatter;
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

    pub fn create_formatter(assembler: Assembler) -> Box<dyn Formatter> {
        match assembler {
            Assembler::Tass64 => Box::new(TassFormatter),
            Assembler::Acme => Box::new(AcmeFormatter),
        }
    }

    pub fn disassemble(
        &self,
        data: &[u8],
        address_types: &[AddressType],
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        settings: &DocumentSettings,
    ) -> Vec<DisassemblyLine> {
        let formatter = Self::create_formatter(settings.assembler);

        let mut lines = Vec::new();
        let mut pc = 0;

        while pc < data.len() {
            let address = origin.wrapping_add(pc as u16);
            let label_name = self.get_label_name(address, labels, formatter.as_ref());
            let comment = self.get_comment(address, labels, settings);

            let current_type = address_types.get(pc).copied().unwrap_or(AddressType::Code);

            let (bytes_consumed, line) = match current_type {
                AddressType::Code => self.handle_code(
                    pc,
                    data,
                    address_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    settings,
                    label_name,
                    comment,
                ),
                AddressType::DataByte => self.handle_data_byte(
                    pc,
                    data,
                    address_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                AddressType::DataWord => self.handle_data_word(
                    pc,
                    data,
                    address_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                AddressType::Address => self.handle_address(
                    pc,
                    data,
                    address_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
            };

            lines.push(line);
            pc += bytes_consumed;
        }

        lines
    }

    fn get_label_name(
        &self,
        address: u16,
        labels: &HashMap<u16, Vec<Label>>,
        formatter: &dyn Formatter,
    ) -> Option<String> {
        labels
            .get(&address)
            .and_then(|v| v.first())
            .map(|l| formatter.format_label(&l.name))
    }

    fn get_comment(
        &self,
        address: u16,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
    ) -> String {
        if let Some(label_vec) = labels.get(&address) {
            let mut all_refs: Vec<u16> = Vec::new();
            for l in label_vec {
                all_refs.extend(l.refs.iter().cloned());
            }
            if !all_refs.is_empty() && settings.max_xref_count > 0 {
                all_refs.sort_unstable();
                all_refs.dedup();

                let refs_str: Vec<String> = all_refs
                    .iter()
                    .take(settings.max_xref_count)
                    .map(|r| format!("${:04X}", r))
                    .collect();
                return format!("x-ref: {}", refs_str.join(", "));
            }
        }
        String::new()
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_code(
        &self,
        pc: usize,
        data: &[u8],
        address_types: &[AddressType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, DisassemblyLine) {
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

                    let target_context = match opcode.mode {
                        crate::cpu::AddressingMode::ZeroPage => {
                            Some(crate::state::LabelType::ZeroPageAbsoluteAddress)
                        }
                        crate::cpu::AddressingMode::ZeroPageX => {
                            Some(crate::state::LabelType::ZeroPageField)
                        }
                        crate::cpu::AddressingMode::ZeroPageY => {
                            Some(crate::state::LabelType::ZeroPageField)
                        }
                        crate::cpu::AddressingMode::Relative => {
                            Some(crate::state::LabelType::Branch)
                        }
                        crate::cpu::AddressingMode::Absolute => {
                            if opcode.mnemonic == "JSR" {
                                Some(crate::state::LabelType::Subroutine)
                            } else if opcode.mnemonic == "JMP" {
                                Some(crate::state::LabelType::Jump)
                            } else {
                                Some(crate::state::LabelType::AbsoluteAddress)
                            }
                        }
                        crate::cpu::AddressingMode::AbsoluteX => {
                            Some(crate::state::LabelType::Field)
                        }
                        crate::cpu::AddressingMode::AbsoluteY => {
                            Some(crate::state::LabelType::Field)
                        }
                        crate::cpu::AddressingMode::Indirect => {
                            Some(crate::state::LabelType::Pointer)
                        }
                        crate::cpu::AddressingMode::IndirectX => {
                            Some(crate::state::LabelType::ZeroPagePointer)
                        }
                        crate::cpu::AddressingMode::IndirectY => {
                            Some(crate::state::LabelType::ZeroPagePointer)
                        }
                        _ => None,
                    };

                    let (mnemonic, operand_str) = formatter.format_instruction(
                        opcode,
                        &bytes[1..],
                        address,
                        target_context,
                        labels,
                        settings,
                    );

                    return (
                        opcode.size as usize,
                        DisassemblyLine {
                            address,
                            bytes,
                            mnemonic,
                            operand: operand_str,
                            comment,
                            label: label_name,
                            opcode: Some(opcode.clone()),
                        },
                    );
                }
            }
        }

        // Fallthrough / Invalid instruction
        let mut line_comment = "Invalid or partial instruction".to_string();
        if !comment.is_empty() {
            line_comment = format!("{}; {}", comment, line_comment);
        }
        (
            1,
            DisassemblyLine {
                address,
                bytes: vec![opcode_byte],
                mnemonic: formatter.byte_directive().to_string(),
                operand: format!("${:02X}", opcode_byte),
                comment: line_comment,
                label: label_name,
                opcode: None,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_data_byte(
        &self,
        pc: usize,
        data: &[u8],
        address_types: &[AddressType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, DisassemblyLine) {
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

            // Stop if label exists (except for the first byte)
            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            bytes.push(b);
            operands.push(format!("${:02X}", b));
            count += 1;
        }

        (
            count,
            DisassemblyLine {
                address,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment,
                label: label_name,
                opcode: None,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_data_word(
        &self,
        pc: usize,
        data: &[u8],
        address_types: &[AddressType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, DisassemblyLine) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0; // Number of words

        while pc + (count * 2) + 1 < data.len() && count < 4 {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);

            if address_types.get(current_pc_start) != Some(&AddressType::DataWord)
                || address_types.get(current_pc_start + 1) != Some(&AddressType::DataWord)
            {
                break;
            }

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
            (
                count * 2,
                DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment,
                    label: label_name,
                    opcode: None,
                },
            )
        } else {
            // Fallback for partial word
            self.handle_partial_data(pc, data, address, formatter, label_name, comment, "Word")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_address(
        &self,
        pc: usize,
        data: &[u8],
        address_types: &[AddressType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, DisassemblyLine) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + (count * 2) + 1 < data.len() && count < 4 {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);

            if address_types.get(current_pc_start) != Some(&AddressType::Address)
                || address_types.get(current_pc_start + 1) != Some(&AddressType::Address)
            {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let low = data[current_pc_start];
            let high = data[current_pc_start + 1];
            let val = (high as u16) << 8 | (low as u16);

            bytes.push(low);
            bytes.push(high);

            let operand = if let Some(label_vec) = labels.get(&val) {
                label_vec
                    .first()
                    .map(|l| l.name.clone())
                    .unwrap_or(format!("${:04X}", val))
            } else {
                format!("${:04X}", val)
            };
            operands.push(operand);

            count += 1;
        }

        if count > 0 {
            (
                count * 2,
                DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment,
                    label: label_name,
                    opcode: None,
                },
            )
        } else {
            self.handle_partial_data(pc, data, address, formatter, label_name, comment, "Address")
        }
    }

    fn handle_partial_data(
        &self,
        pc: usize,
        data: &[u8],
        address: u16,
        formatter: &dyn Formatter,
        label_name: Option<String>,
        comment: String,
        type_name: &str,
    ) -> (usize, DisassemblyLine) {
        if pc < data.len() {
            let b = data[pc];
            let mut line_comment = format!("Partial {}", type_name);
            if !comment.is_empty() {
                line_comment = format!("{}; {}", comment, line_comment);
            }
            (
                1,
                DisassemblyLine {
                    address,
                    bytes: vec![b],
                    mnemonic: formatter.byte_directive().to_string(),
                    operand: format!("${:02X}", b),
                    comment: line_comment,
                    label: label_name,
                    opcode: None,
                },
            )
        } else {
            // Should not happen if loop condition is correct
            (0, DisassemblyLine {
                address,
                bytes: vec![],
                mnemonic: "???".to_string(),
                operand: "".to_string(),
                comment: "Error: Out of bounds".to_string(),
                label: None,
                opcode: None,
            })
        }
    }
}
