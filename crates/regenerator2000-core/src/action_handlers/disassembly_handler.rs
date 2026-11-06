//! Disassembly domain action handler for block types, labels, comments, scopes, bookmarks, and analysis.

use super::{ActionContext, CoreError, DomainActionHandler};
use crate::event::CoreEvent;
use crate::state::Addr;
use crate::state::actions::AppAction;
use crate::state::types::CommentKind;
use crate::view_state::ActivePane;

/// Handler for disassembly manipulation actions.
#[derive(Debug, Default)]
pub struct DisassemblyActionHandler;

impl DisassemblyActionHandler {
    /// Creates a new [`DisassemblyActionHandler`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Returns the half-open binary byte offset range `start..end` covered by `line`.
///
/// Returns `None` for external equates, preamble headers, splitters, and scope delimiters.
fn line_byte_offset_range(
    state: &crate::state::AppState,
    line: &crate::disassembler::DisassemblyLine,
) -> Option<std::ops::Range<usize>> {
    if line.external_label_address.is_some() || state.is_external(line.address) {
        return None;
    }
    let start_offset = line.address.offset_from(state.origin);
    if start_offset >= state.raw_data.len() {
        return None;
    }
    if line.is_collapsed {
        if let Some(&(_, end_offset)) = state
            .collapsed_blocks
            .iter()
            .find(|&&(s, _)| s == start_offset)
        {
            let end_excl = (end_offset + 1).min(state.raw_data.len());
            return Some(start_offset..end_excl);
        }
        if let Some((start_addr, end_addr)) = state.get_block_range(line.address) {
            let s = start_addr.offset_from(state.origin);
            let e = (end_addr.offset_from(state.origin) + 1).min(state.raw_data.len());
            return Some(s..e);
        }
    }
    if line.bytes.is_empty() {
        return None;
    }
    let end_excl = (start_offset + line.bytes.len()).min(state.raw_data.len());
    Some(start_offset..end_excl)
}

/// Applies `block_cmd`, records the pre-command analysis snapshot, rebuilds disassembly, and pushes a batch command.
fn apply_block_command_with_analysis(
    state: &mut crate::state::AppState,
    block_cmd: crate::commands::Command,
) {
    let old_labels = state.labels.clone();
    let old_cross_refs = state.cross_refs.clone();
    block_cmd.apply(state);
    let analysis_cmd = crate::commands::Command::SetAnalysisData {
        labels: state.labels.clone(),
        cross_refs: state.cross_refs.clone(),
        old_labels,
        old_cross_refs,
    };
    state.disassemble();
    state.push_command(crate::commands::Command::Batch(vec![
        analysis_cmd,
        block_cmd,
    ]));
}

/// Applies `new_type` to the active line, range selection, or selected block in `Blocks` view.
fn apply_block_type(ctx: &mut ActionContext<'_>, new_type: crate::state::BlockType) {
    let selected_range = match ctx.view.active_pane {
        ActivePane::Disassembly => {
            // Support both persistent visual mode (`Shift+V`) and `Shift+Arrow` range selection.
            if ctx.view.is_visual_mode || ctx.view.selection_start.is_some() {
                let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
                let end_idx = ctx.view.cursor_index;
                let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));

                ctx.state
                    .disassembly
                    .get(min_idx..=max_idx)
                    .and_then(|slice| {
                        let first = slice
                            .iter()
                            .find_map(|l| line_byte_offset_range(ctx.state, l))?;
                        let last = slice
                            .iter()
                            .rev()
                            .find_map(|l| line_byte_offset_range(ctx.state, l))?;
                        (first.start < last.end).then_some(first.start..last.end)
                    })
            } else {
                ctx.state
                    .disassembly
                    .get(ctx.view.cursor_index)
                    .and_then(|line| line_byte_offset_range(ctx.state, line))
            }
        }
        ActivePane::Blocks => {
            let blocks = ctx.state.get_blocks_view_items();
            let idx = ctx.view.blocks_selected_index.unwrap_or(0);
            if let Some(crate::state::BlockItem::Block { start, end, .. }) = blocks.get(idx)
                && !ctx.state.is_external(*start)
                && !ctx.state.is_external(*end)
            {
                let start_offset = start.offset_from(ctx.state.origin);
                let end_offset = end.offset_from(ctx.state.origin);
                (start_offset <= end_offset && end_offset < ctx.state.raw_data.len())
                    .then_some(start_offset..(end_offset + 1))
            } else {
                None
            }
        }
        _ => None,
    };

    let Some(range) = selected_range else {
        return;
    };

    if (new_type == crate::state::BlockType::LoHiAddress
        || new_type == crate::state::BlockType::HiLoAddress
        || new_type == crate::state::BlockType::LoHiWord
        || new_type == crate::state::BlockType::HiLoWord)
        && range.len() % 2 != 0
    {
        ctx.events.push(CoreEvent::StatusMessage(
            "Address/Word block requires even number of bytes".to_string(),
        ));
        return;
    }

    let old_types = ctx.state.block_types[range.clone()].to_vec();
    let cmd1 = crate::commands::Command::SetBlockType {
        range,
        new_type,
        old_types,
    };

    ctx.preserve_cursor(|c| {
        apply_block_command_with_analysis(c.state, cmd1);
        // Clear both visual mode and Shift+Arrow range selection after conversion.
        c.view.selection_start = None;
        c.view.is_visual_mode = false;
    });

    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Set block type to {new_type:?}"
    )));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Returns `true` if `line` represents in-range binary content that can anchor a lexical scope.
fn is_scope_content_line(
    state: &crate::state::AppState,
    line: &crate::disassembler::DisassemblyLine,
) -> bool {
    line.external_label_address.is_none()
        && !state.is_external(line.address)
        && (!line.bytes.is_empty() || line.is_collapsed)
}

