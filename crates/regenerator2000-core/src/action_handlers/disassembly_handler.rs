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

fn apply_block_type(ctx: &mut ActionContext<'_>, new_type: crate::state::BlockType) {
    let origin = ctx.state.origin.0;
    let selected_range = match ctx.view.active_pane {
        ActivePane::Disassembly => {
            if ctx.view.is_visual_mode {
                let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
                let end_idx = ctx.view.cursor_index;
                let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));

                let min_addr = ctx.state.disassembly.get(min_idx).map(|l| l.address);
                let max_addr = ctx.state.disassembly.get(max_idx).map(|l| {
                    if l.bytes.is_empty() {
                        l.address
                    } else {
                        l.address.wrapping_add(l.bytes.len() as u16 - 1)
                    }
                });

                if let (Some(start), Some(end)) = (min_addr, max_addr) {
                    if start.0 >= origin && end.0 >= origin && start.0 <= end.0 {
                        let start_offset = (start.0 - origin) as usize;
                        let end_offset = (end.0 - origin) as usize;
                        if start_offset < ctx.state.raw_data.len()
                            && end_offset < ctx.state.raw_data.len()
                        {
                            Some(start_offset..(end_offset + 1))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
                let addr = line.address.0;
                if addr >= origin {
                    let offset = (addr - origin) as usize;
                    if offset < ctx.state.raw_data.len() {
                        let len = if line.bytes.is_empty() {
                            1
                        } else {
                            line.bytes.len()
                        };
                        let end_offset = (offset + len).min(ctx.state.raw_data.len());
                        Some(offset..end_offset)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        ActivePane::Blocks => {
            let blocks = ctx.state.get_blocks_view_items();
            let idx = ctx.view.blocks_selected_index.unwrap_or(0);
            if idx < blocks.len()
                && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
            {
                if start.0 >= origin && end.0 >= origin && start.0 <= end.0 {
                    let start_offset = (start.0 - origin) as usize;
                    let end_offset = (end.0 - origin) as usize;
                    if start_offset < ctx.state.raw_data.len()
                        && end_offset < ctx.state.raw_data.len()
                    {
                        Some(start_offset..(end_offset + 1))
                    } else {
                        None
                    }
                } else {
                    None
                }
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
        cmd1.apply(c.state);
        let (cmd2, _) = c.state.perform_analysis();
        c.state
            .push_command(crate::commands::Command::Batch(vec![cmd1, cmd2]));
    });

    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Set block type to {new_type:?}"
    )));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

fn handle_add_scope(ctx: &mut ActionContext<'_>) {
    let scope_range = if ctx.view.is_visual_mode {
        let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
        let end_idx = ctx.view.cursor_index;
        let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));

        let start_addr = ctx.state.disassembly.get(min_idx).map(|l| l.address);
        let end_addr = ctx.state.disassembly.get(max_idx).map(|l| l.address);

        if let (Some(start), Some(end)) = (start_addr, end_addr) {
            Some((start, end))
        } else {
            None
        }
    } else if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let start_addr = line.address;
        let current_scope = ctx
            .state
            .scopes
            .iter()
            .find(|&(&s, &e)| start_addr >= s && start_addr <= e);

        if current_scope.is_some() {
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
        let overlaps = ctx
            .state
            .scopes
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

        let old_scope_end = ctx.state.scopes.get(&start_addr).copied();
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
        });

        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Added scope from ${:04X} to ${:04X}",
            start_addr.0, end_addr.0
        )));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
    }
}

