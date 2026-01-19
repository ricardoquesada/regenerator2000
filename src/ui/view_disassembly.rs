use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct DisassemblyView;

impl Widget for DisassemblyView {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Disassembly;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Disassembly ")
            .style(
                Style::default()
                    .bg(ui_state.theme.background)
                    .fg(ui_state.theme.foreground),
            );
        let inner_area = block.inner(area);

        let formatter = app_state.get_formatter();

        let visible_height = inner_area.height as usize;
        let total_items = app_state.disassembly.len();
        let context_lines = visible_height / 2;
        let offset = ui_state.cursor_index.saturating_sub(context_lines);

        // --- Arrow Calculation Start ---
        // We want to find all arrows that overlap with the visible range: [offset, offset + visible_height]
        #[derive(Clone, Copy)]
        struct ArrowInfo {
            start: usize,
            end: usize,
            col: usize,
            target_addr: Option<u16>,
            start_visible: bool,
            end_visible: bool,
        }

        let end_view = offset + visible_height;

        // Optimization: Pre-calculate map for address -> index for relevant targets
        // Instead of full map, we just iterate.
        let mut relevant_arrows: Vec<ArrowInfo> = Vec::new();

        // Helper to find index
        let find_index = |addr: u16| -> Option<usize> {
            app_state
                .disassembly
                .binary_search_by_key(&addr, |l| l.address)
                .ok()
                .or_else(|| {
                    let idx = app_state.disassembly.partition_point(|l| l.address < addr);
                    if idx > 0 {
                        let prev = &app_state.disassembly[idx - 1];
                        let len = prev.bytes.len() as u16;
                        if addr >= prev.address && addr < prev.address.wrapping_add(len) {
                            return Some(idx - 1);
                        }
                    }
                    None
                })
        };