/// Creates a lexical scope for the active selection or the subroutine at the cursor.
fn handle_add_scope(ctx: &mut ActionContext<'_>) {
    // Support both persistent visual mode (`Shift+V`) and `Shift+Arrow` range selection.
    let scope_range = if ctx.view.is_visual_mode || ctx.view.selection_start.is_some() {
        let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
        let end_idx = ctx.view.cursor_index;
        let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));

        ctx.state
            .disassembly
            .get(min_idx..=max_idx)
            .and_then(|slice| {
                let start = slice
                    .iter()
                    .find(|l| is_scope_content_line(ctx.state, l))
                    .map(|l| l.address)?;
                let end = slice
                    .iter()
                    .rev()
                    .find(|l| is_scope_content_line(ctx.state, l))
                    .map(|l| l.address)?;
                (start <= end).then_some((start, end))
            })
    } else if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        if !is_scope_content_line(ctx.state, line) {
            return;
        }
        let start_addr = line.address;
        let in_existing_scope = ctx
            .state
            .annotations
            .scopes()
            .iter()
            .any(|(&s, &e)| start_addr >= s && start_addr <= e);

        if in_existing_scope {
            ctx.events.push(CoreEvent::StatusMessage(
                "Scope already exists at target position".to_string(),
            ));
            return;
        }

        let mut end_addr = start_addr;
        for line in &ctx.state.disassembly[ctx.view.cursor_index..] {
            if line.address > start_addr
                && let Some(op) = &line.opcode
                && matches!(op.mnemonic, "RTS" | "RTI" | "JMP")
            {
                end_addr = line.address;
                break;
            }
        }
        Some((start_addr, end_addr))
    } else {
        None
    };

    if let Some((start_addr, end_addr)) = scope_range {
        if start_addr > end_addr
            || ctx.state.is_external(start_addr)
            || ctx.state.is_external(end_addr)
        {
            return;
        }

        let overlaps = ctx
            .state
            .annotations
            .scopes()
            .iter()
            .any(|(&s, &e)| !(end_addr < s || start_addr > e));

        if overlaps {
            ctx.events.push(CoreEvent::StatusMessage(
                "New scope overlaps with an existing scope".to_string(),
            ));
            return;
        }

        let has_label = ctx
            .state
            .labels
            .get(&start_addr)
            .is_some_and(|v| !v.is_empty());
        let mut commands = Vec::new();

        if !has_label {
            let default_name = format!("scope_{:04X}", start_addr.0);
            if let Ok(cmd_label) =
                ctx.state
                    .create_set_user_label_command(start_addr, &default_name, false)
            {
                commands.push(cmd_label);
            }
        }

        let old_scope_end = ctx.state.annotations.get(start_addr).and_then(|e| e.scope);
        commands.push(crate::commands::Command::AddScope {
            start: start_addr,
            end: end_addr,
            old_end: old_scope_end,
        });

        let batch = crate::commands::Command::Batch(commands);
        ctx.preserve_cursor(|c| {
            batch.apply(c.state);
            let (analysis_cmd, _) = c.state.perform_analysis();
            let final_cmd = crate::commands::Command::Batch(vec![batch, analysis_cmd]);
            c.state.push_command(final_cmd);
            c.view.selection_start = None;
            c.view.is_visual_mode = false;
        });

        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Added scope from ${:04X} to ${:04X}",
            start_addr.0, end_addr.0
        )));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    }
}

/// Expands or shrinks the end boundary of the active scope by one disassembly content line.
fn handle_nudge_scope_boundary(ctx: &mut ActionContext<'_>, expand: bool) {
    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        return;
    };
    let current_addr = line.address;
    let active_scope = ctx
        .state
        .annotations
        .scopes()
        .iter()
        .find(|&(&s, &e)| current_addr >= s && current_addr <= e)
        .map(|(&s, &e)| (s, e));

    let Some((start_addr, end_addr)) = active_scope else {
        ctx.events.push(CoreEvent::StatusMessage(
            "Cursor is not inside an active scope".to_string(),
        ));
        return;
    };

    let Some(end_idx) = ctx
        .state
        .get_line_index_containing_address(end_addr)
        .or_else(|| ctx.state.get_line_index_for_address(end_addr))
    else {
        return;
    };

    let target_line_idx = if expand {
        (end_idx + 1..ctx.state.disassembly.len())
            .find(|&i| !ctx.state.disassembly[i].bytes.is_empty())
    } else {
        (0..end_idx)
            .rev()
            .find(|&i| !ctx.state.disassembly[i].bytes.is_empty())
    };
    let Some(target_line_idx) = target_line_idx else {
        return;
    };

    let new_end_addr = ctx.state.disassembly[target_line_idx].address;
    if new_end_addr < start_addr || new_end_addr == end_addr {
        return;
    }

    let other_overlap = ctx
        .state
        .annotations
        .scopes()
        .iter()
        .any(|(&s, &e)| s != start_addr && !(new_end_addr < s || start_addr > e));

    if other_overlap {
        ctx.events.push(CoreEvent::StatusMessage(
            "Cannot expand scope: overlaps with adjacent scope".to_string(),
        ));
        return;
    }

    let cmd_scope = crate::commands::Command::AddScope {
        start: start_addr,
        end: new_end_addr,
        old_end: Some(end_addr),
    };

    ctx.preserve_cursor(|c| {
        cmd_scope.apply(c.state);
        let (analysis_cmd, _) = c.state.perform_analysis();
        let final_cmd = crate::commands::Command::Batch(vec![cmd_scope, analysis_cmd]);
        c.state.push_command(final_cmd);
    });

    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Scope end updated to ${:04X}",
        new_end_addr.0
    )));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Removes the lexical scope that contains the current cursor address.
