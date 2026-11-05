//! Block-level handlers for split low/high byte tables (`LoHiAddress`, `HiLoAddress`,
//! `LoHiWord`, `HiLoWord`) and undefined bytes (`Undefined`).

use super::DisassemblyLine;
use super::context::{DisassemblyContext, HandleArgs};
use super::symbols::{resolve_label_name, resolve_line_label};
use crate::state::{Addr, BlockType};

/// Disassembles a split address table where the low bytes precede the high bytes.
#[must_use]
pub fn handle_lohi_address(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::LoHiAddress, false, args)
}

/// Disassembles a split address table where the high bytes precede the low bytes.
#[must_use]
pub fn handle_hilo_address(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::HiLoAddress, true, args)
}

/// Disassembles a split 16-bit word table where the low bytes precede the high bytes.
#[must_use]
pub fn handle_lohi_word(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::LoHiWord, false, args)
}

/// Disassembles a split 16-bit word table where the high bytes precede the low bytes.
#[must_use]
pub fn handle_hilo_word(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::HiLoWord, true, args)
}

/// Disassembles a contiguous split-byte table of `target_type`.
///
/// The table consists of `pair_count` low bytes and `pair_count` high bytes.
/// When `hi_first` is `true`, the high-byte half comes first; otherwise the low-byte half
/// comes first.
fn handle_split_byte_table(
    ctx: &DisassemblyContext,
    target_type: BlockType,
    hi_first: bool,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    let HandleArgs {
        pc,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
        local_label_names,
        label_scope_names,
        current_scope_name,
    } = args;

    let mut count = 0;
    while pc + count < ctx.data.len() {
        let current_pc = pc + count;

        if count > 0 {
            let current_addr = ctx.origin.wrapping_add(current_pc as u16);
            if ctx.is_virtual_splitter(current_addr) {
                break;
            }
            if let Some(entry) = ctx.annotations.get(current_addr)
                && (entry.user_line_comment.is_some() || entry.user_side_comment.is_some())
            {
                break;
            }
        }

        if ctx.block_types.get(current_pc) != Some(&target_type) {
            break;
        }
        count += 1;
    }

    let pair_count = count / 2;
    if pair_count == 0 {
        return handle_undefined_byte(
            ctx,
            HandleArgs {
                pc,
                address,
                formatter,
                label_name,
                side_comment,
                line_comment,
                local_label_names,
                label_scope_names,
                current_scope_name,
            },
        );
    }

    let total_bytes = pair_count * 2;
    let split_offset = pair_count;
    let is_address_block =
        target_type == BlockType::LoHiAddress || target_type == BlockType::HiLoAddress;

    let get_operand = |idx: usize, is_lo: bool| -> String {
        let val = if hi_first {
            let hi = ctx.data[pc + idx];
            let lo = ctx.data[pc + split_offset + idx];
            u16::from(hi) << 8 | u16::from(lo)
        } else {
            let lo = ctx.data[pc + idx];
            let hi = ctx.data[pc + split_offset + idx];
            u16::from(hi) << 8 | u16::from(lo)
        };

        let label_part = if is_address_block {
            resolve_label_name(
                Addr(val),
                ctx.labels,
                ctx.settings,
                local_label_names,
                label_scope_names,
                current_scope_name.as_deref(),
                formatter.scope_resolution_separator(),
                formatter.local_label_prefix(),
            )
            .unwrap_or_else(|| formatter.format_address(Addr(val)))
        } else {
            formatter.format_address(Addr(val))
        };

        if is_lo {
            format!("<{label_part}")
        } else {
            format!(">{label_part}")
        }
    };

    let mut lines = Vec::new();

    for (chunk_offset, is_lo) in [(0, !hi_first), (split_offset, hi_first)] {
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(ctx.settings.addresses_per_line);
            let mut bytes = Vec::with_capacity(chunk_size);
            let mut operands = Vec::with_capacity(chunk_size);

            for k in 0..chunk_size {
                bytes.push(ctx.data[pc + chunk_offset + i + k]);
                operands.push(get_operand(i + k, is_lo));
            }

            let is_first_line = chunk_offset == 0 && i == 0;
            let current_line_addr = ctx.origin.wrapping_add((pc + chunk_offset + i) as u16);

            let (line_label, line_side_comment, line_line_comment) = if is_first_line {
                (
                    label_name.clone(),
                    side_comment.clone(),
                    line_comment.clone(),
                )
            } else {
                (
                    resolve_line_label(
                        current_line_addr,
                        local_label_names,
                        ctx.labels,
                        formatter,
                        ctx.settings,
                    ),
                    ctx.get_side_comment(current_line_addr, formatter.comment_prefix()),
                    ctx.annotations
                        .get(current_line_addr)
                        .and_then(|e| e.user_line_comment.clone()),
                )
            };

            lines.push(DisassemblyLine {
                address: current_line_addr,
                bytes,
                mnemonic: formatter.byte_directive().to_string(),
                operand: operands.join(", "),
                comment: line_side_comment,
                line_comment: line_line_comment,
                label: line_label,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            });

            i += chunk_size;
        }
    }

    (total_bytes, lines)
}

/// Disassembles a single undefined byte (`BlockType::Undefined`) as a 1-byte directive.
#[must_use]
pub fn handle_undefined_byte(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    let HandleArgs {
        pc,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
        ..
    } = args;
    let b = ctx.data[pc];
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
            external_label_address: None,
            is_collapsed: false,
        }],
    )
}
