use crate::cpu::{Opcode, get_opcodes};
use crate::state::{Assembler, BlockType, DocumentSettings, Label};
use std::collections::{BTreeMap, BTreeSet};

pub mod context;
pub mod formatter;
pub mod formatter_64tass;
pub mod formatter_acme;
pub mod formatter_ca65;
pub mod formatter_kickasm;
pub mod handlers;

use crate::state::LabelKind;
use formatter::Formatter;
use formatter_64tass::TassFormatter;
use formatter_acme::AcmeFormatter;
use formatter_ca65::Ca65Formatter;
use formatter_kickasm::KickAsmFormatter;

pub fn resolve_label<'a>(
    labels: &'a [Label],
    _address: u16,
    _settings: &DocumentSettings,
) -> Option<&'a Label> {
    if labels.is_empty() {
        return None;
    }

    // Filter and Sort
    // We want to pick ONE label.
    // Precedence:
    // 1. User
    // 2. System
    // 3. Auto

    // Priority Score (Higher is better)
    let get_priority = |k: &LabelKind| -> u8 {
        match k {
            LabelKind::User => 100,
            LabelKind::System => 50,
            LabelKind::Auto => 0,
        }
    };

    let mut best_label: Option<&Label> = None;

    for label in labels {
        if let Some(curr) = best_label {
            // Priority Check
            let curr_prio = get_priority(&curr.kind);
            let new_prio = get_priority(&label.kind);

            if new_prio > curr_prio {
                best_label = Some(label);
            } else if new_prio == curr_prio {
                // Tie-break with name (stability)
                // We prefer alphabetically smaller names to be deterministic
                if label.name < curr.name {
                    best_label = Some(label);
                }
            }
        } else {
            best_label = Some(label);
        }
    }
    best_label
}

pub use context::DisassemblyContext;

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
    pub external_label_address: Option<u16>,
    pub is_collapsed: bool,
}

pub struct Disassembler {
    pub opcodes: [Option<Opcode>; 256],
}