fn handle_remove_scope(ctx: &mut ActionContext<'_>) {
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let current_addr = line.address;
        let active_scope = ctx
            .state
            .annotations
            .scopes()
            .iter()
            .find(|&(&s, &e)| current_addr >= s && current_addr <= e)
            .map(|(&s, &e)| (s, e));

        if let Some((start_addr, end_addr)) = active_scope {
            let cmd_scope = crate::commands::Command::RemoveScope {
                address: start_addr,
                old_end: end_addr,
            };

            ctx.preserve_cursor(|c| {
                cmd_scope.apply(c.state);
                let (analysis_cmd, _) = c.state.perform_analysis();
                let final_cmd = crate::commands::Command::Batch(vec![cmd_scope, analysis_cmd]);
                c.state.push_command(final_cmd);
            });

            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Removed scope at ${:04X}",
                start_addr.0
            )));
            ctx.events.push(CoreEvent::StateChanged);
            ctx.events.push(CoreEvent::ViewChanged);
        } else {
            ctx.events.push(CoreEvent::StatusMessage(
                "No scope to remove at cursor".to_string(),
            ));
        }
    }
}

/// Applies or clears a user label at `address` and emits status and state events.
fn handle_apply_label(ctx: &mut ActionContext<'_>, address: Addr, name: &str, is_local: bool) {
    let trimmed = name.trim();
    match ctx
        .state
        .create_set_user_label_command(address, trimmed, is_local)
    {
        Ok(command) => {
            ctx.preserve_cursor(|c| {
                command.apply(c.state);
                c.state.push_command(command);
                c.state.disassemble();
            });
            let msg = if trimmed.is_empty() {
                format!("Label cleared at ${address:04X}")
            } else {
                format!("Label '{name}' applied to ${address:04X}")
            };
            ctx.events.push(CoreEvent::StatusMessage(msg));
            ctx.events.push(CoreEvent::StateChanged);
            ctx.events.push(CoreEvent::ViewChanged);
        }
        Err(err) => {
            ctx.events.push(CoreEvent::StatusMessage(err));
        }
    }
}

/// Applies or clears an enum operand annotation at `address`.
fn handle_apply_enum_usage(ctx: &mut ActionContext<'_>, address: Addr, enum_name: Option<&str>) {
    let old_enum = ctx
        .state
        .annotations
        .get(address)
        .and_then(|e| e.enum_usage.clone());
    let command = crate::commands::Command::SetEnumUsage {
        address,
        new_enum: enum_name.map(String::from),
        old_enum,
    };
    ctx.preserve_cursor(|c| {
        command.apply(c.state);
        c.state.push_command(command);
        c.state.disassemble();
    });
    let msg = match enum_name {
        Some(name) => format!("Applied enum '{name}' at ${address:04X}"),
        None => format!("Cleared enum usage at ${address:04X}"),
    };
    ctx.events.push(CoreEvent::StatusMessage(msg));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Applies a user side comment or line comment at `address`.
fn handle_apply_comment(
    ctx: &mut ActionContext<'_>,
    address: Addr,
    text: String,
    kind: CommentKind,
) {
    let command = ctx
        .state
        .create_set_comment_command(address, Some(&text), kind);
    ctx.preserve_cursor(|c| {
        command.apply(c.state);
        c.state.push_command(command);
        c.state.disassemble();
    });
    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Comment updated at ${address:04X}"
    )));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Opens the label editor dialog for the 6502 [`Addr`] at `(cursor_index, sub_cursor_index)`.
///
/// Uses [`DisassemblyLine::address_at_sub_index`](crate::disassembler::DisassemblyLine::address_at_sub_index)
/// so mid-instruction label rows (`* + offset`) edit the intermediate byte label.
fn handle_set_label(ctx: &mut ActionContext<'_>) {
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let is_external = line.external_label_address.is_some();
        let target_addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);

        if is_external {
            let current = ctx
                .state
                .labels
                .get(&target_addr)
                .and_then(|v| v.first().map(|l| l.name.clone()));
            ctx.events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Label {
                    address: target_addr,
                    initial_name: current.unwrap_or_default(),
                    is_external: true,
                },
            ));
            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Edit External Label for ${target_addr:04X}"
            )));
        } else {
            let current = ctx
                .state
                .labels
                .get(&target_addr)
                .and_then(|v| v.iter().find(|l| l.kind == crate::state::LabelKind::User))
                .map(|l| l.name.clone());
            ctx.events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Label {
                    address: target_addr,
                    initial_name: current.unwrap_or_default(),
                    is_external: false,
                },
            ));
            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Set User Label at ${target_addr:04X}"
            )));
        }
    }
}

/// Returns the immediate operand byte if `line` is an immediate `LDA`, `LDX`, or `LDY` instruction.
fn extract_immediate_load_byte(line: &crate::disassembler::DisassemblyLine) -> Option<u8> {
    if let Some(op) = &line.opcode
        && op.mode == crate::cpu::AddressingMode::Immediate
        && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
    {
        line.bytes.get(1).copied()
    } else {
        None
    }
}

