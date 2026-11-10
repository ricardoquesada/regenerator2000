//! Disassembly domain action handler for block types, labels, symbolic offsets (`o`, `+`/`=`, `-`),
//! comments, scopes, immediate operand packing, bookmarks, and auto-analysis.

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

/// Returns the half-open binary byte offset range `first.start..last.end` across `slice`.
fn slice_byte_offset_range(
    state: &crate::state::AppState,
    slice: &[crate::disassembler::DisassemblyLine],
) -> Option<std::ops::Range<usize>> {
    let first = slice
        .iter()
        .find_map(|l| line_byte_offset_range(state, l))?;
    let last = slice
        .iter()
        .rev()
        .find_map(|l| line_byte_offset_range(state, l))?;
    (first.start < last.end).then_some(first.start..last.end)
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
                    .and_then(|slice| slice_byte_offset_range(ctx.state, slice))
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
        let active_scope = ctx.state.enclosing_scope_for_address(current_addr);

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

/// Applies or clears a user label or Tier 2 symbolic offset expression at `address`.
fn handle_apply_label(ctx: &mut ActionContext<'_>, address: Addr, name: &str, is_local: bool) {
    let trimmed = name.trim();
    let site_addr = ctx
        .state
        .disassembly
        .get(ctx.view.cursor_index)
        .filter(|l| l.external_label_address.is_none() && !ctx.state.is_external(l.address))
        .map(|l| l.address);

    let mut outcome: Result<Option<crate::state::SymbolicOffset>, String> = Ok(None);
    ctx.preserve_cursor(|c| {
        outcome = c
            .state
            .apply_user_label_or_address_offset(address, trimmed, is_local, site_addr);
    });

    match outcome {
        Ok(Some(sym)) => {
            let expr = ctx
                .state
                .format_symbolic_offset(sym, site_addr, address.0 <= 0x00FF);
            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Applied symbolic offset '{expr}' to ${address:04X}"
            )));
            ctx.events.push(CoreEvent::StateChanged);
            ctx.events.push(CoreEvent::ViewChanged);
        }
        Ok(None) => {
            let msg = if trimmed.is_empty() {
                format!("Cleared label at ${address:04X}")
            } else if trimmed.contains(['+', '-']) {
                format!("Cleared symbolic offset at ${address:04X}")
            } else {
                format!("Applied label '{trimmed}' to ${address:04X}")
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
/// so mid-instruction label rows (`* + offset`) edit the intermediate byte label. When no user
/// label exists and a Tier 2 `address_offset` is active, pre-fills `initial_name` with the
/// formatted offset expression.
fn handle_set_label(ctx: &mut ActionContext<'_>) {
    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        return;
    };
    let is_external = line.external_label_address.is_some();
    let target_addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
    let user_label = ctx
        .state
        .labels
        .get(&target_addr)
        .and_then(|v| v.iter().find(|l| l.kind == crate::state::LabelKind::User));

    let active_addr_offset = if user_label.is_none()
        && crate::disassembler::symbols::has_active_address_offset(
            target_addr,
            &ctx.state.labels,
            &ctx.state.annotations,
        ) {
        ctx.state
            .annotations
            .get(target_addr)
            .and_then(|e| e.address_offset)
            .filter(|sym| sym.matches_target(target_addr))
    } else {
        None
    };

    if is_external {
        let (initial_name, is_local) = if let Some(sym) = active_addr_offset {
            (
                ctx.state
                    .format_symbolic_offset(sym, None, target_addr.0 <= 0x00FF),
                false,
            )
        } else {
            let existing =
                user_label.or_else(|| ctx.state.labels.get(&target_addr).and_then(|v| v.first()));
            let local =
                existing.is_some_and(|l| l.label_type == crate::state::LabelType::LocalUserDefined);
            (existing.map(|l| l.name.clone()).unwrap_or_default(), local)
        };
        ctx.events.push(CoreEvent::DialogRequested(
            crate::event::DialogType::Label {
                address: target_addr,
                initial_name,
                is_local,
                is_external: true,
            },
        ));
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Edit External Label for ${target_addr:04X}"
        )));
    } else {
        let (initial_name, is_local) = if let Some(sym) = active_addr_offset {
            (
                ctx.state
                    .format_symbolic_offset(sym, Some(target_addr), target_addr.0 <= 0x00FF),
                false,
            )
        } else {
            let local = user_label
                .is_some_and(|l| l.label_type == crate::state::LabelType::LocalUserDefined);
            (
                user_label.map(|l| l.name.clone()).unwrap_or_default(),
                local,
            )
        };
        ctx.events.push(CoreEvent::DialogRequested(
            crate::event::DialogType::Label {
                address: target_addr,
                initial_name,
                is_local,
                is_external: false,
            },
        ));
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Set User Label at ${target_addr:04X}"
        )));
    }
}

/// Submission payload checked by [`validate_symbolic_offset_submission`] and applied by
/// [`AppAction::ApplySymbolicOffset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolicOffsetSubmission<'a> {
    /// Referencing instruction or table entry [`Addr`], when editing an operand site.
    pub site_address: Option<Addr>,
    /// Primary effective target [`Addr`].
    pub target_address: Addr,
    /// Optional inclusive `(start, end)` [`Addr`] range when applying across a visual selection.
    pub address_range: Option<(Addr, Addr)>,
    /// Raw user-entered expression (empty or `"0"` clears the offset).
    pub input: &'a str,
    /// `true` to apply Tier 2 (`address_offset`) globally, or `false` to apply Tier 1 (`operand_offset`).
    pub is_global: bool,
    /// `true` when checking a 2-byte zero-page instruction operand (`base <= $00FF`).
    pub is_zero_page_mode: bool,
}

/// Checked symbolic offset mutation plan ready for execution on [`AppState`](crate::state::AppState).
enum SymbolicOffsetPlan {
    Operand {
        updates: Vec<(Addr, Option<crate::state::SymbolicOffset>)>,
        site_count: usize,
        is_clear: bool,
    },
    Address {
        updates: Vec<(Addr, Option<crate::state::SymbolicOffset>)>,
        site_addresses: Vec<Addr>,
        is_clear: bool,
    },
}

/// Checks `req` against `state` and produces the exact Tier 1 or Tier 2 update plan.
fn plan_symbolic_offset_submission(
    state: &crate::state::AppState,
    req: &SymbolicOffsetSubmission<'_>,
) -> Result<SymbolicOffsetPlan, String> {
    let is_clear = crate::state::AppState::is_symbolic_clear_input(req.input);

    let Some((start_addr, end_addr)) = req.address_range else {
        if !req.is_global {
            let Some(site) = req.site_address else {
                return Err(
                    "Operand-only symbolic offset requires an instruction or table site"
                        .to_string(),
                );
            };
            let scope = state.enclosing_scope_for_address(site);
            let parsed = state.parse_symbolic_offset_input(
                req.input,
                req.target_address,
                req.is_zero_page_mode,
                scope,
            )?;
            let mut updates = Vec::with_capacity(2);
            if let Some((first_addr, second_addr, _)) = state.split_table_pair_at_address(site) {
                // Canonicalize Tier 1 split-table annotations onto `first_addr` and clear any
                // legacy/hand-edited annotation on `second_addr`.
                updates.push((first_addr, parsed));
                if second_addr != first_addr
                    && state
                        .annotations
                        .get(second_addr)
                        .and_then(|e| e.operand_offset)
                        .is_some()
                {
                    updates.push((second_addr, None));
                }
            } else {
                updates.push((site, parsed));
            }
            return Ok(SymbolicOffsetPlan::Operand {
                updates,
                site_count: 1,
                is_clear: parsed.is_none(),
            });
        }

        let scope = req
            .site_address
            .and_then(|sa| state.enclosing_scope_for_address(sa))
            .or_else(|| state.enclosing_scope_for_address(req.target_address));
        let parsed = state.parse_symbolic_offset_input(
            req.input,
            req.target_address,
            req.is_zero_page_mode || req.target_address.0 <= 0x00FF,
            scope,
        )?;
        return Ok(SymbolicOffsetPlan::Address {
            updates: vec![(req.target_address, parsed)],
            site_addresses: req.site_address.into_iter().collect(),
            is_clear: parsed.is_none(),
        });
    };

    if start_addr > end_addr {
        return Err(format!(
            "Invalid symbolic offset range ${start_addr:04X}..=${end_addr:04X}"
        ));
    }

    // Share `SymbolicBaseCache` across the range so `K` entries against `L` labels and `S` scopes
    // resolve in `O(L + K log S)` rather than `O(K * L)`.
    let mut base_cache = crate::state::app_state::SymbolicBaseCache::default();

    if !req.is_global {
        if state.is_external(start_addr) || state.is_external(end_addr) {
            return Err(format!(
                "Operand scope requires addresses inside loaded binary range, got ${start_addr:04X}..=${end_addr:04X}"
            ));
        }
        // Extract `OperandTargetSite` records in a single `O(N)` pass.
        let targets = state.effective_targets_with_pair_in_range(start_addr, end_addr);
        if targets.is_empty() {
            return Err(format!(
                "No instruction operands or address table entries found in ${start_addr:04X}..=${end_addr:04X}"
            ));
        }
        let mut updates = Vec::with_capacity(targets.len() * 2);
        for t in &targets {
            let scope = state.enclosing_scope_for_address(t.site_addr);
            let parsed = state.parse_symbolic_offset_input_cached(
                req.input,
                t.raw_target,
                t.is_zero_page_mode,
                scope,
                &mut base_cache,
            )?;
            updates.push((t.site_addr, parsed));
            if let Some(second_addr) = t.second_pair_addr
                && second_addr != t.site_addr
                && state
                    .annotations
                    .get(second_addr)
                    .and_then(|e| e.operand_offset)
                    .is_some()
            {
                updates.push((second_addr, None));
            }
        }
        return Ok(SymbolicOffsetPlan::Operand {
            updates,
            site_count: targets.len(),
            is_clear,
        });
    }

    // When `is_global` is toggled on an operand-bearing selection (`site_address.is_some()`),
    // apply Tier 2 to the referenced target addresses rather than raw instruction opcode bytes.
    if req.site_address.is_some() && !state.is_external(start_addr) && !state.is_external(end_addr)
    {
        let targets = state.effective_targets_with_pair_in_range(start_addr, end_addr);
        if !targets.is_empty() {
            let mut target_reqs = std::collections::BTreeMap::<Addr, (Addr, bool)>::new();
            let mut site_addresses = Vec::with_capacity(targets.len() * 2);
            for t in &targets {
                let zp = t.is_zero_page_mode || t.raw_target.0 <= 0x00FF;
                target_reqs
                    .entry(t.raw_target)
                    .and_modify(|(_, existing_zp)| *existing_zp |= zp)
                    .or_insert((t.site_addr, zp));
                site_addresses.push(t.site_addr);
                if let Some(second_addr) = t.second_pair_addr {
                    site_addresses.push(second_addr);
                }
            }
            let is_multi_targets = target_reqs.len() > 1;
            let mut updates = Vec::with_capacity(target_reqs.len());
            for (raw_target, (site_addr, is_zp_mode)) in target_reqs {
                let scope = state
                    .enclosing_scope_for_address(site_addr)
                    .or_else(|| state.enclosing_scope_for_address(raw_target));
                let parsed = state.parse_symbolic_offset_input_cached(
                    req.input,
                    raw_target,
                    is_zp_mode,
                    scope,
                    &mut base_cache,
                )?;
                if is_multi_targets && !is_clear && parsed.is_none() {
                    continue;
                }
                updates.push((raw_target, parsed));
            }
            return Ok(SymbolicOffsetPlan::Address {
                updates,
                site_addresses,
                is_clear,
            });
        }
    }

    // Contiguous Tier 2 struct / data / external address range `start_addr..=end_addr`.
    let is_multi_range = start_addr < end_addr;
    let mut updates = Vec::with_capacity(usize::from(end_addr.0 - start_addr.0) + 1);
    for addr_u16 in start_addr.0..=end_addr.0 {
        let addr = Addr(addr_u16);
        let scope = state.enclosing_scope_for_address(addr);
        let parsed = state.parse_symbolic_offset_input_cached(
            req.input,
            addr,
            addr.0 <= 0x00FF,
            scope,
            &mut base_cache,
        )?;
        // Skip `addr == base` on multi-address struct ranges so the struct base address
        // retains its explicit `LabelKind::User` label untouched.
        if is_multi_range && !is_clear && parsed.is_none() {
            continue;
        }
        updates.push((addr, parsed));
    }

    Ok(SymbolicOffsetPlan::Address {
        updates,
        site_addresses: Vec::new(),
        is_clear,
    })
}

