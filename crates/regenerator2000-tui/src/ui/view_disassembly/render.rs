//! Disassembly view rendering, line formatting, and Ratatui list construction.
//!
//! This module renders the interactive 6502 disassembly viewport at 60 FPS by combining:
//!
//! - Shared per-[`DisassemblyLine`] semantic lowering ([`DisassemblyLine::classify_kind`],
//!   [`DisassemblyLine::mid_instruction_labels`], [`DisassemblyLine::formatted_label`], and
//!   [`DisassemblyLine::label_placement`]) from [`regenerator2000_core::disassembler`].
//! - Viewport scroll stabilization ([`stabilize_viewport_scroll`]) and greedy control-flow
//!   arrow column allocation ([`compute_visible_arrows`]).
//! - Sub-row span construction for mid-instruction relative labels ([`render_mid_instruction_sub_rows`]),
//!   multi-line block comments ([`render_line_comment_sub_rows`]), standalone [`LabelPlacement::OwnLine`]
//!   labels, and main instruction/directive rows ([`render_instruction_sub_rows`]).
//! - Lexical scope tree gutter drawing (`┌─`, `│ `, `└─` via [`build_scope_gutter_span`]).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use regenerator2000_core::disassembler::formatter::Formatter;
use regenerator2000_core::disassembler::{
    CODE_COLUMN_WIDTH, Disassembler, DisassemblyLine, DisassemblyLineKind, LABEL_COLUMN_WIDTH,
    LabelPlacement,
};
use regenerator2000_core::state::AppState;

use super::layout::get_visual_line_count_for_instruction;
use crate::ui_state::{ActivePane, UIState};

/// Projected control-flow arrow overlapping the currently visible disassembly viewport.
#[derive(Clone, Copy)]
struct ArrowInfo {
    /// Source instruction index in `app_state.disassembly`.
    start: usize,
    /// Destination instruction index in `app_state.disassembly`.
    end: usize,
    /// Assigned horizontal arrow gutter column (`0..max_arrow_columns`).
    col: usize,
    /// Intermediate target byte address (`Some(mid_addr)`) when the branch/jump targets a
    /// mid-instruction relative label (`* + offset`) rather than `line.address` (`None`).
    target_addr: Option<u16>,
    /// Whether `start` lies inside the currently visible `[scroll_inst_idx..end_view)` window.
    start_visible: bool,
    /// Whether `end` lies inside the currently visible `[scroll_inst_idx..end_view)` window.
    end_visible: bool,
}

/// Distinguishes the main instruction row from pre-instruction sub-rows when rendering
/// the control-flow arrow gutter.
#[derive(Clone, Copy)]
enum ArrowRowKind {
    /// Main instruction, directive, splitter, or collapsed-block row.
    Instruction,
    /// Pre-instruction sub-row: mid-instruction label (`Some(mid_addr)`), line comment (`None`),
    /// or standalone [`LabelPlacement::OwnLine`] label (`None`).
    SubRow {
        /// Intermediate byte address for mid-instruction label sub-rows, or `None` for
        /// line comments and standalone own-line labels.
        sub_addr: Option<u16>,
    },
}

/// Lexical scope membership flags for a single [`DisassemblyLine`].
#[derive(Clone, Copy)]
struct ScopeBounds {
    /// `true` if this line is the first instruction of a lexical scope.
    is_scope_start: bool,
    /// `true` if this line is the closing delimiter or last instruction of a lexical scope.
    is_scope_end: bool,
    /// `true` if this line lies anywhere inside an active lexical scope.
    is_in_scope: bool,
}

/// Shared per-instruction rendering context passed to sub-row helper functions.
///
/// Groups immutable per-row references and geometry values so rendering helpers stay below the
/// Clippy parameter limit.
struct RowRenderContext<'a> {
    /// Read-only application state.
    app_state: &'a AppState,
    /// Read-only UI state (theme, active pane, cursor).
    ui_state: &'a UIState,
    /// Static assembler formatter for the active [`AppState::settings`].
    formatter: &'a dyn Formatter,
    /// Control-flow arrows overlapping the current viewport.
    active_arrows: &'a [ArrowInfo],
    /// Current [`DisassemblyLine`] being rendered.
    line: &'a DisassemblyLine,
    /// Index of `line` in `app_state.disassembly`.
    current_inst: usize,
    /// Character width of the control-flow arrow gutter (`max_arrow_columns * 2 + 1`).
    arrow_width: usize,
    /// Inner width of the disassembly [`List`] area (used to size `{splitter}` rulers).
    list_width: u16,
    /// Base style for `line` (including range-selection highlight and unexplored-block dimming).
    base_style: Style,
}

/// Formats a byte slice as space-separated uppercase two-digit hexadecimal values (e.g. `"A9 00"`).
fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Walks backwards `steps` visual sub-lines from `(start_inst, start_sub)`, skipping
/// non-navigable separator lines.
fn walk_back_visual_lines(
    app_state: &AppState,
    start_inst: usize,
    start_sub: usize,
    steps: usize,
) -> (usize, usize) {
    let mut curr_inst = start_inst;
    let mut curr_sub = start_sub;
    for _ in 0..steps {
        if curr_sub > 0 {
            curr_sub -= 1;
        } else if curr_inst > 0 {
            let mut prev_idx = curr_inst - 1;
            while prev_idx > 0 {
                let prev_line = &app_state.disassembly[prev_idx];
                if prev_line.is_navigable() {
                    break;
                }
                prev_idx -= 1;
            }
            curr_inst = prev_idx;
            let prev_line = &app_state.disassembly[curr_inst];
            if prev_line.is_navigable() {
                let prev_lines = get_visual_line_count_for_instruction(prev_line, app_state);
                curr_sub = prev_lines.saturating_sub(1);
            } else {
                return (0, 0);
            }
        } else {
            return (0, 0);
        }
    }
    (curr_inst, curr_sub)
}

/// Clamps cursor and viewport scroll indices using a vertical scrolloff margin so the
/// active cursor remains comfortably visible inside `visible_height`.
fn stabilize_viewport_scroll(
    app_state: &AppState,
    ui_state: &mut UIState,
    visible_height: usize,
) -> (usize, usize) {
    let total_items = app_state.disassembly.len();

    if ui_state.cursor_index >= total_items {
        ui_state.cursor_index = total_items.saturating_sub(1);
        ui_state.sub_cursor_index = 0;
    }

    let mut scroll_inst_idx = ui_state.scroll_index;
    let mut scroll_sub_idx = ui_state.scroll_sub_index;

    if scroll_inst_idx >= total_items {
        scroll_inst_idx = total_items.saturating_sub(1);
        scroll_sub_idx = 0;
    }

    let margin = 5.min(visible_height / 3);
    let (max_scroll_inst, max_scroll_sub) = walk_back_visual_lines(
        app_state,
        ui_state.cursor_index,
        ui_state.sub_cursor_index,
        margin,
    );

    let window_size = visible_height.saturating_sub(2 * margin).saturating_sub(1);
    let (min_scroll_inst, min_scroll_sub) = if window_size > 0 {
        walk_back_visual_lines(app_state, max_scroll_inst, max_scroll_sub, window_size)
    } else {
        (max_scroll_inst, max_scroll_sub)
    };

    if scroll_inst_idx > max_scroll_inst
        || (scroll_inst_idx == max_scroll_inst && scroll_sub_idx > max_scroll_sub)
    {
        scroll_inst_idx = max_scroll_inst;
        scroll_sub_idx = max_scroll_sub;
    }

    if scroll_inst_idx < min_scroll_inst
        || (scroll_inst_idx == min_scroll_inst && scroll_sub_idx < min_scroll_sub)
    {
        scroll_inst_idx = min_scroll_inst;
        scroll_sub_idx = min_scroll_sub;
    }

    (scroll_inst_idx, scroll_sub_idx)
}

