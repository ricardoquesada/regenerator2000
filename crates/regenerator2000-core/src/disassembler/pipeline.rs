//! Main disassembly loop and instruction decoding pipeline.
//!
//! This module drives the byte-by-byte traversal of `ctx.data` ([`disassemble_ctx`]).
//! It emits lexical scope delimiters, virtual block splitters ([`super::SPLITTER_MNEMONIC`]),
//! collapsed block summaries, code instructions ([`disassemble_code_instruction`]), and
//! formatted data directives.

use crate::cpu::Opcode;
use crate::disassembler::DisassemblyLine;
use crate::disassembler::context::{DisassemblyContext, HandleArgs};
use crate::disassembler::data_blocks;
use crate::disassembler::formatter::Formatter;
use crate::disassembler::handlers;
use crate::disassembler::symbols::{
    ScopeNameMap, compute_local_label_names, compute_scope_names,
    get_arrow_target_address_for_opcode, get_label_name, get_referenced_address_for_opcode,
    has_active_address_offset, resolve_effective_symbolic_offset_for_mode, resolve_line_label,
};
use crate::state::{Addr, BlockType};
use std::collections::BTreeMap;

/// Decodes one MOS 6502/6510 instruction starting at byte offset `args.pc`.
///
/// Returns `(bytes_consumed, emitted_lines)`. This function handles two-byte `BRK` instructions
/// (`!settings.brk_single_byte`) and skips undocumented opcodes when `!settings.use_illegal_opcodes`.
/// Incomplete instructions or instructions that cross a block boundary emit a `.byte` directive.
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

    let Some(&opcode_byte) = data.get(pc) else {
        return (0, vec![]);
    };

    let opcode_opt = opcodes.get(opcode_byte as usize).and_then(|op| op.as_ref());

    if let Some(opcode) = opcode_opt
        && (!opcode.illegal || settings.use_illegal_opcodes)
    {
        if opcode.mnemonic == "BRK"
            && !settings.brk_single_byte
            && block_types.get(pc + 1) == Some(&BlockType::Code)
            && let Some(&byte_val) = data.get(pc + 1)
        {
            if settings.patch_brk {
                let next_addr = address.wrapping_add(1);
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
                            address: next_addr,
                            bytes: vec![byte_val],
                            mnemonic: formatter.byte_directive().to_string(),
                            operand: formatter.format_byte(byte_val),
                            comment: String::new(),
                            line_comment: None,
                            label: resolve_line_label(
                                next_addr,
                                local_label_names,
                                labels,
                                ctx.annotations,
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

            return (
                2,
                vec![DisassemblyLine {
                    address,
                    bytes: vec![opcode_byte, byte_val],
                    mnemonic: formatter.format_mnemonic(opcode.mnemonic),
                    operand: format!("#{}", formatter.format_byte(byte_val)),
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

        let end_pc = pc + usize::from(opcode.size);
        if let Some(inst_bytes) = data.get(pc..end_pc)
            && (pc + 1..end_pc).all(|i| block_types.get(i) == Some(&BlockType::Code))
        {
            let bytes = inst_bytes.to_vec();

            if let Some(target_addr) = get_referenced_address_for_opcode(opcode, &bytes, address) {
                // Prefer an explicit comment on `target_addr` (for example, a specific struct
                // field note); otherwise fall back to `sym.base`'s comment when a `SymbolicOffset`
                // redirects the operand (such as `zp_ptr + 1` inheriting `zp_ptr`'s note).
                let sym = resolve_effective_symbolic_offset_for_mode(
                    address,
                    target_addr,
                    opcode.mode.is_zero_page(),
                    labels,
                    ctx.annotations,
                );
                if let Some(target_comment) = ctx
                    .get_target_comment(target_addr)
                    .or_else(|| sym.and_then(|s| ctx.get_target_comment(s.base)))
                {
                    if !side_comment.is_empty() {
                        side_comment.push_str("; ");
                    }
                    side_comment.push_str(target_comment);
                }
            }

            let operands_slice = bytes.get(1..).unwrap_or(&[]);
            let fmt_ctx = crate::disassembler::formatter::FormatContext {
                opcode,
                operands: operands_slice,
                address,
                target_context: None,
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
                usize::from(opcode.size),
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

/// Appends a closing lexical scope line from `formatter` at `address`, if the formatter
/// emits one.
fn push_scope_end(lines: &mut Vec<DisassemblyLine>, formatter: &dyn Formatter, address: Addr) {
    if let Some(pend) = formatter.format_scope_end() {
        lines.push(DisassemblyLine {
            address,
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
}

/// Disassembles `ctx.data` into a sequence of [`DisassemblyLine`] rows.
///
/// This function resolves lexical scopes, virtual splitters, collapsed block ranges,
/// instructions, and data blocks across the entire buffer.
#[must_use]
pub fn disassemble_ctx(
    opcodes: &[Option<Opcode>; 256],
    ctx: &DisassemblyContext<'_>,
) -> Vec<DisassemblyLine> {
    let formatter = crate::disassembler::Disassembler::formatter(ctx.settings.assembler);

    let mut lines = Vec::new();
    let mut pc = 0;
    let mut current_scope_end_pc: Option<usize> = None;
    let mut current_scope_name: Option<String> = None;

    let supports_scopes = formatter.supports_scopes();
    let label_scope_names = if supports_scopes {
        compute_scope_names(ctx, formatter)
    } else {
        ScopeNameMap::default()
    };

    let mut all_local_names: Option<BTreeMap<Addr, String>> = None;
    if supports_scopes {
        let mut locals = BTreeMap::new();
        for (&start_addr, &end_addr) in ctx.annotations.scopes() {
            let start_pc = match start_addr.0.checked_sub(ctx.origin.0) {
                Some(off) if (off as usize) < ctx.data.len() => off as usize,
                _ => continue,
            };
            let end_pc = match end_addr.0.checked_sub(ctx.origin.0) {
                Some(off) => (off as usize).min(ctx.data.len().saturating_sub(1)),
                _ => continue,
            };
            locals.extend(compute_local_label_names(
                opcodes, ctx, start_pc, end_pc, formatter,
            ));
        }
        if !locals.is_empty() {
            all_local_names = Some(locals);
        }
    }

    while pc < ctx.data.len() {
        let address = ctx.origin.wrapping_add(pc as u16);

        if let Some(end) = current_scope_end_pc
            && pc > end
        {
            push_scope_end(&mut lines, formatter, address);
            current_scope_end_pc = None;
            current_scope_name = None;
        }

        // Emit a virtual splitter marker (`SPLITTER_MNEMONIC`) when a user splitter or
        // lexical scope boundary exists at `address`. The TUI renders this marker as a
        // full-width horizontal ruler, and the batch exporter skips it via `is_splitter()`.
        if ctx.is_virtual_splitter(address) {
            lines.push(DisassemblyLine {
                address,
                bytes: vec![],
                mnemonic: super::SPLITTER_MNEMONIC.to_string(),
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

        if supports_scopes
            && let Some(end_addr) = ctx.annotations.get(address).and_then(|e| e.scope)
            && current_scope_end_pc.is_none()
        {
            let end = match end_addr.0.checked_sub(ctx.origin.0) {
                Some(off) if (off as usize) < ctx.data.len() => off as usize,
                _ => pc,
            };
            current_scope_end_pc = Some(end);

            let label_name_opt = if has_active_address_offset(address, ctx.labels, ctx.annotations)
            {
                None
            } else {
                get_label_name(address, ctx.labels, formatter, ctx.settings)
            };

            if let Some((label, mnemonic, operand)) =
                formatter.format_scope_start(label_name_opt.as_deref())
            {
                current_scope_name = label_name_opt;

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

        let label_name = if suppress_label {
            None
        } else {
            resolve_line_label(
                address,
                all_local_names.as_ref(),
                ctx.labels,
                ctx.annotations,
                formatter,
                ctx.settings,
            )
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
            formatter,
            label_name,
            side_comment,
            line_comment,
            local_label_names: all_local_names.as_ref(),
            label_scope_names: Some(&label_scope_names),
            current_scope_name: current_scope_name.clone(),
        };

        let (bytes_consumed, new_lines) = match current_type {
            BlockType::Code => disassemble_code_instruction(opcodes, ctx, args),
            BlockType::DataByte => data_blocks::disassemble_bytes(ctx, args),
            BlockType::DataWord => data_blocks::disassemble_words(ctx, args),
            BlockType::Address => data_blocks::disassemble_addresses(ctx, args),
            BlockType::PetsciiText => data_blocks::disassemble_petscii(ctx, args),
            BlockType::ScreencodeText => data_blocks::disassemble_screencode(ctx, args),
            BlockType::LoHiAddress => handlers::handle_lohi_address(ctx, args),
            BlockType::HiLoAddress => handlers::handle_hilo_address(ctx, args),
            BlockType::LoHiWord => handlers::handle_lohi_word(ctx, args),
            BlockType::HiLoWord => handlers::handle_hilo_word(ctx, args),
            BlockType::ExternalFile => data_blocks::disassemble_external_file(ctx, args),
            BlockType::Undefined => handlers::handle_undefined_byte(ctx, args),
        };

        lines.extend(new_lines);
        pc += bytes_consumed;
    }

    if current_scope_end_pc.is_some() {
        let end_address = ctx.origin.wrapping_add(pc as u16);
        push_scope_end(&mut lines, formatter, end_address);
    }

    lines
}