/// Checks a symbolic offset dialog submission without mutating [`AppState`](crate::state::AppState).
///
/// Frontends call this before emitting [`AppAction::ApplySymbolicOffset`]. Invalid expressions
/// show an inline error and keep the dialog open with user text intact.
///
/// # Errors
///
/// Returns an error message when `req` fails expression parsing, 16-bit non-wrapping bounds,
/// or 2-byte zero-page base rules.
///
/// # Examples
///
/// ```
/// use regenerator2000_core::action_handlers::disassembly_handler::{
///     SymbolicOffsetSubmission, validate_symbolic_offset_submission,
/// };
/// use regenerator2000_core::state::{Addr, AppState, Label, LabelKind, LabelType};
///
/// let mut state = AppState::new();
/// state.labels.insert(
///     Addr(0x00FB),
///     vec![Label {
///         name: "zp_ptr".to_string(),
///         kind: LabelKind::User,
///         label_type: LabelType::ZeroPagePointer,
///     }],
/// );
///
/// let valid = SymbolicOffsetSubmission {
///     site_address: Some(Addr(0x1000)),
///     target_address: Addr(0x00FC),
///     address_range: None,
///     input: "zp_ptr + 1",
///     is_global: true,
///     is_zero_page_mode: true,
/// };
/// assert!(validate_symbolic_offset_submission(&state, &valid).is_ok());
///
/// let mismatch = SymbolicOffsetSubmission {
///     input: "zp_ptr + 2",
///     ..valid
/// };
/// assert!(validate_symbolic_offset_submission(&state, &mismatch).is_err());
/// ```
pub fn validate_symbolic_offset_submission(
    state: &crate::state::AppState,
    req: &SymbolicOffsetSubmission<'_>,
) -> Result<(), String> {
    plan_symbolic_offset_submission(state, req).map(|_| ())
}

/// Suggests an initial expression for a multi-target operand range (such as an `RTS` dispatch table).
#[must_use]
fn suggest_multi_operand_expression(
    state: &crate::state::AppState,
    targets: &[crate::state::OperandTargetSite],
) -> String {
    let Some(&first) = targets.first() else {
        return String::new();
    };

    let resolve_tier1 =
        |t: crate::state::OperandTargetSite| -> Option<crate::state::SymbolicOffset> {
            state
                .annotations
                .get(t.site_addr)
                .and_then(|e| e.operand_offset)
                .or_else(|| {
                    t.second_pair_addr
                        .and_then(|s| state.annotations.get(s))
                        .and_then(|e| e.operand_offset)
                })
                .filter(|sym| sym.matches_target(t.raw_target))
        };

    if let Some(first_sym) = resolve_tier1(first) {
        let same_base = targets
            .iter()
            .all(|&t| resolve_tier1(t).is_some_and(|s| s.base == first_sym.base));
        if same_base {
            let full = state.format_symbolic_offset(
                first_sym,
                Some(first.site_addr),
                first.is_zero_page_mode,
            );
            if let Some(op_idx) = full.find(['+', '-']) {
                return full[..op_idx].trim().to_string();
            }
        }
        let same_offset = targets
            .iter()
            .all(|&t| resolve_tier1(t).is_some_and(|s| s.offset == first_sym.offset));
        if same_offset {
            let sign = if first_sym.offset >= 0 { '+' } else { '-' };
            let mag = first_sym.offset.unsigned_abs();
            return match first_sym.radix {
                crate::state::OffsetRadix::Decimal => format!("{sign}{mag}"),
                crate::state::OffsetRadix::Hex => format!("{sign}${mag:x}"),
            };
        }
    }

    // Check RTS dispatch and 1-based tables: suggest "-1" when every `target + 1` has a label.
    let all_plus1_labelled = targets.iter().all(|t| {
        t.raw_target.0 < 0xFFFF && {
            let cand = Addr(t.raw_target.0 + 1);
            (!t.is_zero_page_mode || cand.0 <= 0x00FF)
                && !state.effective_labels_at(cand).is_empty()
                && !crate::disassembler::symbols::has_active_address_offset(
                    cand,
                    &state.labels,
                    &state.annotations,
                )
        }
    });
    if all_plus1_labelled {
        return "-1".to_string();
    }

    String::new()
}

/// Extracts the bare base token before any `+` or `-` operator in `expr`.
#[must_use]
fn extract_base_token(expr: &str) -> String {
    if let Some(op_idx) = expr.find(['+', '-']) {
        expr[..op_idx].trim().to_string()
    } else {
        expr.trim().to_string()
    }
}

/// Suggests a bare struct base symbol name when opening `SymbolicOffsetDialog` across `start_addr..=end_addr`.
#[must_use]
fn suggest_struct_range_base(
    state: &crate::state::AppState,
    start_addr: Addr,
    end_addr: Addr,
) -> String {
    let start_suppressed = crate::disassembler::symbols::has_active_address_offset(
        start_addr,
        &state.labels,
        &state.annotations,
    );
    if !start_suppressed {
        if let Some(label) = state
            .labels
            .get(&start_addr)
            .filter(|v| {
                v.iter().any(|l| {
                    matches!(
                        l.kind,
                        crate::state::LabelKind::User | crate::state::LabelKind::System
                    )
                })
            })
            .and_then(|v| crate::disassembler::resolve_label(v, start_addr.0, &state.settings))
        {
            return label.name.clone();
        }
        if !state.excluded_addresses.contains(&start_addr)
            && let Some(sys_label) = state
                .system_labels
                .get(&start_addr)
                .and_then(|v| crate::disassembler::resolve_label(v, start_addr.0, &state.settings))
        {
            return sys_label.name.clone();
        }
    } else if let Some(sym) = state
        .annotations
        .get(start_addr)
        .and_then(|e| e.address_offset)
        .filter(|sym| sym.matches_target(start_addr))
    {
        return extract_base_token(&state.format_symbolic_offset(
            sym,
            None,
            start_addr.0 <= 0x00FF,
        ));
    }

    if start_addr < end_addr && start_addr.0 < 0xFFFF {
        let next_addr = Addr(start_addr.0 + 1);
        if crate::disassembler::symbols::has_active_address_offset(
            next_addr,
            &state.labels,
            &state.annotations,
        ) && let Some(sym) = state
            .annotations
            .get(next_addr)
            .and_then(|e| e.address_offset)
            .filter(|sym| sym.matches_target(next_addr))
        {
            return extract_base_token(&state.format_symbolic_offset(
                sym,
                None,
                next_addr.0 <= 0x00FF,
            ));
        }
    }

    if !start_suppressed
        && let Some(auto_label) = state
            .labels
            .get(&start_addr)
            .and_then(|v| crate::disassembler::resolve_label(v, start_addr.0, &state.settings))
    {
        return auto_label.name.clone();
    }

    let (suggested, _) = state.suggest_symbolic_offset(None, start_addr, start_addr.0 <= 0x00FF);
    extract_base_token(&suggested)
}

/// Opens a single-address global Tier 2 `DialogType::SymbolicOffset` for `addr`.
fn open_single_global_symbolic_offset_dialog(ctx: &mut ActionContext<'_>, addr: Addr) {
    let is_zp = addr.0 <= 0x00FF;
    let (initial_input, _) = ctx.state.suggest_symbolic_offset(None, addr, is_zp);
    ctx.events.push(CoreEvent::DialogRequested(
        crate::event::DialogType::SymbolicOffset {
            site_address: None,
            target_address: addr,
            address_range: None,
            initial_input,
            is_global: true,
            lock_global: true,
            is_zero_page_mode: is_zp,
        },
    ));
    ctx.events.push(CoreEvent::StatusMessage(format!(
        "Set Symbolic Offset for ${addr:04X}"
    )));
}