/// Returns the inclusive instruction-index span occupied by `arrow` in its assigned column.
fn arrow_visible_range(arrow: &ArrowInfo) -> (usize, usize) {
    if arrow.start_visible && arrow.end_visible {
        if arrow.start < arrow.end {
            (arrow.start, arrow.end)
        } else {
            (arrow.end, arrow.start)
        }
    } else if arrow.start_visible {
        if arrow.start < arrow.end {
            (arrow.start, arrow.start + 1)
        } else {
            (arrow.start.saturating_sub(1), arrow.start)
        }
    } else if arrow.end_visible {
        if arrow.start < arrow.end {
            (arrow.end.saturating_sub(1), arrow.end)
        } else {
            (arrow.end, arrow.end + 1)
        }
    } else {
        (0, 0)
    }
}

/// Projects `app_state.cached_arrows` onto the visible `[scroll_inst_idx..end_view)` window
/// and greedily assigns non-overlapping arrow gutter columns (`0..max_arrow_columns`).
fn compute_visible_arrows(
    app_state: &AppState,
    scroll_inst_idx: usize,
    scroll_sub_idx: usize,
    visible_height: usize,
) -> Vec<ArrowInfo> {
    let total_items = app_state.disassembly.len();
    let end_view = {
        let mut visible_rows_used = 0;
        let mut inst = scroll_inst_idx;
        if inst < total_items {
            let line = &app_state.disassembly[inst];
            if line.is_navigable() {
                let count = get_visual_line_count_for_instruction(line, app_state);
                visible_rows_used += count.saturating_sub(scroll_sub_idx);
            }
            inst += 1;
        }
        while inst < total_items && visible_rows_used < visible_height {
            let line = &app_state.disassembly[inst];
            if !line.is_navigable() {
                inst += 1;
                continue;
            }
            visible_rows_used += get_visual_line_count_for_instruction(line, app_state);
            inst += 1;
        }
        inst
    };

    let mut relevant_arrows: Vec<ArrowInfo> = Vec::with_capacity(app_state.cached_arrows.len());
    for arrow in &app_state.cached_arrows {
        let src_idx = arrow.start;
        let dst_idx = arrow.end;
        let low = std::cmp::min(src_idx, dst_idx);
        let high = std::cmp::max(src_idx, dst_idx);

        if low < end_view && high >= scroll_inst_idx {
            let target_addr_val = arrow.target_addr.map_or(0, |a| a.0);
            let exact_match = app_state
                .disassembly
                .get(dst_idx)
                .is_some_and(|line| line.address == target_addr_val);
            let relative_target = if exact_match { None } else { arrow.target_addr };

            relevant_arrows.push(ArrowInfo {
                start: src_idx,
                end: dst_idx,
                col: 0,
                target_addr: relative_target.map(|a| a.0),
                start_visible: src_idx >= scroll_inst_idx && src_idx < end_view,
                end_visible: dst_idx >= scroll_inst_idx && dst_idx < end_view,
            });
        }
    }

    relevant_arrows.sort_by_key(|a| (a.start as isize - a.end as isize).abs());

    let max_allowed_cols = app_state.settings.max_arrow_columns;
    let (full_arrows, mut partial_arrows): (Vec<_>, Vec<_>) = relevant_arrows
        .into_iter()
        .partition(|a| a.start_visible && a.end_visible);

    let mut final_arrows: Vec<ArrowInfo> = Vec::new();

    for mut arrow in full_arrows {
        let (range_low, range_high) = if arrow.start < arrow.end {
            (arrow.start, arrow.end)
        } else {
            (arrow.end, arrow.start)
        };

        let mut best_col = None;
        let mut col = (max_allowed_cols as isize) - 1;
        while col >= 0 {
            let has_conflict = final_arrows.iter().any(|a| {
                if a.col != col as usize {
                    return false;
                }
                let (a_low, a_high) = arrow_visible_range(a);
                !(a_high < range_low || a_low > range_high)
            });
            if !has_conflict {
                best_col = Some(col as usize);
                break;
            }
            col -= 1;
        }

        if let Some(c) = best_col {
            arrow.col = c;
            final_arrows.push(arrow);
        } else {
            partial_arrows.push(arrow);
        }
    }

    for mut arrow in partial_arrows {
        let (range_low, range_high) = if arrow.start_visible || arrow.end_visible {
            arrow_visible_range(&arrow)
        } else {
            continue;
        };

        let mut best_col = None;
        for col in 0..max_allowed_cols {
            let has_conflict = final_arrows.iter().any(|a| {
                if a.col != col {
                    return false;
                }
                let (a_low, a_high) = arrow_visible_range(a);
                !(a_high < range_low || a_low > range_high)
            });
            if !has_conflict {
                best_col = Some(col);
                break;
            }
        }

        if let Some(c) = best_col {
            arrow.col = c;
            final_arrows.push(arrow);
        }
    }

    final_arrows
}

/// Renders the control-flow arrow glyph string for the main instruction row at `current_line`.
fn format_instruction_arrow_str(
    app_state: &AppState,
    active_arrows: &[ArrowInfo],
    current_line: usize,
) -> String {
    let cols = app_state.settings.max_arrow_columns;
    let mut chars = vec![' '; cols * 2 + 1];

    if active_arrows.is_empty() {
        return chars.iter().collect();
    }

    for arrow in active_arrows {
        let c_idx = arrow.col * 2;
        let is_down = arrow.start < arrow.end;
        let is_relative_target = arrow.target_addr.is_some() && current_line == arrow.end;
        let (low, high) = if is_down {
            (arrow.start, arrow.end)
        } else {
            (arrow.end, arrow.start)
        };

        if current_line > low && current_line < high {
            if chars[c_idx] == ' ' {
                chars[c_idx] = '│';
            } else if chars[c_idx] == '─' {
                chars[c_idx] = '┼';
            }
        }

        if is_relative_target && !is_down && current_line == arrow.end && chars[c_idx] == ' ' {
            chars[c_idx] = '│';
        }

        if arrow.start_visible && current_line == arrow.start {
            if app_state.disassembly[current_line].target_address.is_some() {
                chars[c_idx] = if is_down { '┌' } else { '└' };
                chars[c_idx + 1] = '─';
            }
        } else if arrow.end_visible && current_line == arrow.end && !is_relative_target {
            chars[c_idx] = if is_down { '└' } else { '┌' };
            chars[c_idx + 1] = '─';
        }

        if arrow.start_visible && !arrow.end_visible {
            if is_down && current_line == arrow.start + 1 {
                chars[c_idx] = '▼';
            } else if !is_down && current_line == arrow.start.saturating_sub(1) {
                chars[c_idx] = '▲';
            }
        } else if !arrow.start_visible
            && arrow.end_visible
            && ((is_down && current_line == arrow.end.saturating_sub(1))
                || (!is_down && current_line == arrow.end + 1))
        {
            chars[c_idx] = '│';
        }
    }

    for arrow in active_arrows {
        let is_relative_target = arrow.target_addr.is_some();
        let is_end_line = current_line == arrow.end;
        let is_start_line = current_line == arrow.start;
        let c_idx = arrow.col * 2;

        if arrow.start == arrow.end && current_line == arrow.start && arrow.start_visible {
            chars[c_idx] = '∞';
        }

        let is_valid_source = app_state.disassembly[current_line].target_address.is_some();
        let safe_is_start_line = is_start_line && arrow.start_visible && is_valid_source;

        let draw_horizontal = if arrow.start == arrow.end {
            safe_is_start_line
        } else {
            safe_is_start_line || (is_end_line && arrow.end_visible && !is_relative_target)
        };

        if draw_horizontal {
            for c in chars.iter_mut().skip(c_idx + 1) {
                if *c == ' ' {
                    *c = '─';
                } else if *c == '│' {
                    *c = '┼';
                }
            }

            if is_end_line && arrow.end_visible {
                let last = chars.len() - 1;
                chars[last] = '►';
            }
        }
    }
    chars.iter().collect()
}