/// Pairs immediate load instructions into 16-bit low/high or high/low address operands.
fn handle_lo_hi_packing(ctx: &mut ActionContext<'_>, lo_first: bool) {
    let mut indices = Vec::new();
    if let Some(start) = ctx.view.selection_start {
        let end = ctx.view.cursor_index;
        let (low, high) = (start.min(end), start.max(end));
        indices.extend(low..=high);
    } else {
        indices.push(ctx.view.cursor_index);
    }

    if indices.len() == 1 {
        let idx = indices[0];
        if idx + 1 < ctx.state.disassembly.len() {
            indices.push(idx + 1);
        }
    }

    let mut batch_commands = Vec::new();
    let mut i = 0;
    let mut last_target = 0;

    while i < indices.len() {
        let idx1 = indices[i];
        let val1 = ctx
            .state
            .disassembly
            .get(idx1)
            .and_then(extract_immediate_load_byte);

        if let Some(v1) = val1 {
            let mut j = i + 1;
            let mut found_match = None;
            while j < indices.len() {
                let idx2 = indices[j];
                if let Some(v2) = ctx
                    .state
                    .disassembly
                    .get(idx2)
                    .and_then(extract_immediate_load_byte)
                {
                    found_match = Some((j, idx2, v2));
                    break;
                }
                j += 1;
            }

            if let Some((match_idx, idx2, v2)) = found_match {
                let (lo, hi) = if lo_first { (v1, v2) } else { (v2, v1) };
                let target = (u16::from(hi) << 8) | u16::from(lo);
                last_target = target;

                let addr1 = ctx.state.disassembly[idx1].address;
                let addr2 = ctx.state.disassembly[idx2].address;

                let fmt1 = if lo_first {
                    crate::state::ImmediateFormat::LowByte(crate::state::Addr(target))
                } else {
                    crate::state::ImmediateFormat::HighByte(crate::state::Addr(target))
                };
                let fmt2 = if lo_first {
                    crate::state::ImmediateFormat::HighByte(crate::state::Addr(target))
                } else {
                    crate::state::ImmediateFormat::LowByte(crate::state::Addr(target))
                };

                batch_commands.push(crate::commands::Command::SetImmediateFormat {
                    address: addr1,
                    new_format: Some(fmt1),
                    old_format: ctx
                        .state
                        .annotations
                        .get(addr1)
                        .and_then(|e| e.immediate_format),
                });
                batch_commands.push(crate::commands::Command::SetImmediateFormat {
                    address: addr2,
                    new_format: Some(fmt2),
                    old_format: ctx
                        .state
                        .annotations
                        .get(addr2)
                        .and_then(|e| e.immediate_format),
                });

                i = match_idx + 1;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    if !batch_commands.is_empty() {
        ctx.preserve_cursor(|c| {
            let batch = crate::commands::Command::Batch(batch_commands);
            batch.apply(c.state);
            let (analysis_cmd, _) = c.state.perform_analysis();
            let final_cmd = crate::commands::Command::Batch(vec![batch, analysis_cmd]);
            c.state.push_command(final_cmd);

            c.view.selection_start = None;
            c.view.is_visual_mode = false;
        });

        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Packed Lo/Hi address for ${last_target:04X}"
        )));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    } else if ctx.view.selection_start.is_none() {
        let idx = ctx.view.cursor_index;
        if let Some(line) = ctx.state.disassembly.get(idx)
            && let Some(known_byte) = extract_immediate_load_byte(line)
        {
            ctx.events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::CompleteAddress {
                    known_byte,
                    lo_first,
                    address: line.address,
                },
            ));
        } else {
            ctx.events
                .push(CoreEvent::StatusMessage("No Lo/Hi pairs found".to_string()));
        }
    } else {
        ctx.events
            .push(CoreEvent::StatusMessage("No Lo/Hi pairs found".to_string()));
    }
}

/// Cycles the immediate operand display format at the current disassembly line.
fn cycle_immediate_format(ctx: &mut ActionContext<'_>, forward: bool) {
    use crate::state::types::ImmediateFormat;
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let address = line.address;

        let formats = [
            ImmediateFormat::Hex,
            ImmediateFormat::InvertedHex,
            ImmediateFormat::Decimal,
            ImmediateFormat::NegativeDecimal,
            ImmediateFormat::Binary,
            ImmediateFormat::InvertedBinary,
        ];

        let current_fmt = ctx
            .state
            .annotations
            .get(address)
            .and_then(|e| e.immediate_format)
            .unwrap_or(ImmediateFormat::Hex);

        let current_idx = formats.iter().position(|&f| f == current_fmt).unwrap_or(0);

        let new_idx = if forward {
            (current_idx + 1) % formats.len()
        } else {
            (current_idx + formats.len() - 1) % formats.len()
        };

        let new_fmt = formats[new_idx];
        let command = crate::commands::Command::SetImmediateFormat {
            address,
            new_format: Some(new_fmt),
            old_format: ctx
                .state
                .annotations
                .get(address)
                .and_then(|e| e.immediate_format),
        };

        ctx.preserve_cursor(|c| {
            command.apply(c.state);
            c.state.push_command(command);
            c.state.disassemble();
        });

        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Immediate format: {new_fmt:?}"
        )));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    }
}

/// Toggles the collapsed state of the target block in `Blocks` view or `Disassembly` view.
fn toggle_collapsed_block(ctx: &mut ActionContext<'_>) {
    // In the Blocks pane (`Ctrl+K`), resolve the block from `blocks_selected_index`.
    // This targets the highlighted block even when `sync_blocks_view` is disabled.
    let block_range = if ctx.view.active_pane == ActivePane::Blocks {
        let blocks = ctx.state.get_blocks_view_items();
        let idx = ctx.view.blocks_selected_index.unwrap_or(0);
        if let Some(crate::state::BlockItem::Block { start, end, .. }) = blocks.get(idx) {
            Some((*start, *end))
        } else {
            None
        }
    } else {
        ctx.state
            .disassembly
            .get(ctx.view.cursor_index)
            .and_then(|line| ctx.state.get_block_range(line.address))
    };

    if let Some((start_addr, end_addr)) = block_range {
        let origin = ctx.state.origin;
        let start = start_addr.offset_from(origin);
        let end = end_addr.offset_from(origin);
        let range = (start, end);
        let is_collapsed = ctx.state.collapsed_blocks.contains(&range);
        let command = if is_collapsed {
            crate::commands::Command::UncollapseBlock { range }
        } else {
            crate::commands::Command::CollapseBlock { range }
        };
        ctx.preserve_cursor(|c| {
            command.apply(c.state);
            c.state.push_command(command);
            c.state.disassemble();
        });
        let action_str = if is_collapsed {
            "Expanded"
        } else {
            "Collapsed"
        };
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "{action_str} block ${:04X}-${:04X}",
            start_addr.0, end_addr.0
        )));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    }
}

