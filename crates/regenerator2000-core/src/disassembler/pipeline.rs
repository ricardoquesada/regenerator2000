use crate::cpu::Opcode;
use crate::disassembler::DisassemblyLine;
use crate::disassembler::context::{DisassemblyContext, HandleArgs};
use crate::disassembler::data_blocks;
use crate::disassembler::handlers;
use crate::disassembler::symbols::{
    compute_local_label_names, compute_scope_names, get_arrow_target_address_for_opcode,
    get_label_name, get_referenced_address_for_opcode,
};
use crate::state::{Addr, BlockType};
use std::collections::BTreeMap;

/// Disassembles a single 6502 code instruction at `pc`.
#[must_use]
pub fn disassemble_code_instruction(
    opcodes: &[Option<Opcode>; 256],
    ctx: &DisassemblyContext<'_>,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    let HandleArgs {
        pc,
        address,
        formatter,
        label_name,
        mut side_comment,
        line_comment,
        local_label_names,
        label_scope_names,
        current_scope_name,
    } = args;

    let data = ctx.data;
    let block_types = ctx.block_types;
    let labels = ctx.labels;
    let settings = ctx.settings;

    let opcode_byte = match data.get(pc) {
        Some(&b) => b,
        None => return (0, vec![]),
    };

    let opcode_opt = opcodes.get(opcode_byte as usize).and_then(|op| op.as_ref());

    if let Some(opcode) = opcode_opt
        && (!opcode.illegal || settings.use_illegal_opcodes)
    {
        let mut bytes = vec![opcode_byte];

        // Special handling for BRK
        if opcode.mnemonic == "BRK" && !settings.brk_single_byte && pc + 1 < data.len() {
            let collision = block_types
                .get(pc + 1)
                .is_some_and(|t| *t != BlockType::Code);

            if !collision {
                if settings.patch_brk {
                    if let Some(&byte_val) = data.get(pc + 1) {
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
                                    label: get_label_name(
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
                    }
                } else if let Some(&byte_val) = data.get(pc + 1) {
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
                if block_types
                    .get(pc + i as usize)
                    .is_some_and(|t| *t != BlockType::Code)
                {
                    collision = true;
                    break;
                }
            }

            if !collision {
                for i in 1..opcode.size {
                    if let Some(&b) = data.get(pc + i as usize) {
                        bytes.push(b);
                    }
                }

                // Append referenced address comment if any
                if let Some(target_addr) =
                    get_referenced_address_for_opcode(opcode, &bytes, address)
                    && let Some(target_comment) = ctx.get_target_comment(target_addr)
                {
                    if !side_comment.is_empty() {
                        side_comment.push_str("; ");
                    }
                    side_comment.push_str(target_comment);
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
                    crate::cpu::AddressingMode::Relative => Some(crate::state::LabelType::Branch),
                    crate::cpu::AddressingMode::Absolute => {
                        if opcode.mnemonic == "JSR" {
                            Some(crate::state::LabelType::Subroutine)
                        } else if opcode.mnemonic == "JMP" {
                            Some(crate::state::LabelType::Jump)
                        } else {
                            Some(crate::state::LabelType::AbsoluteAddress)
                        }
                    }
                    crate::cpu::AddressingMode::AbsoluteX => Some(crate::state::LabelType::Field),
                    crate::cpu::AddressingMode::AbsoluteY => Some(crate::state::LabelType::Field),
                    crate::cpu::AddressingMode::Indirect => Some(crate::state::LabelType::Pointer),
                    crate::cpu::AddressingMode::IndirectX => {
                        Some(crate::state::LabelType::ZeroPagePointer)
                    }
                    crate::cpu::AddressingMode::IndirectY => {
                        Some(crate::state::LabelType::ZeroPagePointer)
                    }
                    _ => None,
                };

                let operands_slice = bytes.get(1..).unwrap_or(&[]);
                let fmt_ctx = crate::disassembler::formatter::FormatContext {
                    opcode,
                    operands: operands_slice,
                    address,
                    target_context,
                    labels,
                    settings,
                    annotations: ctx.annotations,
                    local_label_names,
                    label_scope_names,
                    current_scope_name: current_scope_name.as_deref(),
                    scope_separator: formatter.scope_resolution_separator(),
                    local_prefix: formatter.local_label_prefix(),
                    enums: ctx.enums,
                    user_global_enums: ctx.user_global_enums,
                    builtin_enums: ctx.builtin_enums,
                };
                let (mnemonic, operand_str) = formatter.format_instruction(&fmt_ctx);

                let target_address = get_arrow_target_address_for_opcode(opcode, &bytes, address);

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
        side_comment_final = format!("{side_comment}; {side_comment_final}");
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

/// Drives the main disassembly loop over `ctx.data`.
#[must_use]
pub fn disassemble_ctx(
    opcodes: &[Option<Opcode>; 256],
    ctx: &DisassemblyContext<'_>,
) -> Vec<DisassemblyLine> {
    let formatter = crate::disassembler::Disassembler::create_formatter(ctx.settings.assembler);

    let mut lines = Vec::new();
    let mut pc = 0;
    let mut current_scope_end_pc: Option<usize> = None;
    let mut current_scope_name: Option<String> = None;

    let supports_scopes = formatter.supports_scopes();
    let label_scope_names = if supports_scopes {
        compute_scope_names(ctx, formatter.as_ref())
    } else {
        BTreeMap::new()
    };

    let mut all_local_names: Option<BTreeMap<Addr, String>> = None;
    if supports_scopes {
        let mut locals = BTreeMap::new();
        for (start_addr, end_addr) in ctx
            .annotations
            .iter()
            .filter_map(|(s, e)| e.scope.map(|end| (s, end)))
        {
            let start_pc = match start_addr.0.checked_sub(ctx.origin.0) {
                Some(off) if (off as usize) < ctx.data.len() => off as usize,
                _ => continue,
            };
            let end_pc = match end_addr.0.checked_sub(ctx.origin.0) {
                Some(off) => (off as usize).min(ctx.data.len().saturating_sub(1)),
                _ => continue,
            };
            locals.extend(compute_local_label_names(
                opcodes,
                ctx,
                start_pc,
                end_pc,
                formatter.as_ref(),
            ));
        }
        if !locals.is_empty() {
            all_local_names = Some(locals);
        }
    }

    while pc < ctx.data.len() {
        let address = ctx.origin.wrapping_add(pc as u16);

        // Check if we just exited a Scope
        if let Some(end) = current_scope_end_pc
            && pc > end
        {
            if let Some(pend) = formatter.format_scope_end() {
                lines.push(DisassemblyLine {
                    address: ctx.origin.wrapping_add(pc as u16),
                    bytes: vec![],
                    mnemonic: pend,
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });
            }
            current_scope_end_pc = None;
            current_scope_name = None;
        }

        // Render splitter if exists at this address
        if ctx.is_virtual_splitter(address) {
            lines.push(DisassemblyLine {
                address,
                bytes: vec![],
                mnemonic: "{splitter}".to_string(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: None,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            });
        }

        let mut suppress_label = false;
        let mut suppress_line_comment = false;

        // Check if we are starting a NEW Scope
        if supports_scopes
            && let Some(end_addr) = ctx.annotations.get(address).and_then(|e| e.scope)
            && current_scope_end_pc.is_none()
        {
            let end = match end_addr.0.checked_sub(ctx.origin.0) {
                Some(off) if (off as usize) < ctx.data.len() => off as usize,
                _ => pc,
            };
            current_scope_end_pc = Some(end);

            let label_name_opt =
                get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings);

            if let Some((label, mnemonic, operand)) =
                formatter.format_scope_start(label_name_opt.as_deref())
            {
                current_scope_name = label_name_opt.clone();

                let line_comment = ctx
                    .annotations
                    .get(address)
                    .and_then(|e| e.user_line_comment.clone());

                lines.push(DisassemblyLine {
                    address,
                    bytes: vec![],
                    mnemonic,
                    operand: operand.unwrap_or_default(),
                    comment: String::new(),
                    line_comment,
                    label,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });

                suppress_label = true;
                suppress_line_comment = true;
            }
        }

        // Check for collapsed block
        if let Some((_start, end)) = ctx.collapsed_blocks.iter().find(|(s, _)| *s == pc) {
            let start_addr = ctx.origin.wrapping_add(pc as u16);
            let end_addr = ctx.origin.wrapping_add(*end as u16);
            let block_type = ctx.block_types.get(pc).unwrap_or(&BlockType::Code);

            lines.push(DisassemblyLine {
                address: start_addr,
                bytes: vec![],
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

        let label_name = if suppress_label {
            None
        } else if let Some(local_names) = &all_local_names {
            if let Some(local_name) = local_names.get(&address) {
                Some(local_name.clone())
            } else {
                get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings)
            }
        } else {
            get_label_name(address, ctx.labels, formatter.as_ref(), ctx.settings)
        };

        let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
        let line_comment = if suppress_line_comment {
            None
        } else {
            ctx.annotations
                .get(address)
                .and_then(|e| e.user_line_comment.clone())
        };

        let current_type = ctx.block_types.get(pc).copied().unwrap_or(BlockType::Code);

        let args = HandleArgs {
            pc,
            address,
            formatter: formatter.as_ref(),
            label_name,
            side_comment,
            line_comment,
            local_label_names: all_local_names.as_ref(),
            label_scope_names: Some(&label_scope_names),
            current_scope_name: current_scope_name.clone(),
        };

        let (bytes_consumed, new_lines) = match current_type {
            BlockType::Code => disassemble_code_instruction(opcodes, ctx, args),
            BlockType::DataByte => data_blocks::disassemble_bytes(ctx, pc, formatter.as_ref()),
            BlockType::DataWord => data_blocks::disassemble_words(ctx, pc, formatter.as_ref()),
            BlockType::Address => data_blocks::disassemble_addresses(
                ctx,
                pc,
                formatter.as_ref(),
                all_local_names.as_ref(),
                Some(&label_scope_names),
                current_scope_name.as_deref(),
            ),
            BlockType::PetsciiText => data_blocks::disassemble_petscii(ctx, pc, formatter.as_ref()),
            BlockType::ScreencodeText => {
                data_blocks::disassemble_screencode(ctx, pc, formatter.as_ref())
            }
            BlockType::LoHiAddress => handlers::handle_lohi_address(ctx, args),
            BlockType::HiLoAddress => handlers::handle_hilo_address(ctx, args),
            BlockType::LoHiWord => handlers::handle_lohi_word(ctx, args),
            BlockType::HiLoWord => handlers::handle_hilo_word(ctx, args),
            BlockType::ExternalFile => {
                data_blocks::disassemble_external_file(ctx, pc, formatter.as_ref())
            }
            BlockType::Undefined => handlers::handle_undefined_byte(ctx, args),
        };

        lines.extend(new_lines);
        pc += bytes_consumed;
    }

    lines
}