impl Default for Disassembler {
    fn default() -> Self {
        Self::new()
    }
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
        let ctx = DisassemblyContext {
            data,
            block_types,
            labels,
            origin,
            settings,
            system_comments,
            user_side_comments,
            user_line_comments,
            immediate_value_formats,
            cross_refs,
            collapsed_blocks,
            splitters,
        };
        self.disassemble_ctx(&ctx)
    }

    pub fn disassemble_ctx(&self, ctx: &DisassemblyContext) -> Vec<DisassemblyLine> {
        let formatter = Self::create_formatter(ctx.settings.assembler);

        let mut lines = Vec::new();
        let mut pc = 0;

        // Convert collapsed blocks to a map for O(1) lookup? Or just iterate since it's likely small?
        // Let's iterate for now, but sorting would be better if we had to do it often.
        // Actually, we can just check if pc is in a collapsed block.

        while pc < ctx.data.len() {
            // Check for collapsed block
            if let Some((_start, end)) = ctx.collapsed_blocks.iter().find(|(s, _)| *s == pc) {
                let start_addr = ctx.origin.wrapping_add(pc as u16);
                let end_addr = ctx.origin.wrapping_add(*end as u16);
                let block_type = ctx.block_types.get(pc).unwrap_or(&BlockType::Code);

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
                    external_label_address: None,
                    is_collapsed: true,
                });

                pc = *end + 1;
                continue;
            }

            let address = ctx.origin.wrapping_add(pc as u16);

            let label_name =
                self.get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings);
            let side_comment = self.get_side_comment(
                address,
                ctx.labels,
                ctx.settings,
                ctx.system_comments,
                ctx.user_side_comments,
                ctx.cross_refs,
                formatter.comment_prefix(),
            );
            let line_comment = ctx.user_line_comments.get(&address).cloned();

            let current_type = ctx.block_types.get(pc).copied().unwrap_or(BlockType::Code);

            let (bytes_consumed, new_lines) = match current_type {
                BlockType::Code => self.handle_code(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.settings,
                    ctx.origin,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.system_comments,
                    ctx.user_side_comments,
                    ctx.immediate_value_formats,
                ),
                BlockType::DataByte => self.handle_data_byte(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.splitters,
                    ctx.settings,
                    ctx.user_line_comments,
                ),
                BlockType::DataWord => self.handle_data_word(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.splitters,
                    ctx.settings,
                    ctx.user_line_comments,
                ),
                BlockType::Address => self.handle_address(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.system_comments,
                    ctx.user_side_comments,
                    ctx.splitters,
                    ctx.settings,
                    ctx.user_line_comments,
                ),
                BlockType::PetsciiText => self.handle_petscii_text(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    ctx.settings,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.splitters,
                    ctx.user_line_comments,
                ),
                BlockType::ScreencodeText => self.handle_screencode_text(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    ctx.settings,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.splitters,
                    ctx.user_line_comments,
                ),
                BlockType::LoHiAddress => handlers::handle_lohi_address(
                    ctx,
                    pc,
                    address,
                    formatter.as_ref(),
                    label_name,
                    side_comment,
                    line_comment,
                ),
                BlockType::HiLoAddress => handlers::handle_hilo_address(
                    ctx,
                    pc,
                    address,
                    formatter.as_ref(),
                    label_name,
                    side_comment,
                    line_comment,
                ),
                BlockType::LoHiWord => handlers::handle_lohi_word(
                    ctx,
                    pc,
                    address,
                    formatter.as_ref(),
                    label_name,
                    side_comment,
                    line_comment,
                ),
                BlockType::HiLoWord => handlers::handle_hilo_word(
                    ctx,
                    pc,
                    address,
                    formatter.as_ref(),
                    label_name,
                    side_comment,
                    line_comment,
                ),
                BlockType::ExternalFile => self.handle_external_file(
                    pc,
                    ctx.data,
                    ctx.block_types,
                    address,
                    formatter.as_ref(),
                    ctx.labels,
                    ctx.origin,
                    label_name,
                    side_comment,
                    line_comment,
                    ctx.splitters,
                    ctx.settings,
                    ctx.user_line_comments,
                ),
                BlockType::Undefined => handlers::handle_undefined_byte(
                    ctx.data,
                    pc,
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

        if !opcode.is_flow_control_with_target() {
            return None;
        }

        match opcode.mode {
            AddressingMode::Absolute => {
                if bytes.len() >= 3 {
                    Some((bytes[2] as u16) << 8 | (bytes[1] as u16))
                } else {
                    None
                }
            }
            AddressingMode::Relative => {
                if bytes.len() >= 2 {
                    let offset = bytes[1] as i8;
                    Some(address.wrapping_add(2).wrapping_add(offset as u16))
                } else {
                    None
                }
            }
            _ => None,
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
        settings: &DocumentSettings,
    ) -> Option<String> {
        labels.get(&address).and_then(|v| {
            resolve_label(v, address, settings).map(|l| formatter.format_label(&l.name))
        })
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

        if let Some(refs) = cross_refs.get(&address)
            && !refs.is_empty()
            && settings.max_xref_count > 0
        {
            comment_parts.push(format_cross_references(refs, settings.max_xref_count));
        }

        let separator = format!(" {} ", comment_prefix); // e.g. " ; " or " // "
        comment_parts.join(&separator)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_code(
        &self,
        pc: usize,
        data: &[u8],
        block_types: &[BlockType],
        address: u16,
        formatter: &dyn Formatter,
        labels: &BTreeMap<u16, Vec<Label>>,
        settings: &DocumentSettings,
        _origin: u16, // Add origin
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
                                    external_label_address: None,
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
                                        settings,
                                    ),
                                    opcode: None,
                                    show_bytes: true,
                                    target_address: None,
                                    external_label_address: None,
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
                                external_label_address: None,
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
                            external_label_address: None,
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
                external_label_address: None,
                is_collapsed: false,
            }],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_data_byte(
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
        user_line_comments: &BTreeMap<u16, String>,
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

            // Stop if line comment exists (except start)
            if count > 0 && user_line_comments.contains_key(&current_address) {
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
                external_label_address: None,
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
        user_line_comments: &BTreeMap<u16, String>,
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

            // Stop if line comment exists (except start)
            // Note: DataWord is 2 bytes. We check if comment is at the start of the word.
            if count > 0 && user_line_comments.contains_key(&current_address) {
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
                    external_label_address: None,
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
        user_line_comments: &BTreeMap<u16, String>,
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

            if count > 0 && user_line_comments.contains_key(&current_address) {
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
                external_label_address: None,
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
        user_line_comments: &BTreeMap<u16, String>,
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

            if count > 0 && user_line_comments.contains_key(&current_address) {
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

            let operand = if let Some(label_vec) = labels.get(&val)
                && let Some(label) = resolve_label(label_vec, val, settings)
            {
                formatter.format_label(&label.name)
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
                    external_label_address: None,
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
    fn handle_petscii_text(
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
        user_line_comments: &BTreeMap<u16, String>,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::PetsciiText) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
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

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::PetsciiText);
        let next_pc = pc + count;
        let is_end =
            next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::PetsciiText);

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
                    external_label_address: None,
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
    fn handle_screencode_text(
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
        user_line_comments: &BTreeMap<u16, String>,
    ) -> (usize, Vec<DisassemblyLine>) {
        use crate::disassembler::formatter::TextFragment;

        let mut fragments = Vec::new();
        let mut current_literal = String::new();
        let mut count = 0;

        while pc + count < data.len() && count < settings.text_char_limit {
            let current_pc = pc + count;
            let current_address = origin.wrapping_add(current_pc as u16);

            if block_types.get(current_pc) != Some(&BlockType::ScreencodeText) {
                break;
            }

            if count > 0 && splitters.contains(&current_address) {
                break;
            }

            if count > 0 && labels.contains_key(&current_address) {
                break;
            }

            if count > 0 && user_line_comments.contains_key(&current_address) {
                break;
            }

            let b = data[current_pc];

            // bytes >= $5f should be treated as bytes. Not as screencodes.
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

        let is_start = pc == 0 || block_types.get(pc - 1) != Some(&BlockType::ScreencodeText);
        let next_pc = pc + count;
        let is_end =
            next_pc >= data.len() || block_types.get(next_pc) != Some(&BlockType::ScreencodeText);

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
                    external_label_address: None,
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
                    external_label_address: None,
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
                    external_label_address: None,
                    is_collapsed: false,
                }],
            )
        }
    }
}

pub fn format_cross_references(refs: &[u16], max_count: usize) -> String {
    if refs.is_empty() || max_count == 0 {
        return String::new();
    }

    let mut all_refs = refs.to_vec();
    all_refs.sort_unstable();
    all_refs.dedup();

    let refs_str: Vec<String> = all_refs
        .iter()
        .take(max_count)
        .map(|r| format!("${:04x}", r))
        .collect();

    let suffix = if all_refs.len() > max_count {
        ", ..."
    } else {
        ""
    };

    format!("x-ref: {}{}", refs_str.join(", "), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cross_references() {
        // Test truncation
        let refs = vec![0x2000, 0x3000, 0x4000];
        let output = format_cross_references(&refs, 2);
        assert_eq!(output, "x-ref: $2000, $3000, ...");

        // Test no truncation
        let output_full = format_cross_references(&refs, 5);
        assert_eq!(output_full, "x-ref: $2000, $3000, $4000");

        // Test deduplication
        let refs_dup = vec![0x2000, 0x2000];
        let output_dup = format_cross_references(&refs_dup, 2);
        assert_eq!(output_dup, "x-ref: $2000");
    }
}
