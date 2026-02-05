use super::DisassemblyLine;
use super::context::DisassemblyContext;
use super::formatter::Formatter;
use crate::state::BlockType;

pub fn handle_lohi_address(
    ctx: &DisassemblyContext,
    pc: usize,
    address: u16,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(
        ctx,
        pc,
        BlockType::LoHiAddress,
        false,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
    )
}

pub fn handle_hilo_address(
    ctx: &DisassemblyContext,
    pc: usize,
    address: u16,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(
        ctx,
        pc,
        BlockType::HiLoAddress,
        true,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
    )
}

pub fn handle_lohi_word(
    ctx: &DisassemblyContext,
    pc: usize,
    address: u16,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(
        ctx,
        pc,
        BlockType::LoHiWord,
        false,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
    )
}

pub fn handle_hilo_word(
    ctx: &DisassemblyContext,
    pc: usize,
    address: u16,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
) -> (usize, Vec<DisassemblyLine>) {
    handle_split_byte_table(
        ctx,
        pc,
        BlockType::HiLoWord,
        true,
        address,
        formatter,
        label_name,
        side_comment,
        line_comment,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_split_byte_table(
    ctx: &DisassemblyContext,
    pc: usize,
    target_type: BlockType,
    hi_first: bool,
    address: u16,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
) -> (usize, Vec<DisassemblyLine>) {
    let mut count = 0;
    // Find extent of block
    while pc + count < ctx.data.len() {
        let current_pc = pc + count;

        if count > 0 {
            let current_addr = ctx.origin.wrapping_add(current_pc as u16);
            if ctx.splitters.contains(&current_addr) {
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
            ctx.data,
            pc,
            address,
            formatter,
            label_name,
            side_comment,
            line_comment,
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
            (hi as u16) << 8 | (lo as u16)
        } else {
            let lo = ctx.data[pc + idx];
            let hi = ctx.data[pc + split_offset + idx];
            (hi as u16) << 8 | (lo as u16)
        };

        // Try to resolve label.
        let label_part = if let Some(label_vec) = ctx.labels.get(&val)
            && let Some(label) = crate::disassembler::resolve_label(label_vec, val, ctx.settings)
        {
            formatter.format_label(&label.name)
        } else {
            formatter.format_address(val)
        };

        if is_lo {
            format!("<{}", label_part)
        } else {
            format!(">{}", label_part)
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

        lines.push(DisassemblyLine {
            address: ctx.origin.wrapping_add((pc + i) as u16),
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

        let current_chunk_addr = ctx.origin.wrapping_add((pc + split_offset + i) as u16);
        let chunk_label = ctx
            .labels
            .get(&current_chunk_addr)
            .and_then(|v| v.first())
            .map(|l| l.name.clone());

        lines.push(DisassemblyLine {
            address: current_chunk_addr,
            bytes,
            mnemonic: formatter.byte_directive().to_string(),
            operand: operands.join(", "),
            comment: String::new(),
            line_comment: None,
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
    data: &[u8],
    pc: usize,
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
            external_label_address: None,
            is_collapsed: false,
        }],
    )
}
