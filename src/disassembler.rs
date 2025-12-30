use crate::cpu::{get_opcodes, Opcode};
use crate::state::{Assembler, BlockType, DocumentSettings, Label};
use std::collections::BTreeMap;

mod acme;
pub mod formatter;
mod tass;

use acme::AcmeFormatter;
use formatter::Formatter;
use tass::TassFormatter;

#[cfg(test)]
mod line_comment_tests;
#[cfg(test)]
mod system_comments_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod user_comments_tests;

#[derive(Debug, Clone)]
pub struct DisassemblyLine {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub mnemonic: String,
    pub operand: String,
    pub comment: String,
    pub line_comment: Option<String>,
    #[allow(dead_code)]
    pub label: Option<String>,
    pub opcode: Option<Opcode>,
    pub show_bytes: bool,
    pub target_address: Option<u16>,
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

    #[allow(clippy::too_many_arguments)]
    pub fn disassemble(
        &self,
        data: &[u8],
        block_types: &[BlockType],
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        settings: &DocumentSettings,
        system_comments: &BTreeMap<u16, String>,
        user_side_comments: &BTreeMap<u16, String>,
        user_line_comments: &BTreeMap<u16, String>,
    ) -> Vec<DisassemblyLine> {
        let formatter = Self::create_formatter(settings.assembler);

        let mut lines = Vec::new();
        let mut pc = 0;

        while pc < data.len() {
            let address = origin.wrapping_add(pc as u16);

            let label_name = self.get_label_name(address, labels, formatter.as_ref());
            let side_comment = self.get_side_comment(
                address,
                labels,
                settings,
                system_comments,
                user_side_comments,
            );
            let line_comment = user_line_comments.get(&address).cloned();

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
                    side_comment,
                    line_comment,
                    system_comments,
                    user_side_comments,
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
                    side_comment,
                    line_comment,
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
                    side_comment,
                    line_comment,
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
                    side_comment,
                    line_comment,
                    system_comments,
                    user_side_comments,
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
                    side_comment,
                    line_comment,
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
                    side_comment,
                    line_comment,
                ),
                BlockType::Undefined => self.handle_undefined_byte(
                    pc,
                    data,
                    address,
                    formatter.as_ref(),
                    label_name,
                    side_comment,
                    line_comment,
                ),
            };

