use crate::cpu::{Opcode, get_opcodes};
use crate::state::{Assembler, BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

mod acme;
mod ca65;
pub mod formatter;
mod kickasm;
mod tass;

use acme::AcmeFormatter;
use ca65::Ca65Formatter;
use formatter::Formatter;
use kickasm::KickAsmFormatter;
use tass::TassFormatter;

#[cfg(test)]
mod brk_tests;
#[cfg(test)]
mod ca65_tests;
#[cfg(test)]
mod collapsed_tests;
#[cfg(test)]
mod illegal_opcodes_tests;
#[cfg(test)]
mod kickasm_tests;
#[cfg(test)]
mod label_placement_tests;
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
    pub comment_address: Option<u16>,
    pub is_collapsed: bool,
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
            Assembler::Ca65 => Box::new(Ca65Formatter),
            Assembler::Kick => Box::new(KickAsmFormatter),
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

        immediate_value_formats: &BTreeMap<u16, crate::state::ImmediateFormat>,
        cross_refs: &BTreeMap<u16, Vec<u16>>,
        collapsed_blocks: &[(usize, usize)],
        splitters: &BTreeSet<u16>,
    ) -> Vec<DisassemblyLine> {
        let formatter = Self::create_formatter(settings.assembler);

        let mut lines = Vec::new();
        let mut pc = 0;

        // Convert collapsed blocks to a map for O(1) lookup? Or just iterate since it's likely small?
        // Let's iterate for now, but sorting would be better if we had to do it often.
        // Actually, we can just check if pc is in a collapsed block.

        while pc < data.len() {
            // Check for collapsed block
            if let Some((_start, end)) = collapsed_blocks.iter().find(|(s, _)| *s == pc) {
                let start_addr = origin.wrapping_add(pc as u16);
                let end_addr = origin.wrapping_add(*end as u16);
                let block_type = block_types.get(pc).unwrap_or(&BlockType::Code);

                lines.push(DisassemblyLine {
                    address: start_addr,
                    bytes: vec![], // No bytes shown
                    mnemonic: format!(
                        "{} Collapsed {} block from ${:04X}-${:04X}",
                        formatter.comment_prefix(),
                        block_type,
                        start_addr,
                        end_addr
                    ),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    comment_address: None,
                    is_collapsed: true,
                });

                pc = *end + 1;
                continue;
            }

            let address = origin.wrapping_add(pc as u16);

            let label_name = self.get_label_name(address, labels, formatter.as_ref());
            let side_comment = self.get_side_comment(
                address,
                labels,
                settings,
                system_comments,
                user_side_comments,
                cross_refs,
                formatter.comment_prefix(),
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
                    immediate_value_formats,
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
                    splitters,
                    settings,
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
                    splitters,
                    settings,
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
                    splitters,
                    settings,
                ),
                BlockType::Text => self.handle_text(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    settings,
                    label_name,
                    side_comment,
                    line_comment,
                    splitters,
                ),
                BlockType::Screencode => self.handle_screencode(
                    pc,
                    data,
                    block_types,
                    address,
                    formatter.as_ref(),
                    labels,
                    origin,
                    settings,
                    label_name,
                    side_comment,
                    line_comment,
                    splitters,
                ),
                BlockType::LoHi => self.handle_lohi(
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
                    splitters,
                    settings,
                ),
                BlockType::HiLo => self.handle_hilo(
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
                    splitters,
                    settings,
                ),
                BlockType::ExternalFile => self.handle_external_file(
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
                    splitters,
                    settings,
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

    #[allow(clippy::too_many_arguments)]
    fn handle_lohi(
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
        splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut count = 0;
        // Find extent of LoHi block, stopping at end of contiguous LoHi blocks
        while pc + count < data.len() {
            let current_pc = pc + count;

            // Check for splitter (except at start)
            if count > 0 {
                let current_addr = origin.wrapping_add(current_pc as u16);
                if splitters.contains(&current_addr) {
                    break;
                }
            }

            if block_types.get(current_pc) != Some(&BlockType::LoHi) {
                break;
            }
            // IMPORTANT: We do NOT break on labels within the block if the user explicity set LoHi.
            // But we SHOULD respect if the BlockType changes.
            // The user report says "midpoint was not in byte 256" for 512 bytes selection.
            // This is because the loop broke early due to a label at 256 or elsewhere.
            // Removing the label check fixes this.

            count += 1;
        }

        // Enforce even count for valid LoHi pairing
        let pair_count = count / 2;
        if pair_count == 0 {
            return self.handle_undefined_byte(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
            );
        }

        let total_bytes = pair_count * 2;
        let split_offset = pair_count; // Start of Hi bytes relative to pc

        let mut lines = Vec::new();

        // Helper to generate operand string
        let get_operand = |idx: usize, is_lo: bool| -> String {
            let lo = data[pc + idx];
            let hi = data[pc + split_offset + idx];
            let val = (hi as u16) << 8 | (lo as u16);

            // Try to resolve label.
            let label_part = if let Some(label_vec) = labels.get(&val) {
                formatter.format_label(&label_vec[0].name)
            } else {
                // Fallback: If no label, format as label-like reference if that was requested,
                // or just standard address format. User said "converted to <aD000... instead of <$d000".
                // This implies they want the "aD000" style even if it's not a real label?
                // NO, "aD000" IS a real label name automatically generated by valid disassembly.
                // If it's missing, it means analysis didn't find it.
                // We can't conjure it safely without checking collision.
                // However, we can use the hex address which corresponds to what "aXXXX" usually means.
                formatter.format_address(val)
            };

            if is_lo {
                format!("<{}", label_part)
            } else {
                format!(">{}", label_part)
            }
        };

        // Output Lo Lines
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(settings.addresses_per_line);
            let mut bytes = Vec::new();
            let mut operands = Vec::new();

            for k in 0..chunk_size {
                bytes.push(data[pc + i + k]);
                operands.push(get_operand(i + k, true));
            }

            lines.push(DisassemblyLine {
                address: origin.wrapping_add((pc + i) as u16),
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: if i == 0 {
                    side_comment.clone()
                } else {
                    String::new()
                },
                line_comment: if i == 0 { line_comment.clone() } else { None },
                label: if i == 0 { label_name.clone() } else { None },
                opcode: None,
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
            });

            i += chunk_size;
        }

        // Output Hi Lines
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(settings.addresses_per_line);
            let mut bytes = Vec::new();
            let mut operands = Vec::new();

            for k in 0..chunk_size {
                bytes.push(data[pc + split_offset + i + k]);
                operands.push(get_operand(i + k, false));
            }

            let current_hi_addr = origin.wrapping_add((pc + split_offset + i) as u16);
            // Check for label at this exact address (start of Hi chunk)
            let hi_label = labels
                .get(&current_hi_addr)
                .and_then(|v| v.first())
                .map(|l| l.name.clone());

            lines.push(DisassemblyLine {
                address: current_hi_addr,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: String::new(),
                line_comment: None,
                label: hi_label,
                opcode: None,
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
            });

            i += chunk_size;
        }

        (total_bytes, lines)
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_hilo(
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
        splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut count = 0;
        // Find extent of HiLo block, stopping at end of contiguous HiLo blocks
        while pc + count < data.len() {
            let current_pc = pc + count;

            // Check for splitter (except at start)
            if count > 0 {
                let current_addr = origin.wrapping_add(current_pc as u16);
                if splitters.contains(&current_addr) {
                    break;
                }
            }

            if block_types.get(current_pc) != Some(&BlockType::HiLo) {
                break;
            }
            count += 1;
        }

        // Enforce even count for valid HiLo pairing
        let pair_count = count / 2;
        if pair_count == 0 {
            return self.handle_undefined_byte(
                pc,
                data,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
            );
        }

        let total_bytes = pair_count * 2;
        let split_offset = pair_count; // Start of Lo bytes relative to pc

        let mut lines = Vec::new();

        // Helper to generate operand string
        let get_operand = |idx: usize, is_lo: bool| -> String {
            let hi = data[pc + idx];
            let lo = data[pc + split_offset + idx];
            let val = (hi as u16) << 8 | (lo as u16);

            // Try to resolve label.
            let label_part = if let Some(label_vec) = labels.get(&val) {
                formatter.format_label(&label_vec[0].name)
            } else {
                formatter.format_address(val)
            };

            if is_lo {
                format!("<{}", label_part)
            } else {
                format!(">{}", label_part)
            }
        };

        // Output Hi Lines (First half of data)
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(settings.addresses_per_line);
            let mut bytes = Vec::new();
            let mut operands = Vec::new();

            for k in 0..chunk_size {
                bytes.push(data[pc + i + k]);
                operands.push(get_operand(i + k, false));
            }

            lines.push(DisassemblyLine {
                address: origin.wrapping_add((pc + i) as u16),
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: if i == 0 {
                    side_comment.clone()
                } else {
                    String::new()
                },
                line_comment: if i == 0 { line_comment.clone() } else { None },
                label: if i == 0 { label_name.clone() } else { None },
                opcode: None,
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
            });

            i += chunk_size;
        }

        // Output Lo Lines (Second half of data)
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(settings.addresses_per_line);
            let mut bytes = Vec::new();
            let mut operands = Vec::new();

            for k in 0..chunk_size {
                bytes.push(data[pc + split_offset + i + k]);
                operands.push(get_operand(i + k, true));
            }

            let current_lo_addr = origin.wrapping_add((pc + split_offset + i) as u16);
            let lo_label = labels
                .get(&current_lo_addr)
                .and_then(|v| v.first())
                .map(|l| l.name.clone());

            lines.push(DisassemblyLine {
                address: current_lo_addr,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: String::new(),
                line_comment: None,
                label: lo_label,
                opcode: None,
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
            });

            i += chunk_size;
        }

        (total_bytes, lines)
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

    #[allow(clippy::too_many_arguments)]
    fn get_side_comment(
        &self,
        address: u16,
        _labels: &BTreeMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        system_comments: &BTreeMap<u16, String>,

        user_side_comments: &BTreeMap<u16, String>,
        cross_refs: &BTreeMap<u16, Vec<u16>>,
        comment_prefix: &str,
    ) -> String {
        let mut comment_parts = Vec::new();

        if let Some(user_comment) = user_side_comments.get(&address) {
            comment_parts.push(user_comment.clone());
        } else if let Some(sys_comment) = system_comments.get(&address) {
            comment_parts.push(sys_comment.clone());
        }

        if let Some(refs) = cross_refs.get(&address) {
            let mut all_refs = refs.clone();
            if !all_refs.is_empty() && settings.max_xref_count > 0 {
                all_refs.sort_unstable();
                all_refs.dedup();

                let refs_str: Vec<String> = all_refs
                    .iter()
                    .take(settings.max_xref_count)
                    .map(|r| format!("${:04x}", r)) // Use lowercase hex for refs in comments too
                    .collect();
                comment_parts.push(format!("x-ref: {}", refs_str.join(", ")));
            }
        }

        let separator = format!(" {} ", comment_prefix); // e.g. " ; " or " // "
        comment_parts.join(&separator)
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
        immediate_value_formats: &BTreeMap<u16, crate::state::ImmediateFormat>,
    ) -> (usize, Vec<DisassemblyLine>) {
        let opcode_byte = data[pc];
        let opcode_opt = &self.opcodes[opcode_byte as usize];

        if let Some(opcode) = opcode_opt
            && (!opcode.illegal || settings.use_illegal_opcodes)
        {
            let mut bytes = vec![opcode_byte];

            // Special handling for BRK
            // BRK ($00) normally takes 1 byte, but consumes 2 (signature byte).
            if opcode.mnemonic == "BRK" && !settings.brk_single_byte && pc + 1 < data.len() {
                let mut collision = false;
                if let Some(t) = block_types.get(pc + 1)
                    && *t != BlockType::Code
                {
                    collision = true;
                }

                if !collision {
                    if settings.patch_brk {
                        // "Patch BRK": BRK (1 byte) then .byte (1 byte)
                        let byte_val = data[pc + 1];
                        return (
                            2,
                            vec![
                                DisassemblyLine {
                                    address,
                                    bytes: vec![opcode_byte],
                                    mnemonic: formatter.format_mnemonic(opcode.mnemonic),
                                    operand: String::new(),
                                    comment: side_comment,
                                    line_comment,
                                    label: label_name,
                                    opcode: Some(opcode.clone()),
                                    show_bytes: true,
                                    target_address: None,
                                    comment_address: None,
                                    is_collapsed: false,
                                },
                                DisassemblyLine {
                                    address: address.wrapping_add(1),
                                    bytes: vec![byte_val],
                                    mnemonic: formatter.byte_directive().to_string(),
                                    operand: formatter.format_byte(byte_val),
                                    comment: String::new(),
                                    line_comment: None,
                                    label: self.get_label_name(
                                        address.wrapping_add(1),
                                        labels,
                                        formatter,
                                    ),
                                    opcode: None,
                                    show_bytes: true,
                                    target_address: None,
                                    comment_address: None,
                                    is_collapsed: false,
                                },
                            ],
                        );
                    } else {
                        // Default: BRK #$ signature
                        let byte_val = data[pc + 1];
                        let operand_str = format!("#{}", formatter.format_byte(byte_val));

                        return (
                            2,
                            vec![DisassemblyLine {
                                address,
                                bytes: vec![opcode_byte, byte_val],
                                mnemonic: formatter.format_mnemonic(opcode.mnemonic),
                                operand: operand_str,
                                comment: side_comment,
                                line_comment,
                                label: label_name,
                                opcode: Some(opcode.clone()),
                                show_bytes: true,
                                target_address: None,
                                comment_address: None,
                                is_collapsed: false,
                            }],
                        );
                    }
                }
            }

            // Check if we have enough bytes
            if pc + opcode.size as usize <= data.len() {
                let mut collision = false;
                for i in 1..opcode.size {
                    if let Some(t) = block_types.get(pc + i as usize)
                        && *t != BlockType::Code
                    {
                        collision = true;
                        break;
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
                        // Calculate origin to check if target is within our data block
                        let origin = address.wrapping_sub(pc as u16);
                        let target_idx = target_addr.wrapping_sub(origin) as usize;

                        // Check if target is known code
                        let mut is_code_target = false;
                        if target_idx < data.len()
                            && let Some(bt) = block_types.get(target_idx)
                            && *bt == BlockType::Code
                        {
                            is_code_target = true;
                        }

                        // Should we show the user comment?
                        // If it's code, NO (avoids propagation in loops).
                        // If it's data/unknown, YES.
                        let target_comment = if !is_code_target {
                            if let Some(c) = user_side_comments.get(&target_addr) {
                                Some(c)
                            } else {
                                system_comments.get(&target_addr)
                            }
                        } else {
                            // Even if we suppress user comments for code, we might want system comments (e.g. KERNAL)
                            // But usually KERNAL/System targets won't be in our 'data' block types loop unless we disassembled the whole memory.
                            // If they are outside (target_idx >= len), is_code_target is false, so we show them (correct for external system calls).
                            // If they are INSIDE and marked as Code, we suppress user comments (to fix the bug).
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

                    let ctx = crate::disassembler::formatter::FormatContext {
                        opcode,
                        operands: &bytes[1..],
                        address,
                        target_context,
                        labels,
                        settings,
                        immediate_value_formats,
                    };
                    let (mnemonic, operand_str) = formatter.format_instruction(&ctx);

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
                            comment_address: None,
                            is_collapsed: false,
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
                operand: formatter.format_byte(opcode_byte),
                comment: side_comment_final,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: true,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
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
        splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.bytes_per_line {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            // Stop if type changes
            if block_types.get(current_pc) != Some(&BlockType::DataByte) {
                break;
            }

            // Stop if splitter exists (except start)
            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            // Stop if label exists (except for the first byte)
            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            bytes.push(b);
            operands.push(formatter.format_byte(b));
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
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
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
        splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0; // Number of words

        while pc + (count * 2) + 1 < data.len() && count < settings.addresses_per_line {
            let current_pc_start = pc + (count * 2);
            let current_address = origin.wrapping_add(current_pc_start as u16);
            let next_address = current_address.wrapping_add(1);

            if block_types.get(current_pc_start) != Some(&BlockType::DataWord)
                || block_types.get(current_pc_start + 1) != Some(&BlockType::DataWord)
            {
                break;
            }

            if count > 0 {
                // Check if splitter exists at start of this word
                if splitters.contains(&current_address) {
                    break;
                }
            }

            // Check if splitter exists in the middle of this word
            if splitters.contains(&next_address) {
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
            operands.push(formatter.format_address(val));
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
                    show_bytes: false,
                    target_address: None,
                    comment_address: None,
                    is_collapsed: false,
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
    fn handle_external_file(
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
        splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.bytes_per_line {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            // Stop if type changes
            if block_types.get(current_pc) != Some(&BlockType::ExternalFile) {
                break;
            }

            // Stop if splitter exists (except start)
            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            // Stop if label exists (except for the first byte)
            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];
            bytes.push(b);
            operands.push(formatter.format_byte(b));
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
                show_bytes: false,
                target_address: None,
                comment_address: None,
                is_collapsed: false,
            }],
        )
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
        _splitters: &BTreeSet<u16>,
        settings: &DocumentSettings,
    ) -> (usize, Vec<DisassemblyLine>) {
        let mut bytes = Vec::new();
        let mut operands = Vec::new();
        let mut count = 0;

        while pc + (count * 2) + 1 < data.len() && count < settings.addresses_per_line {
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
                    .unwrap_or(formatter.format_address(val))
            } else {
                formatter.format_address(val)
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
                    show_bytes: false,
                    target_address: None,
                    comment_address: None,
                    is_collapsed: false,
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
        settings: &DocumentSettings,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
        splitters: &BTreeSet<u16>,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::Text) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
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

            // Find the first line that emits bytes to attach the label to.
            // For 64tass, this avoids attaching labels to .encode directives where they are not allowed.
            let first_byte_line_index = formatted_lines
                .iter()
                .position(|(_, _, has_bytes)| *has_bytes)
                .unwrap_or(0);

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
                let line_label = if i == first_byte_line_index {
                    label_name.clone()
                } else {
                    None
                };
                let (line_side_comment, line_line_comment) = if i == first_byte_line_index {
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
                    comment_address: None,
                    is_collapsed: false,
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
        settings: &DocumentSettings,
        label_name: Option<String>,
        side_comment: String,
        line_comment: Option<String>,
        splitters: &BTreeSet<u16>,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::Screencode) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
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

            // Find the first line that emits bytes to attach the label to (e.g. the body .text line).
            let first_byte_line_index = all_formatted_parts
                .iter()
                .position(|(_, _, has_bytes)| *has_bytes)
                .unwrap_or(0);

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
                let line_label = if i == first_byte_line_index {
                    label_name.clone()
                } else {
                    None
                };
                let (line_side_comment, line_line_comment) = if i == first_byte_line_index {
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
                    comment_address: None,
                    is_collapsed: false,
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
                    operand: formatter.format_byte(b),
                    comment: side_comment_final,
                    line_comment,
                    label: label_name,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    comment_address: None,
                    is_collapsed: false,
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
                    comment_address: None,
                    is_collapsed: false,
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
                comment_address: None,
                is_collapsed: false,
            }],
        )
    }
}
