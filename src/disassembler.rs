use crate::cpu::{get_opcodes, Opcode};
use crate::state::{Assembler, BlockType, DocumentSettings, Label};
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
    pub show_bytes: bool,
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
        block_types: &[BlockType],
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

            let current_type = block_types.get(pc).copied().unwrap_or(BlockType::Code);

            let (bytes_consumed, new_lines) = match current_type {
                BlockType::Code => self.handle_code(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    settings,
                    label_name,
                    comment,
                ),
                BlockType::DataByte => self.handle_data_byte(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                BlockType::DataWord => self.handle_data_word(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                BlockType::Address => self.handle_address(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                BlockType::Text => self.handle_text(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
                BlockType::Screencode => self.handle_screencode(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    label_name,
                    comment,
                ),
            };

            lines.extend(new_lines);
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
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        let opcode_byte = data[pc];
        let opcode_opt = &self.opcodes[opcode_byte as usize];

        if let Some(opcode) = opcode_opt {
            let mut bytes = vec![opcode_byte];

            // Check if we have enough bytes
            if pc + opcode.size as usize <= data.len() {
                let mut collision = false;
                for i in 1..opcode.size {
                    if let Some(t) = block_types.get(pc + i as usize) {
                        if *t != BlockType::Code {
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
                        vec![DisassemblyLine {
                            address,
                            bytes,
                            mnemonic,
                            operand: operand_str,
                            comment,
                            label: label_name,
                            opcode: Some(opcode.clone()),
                            show_bytes: true,
                        }],
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
            vec![DisassemblyLine {
                address,
                bytes: vec![opcode_byte],
                mnemonic: formatter.byte_directive().to_string(),
                operand: format!("${:02X}", opcode_byte),
                comment: line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
            }],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_data_byte(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + count < data.len() && count < 8 {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            // Stop if type changes
            if block_types.get(current_pc) != Some(&BlockType::DataByte) {
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
            vec![DisassemblyLine {
                address,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
            }],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_data_word(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0; // Number of words

        while pc + (count * 2) + 1 < data.len() && count < 4 {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);

            if block_types.get(current_pc_start) != Some(&BlockType::DataWord)
                || block_types.get(current_pc_start + 1) != Some(&BlockType::DataWord)
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
                vec![DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                }],
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
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + (count * 2) + 1 < data.len() && count < 4 {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);

            if block_types.get(current_pc_start) != Some(&BlockType::Address)
                || block_types.get(current_pc_start + 1) != Some(&BlockType::Address)
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
                vec![DisassemblyLine {
                    address,
                    bytes,
                    mnemonic: formatter.word_directive().to_string(),
                    operand: operands.join(", "),
                    comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                }],
            )
        } else {
            self.handle_partial_data(pc, data, address, formatter, label_name, comment, "Address")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_text(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < 32 {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::Text) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            // Check if "printable" ASCII-ish from 0x20 to 0x7E
            if (0x20..=0x7E).contains(&b) {
                let c = b as char;
                current_literal.push(c);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
            }

            count += 1;
        }

        if !current_literal.is_empty() {
            fragments.push(TextFragment::Text(current_literal));
        }

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::Text);
        let next_pc = pc + count;
        let is_end = next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::Text);

        if count > 0 {
            // Check for assembler type based on formatter directives
            let formatted_lines = formatter.format_text(&fragments, is_start, is_end);
            let mut disassembly_lines = Vec::new();

            // We need to attach bytes to lines.
            // Since we merged everything into one line (usually), we attach ALL bytes to that line ?
            // Or if format_text returning multiple lines (header/footer), we attach to the main one.
            // The formatter returns Vec<(mnemonic, operand, has_bytes)>.

            // We need to collect all bytes consumed
            let mut all_bytes = Vec::new();
            for i in 0..count {
                all_bytes.push(data[pc + i]);
            }

            for (i, (mnemonic, operand, has_bytes)) in formatted_lines.iter().enumerate() {
                let line_bytes = if *has_bytes {
                    all_bytes.clone()
                } else {
                    Vec::new()
                };
                let line_label = if i == 0 { label_name.clone() } else { None };
                let line_comment = if i == 0 {
                    comment.clone()
                } else {
                    String::new()
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false, // Text lines should not show bytes
                });
            }

            (count, disassembly_lines)
        } else {
            // Fallback to byte if no valid chunk (should not happen if block_type is text and len > 0)
            self.handle_partial_data(pc, data, address, formatter, label_name, comment, "Text")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_screencode(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &HashMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        comment: String,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut text_content = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < 32 {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::Screencode) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            // Map Screen Code to ASCII
            // 0-31 (@..left arrow) -> 64..95
            // 32-63 (space..?) -> 32..63
            let ascii = if b < 32 {
                b + 64
            } else if b < 64 {
                b
            } else if b < 96 {
                b + 32
            } else {
                // Extended/Reverse codes
                // Just pass through as is, we'll filter printability later
                b
            };

            if !(0x20..=0x7E).contains(&ascii) {
                // For ScreenCode blocks, we allow all bytes now, as they might be mapped to non-standard chars
                // or just be raw bytes we want to output as .BYTE in the block.
                // We just won't add them to the text content string if they aren't printable.
            }

            bytes.push(b);
            let c = ascii as char;
            text_content.push(c);
            count += 1;
        }

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::Screencode);
        let next_pc = pc + count;
        let is_end =
            next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::Screencode);

        if count > 0 {
            let mut all_formatted_parts = Vec::new();

            if is_start {
                let pre_lines = formatter.format_screencode_pre();
                for (m, o) in pre_lines {
                    all_formatted_parts.push((m, o, false));
                }
            }

            let body_lines = formatter.format_screencode(&bytes, &text_content);
            all_formatted_parts.extend(body_lines);

            if is_end {
                let post_lines = formatter.format_screencode_post();
                for (m, o) in post_lines {
                    all_formatted_parts.push((m, o, false));
                }
            }

            let mut disassembly_lines = Vec::new();

            for (i, (mnemonic, operand, has_bytes)) in all_formatted_parts.iter().enumerate() {
                // Attach bytes only to marked lines by the formatter (usually the body lines).
                // Attach label and comment only to the very first line of the entire block.
                let line_bytes = if *has_bytes {
                    bytes.clone()
                } else {
                    Vec::new()
                };
                let line_label = if i == 0 { label_name.clone() } else { None };
                let line_comment = if i == 0 {
                    comment.clone()
                } else {
                    String::new()
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false, // Hide bytes for screencode blocks logic
                });
            }

            (count, disassembly_lines)
        } else {
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                comment,
                "Screencode",
            )
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
    ) -> (usize, Vec<DisassemblyLine>) {
        if pc < data.len() {
            let b = data[pc];
            let mut line_comment = format!("Partial {}", type_name);
            if !comment.is_empty() {
                line_comment = format!("{}; {}", comment, line_comment);
            }
            (
                1,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![b],
                    mnemonic: formatter.byte_directive().to_string(),
                    operand: format!("${:02X}", b),
                    comment: line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                }],
            )
        } else {
            // Should not happen if loop condition is correct
            (
                0,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![],
                    mnemonic: "???".to_string(),
                    operand: "".to_string(),
                    comment: "Error: Out of bounds".to_string(),
                    label: None,
                    opcode: None,
                    show_bytes: true,
                }],
            )
        }
    }
}