/// Dispatches `DialogType::SymbolicOffset` across an in-binary byte offset range `byte_range`.
fn open_symbolic_offset_dialog_for_byte_range(
    ctx: &mut ActionContext<'_>,
    byte_range: std::ops::Range<usize>,
    has_selection: bool,
) {
    let start_addr = ctx.state.origin.wrapping_add(byte_range.start as u16);
    let end_addr = ctx.state.origin.wrapping_add((byte_range.end - 1) as u16);
    let targets = ctx
        .state
        .effective_targets_with_pair_in_range(start_addr, end_addr);

    if targets.len() == 1 {
        let t = targets[0];
        let (initial_input, is_global) =
            ctx.state
                .suggest_symbolic_offset(Some(t.site_addr), t.raw_target, t.is_zero_page_mode);
        ctx.events.push(CoreEvent::DialogRequested(
            crate::event::DialogType::SymbolicOffset {
                site_address: Some(t.site_addr),
                target_address: t.raw_target,
                address_range: None,
                initial_input,
                is_global,
                lock_global: false,
                is_zero_page_mode: t.is_zero_page_mode,
            },
        ));
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Set Symbolic Offset for ${:04X}",
            t.raw_target.0
        )));
        return;
    }

    if targets.len() > 1 {
        let first = targets[0];
        let initial_input = suggest_multi_operand_expression(ctx.state, &targets);
        ctx.events.push(CoreEvent::DialogRequested(
            crate::event::DialogType::SymbolicOffset {
                site_address: Some(first.site_addr),
                target_address: first.raw_target,
                address_range: Some((start_addr, end_addr)),
                initial_input,
                is_global: false,
                lock_global: false,
                is_zero_page_mode: first.is_zero_page_mode,
            },
        ));
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Set Symbolic Offset for {} entries (${start_addr:04X}..=${end_addr:04X})",
            targets.len()
        )));
        return;
    }

    let has_non_code = (byte_range.start..byte_range.end)
        .any(|pc| ctx.state.block_types.get(pc) != Some(&crate::state::BlockType::Code));
    let has_existing_offset = (byte_range.start..byte_range.end).any(|pc| {
        let addr = ctx.state.origin.wrapping_add(pc as u16);
        ctx.state
            .annotations
            .get(addr)
            .and_then(|e| e.address_offset)
            .is_some()
    });

    if !has_non_code && !has_existing_offset {
        let msg = if has_selection {
            "Selected instructions have no address operands for symbolic offset"
        } else {
            "Current instruction has no address operand for symbolic offset"
        };
        ctx.events.push(CoreEvent::StatusMessage(msg.to_string()));
        return;
    }

    if start_addr < end_addr && (has_non_code || has_selection) {
        let initial_input = suggest_struct_range_base(ctx.state, start_addr, end_addr);
        ctx.events.push(CoreEvent::DialogRequested(
            crate::event::DialogType::SymbolicOffset {
                site_address: None,
                target_address: start_addr,
                address_range: Some((start_addr, end_addr)),
                initial_input,
                is_global: true,
                lock_global: true,
                is_zero_page_mode: start_addr.0 <= 0x00FF,
            },
        ));
        ctx.events.push(CoreEvent::StatusMessage(format!(
            "Set Symbolic Offset for ${start_addr:04X}..=${end_addr:04X}"
        )));
    } else {
        open_single_global_symbolic_offset_dialog(ctx, start_addr);
    }
}

/// Opens [`DialogType::SymbolicOffset`](crate::event::DialogType::SymbolicOffset) prefilled for the
/// current operand, pointer table, external equate, or data/struct selection.
fn handle_symbolic_offset_dialog(ctx: &mut ActionContext<'_>) {
    let has_selection = ctx.view.is_visual_mode || ctx.view.selection_start.is_some();

    if has_selection {
        let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
        let end_idx = ctx.view.cursor_index;
        let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));
        let Some(slice) = ctx.state.disassembly.get(min_idx..=max_idx) else {
            ctx.events.push(CoreEvent::StatusMessage(
                "Invalid selection for symbolic offset".to_string(),
            ));
            return;
        };

        if let Some(byte_range) = slice_byte_offset_range(ctx.state, slice) {
            open_symbolic_offset_dialog_for_byte_range(ctx, byte_range, true);
            return;
        }

        let ext_addrs: Vec<Addr> = slice
            .iter()
            .filter_map(|l| l.external_label_address)
            .collect();
        if let (Some(&first_ext), Some(&last_ext)) = (ext_addrs.first(), ext_addrs.last()) {
            let (min_ext, max_ext) = (first_ext.min(last_ext), first_ext.max(last_ext));
            if min_ext == max_ext {
                open_single_global_symbolic_offset_dialog(ctx, min_ext);
            } else {
                let initial_input = suggest_struct_range_base(ctx.state, min_ext, max_ext);
                ctx.events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::SymbolicOffset {
                        site_address: None,
                        target_address: min_ext,
                        address_range: Some((min_ext, max_ext)),
                        initial_input,
                        is_global: true,
                        lock_global: true,
                        is_zero_page_mode: min_ext.0 <= 0x00FF,
                    },
                ));
                ctx.events.push(CoreEvent::StatusMessage(format!(
                    "Set Symbolic Offset for ${min_ext:04X}..=${max_ext:04X}"
                )));
            }
            return;
        }

        ctx.events.push(CoreEvent::StatusMessage(
            "Symbolic offset requires an instruction operand, pointer table, or data field"
                .to_string(),
        ));
        return;
    }

    // Single-line invocation (no active visual selection).
    let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
        ctx.events.push(CoreEvent::StatusMessage(
            "Invalid cursor position".to_string(),
        ));
        return;
    };

    if let Some(ext_addr) = line.external_label_address {
        open_single_global_symbolic_offset_dialog(ctx, ext_addr);
        return;
    }

    // Mid-instruction label sub-cursor edits that intermediate byte's Tier 2 address offset.
    let sub_addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
    if sub_addr != line.address {
        open_single_global_symbolic_offset_dialog(ctx, sub_addr);
        return;
    }

    let Some(byte_range) = line_byte_offset_range(ctx.state, line) else {
        ctx.events.push(CoreEvent::StatusMessage(
            "Symbolic offset requires an instruction operand, pointer table, or data field"
                .to_string(),
        ));
        return;
    };

    open_symbolic_offset_dialog_for_byte_range(ctx, byte_range, false);
}

/// Applies or clears Tier 1 (`operand_offset`) or Tier 2 (`address_offset`) symbolic offsets
/// from [`AppAction::ApplySymbolicOffset`].
fn handle_apply_symbolic_offset(ctx: &mut ActionContext<'_>, req: &SymbolicOffsetSubmission<'_>) {
    let plan = match plan_symbolic_offset_submission(ctx.state, req) {
        Ok(plan) => plan,
        Err(err) => {
            ctx.events.push(CoreEvent::StatusMessage(err));
            return;
        }
    };

    let status_msg = match plan {
        SymbolicOffsetPlan::Operand {
            updates,
            site_count,
            is_clear,
        } => {
            let single_formatted = (site_count == 1)
                .then(|| updates.first().and_then(|(_, sym)| *sym))
                .flatten();
            ctx.preserve_cursor(|c| {
                c.state.apply_operand_offsets(&updates);
                c.view.selection_start = None;
                c.view.is_visual_mode = false;
            });
            if is_clear {
                if site_count == 1 {
                    format!(
                        "Cleared operand symbolic offset for ${:04X}",
                        req.target_address.0
                    )
                } else {
                    format!("Cleared operand symbolic offset on {site_count} entries")
                }
            } else if let Some(sym) = single_formatted {
                let expr =
                    ctx.state
                        .format_symbolic_offset(sym, req.site_address, req.is_zero_page_mode);
                format!("Set operand symbolic offset to '{expr}'")
            } else {
                format!("Applied operand symbolic offset to {site_count} entries")
            }
        }
        SymbolicOffsetPlan::Address {
            updates,
            site_addresses,
            is_clear,
        } => {
            let count = updates.len();
            let first_site = site_addresses.first().copied();
            let single_formatted = (count == 1)
                .then(|| {
                    updates
                        .first()
                        .and_then(|(addr, sym)| sym.map(|s| (*addr, s)))
                })
                .flatten();
            ctx.preserve_cursor(|c| {
                c.state
                    .apply_address_offsets_for_sites(&updates, &site_addresses);
                c.view.selection_start = None;
                c.view.is_visual_mode = false;
            });
            if is_clear {
                if count == 1 {
                    format!("Cleared symbolic offset at ${:04X}", req.target_address.0)
                } else {
                    format!("Cleared symbolic offset on {count} addresses")
                }
            } else if let Some((addr, sym)) = single_formatted {
                let expr = ctx
                    .state
                    .format_symbolic_offset(sym, first_site, addr.0 <= 0x00FF);
                format!("Applied symbolic offset '{expr}' globally to ${addr:04X}")
            } else {
                format!("Applied global symbolic offset to {count} addresses")
            }
        }
    };

    ctx.events.push(CoreEvent::StatusMessage(status_msg));
    ctx.events.push(CoreEvent::StateChanged);
    ctx.events.push(CoreEvent::ViewChanged);
}

/// Computes the nudged [`SymbolicOffset`](crate::state::SymbolicOffset) (`new_offset = current_offset + delta`,
/// `new_base = effective_target - new_offset`), returning `Ok(None)` when `new_offset == 0`.
fn compute_nudged_symbolic_offset(
    effective_target: Addr,
    current: Option<crate::state::SymbolicOffset>,
    delta: i16,
    is_zero_page_mode: bool,
) -> Result<Option<crate::state::SymbolicOffset>, String> {
    let valid_current = current.filter(|sym| sym.matches_target(effective_target));
    let current_offset = valid_current.map_or(0i16, |sym| sym.offset);
    let radix = valid_current.map_or(crate::state::OffsetRadix::Decimal, |sym| sym.radix);

    let new_offset_i32 = i32::from(current_offset) + i32::from(delta);
    let new_offset = i16::try_from(new_offset_i32).map_err(|_| {
        format!(
            "Symbolic offset {new_offset_i32} overflows signed 16-bit displacement range (-32768..=32767)"
        )
    })?;
    if new_offset == 0 {
        return Ok(None);
    }

    let new_base_i32 = i32::from(effective_target.0) - i32::from(new_offset);
    if !(0..=0xFFFF).contains(&new_base_i32) {
        return Err(format!(
            "Cannot nudge symbolic offset for ${effective_target:04X} by {delta:+}: base address falls outside $0000..=$FFFF"
        ));
    }

    let base = Addr(new_base_i32 as u16);
    if is_zero_page_mode && base.0 > 0x00FF {
        return Err(format!(
            "2-byte zero-page instruction targeting ${:02X} requires a zero-page base address (<= $00FF), got ${base:04X}",
            effective_target.0
        ));
    }

    Ok(Some(crate::state::SymbolicOffset {
        base,
        offset: new_offset,
        radix,
    }))
}