/// Renders the control-flow arrow glyph string for a pre-instruction sub-row (mid-instruction
/// relative label, full-line comment, or standalone `OwnLine` label).
fn format_comment_arrow_str(
    app_state: &AppState,
    active_arrows: &[ArrowInfo],
    current_line: usize,
    sub_addr: Option<u16>,
) -> String {
    let cols = app_state.settings.max_arrow_columns;
    let mut chars = vec![' '; cols * 2 + 1];
    if active_arrows.is_empty() {
        return chars.iter().collect();
    }

    for arrow in active_arrows {
        let c_idx = arrow.col * 2;
        let (low, high) = if arrow.start < arrow.end {
            (arrow.start, arrow.end)
        } else {
            (arrow.end, arrow.start)
        };

        let is_down = arrow.start < arrow.end;
        let strictly_inside = current_line > low && current_line < high;
        let boundary_pass = if is_down {
            current_line == arrow.end
        } else {
            current_line == arrow.start
        };

        let mut passes_through = strictly_inside || boundary_pass;
        let is_relative_target_elsewhere = arrow.target_addr.is_some() && arrow.end == current_line;

        if is_relative_target_elsewhere
            && let Some(target) = arrow.target_addr
            && let Some(this_addr) = sub_addr
        {
            if arrow.start < arrow.end {
                passes_through = this_addr < target;
            } else {
                passes_through = this_addr > target;
            }
        }

        if passes_through {
            chars[c_idx] = '│';
        }

        let is_target_here = sub_addr.is_some_and(|addr| arrow.target_addr == Some(addr));

        if is_target_here {
            chars[c_idx] = if arrow.start < arrow.end {
                '└'
            } else {
                '┌'
            };
            chars[c_idx + 1] = '─';
        }
    }

    for arrow in active_arrows {
        let is_target_here = sub_addr.is_some_and(|addr| arrow.target_addr == Some(addr));

        if is_target_here {
            let c_idx = arrow.col * 2;
            for c in chars.iter_mut().skip(c_idx + 1) {
                if *c == ' ' {
                    *c = '─';
                } else if *c == '│' {
                    *c = '┼';
                }
            }
            let last = chars.len() - 1;
            chars[last] = '►';
        }
    }
    chars.iter().collect()
}

/// Builds the status indicator gutter span (`*`, `+`, `>`, `*>`) and control-flow arrow prefix
/// span for a single visual sub-row.
fn build_arrow_prefix_spans(
    ctx: &RowRenderContext<'_>,
    gutter: &'static str,
    gutter_style: Style,
    row_kind: ArrowRowKind,
) -> Vec<Span<'static>> {
    let arrow_padding = match row_kind {
        ArrowRowKind::Instruction => {
            format_instruction_arrow_str(ctx.app_state, ctx.active_arrows, ctx.current_inst)
        }
        ArrowRowKind::SubRow { sub_addr } => {
            format_comment_arrow_str(ctx.app_state, ctx.active_arrows, ctx.current_inst, sub_addr)
        }
    };
    let arrow_width = ctx.arrow_width;
    vec![
        Span::styled(gutter, gutter_style),
        Span::styled(
            format!("{arrow_padding:<arrow_width$} "),
            Style::default().fg(ctx.ui_state.theme.arrow),
        ),
    ]
}

/// Returns `true` when `line` can participate in a lexical scope box.
fn is_scope_eligible_line(line: &DisassemblyLine, comment_prefix: &str) -> bool {
    line.is_navigable()
        && !line.is_splitter()
        && line.external_label_address.is_none()
        && !line.is_enum_definition(comment_prefix)
}

/// Determines whether `ctx.line` starts, ends, or lies inside a lexical scope.
fn compute_scope_bounds(ctx: &RowRenderContext<'_>) -> ScopeBounds {
    let comment_prefix = ctx.formatter.comment_prefix();
    if !is_scope_eligible_line(ctx.line, comment_prefix) {
        return ScopeBounds {
            is_scope_start: false,
            is_scope_end: false,
            is_in_scope: false,
        };
    }

    let mut is_scope_start = false;
    let mut is_scope_end = false;
    let mut is_in_scope = false;

    if let Some(pend_str) = ctx.formatter.format_scope_end()
        && ctx.line.mnemonic == pend_str
    {
        is_scope_end = true;
        is_in_scope = true;
    }

    if !is_scope_end
        && let Some((&start, &end)) = ctx
            .app_state
            .annotations
            .scopes()
            .range(..=ctx.line.address)
            .next_back()
        && ctx.line.address >= start
        && ctx.line.address <= end
    {
        let has_prev_at_start = ctx.line.address == start
            && ctx.app_state.disassembly[..ctx.current_inst]
                .iter()
                .rev()
                .take_while(|l| l.address == start)
                .any(|prev| is_scope_eligible_line(prev, comment_prefix));
        let has_next_at_end = ctx.line.address == end
            && ctx.app_state.disassembly[ctx.current_inst + 1..]
                .iter()
                .take_while(|l| l.address == end)
                .any(|next| is_scope_eligible_line(next, comment_prefix));

        if ctx.line.address == start && !has_prev_at_start {
            is_scope_start = true;
            is_in_scope = true;
        } else if ctx.line.address == end {
            is_in_scope = true;
            if !has_next_at_end && ctx.formatter.format_scope_end().is_none() {
                is_scope_end = true;
            }
        } else {
            is_in_scope = true;
        }
    }

    ScopeBounds {
        is_scope_start,
        is_scope_end,
        is_in_scope,
    }
}

/// Builds the 2-column lexical scope tree gutter span (`"┌─"`, `"│ "`, `"└─"`, or `"  "`)
/// for sub-row `idx`, honoring `prefix_parts_count` so comments and mid-instruction labels
/// above a scope start render outside the scope box (`"  "`).
fn build_scope_gutter_span(
    ctx: &RowRenderContext<'_>,
    scope: ScopeBounds,
    idx: usize,
    prefix_parts_count: usize,
    part_count: usize,
) -> Span<'static> {
    // At a scope start (`scope.is_scope_start`), `prefix_parts_count` marks the exact boundary
    // between pre-instruction rows (`0..prefix_parts_count`: mid-instruction labels and line
    // comments) and instruction rows (`prefix_parts_count..part_count`: optional `OwnLine` label
    // followed by the opcode row):
    // - `idx < prefix_parts_count`: render `"  "` (outside the scope bracket).
    // - `idx == prefix_parts_count`: render `"┌─"` (opens the scope on the `OwnLine` label or opcode).
    // - `idx > prefix_parts_count`: render `"│ "` (continues the scope down to the opcode row).
    let current_scope_char = if scope.is_scope_start {
        if idx < prefix_parts_count {
            "  "
        } else if idx == prefix_parts_count {
            "┌─"
        } else {
            "│ "
        }
    } else if scope.is_scope_end {
        if idx == part_count.saturating_sub(1) {
            "└─"
        } else {
            "│ "
        }
    } else if scope.is_in_scope {
        "│ "
    } else {
        "  "
    };

    Span::styled(
        current_scope_char,
        ctx.base_style.fg(ctx.ui_state.theme.block_scope_fg),
    )
}