            lines.extend(new_lines);
            pc += bytes_consumed;
        }

        lines
    }

    fn get_arrow_target_address(&self, opcode: &Opcode, bytes: &[u8], address: u16) -> Option<u16> {
        use crate::cpu::AddressingMode;

        // User request:
        // - JSR and JMP (absolute) SHOULD generate arrows
        // - Branches SHOULD generate arrows
        // - JMP Indirect (JMP (addr)) should NOT
        // - BRK, RTI, RTS should NOT

        if opcode.mnemonic == "JSR" || opcode.mnemonic == "JMP" {
            match opcode.mode {
                AddressingMode::Absolute => {
                    if bytes.len() >= 3 {
                        Some((bytes[2] as u16) << 8 | (bytes[1] as u16))
                    } else {
                        None
                    }
                }
                // JMP Indirect ($6C) uses AddressingMode::Indirect -> Should NOT generate arrow
                _ => None,
            }
        } else if opcode.mnemonic == "BRK" || opcode.mnemonic == "RTI" || opcode.mnemonic == "RTS" {
            None
        } else {
            // Check for branches (Relative mode)
            match opcode.mode {
                AddressingMode::Relative => {
                    if bytes.len() >= 2 {
                        let offset = bytes[1] as i8;
                        Some(address.wrapping_add(2).wrapping_add(offset as u16))
                    } else {
                        None
                    }
                }
                // Other instructions (like data ops) generally don't have control flow "arrows" pointing to memory ops
                // Unless we wanted arrows for generic memory access, but request specifically mentioned flow control.
                _ => None,
            }
        }
    }

    /// Returns the address referenced by the instruction, if any.
    /// This is used for looking up comments and X-Refs.
    /// Unlike get_flow_target_address, this returns a value for memory access instructions like STA, LDA, etc.
    fn get_referenced_address(&self, opcode: &Opcode, bytes: &[u8], address: u16) -> Option<u16> {
        use crate::cpu::AddressingMode;

        match opcode.mode {
            AddressingMode::Absolute | AddressingMode::AbsoluteX | AddressingMode::AbsoluteY => {
                if bytes.len() >= 3 {
                    Some((bytes[2] as u16) << 8 | (bytes[1] as u16))
                } else {
                    None
                }
            }
            AddressingMode::ZeroPage | AddressingMode::ZeroPageX | AddressingMode::ZeroPageY => {
                if bytes.len() >= 2 {
                    Some(bytes[1] as u16)
                } else {
                    None
                }
            }
            AddressingMode::Relative => {
                if bytes.len() >= 2 {
                    let offset = bytes[1] as i8;
                    // Branch target
                    Some(address.wrapping_add(2).wrapping_add(offset as u16))
                } else {
                    None
                }
            }
            AddressingMode::Indirect => {
                if bytes.len() >= 3 {
                    Some((bytes[2] as u16) << 8 | (bytes[1] as u16))
                } else {
                    None
                }
            }
            // For IndirectX/Y, we could argue it references the Zero Page address given.
            AddressingMode::IndirectX | AddressingMode::IndirectY => {
                if bytes.len() >= 2 {
                    Some(bytes[1] as u16)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_label_name(
        &self,
        address: u16,
        labels: &BTreeMap<u16, Vec<Label>>,
        formatter: &dyn Formatter,
    ) -> Option<String> {
        labels
            .get(&address)
            .and_then(|v| v.first())
            .map(|l| formatter.format_label(&l.name))
    }

    fn get_side_comment(
        &self,
        address: u16,
        labels: &BTreeMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        system_comments: &BTreeMap<u16, String>,
        user_side_comments: &BTreeMap<u16, String>,
    ) -> String {
        let mut comment_parts = Vec::new();

        if let Some(user_comment) = user_side_comments.get(&address) {
            comment_parts.push(user_comment.clone());
        } else if let Some(sys_comment) = system_comments.get(&address) {
            comment_parts.push(sys_comment.clone());
        }

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
                comment_parts.push(format!("x-ref: {}", refs_str.join(", ")));
            }
        }

        comment_parts.join("; ")
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_code(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &BTreeMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        label_name: Option<String>,
        mut side_comment: String,
        line_comment: Option<String>,
        system_comments: &BTreeMap<u16, String>,
        user_side_comments: &BTreeMap<u16, String>,
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

                    // Append referenced address comment if any
                    // Use get_referenced_address for comments, NOT get_arrow_target_address
                    if let Some(target_addr) = self.get_referenced_address(opcode, &bytes, address)
                    {
                        let target_comment = if let Some(c) = user_side_comments.get(&target_addr) {
                            Some(c)
                        } else {
                            system_comments.get(&target_addr)
                        };

                        if let Some(target_comment) = target_comment {
                            if !side_comment.is_empty() {
                                side_comment.push_str("; ");
                            }
                            side_comment.push_str(target_comment);
                        }
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

                    let target_address = self.get_arrow_target_address(opcode, &bytes, address);

                    return (
                        opcode.size as usize,
                        vec![DisassemblyLine {
                            address,
                            bytes,
                            mnemonic,
                            operand: operand_str,
                            comment: side_comment,
                            line_comment,
                            label: label_name,
                            opcode: Some(opcode.clone()),
                            show_bytes: true,
                            target_address,
                        }],
                    );
                }
            }
        }

        // Fallthrough / Invalid instruction
        let mut side_comment_final = "Invalid or partial instruction".to_string();
        if !side_comment.is_empty() {
            side_comment_final = format!("{}; {}", side_comment, side_comment_final);
        }
        (
            1,
            vec![DisassemblyLine {
                address,
                bytes: vec![opcode_byte],
                mnemonic: formatter.byte_directive().to_string(),
                operand: format!("${:02X}", opcode_byte),
                comment: side_comment_final,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
                target_address: None,
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
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
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
                comment: side_comment,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
                target_address: None,
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
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
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
                    comment: side_comment,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                }],
            )
        } else {
            // Fallback for partial word
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                "Word",
            )
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
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        mut side_comment: String,
        line_comment: Option<String>,
        system_comments: &BTreeMap<u16, String>,
        user_side_comments: &BTreeMap<u16, String>,
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

            // START: Append comment for the address value
            let target_comment = if let Some(c) = user_side_comments.get(&val) {
                Some(c)
            } else {
                system_comments.get(&val)
            };

            if let Some(target_comment) = target_comment {
                if !side_comment.is_empty() {
                    side_comment.push_str("; ");
                }
                side_comment.push_str(target_comment);
            }
            // END: Append comment

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
                    comment: side_comment,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                }],
            )
        } else {
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                "Address",
            )
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
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
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
                let (line_side_comment, line_line_comment) = if i == 0 {
                    (side_comment.clone(), line_comment.clone())
                } else {
                    (String::new(), None)
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_side_comment,
                    line_comment: line_line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                });
            }

            (count, disassembly_lines)
        } else {
            // Fallback to byte if no valid chunk (should not happen if block_type is text and len > 0)
            self.handle_partial_data(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                "Text",
            )
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
        labels: &BTreeMap<u16, Vec<Label>>,
        origin: u16,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
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

            // User Request: bytes >= $5f should be treated as bytes. Not as screencodes.
            if b >= 0x5f {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
                count += 1;
                continue;
            }

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
                // Extended/Reverse codes -> raw byte
                b
            };

            if (0x20..=0x7E).contains(&ascii) {
                let c = ascii as char;
                current_literal.push(c);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b)); // Use ORIGINAL byte
            }

            count += 1;
        }

        if !current_literal.is_empty() {
            fragments.push(TextFragment::Text(current_literal));
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

            let body_lines = formatter.format_screencode(&fragments);
            all_formatted_parts.extend(body_lines);

            if is_end {
                let post_lines = formatter.format_screencode_post();
                for (m, o) in post_lines {
                    all_formatted_parts.push((m, o, false));
                }
            }

            let mut disassembly_lines = Vec::new();

            // Collect all consumed bytes for line association
            let mut all_bytes = Vec::new();
            for i in 0..count {
                all_bytes.push(data[pc + i]);
            }

            for (i, (mnemonic, operand, has_bytes)) in all_formatted_parts.iter().enumerate() {
                // Attach bytes only to marked lines by the formatter (usually the body lines).
                // Attach label and comment only to the very first line of the entire block.
                let line_bytes = if *has_bytes {
                    all_bytes.clone()
                } else {
                    Vec::new()
                };
                let line_label = if i == 0 { label_name.clone() } else { None };
                let (line_side_comment, line_line_comment) = if i == 0 {
                    (side_comment.clone(), line_comment.clone())
                } else {
                    (String::new(), None)
                };

                disassembly_lines.push(DisassemblyLine {
                    address,
                    bytes: line_bytes,
                    mnemonic: mnemonic.clone(),
                    operand: operand.clone(),
                    comment: line_side_comment,
                    line_comment: line_line_comment,
                    label: line_label,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
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
                side_comment,
                line_comment,
                "Screencode",
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_partial_data(
        &self,
        pc: usize,
        data: &[u8],
        address: u16,
        formatter: &dyn Formatter,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
        type_name: &str,
    ) -> (usize, Vec<DisassemblyLine>) {
        if pc < data.len() {
            let b = data[pc];
            let mut side_comment_final = format!("Partial {}", type_name);
            if !side_comment.is_empty() {
                side_comment_final = format!("{}; {}", side_comment, side_comment_final);
            }
            (
                1,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![b],
                    mnemonic: formatter.byte_directive().to_string(),
                    operand: format!("${:02X}", b),
                    comment: side_comment_final,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
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
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                }],
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_undefined_byte(
        &self,
        pc: usize,
        data: &[u8],
        address: u16,
        formatter: &dyn Formatter,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
    ) -> (usize, Vec<DisassemblyLine>) {
        let b = data[pc];
        (
            1,
            vec![DisassemblyLine {
                address,
                bytes: vec![b],
                mnemonic: formatter.byte_directive().to_string(),
                operand: formatter.format_byte(b),
                comment: side_comment,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
                target_address: None,
            }],
        )
    }
}