impl DomainActionHandler for DisassemblyActionHandler {
    fn handle_action(
        &self,
        action: &AppAction,
        ctx: &mut ActionContext<'_>,
    ) -> Result<bool, CoreError> {
        match action {
            AppAction::Analyze => {
                let mut msg = String::new();
                ctx.preserve_cursor(|c| {
                    let (cmd, m) = c.state.perform_analysis();
                    c.state.push_command(cmd);
                    msg = m;
                });
                ctx.events.push(CoreEvent::StatusMessage(msg));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::Undo => {
                let mut msg = String::new();
                ctx.preserve_cursor(|c| {
                    msg = c.state.undo_last_command();
                });
                ctx.events.push(CoreEvent::StatusMessage(msg));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::Redo => {
                let mut msg = String::new();
                ctx.preserve_cursor(|c| {
                    msg = c.state.redo_last_command();
                });
                ctx.events.push(CoreEvent::StatusMessage(msg));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::Code => {
                apply_block_type(ctx, crate::state::BlockType::Code);
                Ok(true)
            }
            AppAction::Byte => {
                apply_block_type(ctx, crate::state::BlockType::DataByte);
                Ok(true)
            }
            AppAction::Word => {
                apply_block_type(ctx, crate::state::BlockType::DataWord);
                Ok(true)
            }
            AppAction::Address => {
                apply_block_type(ctx, crate::state::BlockType::Address);
                Ok(true)
            }
            AppAction::PetsciiText => {
                apply_block_type(ctx, crate::state::BlockType::PetsciiText);
                Ok(true)
            }
            AppAction::ScreencodeText => {
                apply_block_type(ctx, crate::state::BlockType::ScreencodeText);
                Ok(true)
            }
            AppAction::Undefined => {
                apply_block_type(ctx, crate::state::BlockType::Undefined);
                Ok(true)
            }
            AppAction::SetLoHiAddress => {
                apply_block_type(ctx, crate::state::BlockType::LoHiAddress);
                Ok(true)
            }
            AppAction::SetHiLoAddress => {
                apply_block_type(ctx, crate::state::BlockType::HiLoAddress);
                Ok(true)
            }
            AppAction::SetLoHiWord => {
                apply_block_type(ctx, crate::state::BlockType::LoHiWord);
                Ok(true)
            }
            AppAction::SetHiLoWord => {
                apply_block_type(ctx, crate::state::BlockType::HiLoWord);
                Ok(true)
            }
            AppAction::SetExternalFile => {
                apply_block_type(ctx, crate::state::BlockType::ExternalFile);
                Ok(true)
            }
            AppAction::DisassembleAddress => {
                let addr = if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    line.address
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Invalid cursor position".to_string(),
                    ));
                    return Ok(true);
                };
                let ranges = crate::analyzer::flow_analyze(ctx.state, addr);
                let mut commands = Vec::new();
                for range in ranges {
                    let old_types = ctx.state.block_types[range.start..range.end].to_vec();
                    commands.push(crate::commands::Command::SetBlockType {
                        range: range.clone(),
                        new_type: crate::state::BlockType::Code,
                        old_types,
                    });
                }
                if !commands.is_empty() {
                    let batch = crate::commands::Command::Batch(commands);
                    ctx.preserve_cursor(|c| {
                        apply_block_command_with_analysis(c.state, batch);
                    });

                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Flow analyzed from ${:04X}",
                        addr.0
                    )));
                    ctx.events.push(CoreEvent::StateChanged);
                    ctx.events.push(CoreEvent::ViewChanged);
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "No new code found from ${:04X}",
                        addr.0
                    )));
                }
                Ok(true)
            }
            AppAction::SetBytesBlockByOffset { start, end } => {
                if start <= end
                    && *end < ctx.state.block_types.len()
                    && let Some(end_plus_one) = end.checked_add(1)
                {
                    let range = *start..end_plus_one;
                    let old_types = ctx.state.block_types[range.clone()].to_vec();
                    let cmd1 = crate::commands::Command::SetBlockType {
                        range,
                        new_type: crate::state::BlockType::DataByte,
                        old_types,
                    };
                    ctx.preserve_cursor(|c| {
                        apply_block_command_with_analysis(c.state, cmd1);
                    });

                    let origin = ctx.state.origin.0 as usize;
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Converted to bytes from ${:04X} to ${:04X}",
                        origin + start,
                        origin + end
                    )));
                    ctx.events.push(CoreEvent::StateChanged);
                    ctx.events.push(CoreEvent::ViewChanged);
                } else {
                    ctx.events.push(CoreEvent::StatusMessage(
                        "Invalid byte range for conversion".to_string(),
                    ));
                }
                Ok(true)
            }
            AppAction::Scope => {
                handle_add_scope(ctx);
                Ok(true)
            }
            AppAction::NudgeScopeBoundary { expand } => {
                handle_nudge_scope_boundary(ctx, *expand);
                Ok(true)
            }
            AppAction::RemoveScope => {
                handle_remove_scope(ctx);
                Ok(true)
            }
            AppAction::SetLabel => {
                handle_set_label(ctx);
                Ok(true)
            }
            AppAction::ApplyLabel {
                address,
                name,
                is_local,
            } => {
                handle_apply_label(ctx, *address, name, *is_local);
                Ok(true)
            }
            AppAction::PackLoHiAddress => {
                handle_lo_hi_packing(ctx, true);
                Ok(true)
            }
            AppAction::PackHiLoAddress => {
                handle_lo_hi_packing(ctx, false);
                Ok(true)
            }
            AppAction::SideComment => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    let address = line.address;
                    let current = ctx
                        .state
                        .annotations
                        .get(address)
                        .and_then(|e| e.user_side_comment.clone());
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::Comment {
                            address,
                            current,
                            kind: CommentKind::Side,
                        },
                    ));
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Edit Side Comment at ${address:04X}"
                    )));
                }
                Ok(true)
            }
            AppAction::LineComment => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    let address = line.address;
                    let current = ctx
                        .state
                        .annotations
                        .get(address)
                        .and_then(|e| e.user_line_comment.clone());
                    ctx.events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::Comment {
                            address,
                            current,
                            kind: CommentKind::Line,
                        },
                    ));
                    ctx.events.push(CoreEvent::StatusMessage(format!(
                        "Edit Line Comment at ${address:04X}"
                    )));
                }
                Ok(true)
            }
            AppAction::ApplyComment {
                address,
                text,
                kind,
            } => {
                handle_apply_comment(ctx, *address, text.clone(), *kind);
                Ok(true)
            }
            AppAction::ApplyEnumUsage { address, enum_name } => {
                handle_apply_enum_usage(ctx, *address, enum_name.as_deref());
                Ok(true)
            }
            AppAction::ApplyEnumDialog => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    let offset = line.address.0 as isize - ctx.state.origin.0 as isize;
                    if offset >= 0 && (offset as usize) < ctx.state.block_types.len() {
                        let current_type = ctx.state.block_types[offset as usize];
                        let value = match current_type {
                            crate::state::BlockType::DataWord => {
                                if offset + 1 < ctx.state.raw_data.len() as isize {
                                    let low = ctx.state.raw_data[offset as usize];
                                    let high = ctx.state.raw_data[offset as usize + 1];
                                    u16::from(high) << 8 | u16::from(low)
                                } else {
                                    u16::from(ctx.state.raw_data[offset as usize])
                                }
                            }
                            _ => {
                                if let Some(opcode) = &line.opcode {
                                    if opcode.mode == crate::cpu::AddressingMode::Immediate
                                        && line.bytes.len() >= 2
                                    {
                                        u16::from(line.bytes[1])
                                    } else {
                                        u16::from(ctx.state.raw_data[offset as usize])
                                    }
                                } else {
                                    u16::from(ctx.state.raw_data[offset as usize])
                                }
                            }
                        };
                        ctx.events.push(CoreEvent::DialogRequested(
                            crate::event::DialogType::ApplyEnum {
                                address: line.address,
                                value,
                            },
                        ));
                        ctx.events
                            .push(CoreEvent::StatusMessage("Apply Enum".to_string()));
                    }
                }
                Ok(true)
            }
            AppAction::ManageEnums => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ManageEnums,
                ));
                ctx.events
                    .push(CoreEvent::StatusMessage("Manage Enums".to_string()));
                Ok(true)
            }
            AppAction::ApplyEnumDefinition {
                name,
                definition,
                rename_from,
            } => {
                ctx.preserve_cursor(|c| {
                    let command = if let Some(old_name) = &rename_from
                        && old_name != name
                    {
                        let old_definition = c.state.enums.get(old_name).cloned();
                        let cmd_remove = crate::commands::Command::SetEnumDefinition {
                            name: old_name.clone(),
                            new_definition: None,
                            old_definition,
                        };
                        let cmd_add = crate::commands::Command::SetEnumDefinition {
                            name: name.clone(),
                            new_definition: definition.clone(),
                            old_definition: None,
                        };
                        crate::commands::Command::Batch(vec![cmd_remove, cmd_add])
                    } else {
                        let old_definition = c.state.enums.get(name).cloned();
                        crate::commands::Command::SetEnumDefinition {
                            name: name.clone(),
                            new_definition: definition.clone(),
                            old_definition,
                        }
                    };
                    command.apply(c.state);
                    c.state.push_command(command);
                    c.state.disassemble();
                });
                ctx.events
                    .push(CoreEvent::StatusMessage(if definition.is_none() {
                        format!("Deleted project enum '{name}'")
                    } else {
                        format!("Saved project enum '{name}'")
                    }));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                ctx.events.push(CoreEvent::DialogDismissalRequested);
                Ok(true)
            }
            AppAction::ApplyGlobalEnumDefinition {
                name,
                definition,
                rename_from,
            } => {
                let mut status_msg = String::new();
                ctx.preserve_cursor(|c| {
                    let old_name_ref = rename_from.as_ref().unwrap_or(name);
                    let old_def = c.state.user_global_enums.get(old_name_ref).cloned();

                    if let Some(old_name) = &rename_from
                        && old_name != name
                    {
                        let _ = crate::assets::delete_global_enum(old_name, old_def.as_ref());
                        c.state.user_global_enums.remove(old_name);
                    }

                    if let Some(mut def) = definition.clone() {
                        match crate::assets::save_global_enum(&mut def) {
                            Ok(_) => {
                                c.state.user_global_enums.insert(name.clone(), def);
                                status_msg = format!("Saved global enum '{name}'");
                            }
                            Err(e) => {
                                status_msg = format!("Failed to save global enum '{name}': {e}");
                            }
                        }
                    } else {
                        match crate::assets::delete_global_enum(name, old_def.as_ref()) {
                            Ok(_) => {
                                c.state.user_global_enums.remove(name);
                                status_msg = format!("Deleted global enum '{name}'");
                            }
                            Err(e) => {
                                status_msg = format!("Failed to delete global enum '{name}': {e}");
                            }
                        }
                    }
                    c.state.disassemble();
                });
                ctx.events.push(CoreEvent::StatusMessage(status_msg));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                ctx.events.push(CoreEvent::DialogDismissalRequested);
                Ok(true)
            }
            AppAction::ToggleBookmark => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                    if line.external_label_address.is_none() {
                        // Resolve `sub_cursor_index` via `address_at_sub_index`.
                        // Mid-instruction label rows (`* + offset`) bookmark that intermediate byte.
                        let address =
                            line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
                        let is_bookmarked = ctx
                            .state
                            .annotations
                            .get(address)
                            .and_then(|e| e.bookmark.as_ref())
                            .is_some();

                        // Use a non-empty default bookmark name (`"Bookmark"`).
                        // `AnnotationManager` normalizes whitespace and removes empty strings.
                        let command = crate::commands::Command::SetBookmark {
                            address,
                            new_name: if is_bookmarked {
                                None
                            } else {
                                Some("Bookmark".to_string())
                            },
                            old_name: ctx
                                .state
                                .annotations
                                .get(address)
                                .and_then(|e| e.bookmark.clone()),
                        };
                        command.apply(ctx.state);
                        ctx.state.push_command(command);

                        let msg = if is_bookmarked {
                            format!("Bookmark removed at ${address:04X}")
                        } else {
                            format!("Bookmark set at ${address:04X}")
                        };
                        ctx.events.push(CoreEvent::StatusMessage(msg));
                        ctx.events.push(CoreEvent::StateChanged);
                    } else {
                        ctx.events.push(CoreEvent::StatusMessage(
                            "Cannot bookmark external label definition".to_string(),
                        ));
                    }
                }
                Ok(true)
            }
            AppAction::ListBookmarks => {
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Bookmarks,
                ));
                Ok(true)
            }
            AppAction::ExcludeExternalAddress => {
                if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index)
                    && let Some(ext_addr) = line.external_label_address
                {
                    if ctx.state.user_excluded_addresses.contains(&ext_addr) {
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "${ext_addr:04X} is already excluded"
                        )));
                    } else {
                        let old_labels = ctx.state.labels.clone();
                        let old_cross_refs = ctx.state.cross_refs.clone();
                        let command = crate::commands::Command::SetUserExcludedAddress {
                            address: ext_addr,
                            add: true,
                            old_labels,
                            old_cross_refs,
                        };
                        ctx.preserve_cursor(|c| {
                            command.apply(c.state);
                            c.state.push_command(command);
                        });
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Excluded ${ext_addr:04X} from analysis"
                        )));
                        ctx.events.push(CoreEvent::StateChanged);
                        ctx.events.push(CoreEvent::ViewChanged);
                    }
                }
                Ok(true)
            }
            AppAction::NextImmediateFormat => {
                cycle_immediate_format(ctx, true);
                Ok(true)
            }
            AppAction::PreviousImmediateFormat => {
                cycle_immediate_format(ctx, false);
                Ok(true)
            }
            AppAction::ChangeOrigin => {
                ctx.events
                    .push(CoreEvent::DialogRequested(crate::event::DialogType::Origin));
                ctx.events
                    .push(CoreEvent::StatusMessage("Change Origin...".to_string()));
                Ok(true)
            }
            AppAction::ApplyOrigin(new_origin) => {
                let old_origin = ctx.state.origin;
                let cmd_origin = crate::commands::Command::ChangeOrigin {
                    new_origin: *new_origin,
                    old_origin,
                };
                let saved_index = ctx.view.cursor_index;
                cmd_origin.apply(ctx.state);
                let (cmd_analysis, _) = ctx.state.perform_analysis();
                ctx.state.push_command(crate::commands::Command::Batch(vec![
                    cmd_origin,
                    cmd_analysis,
                ]));
                let max_idx = ctx.state.disassembly.len().saturating_sub(1);
                ctx.view.cursor_index = saved_index.min(max_idx);
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Origin changed to ${:04X}",
                    new_origin.0
                )));
                ctx.events.push(CoreEvent::StateChanged);
                ctx.events.push(CoreEvent::ViewChanged);
                Ok(true)
            }
            AppAction::ToggleCollapsedBlock => {
                toggle_collapsed_block(ctx);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Core;
    use crate::state::actions::AppAction;
    use crate::state::types::Addr;

    #[test]
    fn test_pack_lo_hi_address_multi_line_selection() {
        let mut core = Core::new();
        // LDA #$88 ($A9, $88), NOP ($EA), LDA #$16 ($A9, $16)
        let prg = vec![0xA9, 0x88, 0xEA, 0xA9, 0x16];
        let _ = core.state.load_binary(Addr(0x1000), prg);
        core.state.block_types = vec![crate::state::BlockType::Code; 5];
        core.state.disassemble();

        // Line 0 is at 0x1000 (LDA #$88), Line 1 is at 0x1002 (NOP), Line 2 is at 0x1003 (LDA #$16)
        core.view.selection_start = Some(0);
        core.view.cursor_index = 2;

        let events = core.apply_action(AppAction::PackLoHiAddress);

        assert!(
            events.iter().any(|e| matches!(e, CoreEvent::StateChanged)),
            "Events did not contain StateChanged: {events:?}"
        );
        assert_eq!(core.view.selection_start, None);

        // Immediate formats should be assigned: $1000 -> LowByte($1688), $1003 -> HighByte($1688)
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0x1688)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1003))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::HighByte(Addr(0x1688)))
        );
    }

    #[test]
    fn test_pack_hi_lo_address_consecutive_lines() {
        let mut core = Core::new();
        // LDA #$16 ($A9, $16), LDA #$88 ($A9, $88)
        let prg = vec![0xA9, 0x16, 0xA9, 0x88];
        let _ = core.state.load_binary(Addr(0x1000), prg);
        core.state.block_types = vec![crate::state::BlockType::Code; 4];
        core.state.disassemble();

        // Cursor on line 0 (LDA #$16), consecutive line 1 is LDA #$88
        core.view.cursor_index = 0;
        core.view.selection_start = None;

        let events = core.apply_action(AppAction::PackHiLoAddress);

        assert!(
            events.iter().any(|e| matches!(e, CoreEvent::StateChanged)),
            "Events did not contain StateChanged: {events:?}"
        );

        // HiLo address: first byte is High ($16), second is Low ($88) -> Target $1688
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::HighByte(Addr(0x1688)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0x1688)))
        );
    }

    #[test]
    fn test_toggle_bookmark_on_mid_instruction_label_and_opcode_rows() {
        let mut core = Core::new();
        let prg = vec![0x4C, 0x00, 0x20]; // JMP $2000 at $1000..=$1002
        let _ = core.state.load_binary(Addr(0x1000), prg);
        core.state.block_types = vec![crate::state::BlockType::Code; 3];
        core.state.labels.insert(
            Addr(0x1001),
            vec![crate::state::Label {
                name: "mid_target".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.disassemble();

        // Sub-index 0 is the mid-instruction label row for $1001
        core.view.cursor_index = 0;
        core.view.sub_cursor_index = 0;
        let _ = core.apply_action(AppAction::ToggleBookmark);
        assert!(
            core.state
                .annotations
                .get(Addr(0x1001))
                .and_then(|e| e.bookmark.as_ref())
                .is_some()
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.bookmark.as_ref())
                .is_none()
        );

        // Toggle off at sub-index 0
        let _ = core.apply_action(AppAction::ToggleBookmark);
        assert!(
            core.state
                .annotations
                .get(Addr(0x1001))
                .and_then(|e| e.bookmark.as_ref())
                .is_none()
        );

        // Sub-index 1 is the main opcode row for $1000
        core.view.sub_cursor_index = 1;
        let _ = core.apply_action(AppAction::ToggleBookmark);
        assert!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.bookmark.as_ref())
                .is_some()
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x1001))
                .and_then(|e| e.bookmark.as_ref())
                .is_none()
        );
    }

    #[test]
    fn test_set_label_on_mid_instruction_sub_cursor_with_multiple_labels() {
        let mut core = Core::new();
        let prg = vec![0x4C, 0x00, 0x20]; // JMP $2000 at $1000..=$1002
        let _ = core.state.load_binary(Addr(0x1000), prg);
        core.state.block_types = vec![crate::state::BlockType::Code; 3];
        core.state.labels.insert(
            Addr(0x1001),
            vec![
                crate::state::Label {
                    name: "a1001".to_string(),
                    kind: crate::state::LabelKind::Auto,
                    label_type: crate::state::LabelType::AbsoluteAddress,
                },
                crate::state::Label {
                    name: "user_1001".to_string(),
                    kind: crate::state::LabelKind::User,
                    label_type: crate::state::LabelType::UserDefined,
                },
            ],
        );
        core.state.labels.insert(
            Addr(0x1002),
            vec![crate::state::Label {
                name: "user_1002".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.disassemble();

        core.view.cursor_index = 0;

        // Sub-index 0 -> $1001
        core.view.sub_cursor_index = 0;
        let events0 = core.apply_action(AppAction::SetLabel);
        assert!(events0.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::Label {
                address: Addr(0x1001),
                ..
            })
        )));

        // Sub-index 1 -> $1002 (NOT $1001 again!)
        core.view.sub_cursor_index = 1;
        let events1 = core.apply_action(AppAction::SetLabel);
        assert!(events1.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::Label {
                address: Addr(0x1002),
                ..
            })
        )));

        // Sub-index 2 (opcode row) -> $1000
        core.view.sub_cursor_index = 2;
        let events2 = core.apply_action(AppAction::SetLabel);
        assert!(events2.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::Label {
                address: Addr(0x1000),
                ..
            })
        )));
    }

    #[test]
    fn test_apply_label_duplicate_reports_error_without_state_change() {
        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x1000), vec![0xEA, 0xEA])
            .unwrap();

        let events1 = core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x1000),
            name: "main_loop".to_string(),
            is_local: false,
        });
        assert!(events1.contains(&CoreEvent::StateChanged));

        let events2 = core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x1001),
            name: "main_loop".to_string(),
            is_local: false,
        });
        assert!(!events2.contains(&CoreEvent::StateChanged));
        assert!(!events2.contains(&CoreEvent::ViewChanged));
        assert!(
            events2.iter().any(
                |e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("already exists"))
            )
        );
    }

    #[test]
    fn test_apply_block_type_on_collapsed_block_converts_entire_range() {
        let mut core = Core::new();
        core.state.load_binary(Addr(0x1000), vec![0xEA; 4]).unwrap();
        core.state.collapsed_blocks.push((0, 3));
        core.state.disassemble();

        core.view.active_pane = ActivePane::Disassembly;
        core.view.cursor_index = 0;

        core.apply_action(AppAction::Byte);
        assert_eq!(
            core.state.block_types,
            vec![crate::state::BlockType::DataByte; 4]
        );
    }

    #[test]
    fn test_apply_origin_updates_analysis_and_emits_view_changed() {
        let mut core = Core::new();
        // BNE +1 ($D0, $01), NOP ($EA), RTS ($60)
        core.state
            .load_binary(Addr(0x1000), vec![0xD0, 0x01, 0xEA, 0x60])
            .unwrap();
        core.state.block_types = vec![crate::state::BlockType::Code; 4];
        core.state.perform_analysis();
        assert!(core.state.labels.contains_key(&Addr(0x1003)));

        let events = core.apply_action(AppAction::ApplyOrigin(Addr(0x2000)));
        assert!(events.contains(&CoreEvent::StateChanged));
        assert!(events.contains(&CoreEvent::ViewChanged));
        assert!(!core.state.labels.contains_key(&Addr(0x1003)));
        assert!(core.state.labels.contains_key(&Addr(0x2003)));

        // Undo restores origin and prior labels
        core.apply_action(AppAction::Undo);
        assert_eq!(core.state.origin, Addr(0x1000));
        assert!(core.state.labels.contains_key(&Addr(0x1003)));
    }

    #[test]
    fn test_nudge_scope_boundary_shrink_at_first_line_is_noop() {
        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x1000), vec![0xEA, 0x60])
            .unwrap();
        core.state
            .annotations
            .update(Addr(0x1000), |e| e.scope = Some(Addr(0x1000)));
        core.state.disassemble();

        let undo_before = core.state.undo_stack.can_undo();
        core.view.cursor_index = core
            .state
            .get_line_index_for_address(Addr(0x1000))
            .unwrap_or(0);
        let events = core.apply_action(AppAction::NudgeScopeBoundary { expand: false });
        assert!(events.is_empty());
        assert_eq!(core.state.undo_stack.can_undo(), undo_before);
    }
}