fn handle_nudge_scope_boundary(ctx: &mut ActionContext<'_>, expand: bool) {
    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        return;
    };
    let current_addr = line.address;
    let active_scope = ctx
        .state
        .scopes
        .iter()
        .find(|&(&s, &e)| current_addr >= s && current_addr <= e)
        .map(|(&s, &e)| (s, e));

    let Some((start_addr, end_addr)) = active_scope else {
        ctx.events.push(CoreEvent::StatusMessage(
            "Cursor is not inside an active scope".to_string(),
        ));
        return;
    };

    let line_indices: Vec<usize> = ctx
        .state
        .disassembly
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.bytes.is_empty())
        .map(|(idx, _)| idx)
        .collect();

    let Some(end_idx) = ctx.state.get_line_index_for_address(end_addr) else {
        return;
    };
    let Some(curr_line_pos) = line_indices.iter().position(|&i| i == end_idx) else {
        return;
    };

    let new_pos = if expand {
        curr_line_pos + 1
    } else {
        curr_line_pos.saturating_sub(1)
    };

    if new_pos >= line_indices.len() {
        return;
    }

    let target_line_idx = line_indices[new_pos];
    let new_end_addr = ctx.state.disassembly[target_line_idx].address;

    if new_end_addr < start_addr {
        return;
    }

    let other_overlap = ctx
        .state
        .scopes
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

fn handle_remove_scope(ctx: &mut ActionContext<'_>) {
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let current_addr = line.address;
        let active_scope = ctx
            .state
            .scopes
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

fn handle_apply_label(ctx: &mut ActionContext<'_>, address: Addr, name: String, is_local: bool) {
    if name.trim().is_empty() {
        if let Ok(command) = ctx
            .state
            .create_set_user_label_command(address, "", is_local)
        {
            ctx.preserve_cursor(|c| {
                command.apply(c.state);
                c.state.push_command(command);
                c.state.disassemble();
            });
        }
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Label cleared at ${address:04X}"
        )));
    } else {
        if let Ok(command) = ctx
            .state
            .create_set_user_label_command(address, &name, is_local)
        {
            ctx.preserve_cursor(|c| {
                command.apply(c.state);
                c.state.push_command(command);
                c.state.disassemble();
            });
        }
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Label '{name}' applied to ${address:04X}"
        )));
    }
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

fn handle_apply_enum_usage(ctx: &mut ActionContext<'_>, address: Addr, enum_name: Option<&str>) {
    let old_enum = ctx.state.enum_usages.get(&address).cloned();
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

fn handle_apply_comment(
    ctx: &mut ActionContext<'_>,
    address: Addr,
    text: String,
    kind: CommentKind,
) {
    let command = match kind {
        CommentKind::Side => {
            let old_comment = ctx.state.user_side_comments.get(&address).cloned();
            crate::commands::Command::SetUserSideComment {
                address,
                new_comment: if text.is_empty() { None } else { Some(text) },
                old_comment,
            }
        }
        CommentKind::Line => {
            let old_comment = ctx.state.user_line_comments.get(&address).cloned();
            crate::commands::Command::SetUserLineComment {
                address,
                new_comment: if text.is_empty() { None } else { Some(text) },
                old_comment,
            }
        }
    };
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

fn handle_set_label(ctx: &mut ActionContext<'_>) {
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let address = line.address;
        let is_external = line.external_label_address.is_some();
        let target_addr = line.external_label_address.unwrap_or(address);

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
                .get(&address)
                .and_then(|v| v.iter().find(|l| l.kind == crate::state::LabelKind::User))
                .map(|l| l.name.clone());
            ctx.events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Label {
                    address,
                    initial_name: current.unwrap_or_default(),
                    is_external: false,
                },
            ));
            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Set User Label at ${address:04X}"
            )));
        }
    }
}

