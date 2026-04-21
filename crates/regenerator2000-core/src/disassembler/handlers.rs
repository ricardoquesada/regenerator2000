use super::DisassemblyLine;
use super::context::{DisassemblyContext, HandleArgs};
use crate::state::{Addr, BlockType};

pub fn handle_lohi_address(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::LoHiAddress, false, args)
}

pub fn handle_hilo_address(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::HiLoAddress, true, args)
}

pub fn handle_lohi_word(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::LoHiWord, false, args)
}

pub fn handle_hilo_word(
    ctx: &DisassemblyContext,
    args: HandleArgs,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(ctx, BlockType::HiLoWord, true, args)
}

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
    // Find extent of block
    while pc + count < ctx.data.len() {
        let current_pc = pc + count;

        if count > 0 {
            let current_addr = ctx.origin.wrapping_add(current_pc as u16);
            if ctx.is_virtual_splitter(current_addr) {
                break;
            }
            if ctx.user_line_comments.contains_key(&current_addr) {
                break;
            }
            // Stop if a side comment exists on an interior byte.
            if ctx.user_side_comments.contains_key(&current_addr) {
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
                current_scope_name: current_scope_name.clone(),
            },
        );
    }

    let total_bytes = pair_count * 2;
    let split_offset = pair_count;

    let mut lines = Vec::new();

    // Helper to generate operand string
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

        // Try to resolve label only for Address blocks.
        let is_address_block =
            target_type == BlockType::LoHiAddress || target_type == BlockType::HiLoAddress;
        let label_part = if is_address_block {
            crate::disassembler::resolve_label_name(
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

    // Output First Chunk lines
    let mut i = 0;
    while i < pair_count {
        let chunk_size = (pair_count - i).min(ctx.settings.addresses_per_line);
        let mut bytes = Vec::new();
        let mut operands = Vec::new();

        for k in 0..chunk_size {
            bytes.push(ctx.data[pc + i + k]);
            // If hi_first is true, inputs are Hi bytes, so we output > format (is_lo=false)
            // If hi_first is false, inputs are Lo bytes, so we output < format (is_lo=true)
            operands.push(get_operand(i + k, !hi_first));
        }

        let current_line_addr = ctx.origin.wrapping_add((pc + i) as u16);

        lines.push(DisassemblyLine {
            address: current_line_addr,
            bytes,
            mnemonic: formatter.byte_directive().to_string(),
            operand: operands.join(", "),
            comment: if i == 0 {
                side_comment.clone()
            } else {
                ctx.get_side_comment(current_line_addr, formatter.comment_prefix())
            },
            line_comment: if i == 0 {
                line_comment.clone()
            } else {
                ctx.user_line_comments.get(&current_line_addr).cloned()
            },
            label: if i == 0 {
                label_name.clone()
            } else {
                ctx.labels
                    .get(&current_line_addr)
                    .and_then(|v| v.first())
                    .map(|l| l.name.clone())
            },
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        i += chunk_size;
    }

    // Output Second Chunk lines
    let mut i = 0;
    while i < pair_count {
        let chunk_size = (pair_count - i).min(ctx.settings.addresses_per_line);
        let mut bytes = Vec::new();
        let mut operands = Vec::new();

        for k in 0..chunk_size {
            bytes.push(ctx.data[pc + split_offset + i + k]);
            // If hi_first is true, second chunk is Lo bytes, so output < (is_lo=true)
            // If hi_first is false, second chunk is Hi bytes, so output > (is_lo=false)
            operands.push(get_operand(i + k, hi_first));
        }

        let current_line_addr = ctx.origin.wrapping_add((pc + split_offset + i) as u16);
        let chunk_label = ctx
            .labels
            .get(&current_line_addr)
            .and_then(|v| v.first())
            .map(|l| l.name.clone());

        lines.push(DisassemblyLine {
            address: current_line_addr,
            bytes,
            mnemonic: formatter.byte_directive().to_string(),
            operand: operands.join(", "),
            comment: ctx.get_side_comment(current_line_addr, formatter.comment_prefix()),
            line_comment: ctx.user_line_comments.get(&current_line_addr).cloned(),
            label: chunk_label,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        i += chunk_size;
    }

    (total_bytes, lines)
}

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
        local_label_names: _,
        label_scope_names: _,
        current_scope_name: _,
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
