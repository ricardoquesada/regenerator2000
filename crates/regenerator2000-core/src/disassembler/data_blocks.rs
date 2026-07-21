use crate::disassembler::DisassemblyLine;
use crate::disassembler::context::DisassemblyContext;
use crate::disassembler::formatter::{Formatter, TextFragment};
use crate::disassembler::symbols::{get_label_name, resolve_label_name};
use crate::state::{Addr, BlockType};

/// Disassembles contiguous fill runs of identical bytes if configured threshold is met.
#[must_use]
pub fn disassemble_fill_run(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> Option<(usize, Vec<DisassemblyLine>)> {
    let threshold = ctx.settings.fill_run_threshold;
    if threshold == 0 || pc >= ctx.data.len() {
        return None;
    }

    let fill_byte = *ctx.data.get(pc)?;
    let address = ctx.origin.wrapping_add(pc as u16);
    let mut run_len = 0usize;

    for i in 0..ctx.data.len().saturating_sub(pc) {
        let cur_pc = pc + i;
        let cur_addr = ctx.origin.wrapping_add(cur_pc as u16);

        if ctx.block_types.get(cur_pc) != Some(&BlockType::DataByte) {
            break;
        }
        if ctx.data.get(cur_pc) != Some(&fill_byte) {
            break;
        }
        if i > 0 {
            if ctx.is_virtual_splitter(cur_addr) {
                break;
            }
            if ctx.labels.contains_key(&cur_addr) {
                break;
            }
            if ctx.cross_refs.get(&cur_addr).is_some_and(|v| !v.is_empty()) {
                break;
            }
            if ctx
                .annotations
                .get(cur_addr)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
            {
                break;
            }
            if ctx
                .annotations
                .get(cur_addr)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
            {
                break;
            }
        }

        run_len += 1;
    }

    if run_len >= threshold {
        let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
        let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
        let line_comment = ctx
            .annotations
            .get(address)
            .and_then(|e| e.user_line_comment.clone());
        let run_bytes = ctx.data.get(pc..pc + run_len)?.to_vec();

        Some((
            run_len,
            vec![DisassemblyLine {
                address,
                bytes: run_bytes,
                mnemonic: formatter.fill_directive().to_string(),
                operand: format!("{}, {}", run_len, formatter.format_byte(fill_byte)),
                comment: side_comment,
                line_comment,
                label: label_name,
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: None,
                is_collapsed: false,
            }],
        ))
    } else {
        None
    }
}

/// Disassembles byte data blocks (`BlockType::DataByte`).
#[must_use]
pub fn disassemble_bytes(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> (usize, Vec<DisassemblyLine>) {
    if let Some(res) = disassemble_fill_run(ctx, pc, formatter) {
        return res;
    }

    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut bytes = Vec::new();
    let mut operands = Vec::new();
    let mut count = 0;
    let threshold = ctx.settings.fill_run_threshold;

    while pc + count < ctx.data.len() && count < ctx.settings.bytes_per_line {
        let current_pc = pc + count;
        let current_address = ctx.origin.wrapping_add(current_pc as u16);

        if ctx.block_types.get(current_pc) != Some(&BlockType::DataByte) {
            break;
        }

        if count > 0 && ctx.is_virtual_splitter(current_address) {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && threshold > 0
            && let Some(&fill_byte) = ctx.data.get(current_pc)
        {
            let mut run_len = 0usize;
            for j in 0..ctx.data.len().saturating_sub(current_pc) {
                let j_pc = current_pc + j;
                let j_addr = ctx.origin.wrapping_add(j_pc as u16);
                if ctx.block_types.get(j_pc) != Some(&BlockType::DataByte) {
                    break;
                }
                if ctx.data.get(j_pc) != Some(&fill_byte) {
                    break;
                }
                if j > 0 {
                    if ctx.is_virtual_splitter(j_addr) {
                        break;
                    }
                    if ctx.labels.contains_key(&j_addr) {
                        break;
                    }
                    if ctx.cross_refs.get(&j_addr).is_some_and(|v| !v.is_empty()) {
                        break;
                    }
                    if ctx
                        .annotations
                        .get(j_addr)
                        .and_then(|e| e.user_line_comment.as_ref())
                        .is_some()
                    {
                        break;
                    }
                    if ctx
                        .annotations
                        .get(j_addr)
                        .and_then(|e| e.user_side_comment.as_ref())
                        .is_some()
                    {
                        break;
                    }
                }
                run_len += 1;
            }
            if run_len >= threshold {
                break;
            }
        }

        if let Some(&b) = ctx.data.get(current_pc) {
            bytes.push(b);
            let formatted_val = if let Some((enum_name, variant_name)) =
                ctx.resolve_enum_value(current_address, b as u16)
            {
                formatter.format_enum_reference(&enum_name, &variant_name)
            } else {
                formatter.format_byte(b)
            };
            operands.push(formatted_val);
            count += 1;
        } else {
            break;
        }
    }

    if count == 0 {
        return disassemble_partial_data(
            ctx,
            pc,
            formatter,
            label_name,
            side_comment,
            line_comment,
            "DataByte",
        );
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

/// Disassembles 16-bit word data blocks (`BlockType::DataWord`).
#[must_use]
pub fn disassemble_words(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut bytes = Vec::new();
    let mut operands = Vec::new();
    let mut count = 0;

    while pc + (count * 2) + 1 < ctx.data.len() && count < ctx.settings.addresses_per_line {
        let current_pc_start = pc + (count * 2);
        let current_address = ctx.origin.wrapping_add(current_pc_start as u16);
        let next_address = current_address.wrapping_add(1);

        if ctx.block_types.get(current_pc_start) != Some(&BlockType::DataWord)
            || ctx.block_types.get(current_pc_start + 1) != Some(&BlockType::DataWord)
        {
            break;
        }

        if count > 0 && ctx.is_virtual_splitter(current_address) {
            break;
        }
        if ctx.is_virtual_splitter(next_address) {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
        {
            break;
        }

        if let (Some(&low), Some(&high)) = (
            ctx.data.get(current_pc_start),
            ctx.data.get(current_pc_start + 1),
        ) {
            let val = u16::from(high) << 8 | u16::from(low);
            bytes.push(low);
            bytes.push(high);
            let formatted_val = if let Some((enum_name, variant_name)) =
                ctx.resolve_enum_value(current_address, val)
            {
                formatter.format_enum_reference(&enum_name, &variant_name)
            } else {
                formatter.format_address(Addr(val))
            };
            operands.push(formatted_val);
            count += 1;
        } else {
            break;
        }
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
        disassemble_partial_data(
            ctx,
            pc,
            formatter,
            label_name,
            side_comment,
            line_comment,
            "Word",
        )
    }
}

/// Disassembles address pointers (`BlockType::Address`).
#[must_use]
pub fn disassemble_addresses(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
    local_label_names: Option<&std::collections::BTreeMap<Addr, String>>,
    label_scope_names: Option<&std::collections::BTreeMap<Addr, String>>,
    current_scope_name: Option<&str>,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let mut side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut bytes = Vec::new();
    let mut operands = Vec::new();
    let mut count = 0;

    while pc + (count * 2) + 1 < ctx.data.len() && count < ctx.settings.addresses_per_line {
        let current_pc_start = pc + (count * 2);
        let current_address = ctx.origin.wrapping_add(current_pc_start as u16);

        if ctx.block_types.get(current_pc_start) != Some(&BlockType::Address)
            || ctx.block_types.get(current_pc_start + 1) != Some(&BlockType::Address)
        {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
        {
            break;
        }

        if let (Some(&low), Some(&high)) = (
            ctx.data.get(current_pc_start),
            ctx.data.get(current_pc_start + 1),
        ) {
            let val = u16::from(high) << 8 | u16::from(low);

            if let Some(target_comment) = ctx.get_target_comment(Addr(val)) {
                if !side_comment.is_empty() {
                    side_comment.push_str("; ");
                }
                side_comment.push_str(target_comment);
            }

            bytes.push(low);
            bytes.push(high);

            let operand = resolve_label_name(
                Addr(val),
                ctx.labels,
                ctx.settings,
                local_label_names,
                label_scope_names,
                current_scope_name,
                formatter.scope_resolution_separator(),
                formatter.local_label_prefix(),
            )
            .unwrap_or_else(|| formatter.format_address(Addr(val)));
            operands.push(operand);

            count += 1;
        } else {
            break;
        }
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
        disassemble_partial_data(
            ctx,
            pc,
            formatter,
            label_name,
            side_comment,
            line_comment,
            "Address",
        )
    }
}

/// Disassembles PETSCII text blocks (`BlockType::PetsciiText`).
#[must_use]
pub fn disassemble_petscii(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut fragments = Vec::new();
    let mut current_literal = String::new();
    let mut count = 0;

    while pc + count < ctx.data.len() && count < ctx.settings.text_char_limit {
        let current_pc = pc + count;
        let current_address = ctx.origin.wrapping_add(current_pc as u16);

        if ctx.block_types.get(current_pc) != Some(&BlockType::PetsciiText) {
            break;
        }

        if count > 0 && ctx.is_virtual_splitter(current_address) {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
        {
            break;
        }

        if let Some(&b) = ctx.data.get(current_pc) {
            if (0x20..=0x7E).contains(&b) {
                current_literal.push(b as char);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
            }
            count += 1;
        } else {
            break;
        }
    }

    if !current_literal.is_empty() {
        fragments.push(TextFragment::Text(current_literal));
    }

    let is_start = pc == 0
        || pc.checked_sub(1).and_then(|prev| ctx.block_types.get(prev))
            != Some(&BlockType::PetsciiText);
    let next_pc = pc + count;
    let is_end =
        next_pc >= ctx.data.len() || ctx.block_types.get(next_pc) != Some(&BlockType::PetsciiText);

    if count > 0 {
        let formatted_lines = formatter.format_text(&fragments, is_start, is_end);
        let mut disassembly_lines = Vec::new();

        let first_byte_line_index = formatted_lines
            .iter()
            .position(|(_, _, has_bytes)| *has_bytes)
            .unwrap_or(0);

        let all_bytes = ctx.data.get(pc..pc + count).unwrap_or(&[]).to_vec();

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
        disassemble_partial_data(
            ctx,
            pc,
            formatter,
            label_name,
            side_comment,
            line_comment,
            "Text",
        )
    }
}

/// Disassembles Screen Code text blocks (`BlockType::ScreencodeText`).
#[must_use]
pub fn disassemble_screencode(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut fragments = Vec::new();
    let mut current_literal = String::new();
    let mut count = 0;

    while pc + count < ctx.data.len() && count < ctx.settings.text_char_limit {
        let current_pc = pc + count;
        let current_address = ctx.origin.wrapping_add(current_pc as u16);

        if ctx.block_types.get(current_pc) != Some(&BlockType::ScreencodeText) {
            break;
        }

        if count > 0 && ctx.is_virtual_splitter(current_address) {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_side_comment.as_ref())
                .is_some()
        {
            break;
        }

        if let Some(&b) = ctx.data.get(current_pc) {
            let threshold = formatter.screencode_byte_threshold();
            if b >= threshold {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
                count += 1;
                continue;
            }

            let ascii = if b < 0x20 {
                b + 0x40
            } else if b < 0x40 {
                b
            } else {
                b.saturating_add(0x20)
            };

            if (0x20..=0x7E).contains(&ascii) {
                current_literal.push(ascii as char);
            } else {
                if !current_literal.is_empty() {
                    fragments.push(TextFragment::Text(current_literal.clone()));
                    current_literal.clear();
                }
                fragments.push(TextFragment::Byte(b));
            }

            count += 1;
        } else {
            break;
        }
    }

    if !current_literal.is_empty() {
        fragments.push(TextFragment::Text(current_literal));
    }

    let is_start = pc == 0
        || pc.checked_sub(1).and_then(|prev| ctx.block_types.get(prev))
            != Some(&BlockType::ScreencodeText);
    let next_pc = pc + count;
    let is_end = next_pc >= ctx.data.len()
        || ctx.block_types.get(next_pc) != Some(&BlockType::ScreencodeText);

    if count > 0 {
        let mut all_formatted_parts = Vec::new();

        if is_start {
            for (m, o) in formatter.format_screencode_pre() {
                all_formatted_parts.push((m, o, false));
            }
        }

        all_formatted_parts.extend(formatter.format_screencode(&fragments));

        if is_end {
            for (m, o) in formatter.format_screencode_post() {
                all_formatted_parts.push((m, o, false));
            }
        }

        let mut disassembly_lines = Vec::new();
        let first_byte_line_index = all_formatted_parts
            .iter()
            .position(|(_, _, has_bytes)| *has_bytes)
            .unwrap_or(0);

        let all_bytes = ctx.data.get(pc..pc + count).unwrap_or(&[]).to_vec();

        for (i, (mnemonic, operand, has_bytes)) in all_formatted_parts.iter().enumerate() {
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
        disassemble_partial_data(
            ctx,
            pc,
            formatter,
            label_name,
            side_comment,
            line_comment,
            "Screencode",
        )
    }
}

/// Disassembles external file include blocks (`BlockType::ExternalFile`).
#[must_use]
pub fn disassemble_external_file(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);
    let label_name = get_label_name(address, ctx.labels, formatter, ctx.settings);
    let side_comment = ctx.get_side_comment(address, formatter.comment_prefix());
    let line_comment = ctx
        .annotations
        .get(address)
        .and_then(|e| e.user_line_comment.clone());

    let mut bytes = Vec::new();
    let mut operands = Vec::new();
    let mut count = 0;

    while pc + count < ctx.data.len() && count < ctx.settings.bytes_per_line {
        let current_pc = pc + count;
        let current_address = ctx.origin.wrapping_add(current_pc as u16);

        if ctx.block_types.get(current_pc) != Some(&BlockType::ExternalFile) {
            break;
        }

        if count > 0 && ctx.is_virtual_splitter(current_address) {
            break;
        }

        if count > 0 && ctx.labels.contains_key(&current_address) {
            break;
        }

        if count > 0
            && ctx
                .annotations
                .get(current_address)
                .and_then(|e| e.user_line_comment.as_ref())
                .is_some()
        {
            break;
        }

        if let Some(&b) = ctx.data.get(current_pc) {
            bytes.push(b);
            operands.push(formatter.format_byte(b));
            count += 1;
        } else {
            break;
        }
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

/// Fallback renderer for incomplete or partial data bytes.
#[must_use]
pub fn disassemble_partial_data(
    ctx: &DisassemblyContext<'_>,
    pc: usize,
    formatter: &dyn Formatter,
    label_name: Option<String>,
    side_comment: String,
    line_comment: Option<String>,
    type_name: &str,
) -> (usize, Vec<DisassemblyLine>) {
    let address = ctx.origin.wrapping_add(pc as u16);

    if let Some(&b) = ctx.data.get(pc) {
        let mut side_comment_final = format!("Partial {type_name}");
        if !side_comment.is_empty() {
            side_comment_final = format!("{side_comment}; {side_comment_final}");
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
        (
            0,
            vec![DisassemblyLine {
                address,
                bytes: vec![],
                mnemonic: "???".to_string(),
                operand: String::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::Disassembler;
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;
    use std::sync::LazyLock;

    static EMPTY_LABELS: LazyLock<BTreeMap<Addr, Vec<crate::state::project::Label>>> =
        LazyLock::new(BTreeMap::new);
    static EMPTY_SPLITTERS: LazyLock<BTreeSet<Addr>> = LazyLock::new(BTreeSet::new);
    static EMPTY_ENUMS: LazyLock<BTreeMap<String, crate::state::EnumDefinition>> =
        LazyLock::new(BTreeMap::new);

    fn make_fill_ctx<'a>(
        data: &'a [u8],
        block_types: &'a [BlockType],
        cross_refs: &'a BTreeMap<Addr, Vec<Addr>>,
        line_comments: &'a BTreeMap<Addr, String>,
        side_comments: &'a BTreeMap<Addr, String>,
        threshold: usize,
    ) -> DisassemblyContext<'a> {
        let settings = crate::state::DocumentSettings {
            fill_run_threshold: threshold,
            ..Default::default()
        };

        let settings_box: &'static crate::state::DocumentSettings = Box::leak(Box::new(settings));

        let mut annotations = crate::state::AnnotationManager::default();
        for (addr, comment) in line_comments {
            annotations.update(*addr, |e| e.user_line_comment = Some(comment.clone()));
        }
        for (addr, comment) in side_comments {
            annotations.update(*addr, |e| e.user_side_comment = Some(comment.clone()));
        }
        let annotations_box: &'static crate::state::AnnotationManager =
            Box::leak(Box::new(annotations));

        DisassemblyContext {
            data,
            block_types,
            labels: &EMPTY_LABELS,
            origin: Addr(0xc000),
            settings: settings_box,
            annotations: annotations_box,
            cross_refs,
            collapsed_blocks: &[],
            splitters: &EMPTY_SPLITTERS,
            scope_ends: annotations_box.scope_ends(),
            enums: &EMPTY_ENUMS,
            user_global_enums: &EMPTY_ENUMS,
            builtin_enums: &EMPTY_ENUMS,
        }
    }

    fn empty_map<K, V>() -> BTreeMap<K, V> {
        BTreeMap::new()
    }

    #[test]
    fn test_fill_run_basic() {
        let data = vec![0x00u8; 10];
        let block_types = vec![BlockType::DataByte; 10];

        let m1 = empty_map();
        let m2 = empty_map();
        let m3 = empty_map();
        let ctx = make_fill_ctx(&data, &block_types, &m1, &m2, &m3, 8);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 1, "Expected a single .fill line");
        assert_eq!(lines[0].mnemonic, ".fill");
        assert_eq!(lines[0].operand, "10, $00");
        assert_eq!(lines[0].bytes.len(), 10);
    }

    #[test]
    fn test_fill_run_below_threshold() {
        let data = vec![0xFFu8; 5];
        let block_types = vec![BlockType::DataByte; 5];

        let m1 = empty_map();
        let m2 = empty_map();
        let m3 = empty_map();
        let ctx = make_fill_ctx(&data, &block_types, &m1, &m2, &m3, 8);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert!(lines.iter().all(|l| l.mnemonic == ".byte"));
    }

    #[test]
    fn test_fill_run_interrupted_by_xref() {
        let data = vec![0x00u8; 10];
        let block_types = vec![BlockType::DataByte; 10];
        let mut cross_refs: BTreeMap<Addr, Vec<Addr>> = BTreeMap::new();
        cross_refs.insert(Addr(0xc005), vec![Addr(0xd000)]);

        let m1 = empty_map();
        let m2 = empty_map();
        let ctx = make_fill_ctx(&data, &block_types, &cross_refs, &m1, &m2, 8);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert!(
            lines.iter().all(|l| l.mnemonic == ".byte"),
            "Cross-ref should prevent fill: got {:?}",
            lines.iter().map(|l| &l.mnemonic).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_fill_run_interrupted_by_line_comment() {
        let data = vec![0x00u8; 10];
        let block_types = vec![BlockType::DataByte; 10];
        let mut line_comments: BTreeMap<Addr, String> = BTreeMap::new();
        line_comments.insert(Addr(0xc005), "; padding end".to_string());

        let m1 = empty_map();
        let m2 = empty_map();
        let ctx = make_fill_ctx(&data, &block_types, &m1, &line_comments, &m2, 8);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert!(
            lines.iter().all(|l| l.mnemonic == ".byte"),
            "Line-comment should prevent fill: got {:?}",
            lines.iter().map(|l| &l.mnemonic).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_fill_run_interrupted_by_side_comment() {
        let data = vec![0x00u8; 10];
        let block_types = vec![BlockType::DataByte; 10];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc005), "note".to_string());

        let m1 = empty_map();
        let m2 = empty_map();
        let ctx = make_fill_ctx(&data, &block_types, &m1, &m2, &side_comments, 8);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert!(
            lines.iter().all(|l| l.mnemonic == ".byte"),
            "Side-comment should prevent fill: got {:?}",
            lines.iter().map(|l| &l.mnemonic).collect::<Vec<_>>()
        );
    }

    fn make_side_comment_ctx<'a>(
        data: &'a [u8],
        block_types: &'a [BlockType],
        side_comments: &'a BTreeMap<Addr, String>,
    ) -> DisassemblyContext<'a> {
        static EMPTY_XREFS: LazyLock<BTreeMap<Addr, Vec<Addr>>> = LazyLock::new(BTreeMap::new);
        static EMPTY_LINE: LazyLock<BTreeMap<Addr, String>> = LazyLock::new(BTreeMap::new);
        make_fill_ctx(
            data,
            block_types,
            &EMPTY_XREFS,
            &EMPTY_LINE,
            side_comments,
            0,
        )
    }

    #[test]
    fn test_side_comment_groups_data_bytes() {
        let data: Vec<u8> = (0x00..=0x07).collect();
        let block_types = vec![BlockType::DataByte; 8];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc000), "comment 0".to_string());
        side_comments.insert(Addr(0xc004), "comment 1".to_string());
        side_comments.insert(Addr(0xc005), "comment 2".to_string());

        let ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].bytes, vec![0x00, 0x01, 0x02, 0x03]);
        assert_eq!(lines[0].comment, "comment 0");
        assert_eq!(lines[1].bytes, vec![0x04]);
        assert_eq!(lines[1].comment, "comment 1");
        assert_eq!(lines[2].bytes, vec![0x05, 0x06, 0x07]);
        assert_eq!(lines[2].comment, "comment 2");
    }

    #[test]
    fn test_side_comment_on_last_byte() {
        let data = vec![0x0Au8, 0x0B, 0x0C];
        let block_types = vec![BlockType::DataByte; 3];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc002), "last".to_string());

        let ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].bytes, vec![0x0A, 0x0B]);
        assert!(lines[0].comment.is_empty());
        assert_eq!(lines[1].bytes, vec![0x0C]);
        assert_eq!(lines[1].comment, "last");
    }

    #[test]
    fn test_side_comment_every_byte_commented() {
        let data = vec![0xAAu8, 0xBB, 0xCC];
        let block_types = vec![BlockType::DataByte; 3];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc000), "a".to_string());
        side_comments.insert(Addr(0xc001), "b".to_string());
        side_comments.insert(Addr(0xc002), "c".to_string());

        let ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].bytes, vec![0xAA]);
        assert_eq!(lines[0].comment, "a");
        assert_eq!(lines[1].bytes, vec![0xBB]);
        assert_eq!(lines[1].comment, "b");
        assert_eq!(lines[2].bytes, vec![0xCC]);
        assert_eq!(lines[2].comment, "c");
    }

    #[test]
    fn test_side_comment_none_still_groups() {
        let data: Vec<u8> = (0..6).collect();
        let block_types = vec![BlockType::DataByte; 6];
        let side_comments: BTreeMap<Addr, String> = BTreeMap::new();

        let mut ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let settings = crate::state::DocumentSettings {
            bytes_per_line: 8,
            fill_run_threshold: 0,
            ..Default::default()
        };
        let settings_box: &'static crate::state::DocumentSettings = Box::leak(Box::new(settings));
        ctx.settings = settings_box;

        let lines = Disassembler::new().disassemble_ctx(&ctx);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].bytes.len(), 6);
        assert!(lines[0].comment.is_empty());
    }

    #[test]
    fn test_side_comment_data_word() {
        let data: Vec<u8> = (0..8).collect();
        let block_types = vec![BlockType::DataWord; 8];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc000), "comment w0".to_string());
        side_comments.insert(Addr(0xc004), "comment w2".to_string());

        let ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].bytes, vec![0x00, 0x01, 0x02, 0x03]);
        assert_eq!(lines[0].comment, "comment w0");
        assert_eq!(lines[1].bytes, vec![0x04, 0x05, 0x06, 0x07]);
        assert_eq!(lines[1].comment, "comment w2");
    }

    #[test]
    fn test_side_comment_data_word_every_word_commented() {
        let data = vec![0x00u8, 0x01, 0x02, 0x03];
        let block_types = vec![BlockType::DataWord; 4];
        let mut side_comments: BTreeMap<Addr, String> = BTreeMap::new();
        side_comments.insert(Addr(0xc000), "first".to_string());
        side_comments.insert(Addr(0xc002), "second".to_string());

        let ctx = make_side_comment_ctx(&data, &block_types, &side_comments);
        let lines = Disassembler::new().disassemble_ctx(&ctx);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].bytes, vec![0x00, 0x01]);
        assert_eq!(lines[0].comment, "first");
        assert_eq!(lines[1].bytes, vec![0x02, 0x03]);
        assert_eq!(lines[1].comment, "second");
    }
}