fn handle_lo_hi_packing(ctx: &mut ActionContext<'_>, lo_first: bool) {
    let mut indices = Vec::new();
    if let Some(start) = ctx.view.selection_start {
        let end = ctx.view.cursor_index;
        let (low, high) = if start < end {
            (start, end)
        } else {
            (end, start)
        };
        for i in low..=high {
            indices.push(i);
        }
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
        let val1 = ctx.state.disassembly.get(idx1).and_then(|l| {
            if let Some(op) = &l.opcode
                && op.mode == crate::cpu::AddressingMode::Immediate
                && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
            {
                l.bytes.get(1).copied()
            } else {
                None
            }
        });

        if let Some(v1) = val1 {
            let mut j = i + 1;
            let mut found_match = None;
            while j < indices.len() {
                let idx2 = indices[j];
                let val2 = ctx.state.disassembly.get(idx2).and_then(|l| {
                    if let Some(op) = &l.opcode
                        && op.mode == crate::cpu::AddressingMode::Immediate
                        && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
                    {
                        l.bytes.get(1).copied()
                    } else {
                        None
                    }
                });
                if let Some(v2) = val2 {
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
                    old_format: ctx.state.immediate_value_formats.get(&addr1).copied(),
                });
                batch_commands.push(crate::commands::Command::SetImmediateFormat {
                    address: addr2,
                    new_format: Some(fmt2),
                    old_format: ctx.state.immediate_value_formats.get(&addr2).copied(),
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
            c.state.disassemble();

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
            && let Some(op) = &line.opcode
            && op.mode == crate::cpu::AddressingMode::Immediate
            && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
            && let Some(known_byte) = line.bytes.get(1).copied()
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
            .immediate_value_formats
            .get(&address)
            .cloned()
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
            old_format: ctx.state.immediate_value_formats.get(&address).cloned(),
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

fn toggle_collapsed_block(ctx: &mut ActionContext<'_>) {
    if let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) {
        let addr = line.address;
        if let Some((start_addr, end_addr)) = ctx.state.get_block_range(addr) {
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
                    batch.apply(ctx.state);
                    let (analysis_cmd, _) = ctx.state.perform_analysis();
                    let final_cmd = crate::commands::Command::Batch(vec![batch, analysis_cmd]);
                    ctx.state.push_command(final_cmd);

                    if let Some(idx) = ctx.state.get_line_index_containing_address(addr) {
                        ctx.view.cursor_index = idx;
                    } else if let Some(idx) = ctx.state.get_line_index_for_address(addr) {
                        ctx.view.cursor_index = idx;
                    }

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
                        cmd1.apply(c.state);
                        c.state.disassemble();
                        let (cmd2, _) = c.state.perform_analysis();
                        c.state
                            .push_command(crate::commands::Command::Batch(vec![cmd1, cmd2]));
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
                handle_apply_label(ctx, *address, name.clone(), *is_local);
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
                    let current = ctx.state.user_side_comments.get(&address).cloned();
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
                    let current = ctx.state.user_line_comments.get(&address).cloned();
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
                        let address = line.address;
                        let is_bookmarked = ctx.state.bookmarks.contains_key(&address);

                        let command = crate::commands::Command::SetBookmark {
                            address,
                            new_name: if is_bookmarked {
                                None
                            } else {
                                Some(String::new())
                            },
                            old_name: ctx.state.bookmarks.get(&address).cloned(),
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
                        command.apply(ctx.state);
                        ctx.state.push_command(command);
                        ctx.events.push(CoreEvent::StatusMessage(format!(
                            "Excluded ${ext_addr:04X} from analysis"
                        )));
                        ctx.events.push(CoreEvent::StateChanged);
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
                let command = crate::commands::Command::ChangeOrigin {
                    new_origin: *new_origin,
                    old_origin,
                };
                command.apply(ctx.state);
                ctx.state.push_command(command);
                ctx.state.disassemble();
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Origin changed to ${:04X}",
                    new_origin.0
                )));
                ctx.events.push(CoreEvent::StateChanged);
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
            "Events did not contain StateChanged: {:?}",
            events
        );
        assert_eq!(core.view.selection_start, None);

        // Immediate formats should be assigned: $1000 -> LowByte($1688), $1003 -> HighByte($1688)
        assert_eq!(
            core.state.immediate_value_formats.get(&Addr(0x1000)),
            Some(&crate::state::ImmediateFormat::LowByte(Addr(0x1688)))
        );
        assert_eq!(
            core.state.immediate_value_formats.get(&Addr(0x1003)),
            Some(&crate::state::ImmediateFormat::HighByte(Addr(0x1688)))
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
            "Events did not contain StateChanged: {:?}",
            events
        );

        // HiLo address: first byte is High ($16), second is Low ($88) -> Target $1688
        assert_eq!(
            core.state.immediate_value_formats.get(&Addr(0x1000)),
            Some(&crate::state::ImmediateFormat::HighByte(Addr(0x1688)))
        );
        assert_eq!(
            core.state.immediate_value_formats.get(&Addr(0x1002)),
            Some(&crate::state::ImmediateFormat::LowByte(Addr(0x1688)))
        );
    }
}