        for (src_idx, line) in app_state.disassembly.iter().enumerate() {
            if let Some(target_addr) = line.target_address {
                if let Some(opcode) = &line.opcode {
                    if opcode.mnemonic == "JMP"
                        && opcode.mode == crate::cpu::AddressingMode::Indirect
                    {
                        continue;
                    }
                } else if line.mnemonic.eq_ignore_ascii_case("JMP") && line.operand.contains('(') {
                    continue;
                }

                let dst_idx_opt = find_index(target_addr);

                // Logic to refine dst_idx and determine visibility
                // (Copied from ui.rs logic)
                if let Some(dst_idx) = dst_idx_opt {
                    let mut refined_dst = dst_idx;
                    if app_state
                        .disassembly
                        .binary_search_by_key(&target_addr, |l| l.address)
                        .is_ok()
                    {
                        // If multiple lines have same address (unlikely, but safe check)
                        while refined_dst > 0
                            && app_state.disassembly[refined_dst - 1].address == target_addr
                        {
                            refined_dst -= 1;
                        }
                    }

                    let low = std::cmp::min(src_idx, refined_dst);
                    let high = std::cmp::max(src_idx, refined_dst);

                    let is_visible = low < end_view && high >= offset;

                    if is_visible {
                        let relative_target = if dst_idx_opt == Some(refined_dst) {
                            if app_state
                                .disassembly
                                .binary_search_by_key(&target_addr, |l| l.address)
                                .is_err()
                            {
                                Some(target_addr)
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        relevant_arrows.push(ArrowInfo {
                            start: src_idx,
                            end: refined_dst,
                            col: 0,
                            target_addr: relative_target,
                            start_visible: src_idx >= offset && src_idx < end_view,
                            end_visible: refined_dst >= offset && refined_dst < end_view,
                        });
                    }
                }
            }
        }

        // Step 2: Assign columns to arrows

        // Sort: shortest arrows first? Or logic from ui.rs?
        // ui.rs: sorted_arrows.sort_by_key(|(src, dst, _)| (*src as isize - *dst as isize).abs());
        // Since we used struct ArrowInfo, we need to adapt.
        // We already pushed to relevant_arrows which is ArrowInfo.
        // We need to sort it.
        relevant_arrows.sort_by_key(|a| (a.start as isize - a.end as isize).abs());

        let max_allowed_cols = app_state.settings.max_arrow_columns;
        let view_start = offset;
        let view_end = offset + visible_height;

        // Split into Full and Partial
        let (full_arrows, mut partial_arrows): (Vec<_>, Vec<_>) =
            relevant_arrows.into_iter().partition(|a| {
                let start_visible = a.start >= view_start && a.start < view_end;
                let end_visible = a.end >= view_start && a.end < view_end;
                start_visible && end_visible
            });

        // 1. Process Full Arrows: Prefer Inner (Rightmost) columns.
        // Logic restarted below

        let mut final_arrows = Vec::new();

        for mut arrow in full_arrows {
            let (range_low, range_high) = if arrow.start < arrow.end {
                (arrow.start, arrow.end)
            } else {
                (arrow.end, arrow.start)
            };

            let mut best_col = None;
            let mut col = (max_allowed_cols as isize) - 1;
            while col >= 0 {
                let has_conflict = final_arrows.iter().any(|a: &ArrowInfo| {
                    if a.col != col as usize {
                        return false;
                    }
                    let (a_low, a_high) = if a.start_visible && a.end_visible {
                        if a.start < a.end {
                            (a.start, a.end)
                        } else {
                            (a.end, a.start)
                        }
                    } else if a.start_visible {
                        if a.start < a.end {
                            (a.start, a.start + 1)
                        } else {
                            (a.start.saturating_sub(1), a.start)
                        }
                    } else if a.end_visible {
                        if a.start < a.end {
                            (a.end.saturating_sub(1), a.end)
                        } else {
                            (a.end, a.end + 1)
                        }
                    } else {
                        (0, 0)
                    };
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

        // 2. Process Partial Arrows
        for mut arrow in partial_arrows {
            // Re-check start/end visible from struct fields as they are correct
            let (range_low, range_high) = if arrow.start_visible {
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
                continue;
            };

            let mut best_col = None;
            for col in 0..max_allowed_cols {
                let has_conflict = final_arrows.iter().any(|a| {
                    if a.col != col {
                        return false;
                    }
                    let (a_low, a_high) = if a.start_visible && a.end_visible {
                        if a.start < a.end {
                            (a.start, a.end)
                        } else {
                            (a.end, a.start)
                        }
                    } else if a.start_visible {
                        if a.start < a.end {
                            (a.start, a.start + 1)
                        } else {
                            (a.start.saturating_sub(1), a.start)
                        }
                    } else if a.end_visible {
                        if a.start < a.end {
                            (a.end.saturating_sub(1), a.end)
                        } else {
                            (a.end, a.end + 1)
                        }
                    } else {
                        (0, 0)
                    };
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

        let active_arrows = final_arrows;
        // --- Arrow Calculation End ---

        let arrow_width = (app_state.settings.max_arrow_columns * 2) + 1;

        let get_arrow_str = |current_line: usize| -> String {
            let cols = app_state.settings.max_arrow_columns;
            let mut chars = vec![' '; cols * 2 + 1];

            if active_arrows.is_empty() {
                return chars.iter().collect();
            }

            for arrow in &active_arrows {
                let c_idx = arrow.col * 2;
                let is_down = arrow.start < arrow.end;
                let is_relative_target = arrow.target_addr.is_some() && current_line == arrow.end;

                if arrow.start_visible && arrow.end_visible {
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

                    if is_relative_target
                        && !is_down
                        && current_line == arrow.end
                        && chars[c_idx] == ' '
                    {
                        chars[c_idx] = '│';
                    }

                    if current_line == arrow.start {
                        if app_state.disassembly[current_line].target_address.is_some() {
                            chars[c_idx] = if is_down { '┌' } else { '└' };
                            chars[c_idx + 1] = '─';
                        }
                    } else if current_line == arrow.end && !is_relative_target {
                        chars[c_idx] = if is_down { '└' } else { '┌' };
                        chars[c_idx + 1] = '─';
                    }
                } else if arrow.start_visible {
                    if current_line == arrow.start {
                        if app_state.disassembly[current_line].target_address.is_some() {
                            chars[c_idx] = if is_down { '┌' } else { '└' };
                            chars[c_idx + 1] = '─';
                        }
                    } else if is_down {
                        if current_line == arrow.start + 1 {
                            chars[c_idx] = '▼';
                        }
                    } else if current_line == arrow.start.saturating_sub(1) {
                        chars[c_idx] = '▲';
                    }
                } else if arrow.end_visible {
                    if current_line == arrow.end && !is_relative_target {
                        chars[c_idx] = if is_down { '└' } else { '┌' };
                        chars[c_idx + 1] = '─';
                    } else if is_down {
                        if current_line == arrow.end.saturating_sub(1) {
                            chars[c_idx] = '│';
                        }
                    } else if current_line == arrow.end + 1 {
                        chars[c_idx] = '│';
                    }
                }
            }

            for arrow in &active_arrows {
                let is_relative_target = arrow.target_addr.is_some();
                let is_end_line = current_line == arrow.end;
                let is_start_line = current_line == arrow.start;

                let c_idx = arrow.col * 2;

                if arrow.start == arrow.end && current_line == arrow.start && arrow.start_visible {
                    chars[c_idx] = '∞';
                }

                let is_valid_source = app_state.disassembly[current_line].target_address.is_some();
                let safe_is_start_line = is_start_line && is_valid_source;

                let draw_horizontal = if arrow.start == arrow.end {
                    arrow.start_visible && is_valid_source
                } else if arrow.start_visible && arrow.end_visible {
                    safe_is_start_line || (is_end_line && !is_relative_target)
                } else if arrow.start_visible {
                    safe_is_start_line
                } else if arrow.end_visible {
                    is_end_line
                } else {
                    false
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
        };

        let get_comment_arrow_str = |current_line: usize, sub_addr: Option<u16>| -> String {
            let cols = app_state.settings.max_arrow_columns;
            let mut chars = vec![' '; cols * 2 + 1];
            if active_arrows.is_empty() {
                return chars.iter().collect();
            }

            for arrow in &active_arrows {
                let c_idx = arrow.col * 2;
                let (low, high) = if arrow.start < arrow.end {
                    (arrow.start, arrow.end)
                } else {
                    (arrow.end, arrow.start)
                };

                let is_target_here = if let Some(addr) = sub_addr
                    && let Some(target) = arrow.target_addr
                {
                    addr == target
                } else {
                    false
                };

                let is_relative_target_elsewhere =
                    arrow.target_addr.is_some() && arrow.end == current_line;

                let mut passes_through = (current_line > low && current_line < high)
                    || (current_line == arrow.start && arrow.end < arrow.start)
                    || (current_line == arrow.end && arrow.start < arrow.end);

                if is_relative_target_elsewhere {
                    if arrow.start < arrow.end {
                        if let Some(this_addr) = sub_addr
                            && let Some(target) = arrow.target_addr
                        {
                            passes_through = this_addr < target;
                        } else if sub_addr.is_none() {
                            passes_through = false;
                        }
                    } else if let Some(this_addr) = sub_addr
                        && let Some(target) = arrow.target_addr
                    {
                        passes_through = this_addr < target;
                    } else if sub_addr.is_none() {
                        passes_through = true;
                    }
                }

                if passes_through {
                    chars[c_idx] = '│';
                }
                if is_target_here {
                    if arrow.start < arrow.end {
                        chars[c_idx] = '└';
                        chars[c_idx + 1] = '─';
                    } else {
                        chars[c_idx] = '┌'; // Visual same as above logic
                        chars[c_idx + 1] = '─';
                    }
                }
            }

            for arrow in &active_arrows {
                let is_target_here = if let Some(addr) = sub_addr
                    && let Some(target) = arrow.target_addr
                {
                    addr == target
                } else {
                    false
                };

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
        };

        let mut current_line_num: usize = 1;
        for i in 0..offset {
            if let Some(line) = app_state.disassembly.get(i) {
                if line.line_comment.is_some() {
                    current_line_num += 1;
                }
                current_line_num += 1;
            }
        }

        let items: Vec<ListItem> = app_state
            .disassembly
            .iter()
            .skip(offset)
            .take(visible_height)
            .enumerate()
            .map(|(local_i, line)| {
                let i = offset + local_i;
                let is_selected = if let Some(selection_start) = ui_state.selection_start {
                    let (start, end) = if selection_start < ui_state.cursor_index {
                        (selection_start, ui_state.cursor_index)
                    } else {
                        (ui_state.cursor_index, selection_start)
                    };
                    i >= start && i <= end
                } else {
                    false
                };

                let is_cursor_row = i == ui_state.cursor_index;
                let item_base_style = if is_selected {
                    Style::default()
                        .bg(ui_state.theme.selection_bg)
                        .fg(ui_state.theme.selection_fg)
                } else {
                    Style::default()
                };

                let label_text = if let Some(label) = &line.label {
                    formatter.format_label_definition(label)
                } else {
                    String::new()
                };

                let mut item_lines = Vec::new();
                let mut current_sub_index = 0;

                if line.bytes.len() > 1 {
                    for offset in 1..line.bytes.len() {
                        let mid_addr = line.address.wrapping_add(offset as u16);
                        if let Some(labels) = app_state.labels.get(&mid_addr) {
                            let xref_str = if let Some(refs) = app_state.cross_refs.get(&mid_addr) {
                                let mut all_refs = refs.clone();
                                if !all_refs.is_empty() && app_state.settings.max_xref_count > 0 {
                                    all_refs.sort_unstable();
                                    all_refs.dedup();
                                    let refs_str_list: Vec<String> = all_refs
                                        .iter()
                                        .take(app_state.settings.max_xref_count)
                                        .map(|r| format!("${:04x}", r))
                                        .collect();
                                    format!("; x-ref: {}", refs_str_list.join(", "))
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            for label in labels {
                                let is_highlighted = !is_selected
                                    && is_cursor_row
                                    && ui_state.sub_cursor_index == current_sub_index;
                                let line_style = if is_highlighted {
                                    Style::default().bg(ui_state.theme.selection_bg)
                                } else {
                                    item_base_style
                                };

                                let arrow_padding_for_rel =
                                    get_comment_arrow_str(i, Some(mid_addr));
                                let label_def = format!("{} =*+${:02x}", label.name, offset);

                                let mut spans = vec![
                                    Span::styled(
                                        format!("{:5} ", current_line_num),
                                        line_style.fg(ui_state.theme.bytes),
                                    ),
                                    Span::styled(
                                        format!(
                                            "{:<width$} ",
                                            arrow_padding_for_rel,
                                            width = arrow_width
                                        ),
                                        line_style.fg(ui_state.theme.arrow),
                                    ),
                                    Span::styled("                  ".to_string(), line_style),
                                    Span::styled(
                                        format!("{:<36}", label_def),
                                        line_style.fg(ui_state.theme.label_def),
                                    ),
                                ];

                                if !xref_str.is_empty() {
                                    spans.push(Span::styled(
                                        xref_str.clone(),
                                        line_style.fg(ui_state.theme.comment),
                                    ));
                                }

                                item_lines.push(Line::from(spans));
                                current_line_num += 1;
                                current_sub_index += 1;
                            }
                        }
                    }
                }

                let arrow_padding = get_arrow_str(i);

                if let Some(line_comment) = &line.line_comment {
                    let is_highlighted = !is_selected
                        && is_cursor_row
                        && ui_state.sub_cursor_index == current_sub_index;
                    let line_style = if is_highlighted {
                        Style::default().bg(ui_state.theme.selection_bg)
                    } else {
                        Style::default()
                    };

                    let comment_arrow_padding = get_comment_arrow_str(i, None);
                    item_lines.push(Line::from(vec![
                        Span::styled(
                            format!("{:5} ", current_line_num),
                            line_style.fg(ui_state.theme.bytes),
                        ),
                        Span::styled(
                            format!("{:width$} ", comment_arrow_padding, width = arrow_width),
                            line_style.fg(ui_state.theme.arrow),
                        ),
                        Span::styled(
                            format!("; {}", line_comment),
                            line_style.fg(ui_state.theme.comment),
                        ),
                    ]));
                    current_line_num += 1;
                    current_sub_index += 1;
                }

                let is_highlighted =
                    !is_selected && is_cursor_row && ui_state.sub_cursor_index == current_sub_index;
                let is_collapsed = line.is_collapsed;
                let line_style = if is_highlighted {
                    Style::default().bg(ui_state.theme.selection_bg)
                } else if is_collapsed {
                    Style::default().bg(ui_state.theme.collapsed_block_bg)
                } else {
                    Style::default()
                };

                let content = Line::from(vec![
                    Span::styled(
                        format!("{:5} ", current_line_num),
                        line_style.fg(ui_state.theme.bytes),
                    ),
                    Span::styled(
                        format!("{:<width$} ", arrow_padding, width = arrow_width),
                        line_style.fg(ui_state.theme.arrow),
                    ),
                    Span::styled(
                        format!(
                            "${:04X}{} ",
                            line.address,
                            if app_state.splitters.contains(&line.address) {
                                "*"
                            } else {
                                " "
                            }
                        ),
                        if let Some(next_line) = app_state.disassembly.get(i + 1)
                            && app_state.splitters.contains(&next_line.address)
                        {
                            line_style
                                .fg(ui_state.theme.address)
                                .add_modifier(Modifier::UNDERLINED)
                        } else if app_state.splitters.contains(&line.address) {
                            line_style
                                .fg(ui_state.theme.address)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            line_style.fg(ui_state.theme.address)
                        },
                    ),
                    Span::styled(
                        format!(
                            "{: <12}",
                            if line.show_bytes {
                                hex_bytes(&line.bytes)
                            } else {
                                String::new()
                            }
                        ),
                        line_style.fg(ui_state.theme.bytes),
                    ),
                    Span::styled(
                        format!("{: <16}", label_text),
                        line_style
                            .fg(ui_state.theme.label_def)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{: <4} ", line.mnemonic),
                        if is_collapsed {
                            line_style
                                .fg(ui_state.theme.collapsed_block)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            line_style
                                .fg(ui_state.theme.mnemonic)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    Span::styled(
                        format!("{: <15}", line.operand),
                        line_style.fg(ui_state.theme.operand),
                    ),
                    Span::styled(
                        if line.comment.is_empty() {
                            String::new()
                        } else {
                            format!("; {}", line.comment)
                        },
                        line_style.fg(ui_state.theme.comment),
                    ),
                ]);
                item_lines.push(content);
                current_line_num += 1;

                ListItem::new(item_lines).style(item_base_style)
            })
            .collect();

        let list = List::new(items).block(block);
        let mut state = ListState::default();
        if total_items > 0 {
            let local_cursor = ui_state.cursor_index.saturating_sub(offset);
            if local_cursor < visible_height {
                state.select(Some(local_cursor));
            }
        }
        f.render_stateful_widget(list, area, &mut state);
        ui_state.scroll_index = offset;
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Down | KeyCode::Char('j')
                if key.code == KeyCode::Down || key.modifiers.is_empty() =>
            {
                if key.modifiers == KeyModifiers::SHIFT || ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }

                let line = &app_state.disassembly[ui_state.cursor_index];
                let mut sub_count = 1;
                if app_state.user_line_comments.contains_key(&line.address) {
                    sub_count += 1;
                }
                if line.bytes.len() > 1 {
                    for offset in 1..line.bytes.len() {
                        let mid_addr = line.address.wrapping_add(offset as u16);
                        if let Some(labels) = app_state.labels.get(&mid_addr) {
                            sub_count += labels.len();
                        }
                    }
                }

                if ui_state.sub_cursor_index < sub_count - 1 {
                    ui_state.sub_cursor_index += 1;
                } else if ui_state.cursor_index < app_state.disassembly.len().saturating_sub(1) {
                    ui_state.cursor_index += 1;
                    ui_state.sub_cursor_index = 0;
                }
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if key.code == KeyCode::Up || key.modifiers.is_empty() =>
            {
                if key.modifiers == KeyModifiers::SHIFT || ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }

                if ui_state.sub_cursor_index > 0 {
                    ui_state.sub_cursor_index -= 1;
                } else if ui_state.cursor_index > 0 {
                    ui_state.cursor_index -= 1;
                    let line = &app_state.disassembly[ui_state.cursor_index];
                    let mut sub_count = 1;
                    if app_state.user_line_comments.contains_key(&line.address) {
                        sub_count += 1;
                    }
                    if line.bytes.len() > 1 {
                        for offset in 1..line.bytes.len() {
                            let mid_addr = line.address.wrapping_add(offset as u16);
                            if let Some(labels) = app_state.labels.get(&mid_addr) {
                                sub_count += labels.len();
                            }
                        }
                    }
                    ui_state.sub_cursor_index = sub_count - 1;
                }
                WidgetResult::Handled
            }
            KeyCode::PageDown => {
                ui_state.cursor_index =
                    (ui_state.cursor_index + 30).min(app_state.disassembly.len().saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                ui_state.cursor_index =
                    (ui_state.cursor_index + 30).min(app_state.disassembly.len().saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::PageUp => {
                ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Home => {
                ui_state.cursor_index = 0;
                WidgetResult::Handled
            }
            KeyCode::End => {
                ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1);
                WidgetResult::Handled
            }
            KeyCode::F(3) => {
                if key.modifiers == KeyModifiers::SHIFT {
                    WidgetResult::Action(crate::ui_state::MenuAction::FindPrevious)
                } else if key.modifiers.is_empty() {
                    WidgetResult::Action(crate::ui_state::MenuAction::FindNext)
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Char('/') if key.modifiers.is_empty() => {
                ui_state.vim_search_active = true;
                ui_state.vim_search_input.clear();
                WidgetResult::Handled
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                crate::ui::dialog_search::perform_search(app_state, ui_state, true);
                WidgetResult::Handled
            }
            KeyCode::Char('N') if key.modifiers == KeyModifiers::SHIFT => {
                crate::ui::dialog_search::perform_search(app_state, ui_state, false);
                WidgetResult::Handled
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(crate::ui_state::MenuAction::Search)
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
                let is_buffer_empty = ui_state.input_buffer.is_empty();
                ui_state.input_buffer.clear();

                let target_line = if is_buffer_empty {
                    app_state.disassembly.len()
                } else {
                    entered_number
                };

                let new_cursor = if target_line == 0 {
                    app_state.disassembly.len().saturating_sub(1)
                } else {
                    target_line
                        .saturating_sub(1)
                        .min(app_state.disassembly.len().saturating_sub(1))
                };

                if ui_state.is_visual_mode && ui_state.selection_start.is_none() {
                    ui_state.selection_start = Some(ui_state.cursor_index);
                }

                ui_state
                    .navigation_history
                    .push((ui_state.active_pane, ui_state.cursor_index));
                ui_state.cursor_index = new_cursor;
                ui_state.set_status_message(format!("Jumped to line {}", target_line));
                WidgetResult::Handled
            }
            KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                if !app_state.raw_data.is_empty() {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        if ui_state.selection_start.is_none() {
                            ui_state.selection_start = Some(ui_state.cursor_index);
                        }
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.selection_start = None;
                        ui_state.set_status_message("Visual Mode Exited");
                    }
                } else {
                    ui_state.set_status_message("No open document");
                }
                WidgetResult::Handled
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                if !app_state.raw_data.is_empty()
                    && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
                {
                    let mut target_addr = line.address;
                    let mut current_sub_index = 0;
                    let mut found = false;

                    if line.bytes.len() > 1 {
                        for offset in 1..line.bytes.len() {
                            let mid_addr = line.address.wrapping_add(offset as u16);
                            if let Some(labels) = app_state.labels.get(&mid_addr) {
                                for _label in labels {
                                    if current_sub_index == ui_state.sub_cursor_index {
                                        target_addr = mid_addr;
                                        found = true;
                                        break;
                                    }
                                    current_sub_index += 1;
                                }
                            }
                            if found {
                                break;
                            }
                        }
                    }

                    let text = app_state
                        .labels
                        .get(&target_addr)
                        .and_then(|v| v.first())
                        .map(|l| l.name.as_str());
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_label::LabelDialog::new(text, target_addr),
                    ));
                    ui_state.set_status_message("Enter Label");
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Code)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Byte)
            }
            KeyCode::Char('w') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Word)
            }
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Address)
            }
            KeyCode::Char('t') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Text)
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Screencode)
            }
            KeyCode::Char('?') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::Undefined)
            }
            KeyCode::Char('<') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetLoHi)
            }
            KeyCode::Char('>') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetHiLo)
            }
            KeyCode::Char('|') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ToggleSplitter)
            }
            KeyCode::Char(';') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SideComment)
            }
            KeyCode::Char(':') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::LineComment)
            }
            KeyCode::Char('e') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetExternalFile)
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::JumpToOperand)
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::NextImmediateFormat)
            }
            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(MenuAction::PreviousImmediateFormat)
            }
            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(MenuAction::ToggleCollapsedBlock)
            }
            KeyCode::Backspace if key.modifiers.is_empty() => {
                while let Some((pane, _)) = ui_state.navigation_history.last() {
                    if *pane != ActivePane::Disassembly {
                        ui_state.navigation_history.pop();
                    } else {
                        break;
                    }
                }

                if let Some((pane, idx)) = ui_state.navigation_history.pop() {
                    if pane == ActivePane::Disassembly {
                        if idx < app_state.disassembly.len() {
                            ui_state.cursor_index = idx;
                            ui_state.set_status_message("Navigated back");
                        } else {
                            ui_state.set_status_message("History invalid");
                        }
                    }
                } else {
                    ui_state.set_status_message("No history");
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