/// Renders mid-instruction relative label sub-rows (`0..counts.labels`) using
/// [`DisassemblyLine::mid_instruction_labels`] and
/// [`MidInstructionLabel::format_definition`](regenerator2000_core::disassembler::MidInstructionLabel::format_definition),
/// padded to [`CODE_COLUMN_WIDTH`] (`41`) so trailing x-ref comments align with instruction comments.
fn render_mid_instruction_sub_rows(ctx: &RowRenderContext<'_>, parts: &mut Vec<Line<'static>>) {
    for mid in ctx.line.mid_instruction_labels(ctx.app_state) {
        let mid_addr = mid.address;
        let xref_str = if let Some(refs) = ctx.app_state.cross_refs.get(&mid_addr)
            && !refs.is_empty()
            && ctx.app_state.settings.max_xref_count > 0
        {
            format!(
                "{} {}",
                ctx.formatter.comment_prefix(),
                crate::disassembler::format_cross_references(
                    refs,
                    ctx.app_state.settings.max_xref_count,
                )
            )
        } else {
            String::new()
        };

        let is_bookmarked = ctx
            .app_state
            .annotations
            .get(mid_addr)
            .and_then(|e| e.bookmark.as_ref())
            .is_some();
        let has_breakpoint = ctx.app_state.vice_state.has_breakpoint_at(mid_addr.0);
        let gutter = if has_breakpoint {
            "  *  "
        } else if is_bookmarked {
            "  +  "
        } else {
            "     "
        };
        let gutter_style = if has_breakpoint {
            ctx.base_style
                .fg(ctx.ui_state.theme.error_fg)
                .add_modifier(Modifier::BOLD)
        } else if is_bookmarked {
            ctx.base_style.fg(ctx.ui_state.theme.label)
        } else {
            ctx.base_style.fg(ctx.ui_state.theme.bytes)
        };

        let label_def = mid.format_definition(ctx.formatter);
        let mut spans = build_arrow_prefix_spans(
            ctx,
            gutter,
            gutter_style,
            ArrowRowKind::SubRow {
                sub_addr: Some(mid_addr.0),
            },
        );
        // 17 spaces equal the 7-column `$XXXX  ` address field plus the 10-column hex bytes field.
        spans.push(Span::styled(
            "                 ".to_string(),
            ctx.base_style,
        ));
        // Pad the relative label definition (`label = * + offset`) to `CODE_COLUMN_WIDTH` (`41`)
        // so any trailing `; x-ref:` comment starts at the exact same horizontal column as
        // external equates and standard instructions (`20 + 6 + 15 = 41`).
        spans.push(Span::styled(
            format!("{label_def:<CODE_COLUMN_WIDTH$}"),
            ctx.base_style.fg(ctx.ui_state.theme.label_def),
        ));
        if !xref_str.is_empty() {
            spans.push(Span::styled(
                xref_str,
                ctx.base_style.fg(ctx.ui_state.theme.comment),
            ));
        }
        parts.push(Line::from(spans));
    }
}

/// Renders full-line comment sub-rows (`counts.labels..counts.labels + counts.comments`).
fn render_line_comment_sub_rows(ctx: &RowRenderContext<'_>, parts: &mut Vec<Line<'static>>) {
    // External equate rows always occupy a single visual line (`VisualLineCounts { 0, 0, 1 }`)
    // and never render `line_comment` sub-rows.
    if ctx.line.external_label_address.is_some() {
        return;
    }
    if let Some(line_comment) = &ctx.line.line_comment {
        for comment_part in line_comment.lines() {
            let mut spans = build_arrow_prefix_spans(
                ctx,
                "     ",
                ctx.base_style.fg(ctx.ui_state.theme.bytes),
                ArrowRowKind::SubRow { sub_addr: None },
            );
            spans.push(Span::styled(
                "                 ".to_string(),
                ctx.base_style,
            ));
            spans.push(Span::styled(
                format!("{} {}", ctx.formatter.comment_prefix(), comment_part),
                ctx.base_style.fg(ctx.ui_state.theme.comment),
            ));
            parts.push(Line::from(spans));
        }
    }
}

/// Computes the 5-character status indicator gutter (`" *>  "`, `"  >  "`, `"  *  "`, `"  +  "`,
/// or `"     "`) and its style for the main instruction row.
fn compute_instruction_gutter(ctx: &RowRenderContext<'_>) -> (&'static str, Style) {
    let addr = ctx.line.address;
    let is_bookmarked = ctx
        .app_state
        .annotations
        .get(addr)
        .and_then(|e| e.bookmark.as_ref())
        .is_some();
    let is_pc = ctx.app_state.vice_state.pc == Some(addr.0);
    let has_breakpoint = ctx.app_state.vice_state.has_breakpoint_at(addr.0);

    let gutter = if is_pc && has_breakpoint {
        " *>  "
    } else if is_pc {
        "  >  "
    } else if has_breakpoint {
        "  *  "
    } else if is_bookmarked {
        "  +  "
    } else {
        "     "
    };

    let gutter_style = if (is_pc && has_breakpoint) || has_breakpoint {
        ctx.base_style
            .fg(ctx.ui_state.theme.error_fg)
            .add_modifier(Modifier::BOLD)
    } else if is_pc {
        ctx.base_style
            .fg(ctx.ui_state.theme.border_active)
            .add_modifier(Modifier::BOLD)
    } else if is_bookmarked {
        ctx.base_style.fg(ctx.ui_state.theme.label)
    } else {
        ctx.base_style.fg(ctx.ui_state.theme.bytes)
    };

    (gutter, gutter_style)
}

/// Appends the trailing side comment span (`"{comment_prefix} {comment}"` or `""`) to `inst_spans`.
fn append_side_comment_span(ctx: &RowRenderContext<'_>, inst_spans: &mut Vec<Span<'static>>) {
    let text = if ctx.line.comment.is_empty() {
        String::new()
    } else {
        format!("{} {}", ctx.formatter.comment_prefix(), ctx.line.comment)
    };
    inst_spans.push(Span::styled(
        text,
        ctx.base_style.fg(ctx.ui_state.theme.comment),
    ));
}

