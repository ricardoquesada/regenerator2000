//! Block-level handlers for split low/high byte tables (`LoHiAddress`, `HiLoAddress`,
//! `LoHiWord`, `HiLoWord`) and undefined bytes (`Undefined`).
//!
//! For `LoHiAddress` and `HiLoAddress` tables, `handle_split_byte_table` pairs low and high
//! halves within each splitter- or comment-partitioned sub-block. Non-zero
//! [`SymbolicOffset`](crate::state::SymbolicOffset) expressions wrap in parentheses inside unary
//! `<` and `>` byte extractors (`<(sub_C100 - 1)`, `>(sub_C100 - 1)`).

use super::DisassemblyLine;
use super::context::{DisassemblyContext, HandleArgs};
use super::symbols::{
    resolve_line_label, resolve_split_pair_effective_symbolic_offset,
    resolve_split_pair_symbolic_label_name,
};
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
/// When `hi_first` is `true`, the high-byte half comes first. Otherwise, the low-byte half
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
        if count > 0
            && ctx.is_split_table_subblock_boundary(ctx.origin.wrapping_add(current_pc as u16))
        {
            break;
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

    let get_operand_and_comment = |idx: usize, is_lo: bool| -> (String, Option<&str>) {
        let val = if hi_first {
            let hi = ctx.data[pc + idx];
            let lo = ctx.data[pc + split_offset + idx];
            u16::from(hi) << 8 | u16::from(lo)
        } else {
            let lo = ctx.data[pc + idx];
            let hi = ctx.data[pc + split_offset + idx];
            u16::from(hi) << 8 | u16::from(lo)
        };
        let target_addr = Addr(val);

        let (label_part, target_comment) = if is_address_block {
            // Check canonical `first_addr` first and fall back to `second_addr` for hand-edited
            // projects; pass `parenthesize_offset = true` so `<` and `>` bind after `+/-`
            // (`<(sub_C100 - 1)`, `>(sub_C100 - 1)`).
            let first_addr = ctx.origin.wrapping_add((pc + idx) as u16);
            let second_addr = ctx.origin.wrapping_add((pc + split_offset + idx) as u16);
            let sym = resolve_split_pair_effective_symbolic_offset(
                first_addr,
                second_addr,
                target_addr,
                ctx.labels,
                ctx.annotations,
            );
            let comment = ctx
                .get_target_comment(target_addr)
                .or_else(|| sym.and_then(|s| ctx.get_target_comment(s.base)));
            let expr = resolve_split_pair_symbolic_label_name(
                first_addr,
                second_addr,
                target_addr,
                ctx.labels,
                ctx.settings,
                ctx.annotations,
                local_label_names,
                label_scope_names,
                current_scope_name.as_deref(),
                formatter.scope_resolution_separator(),
                formatter.local_label_prefix(),
                crate::state::OffsetParentheses::Parenthesized,
                |base| formatter.format_address(base),
            )
            .unwrap_or_else(|| formatter.format_address(target_addr));
            (expr, comment)
        } else {
            (formatter.format_address(target_addr), None)
        };

        let formatted = if is_lo {
            format!("<{label_part}")
        } else {
            format!(">{label_part}")
        };
        (formatted, target_comment)
    };

    let mut lines = Vec::new();

    for (chunk_offset, is_lo) in [(0, !hi_first), (split_offset, hi_first)] {
        let mut i = 0;
        while i < pair_count {
            let chunk_size = (pair_count - i).min(ctx.settings.addresses_per_line);
            let mut bytes = Vec::with_capacity(chunk_size);
            let mut operands = Vec::with_capacity(chunk_size);
            let mut target_comments = Vec::new();

            for k in 0..chunk_size {
                bytes.push(ctx.data[pc + chunk_offset + i + k]);
                let (operand, target_comment) = get_operand_and_comment(i + k, is_lo);
                operands.push(operand);
                if let Some(tc) = target_comment {
                    target_comments.push(tc);
                }
            }

            let is_first_line = chunk_offset == 0 && i == 0;
            let current_line_addr = ctx.origin.wrapping_add((pc + chunk_offset + i) as u16);

            let (line_label, mut line_side_comment, line_line_comment) = if is_first_line {
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
                        ctx.annotations,
                        formatter,
                        ctx.settings,
                    ),
                    ctx.get_side_comment(current_line_addr, formatter.comment_prefix()),
                    ctx.annotations
                        .get(current_line_addr)
                        .and_then(|e| e.user_line_comment.clone()),
                )
            };

            for tc in target_comments {
                if !line_side_comment.is_empty() {
                    line_side_comment.push_str("; ");
                }
                line_side_comment.push_str(tc);
            }

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