/// Resolves either Tier 1 operand sites or active Tier 2 addresses inside `byte_range`.
fn resolve_nudge_targets_in_byte_range(
    state: &crate::state::AppState,
    byte_range: std::ops::Range<usize>,
) -> (Vec<crate::state::OperandTargetSite>, Vec<Addr>) {
    let start_addr = state.origin.wrapping_add(byte_range.start as u16);
    let end_addr = state.origin.wrapping_add((byte_range.end - 1) as u16);
    let targets = state.effective_targets_with_pair_in_range(start_addr, end_addr);
    if !targets.is_empty() {
        return (targets, Vec::new());
    }
    let tier2 = (start_addr.0..=end_addr.0)
        .map(Addr)
        .filter(|&a| {
            crate::disassembler::symbols::has_active_address_offset(
                a,
                &state.labels,
                &state.annotations,
            )
        })
        .collect();
    (Vec::new(), tier2)
}

/// Nudges (`+1` or `-1`) symbolic offsets across the current operand, visual selection, or external equate.
fn handle_nudge_symbolic_offset(ctx: &mut ActionContext<'_>, delta: i16) {
    if delta == 0 {
        return;
    }

    let has_selection = ctx.view.is_visual_mode || ctx.view.selection_start.is_some();

    // Resolve either a set of Tier 2 external/struct/mid-instruction addresses or Tier 1 `OperandTargetSite`s.
    let (tier1_targets, tier2_addrs) = if has_selection {
        let start_idx = ctx.view.selection_start.unwrap_or(ctx.view.cursor_index);
        let end_idx = ctx.view.cursor_index;
        let (min_idx, max_idx) = (start_idx.min(end_idx), start_idx.max(end_idx));
        let Some(slice) = ctx.state.disassembly.get(min_idx..=max_idx) else {
            return;
        };

        if let Some(byte_range) = slice_byte_offset_range(ctx.state, slice) {
            resolve_nudge_targets_in_byte_range(ctx.state, byte_range)
        } else {
            let ext_set: std::collections::BTreeSet<Addr> = slice
                .iter()
                .filter_map(|l| l.external_label_address)
                .collect();
            (Vec::new(), ext_set.into_iter().collect())
        }
    } else {
        let Some(line) = ctx.state.disassembly.get(ctx.view.cursor_index) else {
            return;
        };
        let sub_addr = line.address_at_sub_index(ctx.state, ctx.view.sub_cursor_index);
        if let Some(ext_addr) = line.external_label_address {
            (Vec::new(), vec![ext_addr])
        } else if sub_addr != line.address {
            (Vec::new(), vec![sub_addr])
        } else if let Some(byte_range) = line_byte_offset_range(ctx.state, line) {
            resolve_nudge_targets_in_byte_range(ctx.state, byte_range)
        } else {
            (Vec::new(), Vec::new())
        }
    };

    if !tier1_targets.is_empty() {
        let mut tier1_updates = Vec::with_capacity(tier1_targets.len() * 2);
        let mut tier1_sites = Vec::with_capacity(tier1_targets.len());
        let mut tier2_reqs = std::collections::BTreeMap::<Addr, (Addr, bool)>::new();

        for t in &tier1_targets {
            let current_tier1 = ctx
                .state
                .annotations
                .get(t.site_addr)
                .and_then(|e| e.operand_offset)
                .or_else(|| {
                    t.second_pair_addr
                        .and_then(|second_addr| ctx.state.annotations.get(second_addr))
                        .and_then(|e| e.operand_offset)
                })
                .filter(|sym| {
                    sym.matches_target(t.raw_target)
                        && (!t.is_zero_page_mode || sym.base.0 <= 0x00FF)
                });

            if current_tier1.is_none()
                && crate::disassembler::symbols::has_active_address_offset(
                    t.raw_target,
                    &ctx.state.labels,
                    &ctx.state.annotations,
                )
            {
                let zp = t.is_zero_page_mode || t.raw_target.0 <= 0x00FF;
                tier2_reqs
                    .entry(t.raw_target)
                    .and_modify(|(_, existing_zp)| *existing_zp |= zp)
                    .or_insert((t.site_addr, zp));
                continue;
            }

            let nudged = match compute_nudged_symbolic_offset(
                t.raw_target,
                current_tier1,
                delta,
                t.is_zero_page_mode,
            ) {
                Ok(nudged) => nudged,
                Err(err) => {
                    ctx.events.push(CoreEvent::StatusMessage(err));
                    return;
                }
            };

            tier1_updates.push((t.site_addr, nudged));
            tier1_sites.push((t.site_addr, t.raw_target, t.is_zero_page_mode, nudged));
            if let Some(second_addr) = t.second_pair_addr
                && second_addr != t.site_addr
                && ctx
                    .state
                    .annotations
                    .get(second_addr)
                    .and_then(|e| e.operand_offset)
                    .is_some()
            {
                tier1_updates.push((second_addr, None));
            }
        }

        let mut tier2_updates = Vec::with_capacity(tier2_reqs.len());
        let mut first_tier2_site = None;
        for (target_addr, (site_addr, is_zp)) in tier2_reqs {
            if first_tier2_site.is_none() {
                first_tier2_site = Some(site_addr);
            }
            let current_tier2 = ctx
                .state
                .annotations
                .get(target_addr)
                .and_then(|e| e.address_offset);
            let nudged =
                match compute_nudged_symbolic_offset(target_addr, current_tier2, delta, is_zp) {
                    Ok(nudged) => nudged,
                    Err(err) => {
                        ctx.events.push(CoreEvent::StatusMessage(err));
                        return;
                    }
                };
            tier2_updates.push((target_addr, nudged));
        }

        let total_count = tier1_sites.len() + tier2_updates.len();
        let single_tier1 =
            (tier1_sites.len() == 1 && tier2_updates.is_empty()).then(|| tier1_sites[0]);
        let single_tier2 = (tier1_sites.is_empty() && tier2_updates.len() == 1)
            .then(|| (tier2_updates[0].0, tier2_updates[0].1, first_tier2_site));

        ctx.preserve_cursor(|c| {
            c.state
                .apply_combined_symbolic_offsets(&tier1_updates, &tier2_updates, &[]);
        });

        let msg = if let Some((site_addr, raw_target, is_zp_mode, maybe_sym)) = single_tier1 {
            if let Some(sym) = maybe_sym {
                let expr = ctx
                    .state
                    .format_symbolic_offset(sym, Some(site_addr), is_zp_mode);
                format!("Symbolic offset: {expr}")
            } else {
                format!("Cleared symbolic offset for ${raw_target:04X}")
            }
        } else if let Some((target_addr, maybe_sym, site_opt)) = single_tier2 {
            if let Some(sym) = maybe_sym {
                let expr = ctx
                    .state
                    .format_symbolic_offset(sym, site_opt, target_addr.0 <= 0x00FF);
                format!("Global symbolic offset for ${target_addr:04X}: {expr}")
            } else {
                format!("Cleared symbolic offset at ${target_addr:04X}")
            }
        } else {
            format!("Nudged symbolic offset ({delta:+}) on {total_count} entries")
        };

        ctx.events.push(CoreEvent::StatusMessage(msg));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
        return;
    }

    if !tier2_addrs.is_empty() {
        let mut updates = Vec::with_capacity(tier2_addrs.len());
        for &target_addr in &tier2_addrs {
            let current = ctx
                .state
                .annotations
                .get(target_addr)
                .and_then(|e| e.address_offset);
            let nudged = match compute_nudged_symbolic_offset(
                target_addr,
                current,
                delta,
                target_addr.0 <= 0x00FF,
            ) {
                Ok(nudged) => nudged,
                Err(err) => {
                    ctx.events.push(CoreEvent::StatusMessage(err));
                    return;
                }
            };
            updates.push((target_addr, nudged));
        }

        let count = updates.len();
        let first_summary = (count == 1).then(|| updates[0]);

        ctx.preserve_cursor(|c| {
            c.state.apply_address_offsets(&updates, None);
        });

        let msg = if let Some((target_addr, maybe_sym)) = first_summary {
            if let Some(sym) = maybe_sym {
                let expr = ctx
                    .state
                    .format_symbolic_offset(sym, None, target_addr.0 <= 0x00FF);
                format!("Global symbolic offset for ${target_addr:04X}: {expr}")
            } else {
                format!("Cleared symbolic offset at ${target_addr:04X}")
            }
        } else {
            format!("Nudged global symbolic offset ({delta:+}) on {count} addresses")
        };

        ctx.events.push(CoreEvent::StatusMessage(msg));
        ctx.events.push(CoreEvent::StateChanged);
        ctx.events.push(CoreEvent::ViewChanged);
        return;
    }

    ctx.events.push(CoreEvent::StatusMessage(
        "No symbolic offset target at cursor".to_string(),
    ));
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

/// Pairs immediate load instructions into 16-bit low/high (`[`) or high/low (`]`) address operands.
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

                // Route through `build_set_immediate_format_commands` so any stale non-matching
                // Tier 1 `operand_offset` on `addr1` or `addr2` clears atomically in the same batch.
                batch_commands.extend(ctx.state.build_set_immediate_format_commands(
                    addr1,
                    Some(fmt1),
                    None,
                ));
                batch_commands.extend(ctx.state.build_set_immediate_format_commands(
                    addr2,
                    Some(fmt2),
                    None,
                ));

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

/// Cycles the immediate operand format on the current line. Calls
/// [`AppState::apply_immediate_format`](crate::state::AppState::apply_immediate_format) to clear
/// any stale Tier 1 `operand_offset` in the same undo batch.
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
        ctx.preserve_cursor(|c| {
            c.state.apply_immediate_format(address, Some(new_fmt));
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
                        ctx.state.set_bookmark(
                            address,
                            if is_bookmarked {
                                None
                            } else {
                                Some("Bookmark".to_string())
                            },
                        );

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
            AppAction::SymbolicOffsetDialog => {
                handle_symbolic_offset_dialog(ctx);
                Ok(true)
            }
            AppAction::NudgeSymbolicOffset { delta } => {
                handle_nudge_symbolic_offset(ctx, *delta);
                Ok(true)
            }
            AppAction::ApplySymbolicOffset {
                site_address,
                target_address,
                address_range,
                input,
                is_global,
                is_zero_page_mode,
            } => {
                let submission = SymbolicOffsetSubmission {
                    site_address: *site_address,
                    target_address: *target_address,
                    address_range: *address_range,
                    input,
                    is_global: *is_global,
                    is_zero_page_mode: *is_zero_page_mode,
                };
                handle_apply_symbolic_offset(ctx, &submission);
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

    #[test]
    fn test_nudge_symbolic_offset_single_instruction_and_external_equate() {
        let mut core = Core::new();
        // $1000: STA $FC (85 FC)
        // $1002: LDA $101F,X (BD 1F 10)
        // $1005: LDA $FF,X (B5 FF) -> 2-byte ZeroPageX boundary test
        // $1007: LDA $FFFF,X (BD FF FF) -> 16-bit $FFFF boundary test
        // $100A: RTS (60)
        let mut prg = vec![0xEA; 0x28];
        prg[0..11].copy_from_slice(&[
            0x85, 0xFC, 0xBD, 0x1F, 0x10, 0xB5, 0xFF, 0xBD, 0xFF, 0xFF, 0x60,
        ]);
        core.state.load_binary(Addr(0x1000), prg).unwrap();
        core.state.block_types = vec![crate::state::BlockType::DataByte; 0x28];
        core.state.block_types[0..11].fill(crate::state::BlockType::Code);
        core.state.labels.insert(
            Addr(0x00FB),
            vec![crate::state::Label {
                name: "zp_ptr".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.labels.insert(
            Addr(0x1020),
            vec![crate::state::Label {
                name: "table".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.settings.all_labels = true;
        core.state.perform_analysis();

        // 1. Nudge on $1002 (`LDA $101F,X`):
        //    delta: -1 -> Tier 1 `table - 1,x` (`base: $1020, offset: -1`)
        //    delta: +1 -> clears back to `None` (`offset == 0`)
        //    delta: +1 -> sets `offset: +1` (`base: $101E`)
        let idx_1002 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.view.cursor_index = idx_1002;

        let ev_dec = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(ev_dec.contains(&CoreEvent::StateChanged));
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x1020),
                offset: -1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );
        let line_1002 = core
            .state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(line_1002.operand, "table - 1,x");

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            None
        );

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x101E),
                offset: 1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );

        // 2. Nudge on external equate `$00FC` (`zpa_FC = $fc`):
        //    delta: +1 sets Tier 2 `address_offset` (`zp_ptr + 1`) globally and removes `$00FC` from preamble!
        let ext_fc_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FC)))
            .expect("zpa_FC preamble equate must exist before nudge");
        core.view.cursor_index = ext_fc_idx;
        let ev_ext = core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert!(ev_ext.contains(&CoreEvent::StateChanged));
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );
        assert!(
            core.state
                .disassembly
                .iter()
                .all(|l| l.external_label_address != Some(Addr(0x00FC))),
            "zpa_FC = $fc must be suppressed from preamble once Tier 2 address_offset is active"
        );
        let sta_fc_line = core
            .state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(sta_fc_line.operand, "zp_ptr + 1");

        // 3. Boundary guards:
        //    (a) 2-byte `LDA $FF,X` at `$1005` with `delta: -1` (`new_base = $0100 > $00FF`)
        let idx_zp_ff = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1005) && !l.bytes.is_empty())
            .unwrap();
        core.view.cursor_index = idx_zp_ff;
        let ev_zp_oob = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(!ev_zp_oob.contains(&CoreEvent::StateChanged));
        assert!(
            ev_zp_oob.iter().any(
                |e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("zero-page base"))
            )
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1005))
                .and_then(|e| e.operand_offset),
            None
        );

        //    (b) `LDA $FFFF,X` at `$1007` with `delta: -1` (`new_base = $10000` > `$FFFF`)
        let idx_ffff = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1007) && !l.bytes.is_empty())
            .unwrap();
        core.view.cursor_index = idx_ffff;
        let ev_16_oob = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(!ev_16_oob.contains(&CoreEvent::StateChanged));
        assert!(
            ev_16_oob.iter().any(
                |e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("$0000..=$FFFF"))
            )
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1007))
                .and_then(|e| e.operand_offset),
            None
        );
    }

    #[test]
    fn test_nudge_symbolic_offset_visual_selection_lohi_deduplication_and_relative_skip() {
        let mut core = Core::new();
        // $1000..=$1005: 3-entry `LoHiAddress` RTS dispatch table ($101F, $1023, $1027)
        //   lo bytes: 1F, 23, 27
        //   hi bytes: 10, 10, 10
        // $1006..=$100B: unrolled loop `LDA $101F,X` (BD 1F 10), `DEX` (CA), `BNE $1006` (D0 F9)
        let mut prg = vec![0xEA; 0x30];
        prg[0..6].copy_from_slice(&[0x1F, 0x23, 0x27, 0x10, 0x10, 0x10]);
        prg[6..12].copy_from_slice(&[0xBD, 0x1F, 0x10, 0xCA, 0xD0, 0xF9]);
        core.state.load_binary(Addr(0x1000), prg).unwrap();
        core.state.block_types = vec![crate::state::BlockType::Code; 0x30];
        core.state.block_types[0..6].fill(crate::state::BlockType::LoHiAddress);
        for (addr, name) in [
            (Addr(0x1020), "sub_a"),
            (Addr(0x1024), "sub_b"),
            (Addr(0x1028), "sub_c"),
        ] {
            core.state.labels.insert(
                addr,
                vec![crate::state::Label {
                    name: name.to_string(),
                    kind: crate::state::LabelKind::User,
                    label_type: crate::state::LabelType::Subroutine,
                }],
            );
        }
        core.state.perform_analysis();

        // Select from first-half `.byte <...` line ($1000) through `BNE $1006` ($100A),
        // spanning BOTH halves of the LoHiAddress table and the loop!
        let lo_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let bne_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x100A) && !l.bytes.is_empty())
            .unwrap();

        core.view.is_visual_mode = true;
        core.view.selection_start = Some(lo_idx);
        core.view.cursor_index = bne_idx;

        // First `-1` nudge: each of the 3 LoHiAddress pairs + `LDA $101F,X` nudged ONCE (`offset == -1`)!
        let ev1 = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(ev1.contains(&CoreEvent::StateChanged));

        for (site, expected_base) in [
            (Addr(0x1000), Addr(0x1020)),
            (Addr(0x1001), Addr(0x1024)),
            (Addr(0x1002), Addr(0x1028)),
            (Addr(0x1006), Addr(0x1020)),
        ] {
            assert_eq!(
                core.state
                    .annotations
                    .get(site)
                    .and_then(|e| e.operand_offset),
                Some(crate::state::SymbolicOffset {
                    base: expected_base,
                    offset: -1,
                    radix: crate::state::OffsetRadix::Decimal,
                }),
                "Expected single -1 nudge at {site:?}"
            );
        }

        // Second-half bytes ($1003..=$1005), DEX ($1009), and BNE ($100A) must remain unannotated!
        for untouched in [
            Addr(0x1003),
            Addr(0x1004),
            Addr(0x1005),
            Addr(0x1009),
            Addr(0x100A),
        ] {
            assert_eq!(
                core.state
                    .annotations
                    .get(untouched)
                    .and_then(|e| e.operand_offset),
                None,
                "{untouched:?} must not receive an operand_offset"
            );
        }

        // Visual selection MUST remain active so a second `-1` nudge advances offsets to `-2`!
        assert!(core.view.is_visual_mode);
        assert!(core.view.selection_start.is_some());

        let ev2 = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(ev2.contains(&CoreEvent::StateChanged));
        for (site, expected_base) in [
            (Addr(0x1000), Addr(0x1021)),
            (Addr(0x1001), Addr(0x1025)),
            (Addr(0x1002), Addr(0x1029)),
            (Addr(0x1006), Addr(0x1021)),
        ] {
            assert_eq!(
                core.state
                    .annotations
                    .get(site)
                    .and_then(|e| e.operand_offset),
                Some(crate::state::SymbolicOffset {
                    base: expected_base,
                    offset: -2,
                    radix: crate::state::OffsetRadix::Decimal,
                }),
                "Expected -2 offset after second nudge at {site:?}"
            );
        }
    }

    #[test]
    fn test_symbolic_offset_dialog_auto_suggest_and_set_label_prefill() {
        let mut core = Core::new();
        // $1000: STA $FC (85 FC)
        // $1002..=$1003: 1-entry LoHiAddress table ($C0FF -> FF C0)
        // $1004: NOP (EA) -> unsupported implied instruction
        // $1005..=$1006: internal struct bytes ($1005 = "struct_base", $1006 = struct_base + 1)
        let prg = vec![0x85, 0xFC, 0xFF, 0xC0, 0xEA, 0x00, 0x00];
        core.state.load_binary(Addr(0x1000), prg).unwrap();
        core.state.block_types = vec![
            crate::state::BlockType::Code,
            crate::state::BlockType::Code,
            crate::state::BlockType::LoHiAddress,
            crate::state::BlockType::LoHiAddress,
            crate::state::BlockType::Code,
            crate::state::BlockType::DataByte,
            crate::state::BlockType::DataByte,
        ];
        core.state.settings.bytes_per_line = 1;
        core.state.labels.insert(
            Addr(0x00FB),
            vec![crate::state::Label {
                name: "zp_ptr".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.labels.insert(
            Addr(0xC100),
            vec![crate::state::Label {
                name: "sub_C100".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::Subroutine,
            }],
        );
        core.state.labels.insert(
            Addr(0x1005),
            vec![crate::state::Label {
                name: "struct_base".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            }],
        );
        core.state.perform_analysis();

        // 1. `STA $FC` at `$1000` -> suggests `"zp_ptr + 1"` with `is_global == true`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let ev_sta = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_sta.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: Some(Addr(0x1000)),
                target_address: Addr(0x00FC),
                initial_input,
                is_global: true,
                lock_global: false,
                is_zero_page_mode: true,
                ..
            }) if initial_input == "zp_ptr + 1"
        )));

        // 2. `LoHiAddress` `$C0FF` at `$1002` -> suggests `"sub_C100 - 1"` with `is_global == false`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        let ev_lohi = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_lohi.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: Some(Addr(0x1002)),
                target_address: Addr(0xC0FF),
                initial_input,
                is_global: false,
                lock_global: false,
                is_zero_page_mode: false,
                ..
            }) if initial_input == "sub_C100 - 1"
        )));

        // 3. Unsupported implied `NOP` at `$1004` emits informative StatusMessage without opening dialog.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1004) && !l.bytes.is_empty())
            .unwrap();
        let ev_nop = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(
            !ev_nop
                .iter()
                .any(|e| matches!(e, CoreEvent::DialogRequested(_)))
        );
        assert!(ev_nop.iter().any(
            |e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("no address operand"))
        ));

        // 4. `SetLabel` prefill when Tier 2 `address_offset` is active (both internal `$1006` and external `$00FC`).
        core.state.apply_address_offsets(
            &[(
                Addr(0x1006),
                Some(crate::state::SymbolicOffset {
                    base: Addr(0x1005),
                    offset: 1,
                    radix: crate::state::OffsetRadix::Decimal,
                }),
            )],
            None,
        );
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1006) && !l.bytes.is_empty())
            .unwrap();
        let ev_lbl_internal = core.apply_action(AppAction::SetLabel);
        assert!(ev_lbl_internal.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::Label {
                address: Addr(0x1006),
                initial_name,
                is_local: false,
                ..
            }) if initial_name == "struct_base + 1"
        )));

        // Also verify external `$00FC` row when Tier 2 `zp_ptr + 1` is active on an external equate line.
        core.state.annotations.update(Addr(0x00FC), |e| {
            e.address_offset = Some(crate::state::SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: crate::state::OffsetRadix::Decimal,
            });
        });
        core.state.disassembly.insert(
            0,
            crate::disassembler::DisassemblyLine {
                address: Addr(0x00FC),
                bytes: Vec::new(),
                mnemonic: String::new(),
                operand: String::new(),
                comment: String::new(),
                line_comment: None,
                label: Some("zpa_FC".to_string()),
                opcode: None,
                show_bytes: false,
                target_address: None,
                external_label_address: Some(Addr(0x00FC)),
                is_collapsed: false,
            },
        );
        core.view.cursor_index = 0;
        let ev_lbl_fc = core.apply_action(AppAction::SetLabel);
        assert!(ev_lbl_fc.iter().any(|e| matches!(
            e,
            CoreEvent::DialogRequested(crate::event::DialogType::Label {
                address: Addr(0x00FC),
                initial_name,
                is_local: false,
                is_external: true,
            }) if initial_name == "zp_ptr + 1"
        )));
    }

    #[test]
    fn test_apply_symbolic_offset_struct_range_split_pair_clear_and_immediate_format_cleanup() {
        let mut core = Core::new();
        // $1000: LDA #$FF (A9 FF) -> LowByte($C0FF) + Tier 1 `sub_C100 - 1`
        // $1002: STA $FC (85 FC) -> shadowing Tier 1 test when ApplyLabel sets Tier 2 on $00FC
        // $1004..=$1005: LoHiAddress pair ($C0FF) with legacy Tier 1 on second half $1005
        // $1006..=$1009: 4-byte DataByte struct (`player` at $1006, fields at $1007..=$1009)
        let prg = vec![0xA9, 0xFF, 0x85, 0xFC, 0xFF, 0xC0, 0x01, 0x02, 0x03, 0x04];
        core.state.load_binary(Addr(0x1000), prg).unwrap();
        core.state.block_types = vec![
            crate::state::BlockType::Code,
            crate::state::BlockType::Code,
            crate::state::BlockType::Code,
            crate::state::BlockType::Code,
            crate::state::BlockType::LoHiAddress,
            crate::state::BlockType::LoHiAddress,
            crate::state::BlockType::DataByte,
            crate::state::BlockType::DataByte,
            crate::state::BlockType::DataByte,
            crate::state::BlockType::DataByte,
        ];
        for (addr, name) in [
            (Addr(0x00FA), "zp_alt"),
            (Addr(0x00FB), "zp_ptr"),
            (Addr(0xC100), "sub_C100"),
            (Addr(0x1006), "player"),
        ] {
            core.state.labels.insert(
                addr,
                vec![crate::state::Label {
                    name: name.to_string(),
                    kind: crate::state::LabelKind::User,
                    label_type: crate::state::LabelType::UserDefined,
                }],
            );
        }
        core.state.apply_immediate_format_with_offset(
            Addr(0x1000),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xC0FF))),
            Some(crate::state::SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: crate::state::OffsetRadix::Decimal,
            }),
        );

        // 1. Multi-address Tier 2 struct range `$1006..=$1009` with input `"player"` skips `$1006 == base`!
        let ev_struct = core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: None,
            target_address: Addr(0x1006),
            address_range: Some((Addr(0x1006), Addr(0x1009))),
            input: "player".to_string(),
            is_global: true,
            is_zero_page_mode: false,
        });
        assert!(ev_struct.contains(&CoreEvent::StateChanged));
        assert!(
            core.state
                .labels
                .get(&Addr(0x1006))
                .is_some_and(|v| v.iter().any(|l| l.name == "player")),
            "Struct base $1006 must keep its User label"
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1006))
                .and_then(|e| e.address_offset),
            None
        );
        for off in 1u16..=3 {
            assert_eq!(
                core.state
                    .annotations
                    .get(Addr(0x1006 + off))
                    .and_then(|e| e.address_offset),
                Some(crate::state::SymbolicOffset {
                    base: Addr(0x1006),
                    offset: off as i16,
                    radix: crate::state::OffsetRadix::Decimal,
                })
            );
        }

        // 2. Secondary split-pair `operand_offset` on `$1005` is cleared when applying Tier 1 via `$1004`/`$1005`.
        core.state.annotations.update(Addr(0x1005), |e| {
            e.operand_offset = Some(crate::state::SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: crate::state::OffsetRadix::Decimal,
            });
        });
        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: Some(Addr(0x1005)),
            target_address: Addr(0xC0FF),
            address_range: None,
            input: "$C100 - $1".to_string(),
            is_global: false,
            is_zero_page_mode: false,
        });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1004))
                .and_then(|e| e.operand_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: crate::state::OffsetRadix::Hex,
            })
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1005))
                .and_then(|e| e.operand_offset),
            None,
            "Secondary split-pair entry at $1005 must be cleared"
        );

        // 3. `ApplyLabel` with `"zp_ptr + 1"` while cursor is on `$1002` (`STA $FC`) clears shadowing Tier 1 at `$1002`!
        core.state.apply_operand_offsets(&[(
            Addr(0x1002),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x00FA),
                offset: 2,
                radix: crate::state::OffsetRadix::Decimal,
            }),
        )]);
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x00FC),
            name: "zp_ptr + 1".to_string(),
            is_local: false,
        });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            None
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );

        // 4. `NextImmediateFormat` on `$1000` (`LDA #<(sub_C100 - 1)`) clears stale `operand_offset` and Undo restores both!
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            None,
            "Cycling immediate format away from LowByte must clear stale operand_offset"
        );
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xC0FF)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );
    }

    #[test]
    fn test_lo_hi_packing_clears_stale_operand_offset_and_undo_redo_restores_atomically() {
        let mut core = Core::new();
        // $1000: LDA #$FF (A9 FF)
        // $1002: LDY #$D0 (A0 D0)
        let prg = vec![0xA9, 0xFF, 0xA0, 0xD0];
        core.state.load_binary(Addr(0x1000), prg).unwrap();
        core.state.block_types = vec![crate::state::BlockType::Code; 4];
        core.state.labels.insert(
            Addr(0xC100),
            vec![crate::state::Label {
                name: "sub_C100".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::Subroutine,
            }],
        );
        core.state.labels.insert(
            Addr(0xD100),
            vec![crate::state::Label {
                name: "sub_D100".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::Subroutine,
            }],
        );
        core.state.perform_analysis();

        // Seed `$1000` with `LowByte($C0FF)` + `operand_offset: sub_C100 - 1` (`$C0FF`),
        // and `$1002` with `HighByte($D000)` + `operand_offset: sub_D100 - $100` (`$D000`).
        let old_sym_1000 = crate::state::SymbolicOffset {
            base: Addr(0xC100),
            offset: -1,
            radix: crate::state::OffsetRadix::Decimal,
        };
        let old_sym_1002 = crate::state::SymbolicOffset {
            base: Addr(0xD100),
            offset: -256,
            radix: crate::state::OffsetRadix::Hex,
        };
        core.state.apply_immediate_format_with_offset(
            Addr(0x1000),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xC0FF))),
            Some(old_sym_1000),
        );
        core.state.apply_immediate_format_with_offset(
            Addr(0x1002),
            Some(crate::state::ImmediateFormat::HighByte(Addr(0xD000))),
            Some(old_sym_1002),
        );

        // Place cursor on `$1000` and press `[` (`PackLoHiAddress`) -> packs target `$D0FF`!
        // Neither `$C0FF` nor `$D000` matches `$D0FF`, so BOTH stale `operand_offset` entries
        // MUST be cleared in the same undoable batch!
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let ev_pack = core.apply_action(AppAction::PackLoHiAddress);
        assert!(ev_pack.contains(&CoreEvent::StateChanged));

        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xD0FF)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::HighByte(Addr(0xD0FF)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            None,
            "PackLoHiAddress must clear non-matching operand_offset on addr1"
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            None,
            "PackLoHiAddress must clear non-matching operand_offset on addr2"
        );

        // Single Undo MUST restore BOTH previous `immediate_format` AND `operand_offset` on `$1000` and `$1002`!
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xC0FF)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(old_sym_1000)
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::HighByte(Addr(0xD000)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(old_sym_1002)
        );

        // Redo MUST re-apply packed `$D0FF` formats and re-clear both stale `operand_offset` entries!
        core.apply_action(AppAction::Redo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.immediate_format),
            Some(crate::state::ImmediateFormat::LowByte(Addr(0xD0FF)))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            None
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            None
        );
    }

    #[test]
    fn test_tier2_zp_rejects_non_zp_base_while_tier1_forced_absolute_allows_it() {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        core.state.settings.all_labels = true;
        // $1000: LDA $FF (A5 FF), $1002: LDA $00FF,X (BD FF 00), $1005: RTS (60)
        core.state.raw_data = vec![0xA5, 0xFF, 0xBD, 0xFF, 0x00, 0x60];
        core.state.block_types = vec![crate::state::BlockType::Code; 6];
        core.state.labels.insert(
            Addr(0x0100),
            vec![crate::state::Label {
                name: "table_0100".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::Field,
            }],
        );
        let _ = core.state.perform_analysis();

        // 1. Pressing `-` on external equate `zpa_FF = $ff` rejects `base = $0100 > $00FF`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FF)))
            .unwrap();
        let ev_nudge = core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(ev_nudge.iter().any(|ev| matches!(
            ev,
            CoreEvent::StatusMessage(m) if m.contains("<= $00FF")
        )));
        assert!(
            core.state
                .annotations
                .get(Addr(0x00FF))
                .and_then(|e| e.address_offset)
                .is_none()
        );

        // 2. `validate_user_label_or_address_offset` (`l`) rejects `"$0100 - 1"` on `$00FF`.
        assert!(
            core.state
                .validate_user_label_or_address_offset(Addr(0x00FF), "$0100 - 1", false, None)
                .is_err()
        );

        // 3. `validate_symbolic_offset_submission` on 3-byte `BD FF 00` rejects `is_global: true`,
        //    and accepts `is_global: false` (Tier 1 `operand_offset`).
        let global_req = SymbolicOffsetSubmission {
            site_address: Some(Addr(0x1002)),
            target_address: Addr(0x00FF),
            address_range: None,
            input: "table_0100 - 1",
            is_global: true,
            is_zero_page_mode: false,
        };
        assert!(validate_symbolic_offset_submission(&core.state, &global_req).is_err());

        let tier1_req = SymbolicOffsetSubmission {
            is_global: false,
            ..global_req
        };
        assert!(validate_symbolic_offset_submission(&core.state, &tier1_req).is_ok());

        // 4. Suggestion and `+` nudge ignore legacy `base = $0100` Tier 1 on 2-byte `LDA $FF` (`$1000`).
        let legacy_non_zp = crate::state::SymbolicOffset {
            base: Addr(0x0100),
            offset: -1,
            radix: crate::state::OffsetRadix::Decimal,
        };
        core.state
            .annotations
            .update(Addr(0x1000), |e| e.operand_offset = Some(legacy_non_zp));
        assert_eq!(
            core.state
                .suggest_symbolic_offset(Some(Addr(0x1000)), Addr(0x00FF), true),
            (String::new(), true)
        );
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FE), 1))
        );
    }

    #[test]
    fn test_nudge_operand_with_active_tier2_address_offset_and_single_step_undo() {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        // $1000: LDA ($FB),Y (B1 FB), $1002: STA $FC (85 FC), $1004: LDA $FC (A5 FC), $1006: RTS (60)
        core.state.raw_data = vec![0xB1, 0xFB, 0x85, 0xFC, 0xA5, 0xFC, 0x60];
        core.state.block_types = vec![crate::state::BlockType::Code; 7];
        core.state.labels.insert(
            Addr(0x00FB),
            vec![crate::state::Label {
                name: "zp_ptr".to_string(),
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::ZeroPagePointer,
            }],
        );
        let _ = core.state.perform_analysis();

        // Apply default `o` -> `Enter` (`zp_ptr + 1`, Tier 2) on `STA $FC`.
        let (suggested, default_global) =
            core.state
                .suggest_symbolic_offset(Some(Addr(0x1002)), Addr(0x00FC), true);
        assert_eq!((suggested.as_str(), default_global), ("zp_ptr + 1", true));
        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: Some(Addr(0x1002)),
            target_address: Addr(0x00FC),
            address_range: None,
            input: suggested,
            is_global: true,
            is_zero_page_mode: true,
        });

        // Position cursor on `sta zp_ptr + 1` and press `+`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });

        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FA), 2))
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );

        // Press `-` twice. The first press restores `+ 1` and the second press shows `sta zpa_FC`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 1))
        );

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        let cleared_line = core
            .state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(cleared_line.operand, "zpa_FC");

        // Single-step `Undo` restores `+ 1`, then `+ 2`, then `+ 1`.
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 1))
        );
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FA), 2))
        );
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 1))
        );

        // Selecting both `STA $FC` and `LDA $FC` deduplicates Tier 2 nudging on `$00FC` once.
        let idx_1002 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1002) && !l.bytes.is_empty())
            .unwrap();
        let idx_1004 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1004) && !l.bytes.is_empty())
            .unwrap();
        core.view.selection_start = Some(idx_1002);
        core.view.cursor_index = idx_1004;
        core.view.is_visual_mode = true;
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FA), 2))
        );
    }

    #[test]
    fn test_symbolic_offset_uncollapsed_multi_entry_lines_and_grouped_selected_byte_structs() {
        let mut core = crate::Core::new();
        // Verify default `DocumentSettings` (`addresses_per_line == 5`, `bytes_per_line == 8`).
        assert_eq!(core.state.settings.addresses_per_line, 5);
        assert_eq!(core.state.settings.bytes_per_line, 8);

        core.state.origin = Addr(0x1000);
        // $1000..=$1005: 3-entry uncollapsed `.word` table ($101F, $101F, $101F)
        // $1006..=$100B: 3-entry uncollapsed `LoHiAddress` table ($101F, $101F, $101F)
        // $100C..=$100F: 4-byte grouped `player: .byte` struct
        // $1010..=$1019: 10-byte multi-line `enemy: .byte` struct (2 lines under `bytes_per_line = 8`)
        // $101A..=$101F: padding + `$1020: RTS` (`sub_1020`)
        let mut raw = vec![
            0x1F, 0x10, 0x1F, 0x10, 0x1F, 0x10, // $1000..=$1005
            0x1F, 0x1F, 0x1F, 0x10, 0x10, 0x10, // $1006..=$100B
            0x01, 0x02, 0x03, 0x04, // $100C..=$100F
        ];
        raw.extend([0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19]); // $1010..=$1019
        raw.extend(vec![0xEA; 6]); // $101A..=$101F
        raw.push(0x60); // $1020
        core.state.raw_data = raw;

        let mut btypes = vec![crate::state::BlockType::Address; 6];
        btypes.extend(vec![crate::state::BlockType::LoHiAddress; 6]);
        btypes.extend(vec![crate::state::BlockType::DataByte; 14]);
        btypes.extend(vec![crate::state::BlockType::Code; 7]);
        core.state.block_types = btypes;
        core.state.splitters.insert(Addr(0x1010));

        core.state.insert_user_labels(&[
            (0x100C, "player"),
            (0x1010, "enemy"),
            (0x1020, "sub_1020"),
        ]);
        let _ = core.state.perform_analysis();

        // 1. 3-entry uncollapsed `.word` line opens range `Some(($1000, $1005))` with `"-1"`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        let ev_word = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_word.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                address_range: Some((Addr(0x1000), Addr(0x1005))),
                initial_input,
                is_global: false,
                ..
            }) if initial_input == "-1"
        )));

        // 2. 3-entry uncollapsed `LoHiAddress` first-half line (`$1006..=$1008`) maps all 3 pairs.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1006) && !l.bytes.is_empty())
            .unwrap();
        let ev_lohi = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_lohi.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                address_range: Some((Addr(0x1006), Addr(0x1008))),
                initial_input,
                is_global: false,
                ..
            }) if initial_input == "-1"
        )));

        // 3. Grouped 4-byte `player: .byte` line opens `Some(($100C, $100F))` prefilled with `"player"`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x100C) && !l.bytes.is_empty())
            .unwrap();
        let ev_struct = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_struct.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                address_range: Some((Addr(0x100C), Addr(0x100F))),
                initial_input,
                is_global: true,
                lock_global: true,
                ..
            }) if initial_input == "player"
        )));

        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: None,
            target_address: Addr(0x100C),
            address_range: Some((Addr(0x100C), Addr(0x100F))),
            input: "player".to_string(),
            is_global: true,
            is_zero_page_mode: false,
        });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x100D))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x100C), 1))
        );

        // Pressing `+` and `-` on grouped `player: .byte` nudges `$100D..=$100F` and keeps `player`.
        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x100C) && !l.bytes.is_empty())
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x100D))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x100B), 2))
        );
        assert_eq!(
            core.state.labels.get(&Addr(0x100C)).unwrap()[0].name,
            "player"
        );
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: -1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x100D))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x100C), 1))
        );

        // 4. Visual Mode selection of multi-line `enemy: .byte` struct (`$1010..=$1019`) + `+`/`-`.
        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: None,
            target_address: Addr(0x1010),
            address_range: Some((Addr(0x1010), Addr(0x1019))),
            input: "enemy".to_string(),
            is_global: true,
            is_zero_page_mode: false,
        });
        let idx_1010 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1010) && !l.bytes.is_empty())
            .unwrap();
        let idx_1018 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1018) && !l.bytes.is_empty())
            .unwrap();
        core.view.selection_start = Some(idx_1010);
        core.view.cursor_index = idx_1018;
        core.view.is_visual_mode = true;
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1019))
                .and_then(|e| e.address_offset)
                .map(|s| s.offset),
            Some(10)
        );
    }

    fn build_two_equate_preamble_core() -> crate::Core {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        core.state.settings.all_labels = true;
        // $1000: JSR $1007 (20 07 10), $1003: STA $FC (85 FC), $1005: STA $FD (85 FD), $1007: RTS (60)
        core.state.raw_data = vec![0x20, 0x07, 0x10, 0x85, 0xFC, 0x85, 0xFD, 0x60];
        core.state.block_types = vec![crate::state::BlockType::Code; 8];
        let _ = core.state.perform_analysis();
        core
    }

    #[test]
    fn test_preamble_equate_suppression_reanchors_to_preceding_equate_or_body() {
        let mut core = build_two_equate_preamble_core();

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FD)))
            .unwrap();
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].external_label_address,
            Some(Addr(0x00FC))
        );
        core.apply_action(AppAction::Undo);

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FD)))
            .unwrap();
        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: None,
            target_address: Addr(0x00FD),
            address_range: None,
            input: "+1".to_string(),
            is_global: true,
            is_zero_page_mode: true,
        });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].external_label_address,
            Some(Addr(0x00FC))
        );
        core.apply_action(AppAction::Undo);

        core.view.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FD)))
            .unwrap();
        core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x00FD),
            name: "$fc + 1".to_string(),
            is_local: false,
        });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].external_label_address,
            Some(Addr(0x00FC))
        );

        // Suppressing the sole remaining preamble equate (`$00FC`) when `$00FB` is excluded
        // empties the preamble and falls back to the first navigable body row (`$1000`).
        core.state.excluded_addresses.insert(Addr(0x00FB));
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert!(
            core.state
                .disassembly
                .iter()
                .all(|l| l.external_label_address.is_none())
        );
        let current = &core.state.disassembly[core.view.cursor_index];
        assert!(current.is_navigable());
        assert_eq!(current.address, Addr(0x1000));
        assert!(!current.bytes.is_empty());
    }

    #[test]
    fn test_visual_mode_preamble_equate_selection_clears_and_reports_full_range() {
        let mut core = build_two_equate_preamble_core();

        let fc_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FC)))
            .unwrap();
        let fd_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0x00FD)))
            .unwrap();
        core.view.selection_start = Some(fc_idx);
        core.view.cursor_index = fd_idx;
        core.view.is_visual_mode = true;

        let open_events = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(open_events.contains(&CoreEvent::StatusMessage(
            "Set Symbolic Offset for $00FC..=$00FD".to_string()
        )));

        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert!(!core.view.is_visual_mode);
        assert!(core.view.selection_start.is_none());

        // Second nudge operates only on the remaining equate/cursor without touching body `$1000: JSR`.
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });
        assert!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        let jsr_line = core
            .state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(jsr_line.operand, "r_1007");
    }

    #[test]
    fn test_sub_cursor_and_scroll_coordinates_clamp_when_lines_or_labels_shrink() {
        let mut core = build_two_equate_preamble_core();

        core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x1000),
            name: "very_long_subroutine_entry_name".to_string(),
            is_local: false,
        });
        let body_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        core.view.cursor_index = body_idx;
        core.view.sub_cursor_index = 1;
        core.view.scroll_index = core.state.disassembly.len() + 10;
        core.view.scroll_sub_index = 99;

        core.apply_action(AppAction::ApplyLabel {
            address: Addr(0x1000),
            name: "main".to_string(),
            is_local: false,
        });
        assert_eq!(core.view.sub_cursor_index, 0);
        assert_eq!(core.view.scroll_index, core.state.disassembly.len() - 1);
        assert_eq!(core.view.scroll_sub_index, 0);
    }

    #[test]
    fn test_nudge_symbolic_offset_targets_mid_instruction_sub_cursor_row() {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        // $1000: LDA $1234 (AD 34 12), $1003: STA $1001 (8D 01 10 -> SMC writes $1001), $1006: RTS (60)
        core.state.raw_data = vec![0xAD, 0x34, 0x12, 0x8D, 0x01, 0x10, 0x60];
        core.state.block_types = vec![crate::state::BlockType::Code; 7];
        core.state.insert_typed_user_labels(&[(
            0x1000,
            "smc_load",
            crate::state::LabelType::Subroutine,
        )]);
        let _ = core.state.perform_analysis();

        let lda_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        // Sub-row 0 is `smc_load = * + 1` (`f_1001 = * + 1` at `$1001`); find the sub-index for `$1001`.
        let line = &core.state.disassembly[lda_idx];
        let sub_idx = (0..10)
            .find(|&idx| line.address_at_sub_index(&core.state, idx) == Addr(0x1001))
            .expect("mid-instruction sub-row for $1001");

        core.view.cursor_index = lda_idx;
        core.view.sub_cursor_index = sub_idx;
        core.apply_action(AppAction::NudgeSymbolicOffset { delta: 1 });

        // `$1001`'s Tier 2 `address_offset` becomes `smc_load + 1`, while `$1000`'s operand (`$1234`) stays untouched.
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1001))
                .and_then(|e| e.address_offset),
            Some(crate::state::SymbolicOffset {
                base: Addr(0x1000),
                offset: 1,
                radix: crate::state::OffsetRadix::Decimal,
            })
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        let sta_line = core
            .state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(sta_line.operand, "smc_load + 1");
    }

    #[test]
    fn test_multi_line_tier2_promotion_clears_all_shadowing_tier1_offsets_and_undoes() {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        // $1000: LDA $FC (A5 FC), $1002: STA $FD (85 FD), $1004: RTS (60)
        core.state.raw_data = vec![0xA5, 0xFC, 0x85, 0xFD, 0x60];
        core.state.block_types = vec![crate::state::BlockType::Code; 5];
        core.state.insert_typed_user_labels(&[(
            0x00FB,
            "zp_ptr",
            crate::state::LabelType::ZeroPagePointer,
        )]);
        let _ = core.state.perform_analysis();

        // Seed shadowing Tier 1 `operand_offset` entries on BOTH `$1000` and `$1002`.
        let t1_1000 = crate::state::SymbolicOffset {
            base: Addr(0x00FA),
            offset: 2,
            radix: crate::state::OffsetRadix::Decimal,
        };
        let t1_1002 = crate::state::SymbolicOffset {
            base: Addr(0x00FA),
            offset: 3,
            radix: crate::state::OffsetRadix::Decimal,
        };
        core.state
            .apply_operand_offsets(&[(Addr(0x1000), Some(t1_1000)), (Addr(0x1002), Some(t1_1002))]);

        // Promote selection `$1000..=$1002` to global Tier 2 (`is_global: true`, `"zp_ptr"`).
        core.apply_action(AppAction::ApplySymbolicOffset {
            site_address: Some(Addr(0x1000)),
            target_address: Addr(0x00FC),
            address_range: Some((Addr(0x1000), Addr(0x1003))),
            input: "zp_ptr".to_string(),
            is_global: true,
            is_zero_page_mode: true,
        });

        assert!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 1))
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x00FD))
                .and_then(|e| e.address_offset)
                .map(|s| (s.base, s.offset)),
            Some((Addr(0x00FB), 2))
        );

        // Single Undo restores BOTH Tier 1 offsets and clears BOTH Tier 2 offsets.
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(t1_1000)
        );
        assert_eq!(
            core.state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(t1_1002)
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        assert!(
            core.state
                .annotations
                .get(Addr(0x00FD))
                .and_then(|e| e.address_offset)
                .is_none()
        );
    }

    #[test]
    fn test_symbolic_offset_dialog_on_collapsed_blocks_and_mid_instruction_sub_cursor() {
        let mut core = crate::Core::new();
        core.state.origin = Addr(0x1000);
        // $1000..=$1002: LDA $1234 (AD 34 12) with two mid-instruction labels ($1001, $1002)
        // $1003..=$1008: 3-pair LoHiAddress table ($100F, $100F, $100F)
        // $1009..=$100C: 4-byte DataByte struct (`player` at $1009)
        // $100D..=$100F: NOP padding, $1010: RTS (`sub_1010`)
        core.state.raw_data = vec![
            0xAD, 0x34, 0x12, // $1000..=$1002
            0x0F, 0x0F, 0x0F, 0x10, 0x10, 0x10, // $1003..=$1008
            0xAA, 0xBB, 0xCC, 0xDD, // $1009..=$100C
            0xEA, 0xEA, 0xEA, // $100D..=$100F
            0x60, // $1010
        ];
        let mut btypes = vec![crate::state::BlockType::Code; 3];
        btypes.extend(vec![crate::state::BlockType::LoHiAddress; 6]);
        btypes.extend(vec![crate::state::BlockType::DataByte; 4]);
        btypes.extend(vec![crate::state::BlockType::Code; 4]);
        core.state.block_types = btypes;
        core.state.insert_user_labels(&[
            (0x1000, "self_mod"),
            (0x1001, "mid_lo"),
            (0x1002, "mid_hi"),
            (0x1009, "player"),
            (0x1010, "sub_1010"),
        ]);
        core.state.collapsed_blocks = vec![(0x03, 0x08), (0x09, 0x0C)];
        core.state.disassemble();

        // 1. Collapsed `LoHiAddress` block (`line.is_collapsed == true`) opens multi-entry operand dialog.
        let idx_collapsed_lohi = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1003) && l.is_collapsed)
            .expect("collapsed LoHiAddress row");
        core.view.cursor_index = idx_collapsed_lohi;
        core.view.sub_cursor_index = 0;
        let ev_collapsed_lohi = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_collapsed_lohi.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: Some(Addr(0x1003)),
                target_address: Addr(0x100F),
                address_range: Some((Addr(0x1003), Addr(0x1008))),
                initial_input,
                is_global: false,
                lock_global: false,
                is_zero_page_mode: false,
            }) if initial_input == "-1"
        )));

        // 2. Collapsed `DataByte` struct block (`line.is_collapsed == true`) opens global struct range dialog.
        let idx_collapsed_struct = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1009) && l.is_collapsed)
            .expect("collapsed DataByte struct row");
        core.view.cursor_index = idx_collapsed_struct;
        let ev_collapsed_struct = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_collapsed_struct.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: None,
                target_address: Addr(0x1009),
                address_range: Some((Addr(0x1009), Addr(0x100C))),
                initial_input,
                is_global: true,
                lock_global: true,
                is_zero_page_mode: false,
            }) if initial_input == "player"
        )));

        // 3. Mid-instruction label sub-cursor (`sub_cursor_index = 1 > 0`, `sub_addr == $1002 != line.address`)
        //    opens a single-address global Tier 2 dialog for `$1002`, whereas the opcode sub-row (`2`) edits `$1234`.
        let idx_1000 = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .expect("$1000 instruction row");
        assert_eq!(
            core.state.disassembly[idx_1000].address_at_sub_index(&core.state, 1),
            Addr(0x1002)
        );
        core.view.cursor_index = idx_1000;
        core.view.sub_cursor_index = 1;
        let ev_mid_sub = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_mid_sub.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: None,
                target_address: Addr(0x1002),
                address_range: None,
                initial_input,
                is_global: true,
                lock_global: true,
                is_zero_page_mode: false,
            }) if initial_input == "mid_lo + 1"
        )));

        core.view.sub_cursor_index = 2;
        let ev_opcode_sub = core.apply_action(AppAction::SymbolicOffsetDialog);
        assert!(ev_opcode_sub.iter().any(|ev| matches!(
            ev,
            CoreEvent::DialogRequested(crate::event::DialogType::SymbolicOffset {
                site_address: Some(Addr(0x1000)),
                target_address: Addr(0x1234),
                address_range: None,
                lock_global: false,
                ..
            })
        )));
    }
}