/// Renders the standalone `OwnLine` label sub-row (when [`LabelPlacement::OwnLine`] holds) and the
/// main instruction, directive, equate, scope delimiter, collapsed block, or splitter sub-row.
fn render_instruction_sub_rows(ctx: &RowRenderContext<'_>, parts: &mut Vec<Line<'static>>) {
    let line = ctx.line;
    let comment_prefix = ctx.formatter.comment_prefix();
    let kind = line.classify_kind(comment_prefix);
    let label_text = line.formatted_label(ctx.formatter).unwrap_or_default();
    let label_placement = line.label_placement(ctx.formatter);

    // When `LabelPlacement::OwnLine` holds (`formatted_label.len() >= LABEL_COLUMN_WIDTH` or an
    // unindented `{` / `}` scope delimiter), emit the standalone label at sub-index `L + C`
    // (`counts.own_line_label_sub_index()`) immediately before the main opcode row at `L + C + 1`
    // (`counts.opcode_sub_index()`).
    if label_placement == LabelPlacement::OwnLine && !label_text.is_empty() {
        let mut own_label_spans = build_arrow_prefix_spans(
            ctx,
            "     ",
            ctx.base_style,
            ArrowRowKind::SubRow { sub_addr: None },
        );
        own_label_spans.push(Span::styled(
            "                 ".to_string(),
            ctx.base_style,
        ));
        own_label_spans.push(Span::styled(
            label_text.clone(),
            ctx.base_style
                .fg(ctx.ui_state.theme.label_def)
                .add_modifier(Modifier::BOLD),
        ));
        parts.push(Line::from(own_label_spans));
    }

    let show_address = !line.bytes.is_empty() || line.is_collapsed || line.label.is_some();
    let address_str = if show_address {
        format!("${:04X}  ", line.address)
    } else {
        "       ".to_string()
    };

    let (gutter, gutter_style) = compute_instruction_gutter(ctx);
    let mut inst_spans =
        build_arrow_prefix_spans(ctx, gutter, gutter_style, ArrowRowKind::Instruction);
    inst_spans.push(Span::styled(
        address_str.clone(),
        ctx.base_style.fg(ctx.ui_state.theme.address),
    ));
    inst_spans.push(Span::styled(
        format!(
            "{: <10}",
            if line.show_bytes {
                hex_bytes(&line.bytes)
            } else {
                String::new()
            }
        ),
        ctx.base_style.fg(ctx.ui_state.theme.bytes),
    ));

    match kind {
        DisassemblyLineKind::Splitter => {
            let prefix_len = gutter.len() + ctx.arrow_width + address_str.len() + 10;
            let ruler_len = (ctx.list_width as usize).saturating_sub(prefix_len + 1);
            let ruler = "─".repeat(ruler_len);
            inst_spans.push(Span::styled(
                ruler,
                ctx.base_style
                    .fg(ctx.ui_state.theme.comment)
                    .add_modifier(Modifier::DIM),
            ));
        }
        DisassemblyLineKind::CollapsedBlock => {
            inst_spans.push(Span::styled(
                line.mnemonic.clone(),
                ctx.base_style
                    .fg(ctx.ui_state.theme.collapsed_block)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        DisassemblyLineKind::ExternalEquate(_)
        | DisassemblyLineKind::EnumDefinition
        | DisassemblyLineKind::HeaderComment => {
            let style = match kind {
                DisassemblyLineKind::ExternalEquate(_) => ctx
                    .base_style
                    .fg(ctx.ui_state.theme.label_def)
                    .add_modifier(Modifier::BOLD),
                DisassemblyLineKind::EnumDefinition => {
                    ctx.base_style.fg(ctx.ui_state.theme.label_def)
                }
                _ => ctx.base_style.fg(ctx.ui_state.theme.comment),
            };
            // Pad `mnemonic` to `CODE_COLUMN_WIDTH` (`41`) so side comments on equates and
            // preamble rows align with instruction side comments (`20 + 6 + 15 = 41`).
            inst_spans.push(Span::styled(
                format!("{:<CODE_COLUMN_WIDTH$}", line.mnemonic),
                style,
            ));
            append_side_comment_span(ctx, &mut inst_spans);
        }
        DisassemblyLineKind::ScopeDelimiter => {
            inst_spans.push(Span::styled(
                format!("{:<LABEL_COLUMN_WIDTH$}", line.mnemonic),
                ctx.base_style
                    .fg(ctx.ui_state.theme.mnemonic)
                    .add_modifier(Modifier::BOLD),
            ));
            inst_spans.push(Span::styled(
                "      ".to_string(),
                ctx.base_style
                    .fg(ctx.ui_state.theme.mnemonic)
                    .add_modifier(Modifier::BOLD),
            ));
            inst_spans.push(Span::styled(
                format!("{: <15}", line.operand),
                ctx.base_style.fg(ctx.ui_state.theme.operand),
            ));
            append_side_comment_span(ctx, &mut inst_spans);
        }
        DisassemblyLineKind::Blank | DisassemblyLineKind::Instruction => {
            let label_column_text = match label_placement {
                LabelPlacement::Inline => format!("{label_text: <LABEL_COLUMN_WIDTH$}"),
                LabelPlacement::None | LabelPlacement::OwnLine => {
                    format!("{: <width$}", "", width = LABEL_COLUMN_WIDTH)
                }
            };
            // The 3 code spans (`20` label + `6` mnemonic + `15` operand) sum to
            // `CODE_COLUMN_WIDTH` (`41`) columns before `append_side_comment_span`.
            inst_spans.push(Span::styled(
                label_column_text,
                ctx.base_style
                    .fg(ctx.ui_state.theme.label_def)
                    .add_modifier(Modifier::BOLD),
            ));
            inst_spans.push(Span::styled(
                format!("{: <5} ", line.mnemonic),
                ctx.base_style
                    .fg(ctx.ui_state.theme.mnemonic)
                    .add_modifier(Modifier::BOLD),
            ));
            inst_spans.push(Span::styled(
                format!("{: <15}", line.operand),
                ctx.base_style.fg(ctx.ui_state.theme.operand),
            ));
            append_side_comment_span(ctx, &mut inst_spans);
        }
    }

    parts.push(Line::from(inst_spans));
}

/// Renders the disassembly view widget into the specified frame area.
///
/// # Arguments
///
/// * `f` - Frame reference for rendering Ratatui widgets.
/// * `area` - Target rectangle [`Rect`] area for view output.
/// * `app_state` - Active application state [`AppState`].
/// * `ui_state` - Mutable UI state [`UIState`] for tracking cursor, scroll, and theme.
pub fn render_disassembly(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Disassembly;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let title = if app_state.vice_state.live_memory.is_some() && app_state.vice_state.connected {
        " Disassembly [LIVE] "
    } else {
        " Disassembly "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let list_area = block.inner(area);
    f.render_widget(block, area);

    let formatter = Disassembler::formatter(app_state.settings.assembler);
    let visible_height = list_area.height as usize;
    let total_items = app_state.disassembly.len();

    let (scroll_inst_idx, scroll_sub_idx) =
        stabilize_viewport_scroll(app_state, ui_state, visible_height);
    let active_arrows =
        compute_visible_arrows(app_state, scroll_inst_idx, scroll_sub_idx, visible_height);
    let arrow_width = (app_state.settings.max_arrow_columns * 2) + 1;

    let mut items = Vec::new();
    let mut current_inst = scroll_inst_idx;
    let mut current_sub = scroll_sub_idx;
    let mut processed_visual_lines = 0;

    while processed_visual_lines < visible_height && current_inst < total_items {
        let line = &app_state.disassembly[current_inst];
        let is_cursor_row = current_inst == ui_state.cursor_index;
        let is_selected_block = ui_state.selection_start.is_some_and(|selection_start| {
            let (start, end) = if selection_start < ui_state.cursor_index {
                (selection_start, ui_state.cursor_index)
            } else {
                (ui_state.cursor_index, selection_start)
            };
            current_inst >= start && current_inst <= end
        });

        let offset = line.address.0.saturating_sub(app_state.origin.0) as usize;
        let is_unexplored = line.external_label_address.is_none()
            && matches!(
                app_state.block_types.get(offset),
                Some(&regenerator2000_core::state::BlockType::Undefined)
            );

        let mut base_style = if is_selected_block {
            Style::default()
                .bg(ui_state.theme.selection_bg)
                .fg(ui_state.theme.selection_fg)
        } else {
            Style::default()
        };
        if is_unexplored && !is_selected_block {
            base_style = base_style.add_modifier(Modifier::DIM);
        }

        let ctx = RowRenderContext {
            app_state,
            ui_state,
            formatter,
            active_arrows: &active_arrows,
            line,
            current_inst,
            arrow_width,
            list_width: list_area.width,
            base_style,
        };

        let mut parts = Vec::new();
        render_mid_instruction_sub_rows(&ctx, &mut parts);
        render_line_comment_sub_rows(&ctx, &mut parts);
        // Record `prefix_parts_count` (`counts.labels + counts.comments`) before appending
        // instruction sub-rows so `build_scope_gutter_span` places the top scope corner `"┌─"`
        // on the `OwnLine` label (or main instruction row) while keeping preceding comments
        // and mid-instruction labels outside the scope bracket (`"  "`).
        let prefix_parts_count = parts.len();
        render_instruction_sub_rows(&ctx, &mut parts);

        let scope = compute_scope_bounds(&ctx);
        let part_count = parts.len();

        for (idx, mut part) in parts.into_iter().enumerate() {
            let scope_span =
                build_scope_gutter_span(&ctx, scope, idx, prefix_parts_count, part_count);
            part.spans.insert(0, scope_span);
            let manipulated_part = part;

            if idx >= current_sub {
                let is_cursor_sub = is_cursor_row && idx == ui_state.sub_cursor_index;
                let style = if is_cursor_sub && !is_selected_block {
                    base_style
                        .bg(ui_state.theme.selection_bg)
                        .remove_modifier(Modifier::DIM)
                } else {
                    base_style.remove_modifier(Modifier::DIM)
                };

                items.push(ListItem::new(manipulated_part).style(style));
                processed_visual_lines += 1;
                if processed_visual_lines >= visible_height {
                    break;
                }
            }
        }

        let mut next = current_inst + 1;
        while next < total_items {
            if app_state.disassembly[next].is_navigable() {
                break;
            }
            next += 1;
        }
        current_inst = next;
        current_sub = 0;
    }

    f.render_widget(List::new(items), list_area);

    ui_state.scroll_index = scroll_inst_idx;
    ui_state.scroll_sub_index = scroll_sub_idx;
    ui_state.disassembly_viewport_height = visible_height;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui::view_disassembly::layout::get_visual_line_counts;
    use crate::ui_state::UIState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use regenerator2000_core::disassembler::SPLITTER_MNEMONIC;
    use regenerator2000_core::state::{
        Addr, AppState, Assembler, BlockType, Label, LabelKind, LabelType,
    };

    #[test]
    fn test_render_disassembly_fills_viewport() {
        let mut app_state = AppState::new();
        let code = vec![0xEA; 100]; // 100 NOP instructions
        app_state.load_binary(Addr(0x0801), code).unwrap();

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 80, 24);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        // Inner area height is 24 - 2 (borders) = 22 lines.
        // Check that lines 1 to 22 all contain disassembly address '$' prefix
        for y in 1..=22 {
            let line_str: String = (0..80)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect();
            assert!(
                line_str.contains('$'),
                "Row {} should contain disassembly address '$', but was: '{}'",
                y,
                line_str
            );
        }
    }

    #[test]
    fn test_render_disassembly_label_alignment() {
        let mut app_state = AppState::new();
        let code = vec![0xAD, 0x00, 0x10, 0x8D, 0x00, 0x20];
        app_state.load_binary(Addr(0x1000), code).unwrap();
        app_state.block_types = vec![BlockType::Code; 6];
        app_state.disassemble();

        if let Some(line) = app_state.disassembly.get_mut(0) {
            line.label = Some("short_lbl".to_string());
        }
        if let Some(line) = app_state.disassembly.get_mut(1) {
            line.label = Some("extremely_long_label_name_exceeding_sixteen".to_string());
        }

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 100, 20);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        let mut short_lbl_line = String::new();
        let mut long_lbl_own_line = String::new();
        let mut long_lbl_inst_line = String::new();

        for y in 1..19 {
            let line_str: String = (0..100)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect();
            if line_str.contains("short_lbl") {
                short_lbl_line = line_str.clone();
            }
            if line_str.contains("extremely_long") {
                long_lbl_own_line = line_str;
                long_lbl_inst_line = (0..100)
                    .map(|x| {
                        buffer
                            .cell((x, y + 1))
                            .map_or(String::new(), |c| c.symbol().to_string())
                    })
                    .collect();
            }
        }

        let short_lbl_lower = short_lbl_line.to_lowercase();
        assert!(
            short_lbl_lower.contains("short_lbl"),
            "Short label should be rendered"
        );
        assert!(
            short_lbl_lower.contains("lda"),
            "Short label should be on same line as instruction"
        );

        let long_lbl_own_lower = long_lbl_own_line.to_lowercase();
        let long_lbl_inst_lower = long_lbl_inst_line.to_lowercase();
        assert!(
            long_lbl_own_lower.contains("extremely_long"),
            "Long label should be on its own line"
        );
        assert!(
            !long_lbl_own_lower.contains("sta"),
            "Long label line should NOT contain instruction mnemonic"
        );
        assert!(
            long_lbl_inst_lower.contains("sta"),
            "Instruction mnemonic should be on line following long label"
        );
    }

    // Verify scope gutter rendering (`┌─`, `│ `, `└─`) with indexed `app_state.annotations.scopes()`
    #[test]
    fn test_disassembly_render_scope_gutter_with_indexed_scopes() {
        for assembler in [Assembler::Tass64, Assembler::Acme] {
            let mut app_state = AppState::new();
            app_state.load_binary(Addr(0x1000), vec![0xEA; 8]).unwrap();
            app_state.block_types = vec![BlockType::Code; 8];
            app_state.settings.assembler = assembler;
            // Scope covering $1001..=$1003 with a line comment at $1001 (multi-part start)
            app_state.annotations.update(Addr(0x1001), |e| {
                e.scope = Some(Addr(0x1003));
                e.user_line_comment = Some("scope header comment".to_string());
            });
            app_state.disassemble();

            let mut ui_state = UIState::new(Theme::default());
            let backend = TestBackend::new(100, 20);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|f| {
                    let area = Rect::new(0, 0, 100, 20);
                    render_disassembly(f, area, &app_state, &mut ui_state);
                })
                .unwrap();

            let buffer = terminal.backend().buffer();
            let rendered_rows: Vec<String> = (1..19)
                .map(|y| {
                    (0..100)
                        .map(|x| {
                            buffer
                                .cell((x, y))
                                .map_or(String::new(), |c| c.symbol().to_string())
                        })
                        .collect()
                })
                .collect();

            assert!(
                rendered_rows.iter().any(|r| r.contains("┌─")),
                "Expected scope start gutter '┌─' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
            assert!(
                rendered_rows.iter().any(|r| r.contains("│ ")),
                "Expected scope middle gutter '│ ' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
            assert!(
                rendered_rows.iter().any(|r| r.contains("└─")),
                "Expected scope end gutter '└─' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
        }
    }

    #[test]
    fn test_render_scope_gutter_prefix_parts_count_with_own_line_label_and_comment() {
        // 1. ACME: scope starts directly on the instruction line (no separate `.proc` line),
        // exercising `prefix_parts_count` across a mid-instruction label, a line comment,
        // an `OwnLine` label, and the main opcode row produced by `app_state.disassemble()`.
        let mut app_state = AppState::new();
        // $1000: NOP (EA), $1001..=$1003: LDA $2000 (AD 00 20), $1004: RTS (60)
        app_state
            .load_binary(Addr(0x1000), vec![0xEA, 0xAD, 0x00, 0x20, 0x60])
            .unwrap();
        app_state.block_types = vec![BlockType::Code; 5];
        app_state.settings.assembler = Assembler::Acme;
        app_state.annotations.update(Addr(0x1001), |e| {
            e.scope = Some(Addr(0x1004));
            e.user_line_comment = Some("scope entry comment".to_string());
        });
        app_state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "twenty_four_char_label_x".to_string(), // OwnLine in ACME
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.labels.insert(
            Addr(0x1002),
            vec![Label {
                name: "mid_lbl".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(100, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_disassembly(f, Rect::new(0, 0, 100, 15), &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let scope_gutter_at = |buf: &ratatui::buffer::Buffer, y: u16| -> String {
            format!(
                "{}{}",
                buf.cell((1, y)).map_or("", |c| c.symbol()),
                buf.cell((2, y)).map_or("", |c| c.symbol()),
            )
        };

        // Row 1: $1000 NOP (outside scope) -> "  "
        assert_eq!(scope_gutter_at(buffer, 1), "  ");
        // Row 2: $1001 virtual splitter before scope -> "  "
        assert_eq!(scope_gutter_at(buffer, 2), "  ");
        // Row 3 (idx 0 < prefix_parts_count): mid-instruction label -> outside scope box ("  ")
        assert_eq!(scope_gutter_at(buffer, 3), "  ");
        // Row 4 (idx 1 < prefix_parts_count): line comment -> outside scope box ("  ")
        assert_eq!(scope_gutter_at(buffer, 4), "  ");
        // Row 5 (idx 2 == prefix_parts_count): OwnLine label -> opens scope box ("┌─")
        assert_eq!(scope_gutter_at(buffer, 5), "┌─");
        // Row 6 (idx 3 > prefix_parts_count): main instruction row -> continues scope box ("│ ")
        assert_eq!(scope_gutter_at(buffer, 6), "│ ");
        // Row 7: $1004 RTS -> closes ACME scope box ("└─")
        assert_eq!(scope_gutter_at(buffer, 7), "└─");

        // 2. ca65: `app_state.disassemble()` emits a `.proc` delimiter line carrying the line
        // comment at $1001, followed by the instruction with its mid-instruction label inside the scope.
        app_state.settings.assembler = Assembler::Ca65;
        app_state.disassemble();

        terminal
            .draw(|f| {
                render_disassembly(f, Rect::new(0, 0, 100, 15), &app_state, &mut ui_state);
            })
            .unwrap();

        let ca65_buf = terminal.backend().buffer();
        // Row 1: $1000 NOP -> "  "
        assert_eq!(scope_gutter_at(ca65_buf, 1), "  ");
        // Row 2: $1001 virtual splitter -> "  "
        assert_eq!(scope_gutter_at(ca65_buf, 2), "  ");
        // Row 3: line comment on `.proc` line -> "  "
        assert_eq!(scope_gutter_at(ca65_buf, 3), "  ");
        // Row 4: `.proc twenty_four_char_label_x` -> opens scope ("┌─")
        assert_eq!(scope_gutter_at(ca65_buf, 4), "┌─");
        // Row 5: mid-instruction label `mid_lbl := * + 1` inside scope -> "│ "
        assert_eq!(scope_gutter_at(ca65_buf, 5), "│ ");
        // Row 6: `lda $2000` inside scope -> "│ "
        assert_eq!(scope_gutter_at(ca65_buf, 6), "│ ");
        // Row 7: `rts` inside scope -> "│ "
        assert_eq!(scope_gutter_at(ca65_buf, 7), "│ ");
        // Row 8: `.endproc` closes scope -> "└─"
        assert_eq!(scope_gutter_at(ca65_buf, 8), "└─");
    }

    #[test]
    fn test_render_disassembly_mid_instruction_label_uses_formatter() {
        let mut app_state = AppState::new();
        // 3-byte instruction JMP $1001 (4C 01 10) at $1000
        app_state
            .load_binary(Addr(0x1000), vec![0x4C, 0x01, 0x10])
            .unwrap();
        app_state.block_types = vec![BlockType::Code; 3];
        app_state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "rel1".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );

        // 1. KickAssembler: expects `.label rel1 = * + 1`
        app_state.settings.assembler = Assembler::Kick;
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 100, 20);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let kick_rows: Vec<String> = (1..19)
            .map(|y| {
                (0..100)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map_or(String::new(), |c| c.symbol().to_string())
                    })
                    .collect()
            })
            .collect();
        assert!(
            kick_rows.iter().any(|r| r.contains(".label rel1 = * + 1")),
            "Expected KickAssembler mid-instruction label '.label rel1 = * + 1', got: {kick_rows:#?}"
        );
        assert!(
            !kick_rows.iter().any(|r| r.contains("rel1 =*+$01")),
            "KickAssembler should not render 'rel1 =*+$01'"
        );

        // 2. 64tass: expects `rel1 =*+$01`
        app_state.settings.assembler = Assembler::Tass64;
        app_state.disassemble();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 100, 20);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let tass_rows: Vec<String> = (1..19)
            .map(|y| {
                (0..100)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map_or(String::new(), |c| c.symbol().to_string())
                    })
                    .collect()
            })
            .collect();
        assert!(
            tass_rows.iter().any(|r| r.contains("rel1 =*+$01")),
            "Expected 64tass mid-instruction label 'rel1 =*+$01', got: {tass_rows:#?}"
        );
    }

    #[test]
    fn test_visual_line_counts_matches_rendered_rows_across_all_kinds_and_placements() {
        for assembler in [
            Assembler::Tass64,
            Assembler::Acme,
            Assembler::Ca65,
            Assembler::Kick,
        ] {
            let mut app_state = AppState::new();
            app_state.settings.assembler = assembler;
            app_state.labels.insert(
                Addr(0x1001),
                vec![
                    Label {
                        name: "a1001".to_string(),
                        kind: LabelKind::Auto,
                        label_type: LabelType::Field,
                    },
                    Label {
                        name: "mid_lbl".to_string(),
                        kind: LabelKind::User,
                        label_type: LabelType::UserDefined,
                    },
                ],
            );

            let comment_prefix = Disassembler::formatter(assembler).comment_prefix();

            let fixtures = vec![
                // 1. ExternalEquate (even with line_comment and long label -> 1 row)
                DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: "ext_lbl = $d020".to_string(),
                    operand: String::new(),
                    comment: "x-ref: $1000".to_string(),
                    line_comment: Some("ignored on external equate".to_string()),
                    label: Some("ignored_twenty_char_label".to_string()),
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: Some(Addr(0xD020)),
                    is_collapsed: false,
                },
                // 2a. EnumDefinition
                DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: "ENUM_VAL = $01".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 2b. KickAssembler enum closing brace at Addr::ZERO
                DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: "}".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 3. HeaderComment
                DisassemblyLine {
                    address: Addr::ZERO,
                    bytes: vec![],
                    mnemonic: format!("{comment_prefix} Header comment"),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 4. Splitter with line comment
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![],
                    mnemonic: SPLITTER_MNEMONIC.to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: Some("splitter note".to_string()),
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 5. CollapsedBlock with line comment
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![],
                    mnemonic: "; Collapsed [1000-1010]".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: Some("collapsed header".to_string()),
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: true,
                },
                // 6a. ScopeDelimiter without label
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![],
                    mnemonic: "{".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 6b. ScopeDelimiter with short label -> LabelPlacement::OwnLine
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![],
                    mnemonic: "{".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: Some("my_scope".to_string()),
                    opcode: None,
                    show_bytes: false,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 7a. Instruction with LabelPlacement::None
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![0xEA],
                    mnemonic: "NOP".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 7b. Instruction with LabelPlacement::Inline
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![0xEA],
                    mnemonic: "NOP".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: Some("short_lbl".to_string()),
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 7c. Instruction with 19-char label (Inline in Tass64/Acme, OwnLine in Ca65/Kick)
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![0xEA],
                    mnemonic: "NOP".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: Some("nineteen_char_label".to_string()),
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 7d. Instruction with 24-char label (OwnLine in all assemblers)
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![0xEA],
                    mnemonic: "NOP".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: Some("twenty_four_char_label_x".to_string()),
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                },
                // 7e. 3-byte instruction at $1000 with mid-instruction label at $1001, 2-line comment, and 19-char label
                DisassemblyLine {
                    address: Addr(0x1000),
                    bytes: vec![0x4C, 0x00, 0x20],
                    mnemonic: "JMP".to_string(),
                    operand: "$2000".to_string(),
                    comment: "jump".to_string(),
                    line_comment: Some("line 1\nline 2".to_string()),
                    label: Some("nineteen_char_label".to_string()),
                    opcode: None,
                    show_bytes: true,
                    target_address: Some(Addr(0x2000)),
                    external_label_address: None,
                    is_collapsed: false,
                },
            ];

            for (idx, fixture) in fixtures.into_iter().enumerate() {
                let expected_rows = get_visual_line_counts(&fixture, &app_state).total();
                let sentinel = DisassemblyLine {
                    address: Addr(0xFFFF),
                    bytes: vec![0xEA],
                    mnemonic: "NOP".to_string(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                };
                app_state.disassembly = vec![fixture, sentinel];

                let mut ui_state = UIState::new(Theme::default());
                let backend = TestBackend::new(120, 25);
                let mut terminal = Terminal::new(backend).unwrap();

                terminal
                    .draw(|f| {
                        let area = Rect::new(0, 0, 120, 25);
                        render_disassembly(f, area, &app_state, &mut ui_state);
                    })
                    .unwrap();

                let buffer = terminal.backend().buffer();
                let sentinel_y = (1..24)
                    .find(|&y| {
                        let row_str: String = (0..120)
                            .map(|x| {
                                buffer
                                    .cell((x, y))
                                    .map_or(String::new(), |c| c.symbol().to_string())
                            })
                            .collect();
                        row_str.to_lowercase().contains("$ffff")
                    })
                    .unwrap_or_else(|| {
                        panic!("Sentinel $FFFF not found for {assembler:?} fixture #{idx}")
                    });

                let rendered_row_count = (sentinel_y - 1) as usize;
                assert_eq!(
                    rendered_row_count, expected_rows,
                    "Visual row count mismatch for {assembler:?} fixture #{idx}"
                );
            }
        }
    }

    #[test]
    fn test_render_mid_instruction_label_xref_comment_aligned_to_code_column_width() {
        let mut app_state = AppState::new();
        app_state.settings.assembler = Assembler::Tass64;
        app_state.settings.max_xref_count = 3;
        app_state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "mid_lbl".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.cross_refs.insert(
            Addr(0x1001),
            vec![regenerator2000_core::state::XRef::new(
                Addr(0x1005),
                regenerator2000_core::state::XRefKind::Jump,
            )],
        );

        let ext_line = DisassemblyLine {
            address: Addr::ZERO,
            bytes: vec![],
            mnemonic: "border = $d020".to_string(),
            operand: String::new(),
            comment: "x-ref: $1000".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: Some(Addr(0xD020)),
            is_collapsed: false,
        };
        let inst_line = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0x4C, 0x00, 0x20],
            mnemonic: "JMP".to_string(),
            operand: "$2000".to_string(),
            comment: "main jump".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: Some(Addr(0x2000)),
            external_label_address: None,
            is_collapsed: false,
        };
        app_state.disassembly = vec![ext_line, inst_line];

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 120, 20);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let row_str = |y: u16| -> String {
            (0..120)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect()
        };

        let row_a = row_str(1); // ExternalEquate row
        let row_b = row_str(2); // Mid-instruction label row
        let row_c = row_str(3); // Main instruction row

        let col_a = row_a.find(';').expect("ExternalEquate row should have ';'");
        let col_b = row_b
            .find(';')
            .expect("Mid-instruction label row should have ';'");
        let col_c = row_c.find(';').expect("Instruction row should have ';'");

        assert_eq!(
            col_a, col_c,
            "ExternalEquate comment column ({col_a}) should match instruction comment column ({col_c})"
        );
        assert_eq!(
            col_b, col_c,
            "Mid-instruction comment column ({col_b}) should match instruction comment column ({col_c}) via CODE_COLUMN_WIDTH = {CODE_COLUMN_WIDTH}"
        );
    }

    #[test]
    fn test_disassemble_scope_start_splitter_and_kickasm_enum_gutter() {
        let mut app_state = AppState::new();
        app_state.load_binary(Addr(0x1000), vec![0xEA; 4]).unwrap();
        app_state.block_types = vec![BlockType::Code; 4];
        app_state.settings.assembler = Assembler::Acme;
        app_state.labels.insert(
            Addr(0x1001),
            vec![Label {
                name: "twenty_four_char_label_x".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.annotations.update(Addr(0x1001), |e| {
            e.scope = Some(Addr(0x1002));
            e.user_line_comment = Some("scope start comment".to_string());
        });
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(100, 15);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                render_disassembly(f, Rect::new(0, 0, 100, 15), &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let scope_gutter_at = |y: u16| -> String {
            format!(
                "{}{}",
                buffer.cell((1, y)).map_or("", |c| c.symbol()),
                buffer.cell((2, y)).map_or("", |c| c.symbol()),
            )
        };

        // Row 1: $1000 NOP (outside scope) -> "  "
        assert_eq!(scope_gutter_at(1), "  ");
        // Row 2: $1001 {splitter} before scope -> "  "
        assert_eq!(scope_gutter_at(2), "  ");
        // Row 3: $1001 line comment -> "  "
        assert_eq!(scope_gutter_at(3), "  ");
        // Row 4: $1001 OwnLine label -> opens scope ("┌─")
        assert_eq!(scope_gutter_at(4), "┌─");
        // Row 5: $1001 NOP instruction -> inside scope ("│ ")
        assert_eq!(scope_gutter_at(5), "│ ");
        // Row 6: $1002 NOP instruction -> closes ACME scope ("└─")
        assert_eq!(scope_gutter_at(6), "└─");

        // Verify KickAssembler enum closing brace `}` at Addr::ZERO gets "  "
        let mut kick_state = AppState::new();
        kick_state.settings.assembler = Assembler::Kick;
        kick_state.disassembly = vec![DisassemblyLine {
            address: Addr::ZERO,
            bytes: vec![],
            mnemonic: "}".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        }];

        terminal
            .draw(|f| {
                render_disassembly(f, Rect::new(0, 0, 100, 15), &kick_state, &mut ui_state);
            })
            .unwrap();

        let kick_buffer = terminal.backend().buffer();
        let kick_gutter = format!(
            "{}{}",
            kick_buffer.cell((1, 1)).map_or("", |c| c.symbol()),
            kick_buffer.cell((2, 1)).map_or("", |c| c.symbol()),
        );
        assert_eq!(kick_gutter, "  ");
    }
}
