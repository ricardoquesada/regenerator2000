use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

const PAGE_SCROLL_AMOUNT: usize = 30;

pub struct VisualLineCounts {
    pub labels: usize,
    pub comments: usize,
    pub instruction: usize,
}

impl VisualLineCounts {
    pub fn total(&self) -> usize {
        self.labels + self.comments + self.instruction
    }
}

pub struct DisassemblyView;

impl DisassemblyView {
    fn move_cursor_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if app_state.disassembly.is_empty() {
            return;
        }

        // Ensure cursor is within bounds
        if ui_state.cursor_index >= app_state.disassembly.len() {
            ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1);
        }

        for _ in 0..amount {
            let line = &app_state.disassembly[ui_state.cursor_index];
            let visual_len = Self::get_visual_line_count_for_instruction(line, app_state);

            if ui_state.sub_cursor_index + 1 < visual_len {
                // Move to next line within same instruction (comment/label/etc)
                ui_state.sub_cursor_index += 1;
            } else {
                // Move to next instruction
                let mut next_idx = ui_state.cursor_index + 1;
                while next_idx < app_state.disassembly.len() {
                    let next_line = &app_state.disassembly[next_idx];
                    if !next_line.bytes.is_empty()
                        || next_line.is_collapsed
                        || next_line.label.is_some()
                        || !next_line.mnemonic.is_empty()
                    {
                        break;
                    }
                    next_idx += 1;
                }

                if next_idx < app_state.disassembly.len() {
                    ui_state.cursor_index = next_idx;
                    // Start at the top of the next instruction block
                    ui_state.sub_cursor_index = 0;
                }
            }
        }
    }

    fn move_cursor_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if app_state.disassembly.is_empty() {
            return;
        }

        // Ensure cursor is within bounds
        if ui_state.cursor_index >= app_state.disassembly.len() {
            ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1);
        }

        for _ in 0..amount {
            if ui_state.sub_cursor_index > 0 {
                ui_state.sub_cursor_index -= 1;
            } else if ui_state.cursor_index > 0 {
                let mut prev_idx = ui_state.cursor_index - 1;
                while prev_idx > 0 {
                    let prev_line = &app_state.disassembly[prev_idx];
                    if !prev_line.bytes.is_empty()
                        || prev_line.is_collapsed
                        || prev_line.label.is_some()
                        || !prev_line.mnemonic.is_empty()
                    {
                        break;
                    }
                    prev_idx -= 1;
                }

                // Check if the found prev_idx is valid
                let prev_line = &app_state.disassembly[prev_idx];
                if !prev_line.bytes.is_empty()
                    || prev_line.is_collapsed
                    || prev_line.label.is_some()
                    || !prev_line.mnemonic.is_empty()
                {
                    ui_state.cursor_index = prev_idx;
                    let prev_counts =
                        Self::get_visual_line_count_for_instruction(prev_line, app_state);
                    ui_state.sub_cursor_index = prev_counts.saturating_sub(1);
                }
            }
        }
    }
    pub fn get_visual_line_counts(
        line: &crate::disassembler::DisassemblyLine,
        app_state: &AppState,
    ) -> VisualLineCounts {
        let mut labels = 0;
        // 1. Labels inside multi-byte instructions
        if line.bytes.len() > 1 {
            for offset in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(offset as u16);
                if let Some(l) = app_state.labels.get(&mid_addr) {
                    labels += l.len();
                }
            }
        }

        let mut comments = 0;
        // 2. Line comment
        if let Some(comment) = &line.line_comment {
            comments += comment.lines().count();
        }

        VisualLineCounts {
            labels,
            comments,
            instruction: 1,
        }
    }

    pub fn get_visual_line_count_for_instruction(
        line: &crate::disassembler::DisassemblyLine,
        app_state: &AppState,
    ) -> usize {
        Self::get_visual_line_counts(line, app_state).total()
    }

    pub fn get_index_for_visual_line(app_state: &AppState, target_line: usize) -> Option<usize> {
        let mut current_visual_line = 1;
        for (index, line) in app_state.disassembly.iter().enumerate() {
            let lines_for_this_instruction =
                Self::get_visual_line_count_for_instruction(line, app_state);

            if target_line >= current_visual_line
                && target_line < current_visual_line + lines_for_this_instruction
            {
                return Some(index);
            }

            current_visual_line += lines_for_this_instruction;
        }
        None
    }

    pub fn get_sub_index_for_address(
        line: &crate::disassembler::DisassemblyLine,
        app_state: &AppState,
        target_addr: u16,
    ) -> usize {
        // Calculate visual index for target_addr within this line.
        // Order:
        // 1. Labels [offset 1..N]
        // 2. Comments (not addressable by jump usually, but occupy sub-indices)
        // 3. Instruction (Base address)

        let mut sub_index = 0;

        // 1. Labels inside multi-byte instructions
        if line.bytes.len() > 1 {
            for offset in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(offset as u16);
                if let Some(l) = app_state.labels.get(&mid_addr) {
                    if mid_addr == target_addr {
                        return sub_index;
                    }
                    sub_index += l.len();
                }
            }
        }

        // 2. Line comment
        if let Some(comment) = &line.line_comment {
            sub_index += comment.lines().count();
        }

        // 3. Instruction
        // If we didn't return early, checking labels, and the target IS the line address,
        // we return the instruction index (current valid sub_index).
        // If the target matched a mid-address with NO label, we default here too?
        // Wait, if mid-address has NO label, it's not visually distinct (no sub-line).
        // So we just return the instruction sub-index.
        sub_index
    }
}

impl Navigable for DisassemblyView {
    fn len(&self, app_state: &AppState) -> usize {
        app_state.disassembly.len()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.selection_start.is_none() {
                ui_state.selection_start = Some(ui_state.cursor_index);
            }
        } else {
            ui_state.selection_start = None;
        }
        self.move_cursor_down(app_state, ui_state, amount);
    }

    fn move_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.selection_start.is_none() {
                ui_state.selection_start = Some(ui_state.cursor_index);
            }
        } else {
            ui_state.selection_start = None;
        }
        self.move_cursor_up(app_state, ui_state, amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        if app_state.disassembly.is_empty() {
            return;
        }
        // PageDown logic: flat 30 lines jump
        ui_state.cursor_index =
            (ui_state.cursor_index + PAGE_SCROLL_AMOUNT).min(self.len(app_state).saturating_sub(1));

        // Ensure sub-cursor is valid for the new line
        let line = &app_state.disassembly[ui_state.cursor_index];
        let max_sub = Self::get_visual_line_count_for_instruction(line, app_state);
        if ui_state.sub_cursor_index >= max_sub {
            ui_state.sub_cursor_index = max_sub.saturating_sub(1);
        }
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        if app_state.disassembly.is_empty() {
            return;
        }
        // PageUp logic: flat 30 lines jump
        ui_state.cursor_index = ui_state.cursor_index.saturating_sub(PAGE_SCROLL_AMOUNT);

        // Ensure sub-cursor is valid for the new line
        let line = &app_state.disassembly[ui_state.cursor_index];
        let max_sub = Self::get_visual_line_count_for_instruction(line, app_state);
        if ui_state.sub_cursor_index >= max_sub {
            ui_state.sub_cursor_index = max_sub.saturating_sub(1);
        }
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let max = self.len(app_state).saturating_sub(1);
        ui_state.cursor_index = index.min(max);
        ui_state.sub_cursor_index = 0;

        // Reset scroll
        ui_state.scroll_index = ui_state.cursor_index;
        ui_state.scroll_sub_index = 0;
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        // G logic (Jump to visual line)
        // If input is 0 (user typed "0G" or similar), treat it as jump to end
        let new_cursor = if input == 0 {
            Some(self.len(app_state).saturating_sub(1))
        } else {
            Self::get_index_for_visual_line(app_state, input)
        };

        if let Some(idx) = new_cursor {
            if ui_state.is_visual_mode && ui_state.selection_start.is_none() {
                ui_state.selection_start = Some(ui_state.cursor_index);
            }
            ui_state.cursor_index = idx;
            ui_state.sub_cursor_index = 0;

            // Reset scroll
            ui_state.scroll_index = idx;
            ui_state.scroll_sub_index = 0;
        }
        // If invalid, Navigable trait doesn't currently support error feedback via return.
        // handle_nav_input will print generic success message if not handled differently?
        // Actually handle_nav_input prints generic success message only if input > 0.
        // Here we perform the jump.
    }

    fn item_name(&self) -> &str {
        "visual line"
    }
}

impl Widget for DisassemblyView {
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                return WidgetResult::Handled;
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                return WidgetResult::Handled;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Proceed with click handling
            }
            _ => return WidgetResult::Ignored,
        }

        let area = ui_state.disassembly_area;
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if mouse.column < inner_area.x
            || mouse.column >= inner_area.x + inner_area.width
            || mouse.row < inner_area.y
            || mouse.row >= inner_area.y + inner_area.height
        {
            return WidgetResult::Ignored;
        }

        let click_row = (mouse.row - inner_area.y) as usize;
        let visible_height = inner_area.height as usize;

        if app_state.disassembly.is_empty() {
            return WidgetResult::Handled;
        }

        // Use scroll state to determine what's actually on screen
        let mut current_inst = ui_state.scroll_index;
        let mut current_sub = ui_state.scroll_sub_index;
        let mut current_y = 0;

        // Ensure scroll index is valid (safety check)
        if current_inst >= app_state.disassembly.len() {
            current_inst = app_state.disassembly.len().saturating_sub(1);
            current_sub = 0;
        }

        while current_inst < app_state.disassembly.len() {
            let line = &app_state.disassembly[current_inst];
            let counts = Self::get_visual_line_counts(line, app_state).total();

            // Number of visual lines associated with this instruction that are actually visible
            // (starting from current_sub)
            let visible_part_count = counts.saturating_sub(current_sub);

            if click_row < current_y + visible_part_count {
                // Found the clicked row
                ui_state.cursor_index = current_inst;
                ui_state.sub_cursor_index = current_sub + (click_row - current_y);

                if ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }
                return WidgetResult::Handled;
            }

            current_y += visible_part_count;
            if current_y >= visible_height {
                break;
            }

            current_inst += 1;
            current_sub = 0; // Next instructions start from top
        }

        WidgetResult::Handled
    }

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

        // Ensure cursor is within valid bounds to avoid panic
        if ui_state.cursor_index >= total_items {
            ui_state.cursor_index = total_items.saturating_sub(1);
            ui_state.sub_cursor_index = 0;
        }

        // 1. Calculate Cursor Visual Offset (distance from top of file)
        // This is expensive O(N) if we scan from 0. Ideally we only care about relative position
        // between scroll_index and cursor_index.

        let mut scroll_inst_idx = ui_state.scroll_index;
        let mut scroll_sub_idx = ui_state.scroll_sub_index;

        // Ensure scroll indices are valid
        if scroll_inst_idx >= total_items {
            scroll_inst_idx = total_items.saturating_sub(1);
            scroll_sub_idx = 0;
        }

        // --- Scrolloff Logic Start ---
        // We calculate the VALID range for the scroll position relative to the cursor.
        // The scroll position (top of view) must be:
        // 1. Not too "below" the cursor (cursor - margin). Ideally <= (cursor - margin).
        // 2. Not too "above" the cursor (cursor - (height - margin)). Ideally >= (cursor - (height - margin)).

        let margin = 5.min(visible_height / 3);

        // Helper to walk backwards visual lines
        let walk_back = |start_inst: usize, start_sub: usize, steps: usize| -> (usize, usize) {
            let mut curr_inst = start_inst;
            let mut curr_sub = start_sub;
            for _ in 0..steps {
                if curr_sub > 0 {
                    curr_sub -= 1;
                } else if curr_inst > 0 {
                    // Find previous valid instruction
                    let mut prev_idx = curr_inst - 1;
                    while prev_idx > 0 {
                        let prev_line = &app_state.disassembly[prev_idx];
                        if !prev_line.bytes.is_empty()
                            || prev_line.is_collapsed
                            || prev_line.label.is_some()
                            || !prev_line.mnemonic.is_empty()
                        {
                            break;
                        }
                        prev_idx -= 1;
                    }
                    curr_inst = prev_idx;
                    // Check if valid line found (handle file start case where loop might fail if nothing valid)
                    let prev_line = &app_state.disassembly[curr_inst];
                    // Re-verify validity (esp for index 0)
                    if !prev_line.bytes.is_empty()
                        || prev_line.is_collapsed
                        || prev_line.label.is_some()
                        || !prev_line.mnemonic.is_empty()
                    {
                        let prev_lines =
                            Self::get_visual_line_count_for_instruction(prev_line, app_state);
                        curr_sub = prev_lines.saturating_sub(1);
                    } else {
                        // Fallback to start of file
                        return (0, 0);
                    }
                } else {
                    return (0, 0);
                }
            }
            (curr_inst, curr_sub)
        };

        let (max_scroll_inst, max_scroll_sub) =
            walk_back(ui_state.cursor_index, ui_state.sub_cursor_index, margin);

        // Calculate min_scroll (furthest UP scroll can be)
        // Since we want cursor to be at most (visible_height - margin - 1) lines away from top,
        // min_scroll is (cursor) walked back by (visible_height - margin - 1).
        // However, max_scroll is (cursor) walked back by (margin).
        // So min_scroll is max_scroll walked back by (visible_height - margin - 1 - margin).

        let window_size = visible_height.saturating_sub(2 * margin).saturating_sub(1);
        let (min_scroll_inst, min_scroll_sub) = if window_size > 0 {
            walk_back(max_scroll_inst, max_scroll_sub, window_size)
        } else {
            (max_scroll_inst, max_scroll_sub)
        };

        // Clamp current scroll (scroll_inst_idx, scroll_sub_idx) to [min_scroll, max_scroll]

        // 1. Check if too high (scroll > max_scroll) -> Move UP to max_scroll
        if scroll_inst_idx > max_scroll_inst
            || (scroll_inst_idx == max_scroll_inst && scroll_sub_idx > max_scroll_sub)
        {
            scroll_inst_idx = max_scroll_inst;
            scroll_sub_idx = max_scroll_sub;
        }

        // 2. Check if too low (scroll < min_scroll) -> Move DOWN to min_scroll
        if scroll_inst_idx < min_scroll_inst
            || (scroll_inst_idx == min_scroll_inst && scroll_sub_idx < min_scroll_sub)
        {
            scroll_inst_idx = min_scroll_inst;
            scroll_sub_idx = min_scroll_sub;
        }

        // --- Scrolloff Logic End ---

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

        // Compute the accurate last visible instruction index by walking the disassembly
        // and counting visual rows (accounting for line comments and mid-address labels),
        // exactly like the render loop does. Using a naive `scroll_inst_idx + visible_height`
        // overestimates when instructions have multi-row comments, causing arrows to be
        // marked as visible when they are actually off-screen.
        let end_view = {
            let mut visual_rows = scroll_sub_idx; // Already-consumed sub-rows at top
            let mut inst = scroll_inst_idx;
            while inst < total_items && visual_rows < visible_height {
                let line = &app_state.disassembly[inst];
                visual_rows += Self::get_visual_line_count_for_instruction(line, app_state);
                inst += 1;
            }
            inst // exclusive upper bound (instruction index)
        };

        // Optimization: Use cached arrows from AppState to avoid O(N) search per frame
        let mut relevant_arrows: Vec<ArrowInfo> = Vec::with_capacity(app_state.cached_arrows.len());

        for arrow in &app_state.cached_arrows {
            let src_idx = arrow.start;
            let dst_idx = arrow.end;

            let low = std::cmp::min(src_idx, dst_idx);
            let high = std::cmp::max(src_idx, dst_idx);

            // Check if arrow overlaps with visible area
            if low < end_view && high >= scroll_inst_idx {
                let target_addr_val = arrow.target_addr.unwrap_or(0);

                // Check if it is an exact match to the line address
                // If exact match, we don't treat it as "relative target" (no special tip)
                // If it's midway (e.g. branch to +1), relative_target is Some(addr).
                let exact_match = if let Some(line) = app_state.disassembly.get(dst_idx) {
                    line.address == target_addr_val
                } else {
                    false
                };

                let relative_target = if !exact_match {
                    arrow.target_addr
                } else {
                    None
                };

                relevant_arrows.push(ArrowInfo {
                    start: src_idx,
                    end: dst_idx,
                    col: 0,
                    target_addr: relative_target,
                    start_visible: src_idx >= scroll_inst_idx && src_idx < end_view,
                    end_visible: dst_idx >= scroll_inst_idx && dst_idx < end_view,
                });
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
        let view_start = scroll_inst_idx;
        let view_end = end_view; // Use the accurate value computed above

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

                let (low, high) = if is_down {
                    (arrow.start, arrow.end)
                } else {
                    (arrow.end, arrow.start)
                };

                // 1. Vertical Body
                // Draw vertical line if we are strictly inside the arrow span
                if current_line > low && current_line < high {
                    if chars[c_idx] == ' ' {
                        chars[c_idx] = '│';
                    } else if chars[c_idx] == '─' {
                        chars[c_idx] = '┼';
                    }
                }

                // Relative target special vertical handling
                // If this is the ending line of the arrow, but it's a relative target (offset),
                // we might need to continue draw the vertical line through the instruction?
                // Logic:
                // Downward relative: target is somewhere inside this instruction or later.
                //   If later, well, `current_line` is `end`.
                //   If target is "inside", we generally draw `└` at `end`.
                //   Wait, `is_relative_target` means `target_addr` matches inside `end` line.
                //   If we are at `end`, and it's relative, do we stop here?
                //   The `target_addr` logic inside `render` handles drawing the `└` at the sub-line.
                //   For the main instruction line, if it's relative, we probably want `│`
                //   to reach the sub-line?
                //   If Unsure, check existing behavior:
                //   "if is_relative_target && !is_down && current_line == arrow.end { chars = | }"
                //   Upward relative: target is inside `end` (start < end? No, end < start).
                //   So arrow comes from below. Reaches `end`. Target is inside.
                //   The arrow line needs to go UP to the sub-line.
                //   Since instruction line is usually the "base", and sub-lines (comments/labels)
                //   are considered "above" or "inside"?
                //   Actually we index sub-lines for display.
                //   If target is inside, we need `│` on the base instruction line IF the target
                //   is "after" the base rendering?
                //   Actually, sub-lines (labels) are rendered BEFORE the instruction line.
                //   For Upward Relative:
                //      We come from below. We hit `end` (Instruction line).
                //      Target is one of the labels ABOVE.
                //      So we need `│` on the instruction line? No.
                //      The line comes from below. It passes `end+1`. It reaches `end`.
                //      It needs to reach the labels ABOVE `end`.
                //      So yes, it must pass through the instruction line to reach labels.
                //      So Upward Relative at `end` -> Need `│`.
                //   For Downward Relative:
                //      We come from above. We hit `end`. Target is label ABOVE?
                //      If target is label ABOVE instruction, we already passed it.
                //      So we don't need `│` on instruction line.
                //      If target is offset bytes INSIDE instruction (rare? jumps to middle of instruction?)
                //      Then we might need it. But typically relative targets are labels.

                if is_relative_target
                    && !is_down
                    && current_line == arrow.end
                    && chars[c_idx] == ' '
                {
                    chars[c_idx] = '│';
                }

                // 2. Start Hook
                // Only if start is visible on this line
                if arrow.start_visible && current_line == arrow.start {
                    if app_state.disassembly[current_line].target_address.is_some() {
                        chars[c_idx] = if is_down { '┌' } else { '└' };
                        chars[c_idx + 1] = '─';
                    }
                }
                // 3. End Hook
                // Only if end is visible on this line
                else if arrow.end_visible && current_line == arrow.end && !is_relative_target {
                    chars[c_idx] = if is_down { '└' } else { '┌' };
                    chars[c_idx + 1] = '─';
                }

                // 4. Edge Pointers (Partial visibility)
                // If start visible but end NOT:
                if arrow.start_visible && !arrow.end_visible {
                    // Check if we need to draw '▼' or '▲' just to indicate direction at edge?
                    // Previous code:
                    if is_down && current_line == arrow.start + 1 {
                        chars[c_idx] = '▼';
                    } else if !is_down && current_line == arrow.start.saturating_sub(1) {
                        chars[c_idx] = '▲';
                    }
                }
                // If end visible but start NOT:
                else if !arrow.start_visible
                    && arrow.end_visible
                    && ((is_down && current_line == arrow.end.saturating_sub(1))
                        || (!is_down && current_line == arrow.end + 1))
                {
                    chars[c_idx] = '│'; // Ensure connection coming in?
                }

                // 5. Fill gaps for partial arrows that fully cross the screen
                // (Handled by Step 1 "Vertical Body" which checks low/high range)
            }

            // Horizontal connectors
            for arrow in &active_arrows {
                let is_relative_target = arrow.target_addr.is_some();
                let is_end_line = current_line == arrow.end;
                let is_start_line = current_line == arrow.start;

                let c_idx = arrow.col * 2;

                if arrow.start == arrow.end && current_line == arrow.start && arrow.start_visible {
                    chars[c_idx] = '∞';
                }

                let is_valid_source = app_state.disassembly[current_line].target_address.is_some();
                // Safe start line: visible and is actually a control flow source
                let safe_is_start_line = is_start_line && arrow.start_visible && is_valid_source;

                // Draw horizontal line if:
                // 1. It's the start line (and valid)
                // 2. It's the end line (and visible), unless it's a relative target (handled elsewhere)

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

                let is_down = arrow.start < arrow.end;

                // Determine if this line should have a vertical pass-through bar.
                // 1. If strictly inside the arrow body, ALWAYS pass through.
                let strictly_inside = current_line > low && current_line < high;

                // 2. Boundary conditions for comments (which appear "above" the instruction line)
                // - If upward jump (start > end):
                //    - At `start`: Comments are above start. Arrow goes UP from start. So pass through.
                //    - At `end`: Comments are above end. Arrow comes DOWN to end. So pass through.
                // - If downward jump (start < end):
                //    - At `start`: Comments are above start. Arrow goes DOWN from start. No pass through.
                //    - At `end`: Comments are above end. Arrow comes DOWN to end. So pass through.

                let boundary_pass = if is_down {
                    // Downward: Start -> End
                    // Start line comments: No line (arrow starts below comments).
                    // End line comments: Yes line (arrow arrives from above).
                    current_line == arrow.end
                } else {
                    // Upward: End <- Start
                    // Start line comments: Yes line (arrow goes up from instruction, passing through comments above it).
                    // End line comments: Yes line (arrow comes down to instruction, passing through comments above it... wait?)
                    // Logic check: Upward arrow `└─` at start goes UP. Comments are above. So Yes.
                    // Upward arrow `┌─` at end comes DOWN. Comments are above. So Yes?
                    // Actually, standard representation:
                    // Upward jump:
                    //   End:  ┌─> Target
                    //         │
                    //   Start:└─ Source
                    //
                    // At End (Target): The arrow arrives from BELOW (it wraps around? No, it's just a line).
                    // Visual:
                    // JMP $1000  (Start)  └──────┐
                    // ...                        │
                    // $1000 NOP  (End)    <──────┘
                    //
                    // Wait, upward arrow usually drawn on the right in some tools, but here on left.
                    // Left side upward jump:
                    //    ┌─ $1000 (End)
                    //    │
                    //    └─ JMP $1000 (Start)
                    //
                    // So at End: Line comes from below? No, from Start (below) to End (above).
                    // So at End, the hook is `┌─`. The vertical line is BELOW the hook.
                    // Comments are ABOVE the instruction.
                    // So comments at End are OUTSIDE the arrow span. No line.

                    // At Start: Hook is `└─`. Vertical line is ABOVE the hook.
                    // Comments are ABOVE the instruction.
                    // So comments at Start are INSIDE the arrow span. Yes line.

                    current_line == arrow.start
                };

                let mut passes_through = strictly_inside || boundary_pass;

                let is_relative_target_elsewhere =
                    arrow.target_addr.is_some() && arrow.end == current_line;

                // Relative target adjustment
                if is_relative_target_elsewhere && let Some(target) = arrow.target_addr {
                    if let Some(this_addr) = sub_addr {
                        // If inside the target line, check if we passed the specific address
                        if arrow.start < arrow.end {
                            // Downward
                            // Target is inside line. We stop at target.
                            passes_through = this_addr < target;
                        } else {
                            // Upward
                            // Target is inside line. We are coming from below.
                            // If we are at End, we are "above" the vertical span.
                            // But we logic said `boundary_pass` is FALSE for `current_line == arrow.end` (Upward).
                            // So `passes_through` is false.
                            // But checking `sub_addr`: if `this_addr < target`, are we inside?
                            // Upward jump arrives at target from below.
                            // If target is in middle of line:
                            //   [Addr 1]
                            //   [Addr 2] <- Target
                            //   [Addr 3]
                            // The line comes from below, reaches Addr 2, turns right.
                            // So Addr 3 "has line". Addr 1 "no line".
                            // So if `this_addr > target`, passes_through = true?
                            if sub_addr.is_some() {
                                passes_through = this_addr > target;
                            }
                        }
                    } else {
                        // No sub-addr (comment).
                        // For upward jump at End: comments are above.
                        // Line comes from below to target.
                        // Comments are "before" target. So "no line"?
                        // Logic above said boundary_pass false for End/Upward.
                        // So this remains false. Correct.
                        // For downward jump at End: comments are above.
                        // Line comes from above to target.
                        // Comments are "before" target. So "yes line".
                        // Logic above said boundary_pass true for End/Downward.
                        // So remains true. Correct.
                    }
                }

                if passes_through {
                    chars[c_idx] = '│';
                }

                let is_target_here = if let Some(addr) = sub_addr
                    && let Some(target) = arrow.target_addr
                {
                    addr == target
                } else {
                    false
                };

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

        // Render Loop: Generate ListItems starting from scroll_inst_idx, scroll_sub_idx
        let mut items = Vec::new();
        let mut current_inst = scroll_inst_idx;
        let mut current_sub = scroll_sub_idx;
        let mut processed_visual_lines = 0;

        // let mut arrow_calc_offset_map = Vec::new(); // Map visual line index (0..visible) to instruction index for arrows

        while processed_visual_lines < visible_height && current_inst < total_items {
            let line = &app_state.disassembly[current_inst];
            // let line_visual_count = Self::get_visual_line_count_for_instruction(line, app_state);

            // We need to generate the specific visual lines for this instruction, starting from current_sub
            // Since the original generate logic produced a single ListItem with multiple Lines,
            // we now need to replicate that logic but conditionally extracting single Lines.

            // Helper specific to this instruction to get "all visual parts" in order
            // 1. Labels [1..len]
            // 2. Comments (lines)
            // 3. Instruction itself

            let mut parts = Vec::new();

            // Part generation logic (copied/adapted from previous render)
            let is_cursor_row = current_inst == ui_state.cursor_index;
            let is_selected_block = if let Some(selection_start) = ui_state.selection_start {
                let (start, end) = if selection_start < ui_state.cursor_index {
                    (selection_start, ui_state.cursor_index)
                } else {
                    (ui_state.cursor_index, selection_start)
                };
                current_inst >= start && current_inst <= end
            } else {
                false
            };

            let base_style = if is_selected_block {
                Style::default()
                    .bg(ui_state.theme.selection_bg)
                    .fg(ui_state.theme.selection_fg)
            } else {
                Style::default()
            };

            // 1. Labels
            if line.bytes.len() > 1 {
                for offset in 1..line.bytes.len() {
                    let mid_addr = line.address.wrapping_add(offset as u16);
                    if let Some(labels) = app_state.labels.get(&mid_addr) {
                        // XREF logic
                        let xref_str = if let Some(refs) = app_state.cross_refs.get(&mid_addr) {
                            if !refs.is_empty() && app_state.settings.max_xref_count > 0 {
                                format!(
                                    "{} {}",
                                    formatter.comment_prefix(),
                                    crate::disassembler::format_cross_references(
                                        refs,
                                        app_state.settings.max_xref_count
                                    )
                                )
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };

                        for label in labels {
                            let arrow_padding = get_comment_arrow_str(current_inst, Some(mid_addr)); // Need accurate active_arrows calculate first?
                            // Arrows calculation depends on 'offset' which is scroll_index (inst).
                            // But with smooth scrolling, arrows should be calculated based on the window.
                            // The arrow logic "get_arrow_str" takes 'current_line' (instruction index).
                            // So it remains valid as long as we pass 'current_inst'.
                            // The VISUAL clipping in arrow logic used 'offset' and 'visible_height'.
                            // This might need tweak if we show "half an instruction".
                            // For now we assume arrow logic works on Instruction granularity.

                            let is_bookmarked = app_state.bookmarks.contains_key(&mid_addr);
                            let gutter = if is_bookmarked { "  *  " } else { "     " };
                            let gutter_style = if is_bookmarked {
                                base_style.fg(ui_state.theme.label)
                            } else {
                                base_style.fg(ui_state.theme.bytes)
                            };

                            let label_def = format!("{} =*+${:02x}", label.name, offset);
                            let mut spans = vec![
                                Span::styled(gutter, gutter_style),
                                Span::styled(
                                    format!("{:<width$} ", arrow_padding, width = arrow_width),
                                    base_style.fg(ui_state.theme.arrow),
                                ),
                                Span::styled("                 ".to_string(), base_style),
                                Span::styled(
                                    format!("{:<36}", label_def),
                                    base_style.fg(ui_state.theme.label_def),
                                ),
                            ];
                            if !xref_str.is_empty() {
                                spans.push(Span::styled(
                                    xref_str.clone(),
                                    base_style.fg(ui_state.theme.comment),
                                ));
                            }
                            parts.push(Line::from(spans));
                        }
                    }
                }
            }

            // 2. Comments
            if let Some(line_comment) = &line.line_comment {
                for comment_part in line_comment.lines() {
                    let arrow_padding = get_comment_arrow_str(current_inst, None);
                    parts.push(Line::from(vec![
                        Span::styled("     ", base_style.fg(ui_state.theme.bytes)),
                        Span::styled(
                            format!("{:width$} ", arrow_padding, width = arrow_width),
                            base_style.fg(ui_state.theme.arrow),
                        ),
                        Span::styled("                 ".to_string(), base_style),
                        Span::styled(
                            format!("{} {}", formatter.comment_prefix(), comment_part),
                            base_style.fg(ui_state.theme.comment),
                        ),
                    ]));
                }
            }

            // 3. Instruction
            let show_address = !line.bytes.is_empty() || line.is_collapsed || line.label.is_some();
            let address_str = if show_address {
                format!(
                    "${:04X}{} ",
                    line.address,
                    if app_state.splitters.contains(&line.address) {
                        "*"
                    } else {
                        " "
                    }
                )
            } else {
                "       ".to_string()
            };

            let label_text = if let Some(label) = &line.label {
                formatter.format_label_definition(label)
            } else {
                String::new()
            };
            let arrow_padding = get_arrow_str(current_inst);

            let is_bookmarked = app_state.bookmarks.contains_key(&line.address);
            let is_pc = app_state.vice_state.pc == Some(line.address);
            let gutter = if is_pc {
                "  >  "
            } else if is_bookmarked {
                "  *  "
            } else {
                "     "
            };

            let gutter_style = if is_pc {
                base_style
                    .fg(ui_state.theme.border_active)
                    .add_modifier(Modifier::BOLD)
            } else if is_bookmarked {
                base_style.fg(ui_state.theme.label)
            } else {
                base_style.fg(ui_state.theme.bytes)
            };

            let mut inst_spans = vec![
                Span::styled(gutter, gutter_style),
                Span::styled(
                    format!("{:<width$} ", arrow_padding, width = arrow_width),
                    base_style.fg(ui_state.theme.arrow),
                ),
                Span::styled(
                    address_str,
                    if let Some(next) = app_state.disassembly.get(current_inst + 1)
                        && app_state.splitters.contains(&next.address)
                        && show_address
                    {
                        base_style
                            .fg(ui_state.theme.address)
                            .add_modifier(Modifier::UNDERLINED)
                    } else if app_state.splitters.contains(&line.address) && show_address {
                        base_style
                            .fg(ui_state.theme.address)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        base_style.fg(ui_state.theme.address)
                    },
                ),
                Span::styled(
                    format!(
                        "{: <10}",
                        if line.show_bytes {
                            hex_bytes(&line.bytes)
                        } else {
                            String::new()
                        }
                    ),
                    base_style.fg(ui_state.theme.bytes),
                ),
            ];

            if line.is_collapsed {
                inst_spans.push(Span::styled(
                    line.mnemonic.to_string(),
                    base_style
                        .fg(ui_state.theme.collapsed_block)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                inst_spans.push(Span::styled(
                    format!("{: <20}", label_text),
                    base_style
                        .fg(ui_state.theme.label_def)
                        .add_modifier(Modifier::BOLD),
                ));
                inst_spans.push(Span::styled(
                    format!("{: <4} ", line.mnemonic),
                    base_style
                        .fg(ui_state.theme.mnemonic)
                        .add_modifier(Modifier::BOLD),
                ));
                inst_spans.push(Span::styled(
                    format!("{: <15}", line.operand),
                    base_style.fg(ui_state.theme.operand),
                ));
                inst_spans.push(Span::styled(
                    if line.comment.is_empty() {
                        String::new()
                    } else {
                        format!("{} {}", formatter.comment_prefix(), line.comment)
                    },
                    base_style.fg(ui_state.theme.comment),
                ));
            }
            parts.push(Line::from(inst_spans));

            // --- Emit processed parts ---
            for (idx, part) in parts.into_iter().enumerate() {
                if idx >= current_sub {
                    // This sub-part is visible
                    // Check highlight for this specific sub-part
                    let is_cursor_sub = is_cursor_row && idx == ui_state.sub_cursor_index;

                    let style = if is_cursor_sub && !is_selected_block {
                        // Sub-cursor Highlight
                        base_style.bg(ui_state.theme.selection_bg)
                    } else {
                        base_style
                    };

                    items.push(ListItem::new(part).style(style));
                    // arrow_calc_offset_map.push(current_inst); // Not used in new logic
                    processed_visual_lines += 1;

                    if processed_visual_lines >= visible_height {
                        break;
                    }
                }
            }

            // Advance to next instruction
            let mut next = current_inst + 1;
            while next < total_items {
                let l = &app_state.disassembly[next];
                if !l.bytes.is_empty()
                    || l.is_collapsed
                    || l.label.is_some()
                    || !l.mnemonic.is_empty()
                {
                    break;
                }
                next += 1;
            }
            current_inst = next;
            current_sub = 0; // Reset sub for next instruction
        }

        let list = List::new(items).block(block);
        f.render_widget(list, area);

        // Update persistent state
        ui_state.scroll_index = scroll_inst_idx;
        ui_state.scroll_sub_index = scroll_sub_idx;
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j')
                if key.code == KeyCode::Down || key.modifiers.is_empty() =>
            {
                // Shift+Down or Visual Mode Down handling (Plain Down handled by Navigable)
                // Navigable handles plain Down/j and clears selection.
                // We only reach here if Navigable returned Ignored.
                // But handle_nav_input returns Handled for Down/j even if modifiers is empty?
                // Wait, handle_nav_input ignores if modifiers is NOT empty.
                // So plain Down is handled there.
                // Shift+Down is ignored there. So we handle it here.

                if key.modifiers == KeyModifiers::SHIFT || ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    // Start visual selection? No, if modifiers empty and NOT visual mode, it should be plain move.
                    // But plain move is handled by Navigable?
                    // YES.
                    // So we only need to handle the case where we start/extend selection.
                    // But wait, if ui_state.is_visual_mode is true, Navigable `move_down` clears selection?
                    // My Navigable impl (planned) handles visual mode!
                    // See Step 87: Navigable::move_down checks is_visual_mode.

                    // So `handle_nav_input` is SUFFICIENT for plain move AND visual mode move (j/k).
                    // BUT `handle_nav_input` ignores Shift+Down.
                    // So here we only handle Shift+Down.
                    ui_state.selection_start = None; // Should not happen for Shift+Down?
                }

                // If we are here, it's Shift+Down (or similar mod).
                // Existing logic extends selection (handled above).
                // Then moves cursor.
                // Use helper.
                self.move_cursor_down(app_state, ui_state, 1);
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
                self.move_cursor_up(app_state, ui_state, 1);
                WidgetResult::Handled
            }
            // PageDown/Up are handled by Navigable

            // Other keys...
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
            KeyCode::Char(c)
                if c.is_ascii_digit()
                    && !key.modifiers.intersects(
                        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                    ) =>
            {
                if ui_state.input_buffer.len() < 10 {
                    ui_state.input_buffer.push(c);
                    ui_state.set_status_message(format!(":{}", ui_state.input_buffer));
                }
                WidgetResult::Handled
            }
            // G is handled by Navigable specific logic for Disassembly?
            // Navigable handles G. So handled by `handle_nav_input`.
            KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                if !app_state.raw_data.is_empty() {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Backspace if key.modifiers.is_empty() => {
                while let Some((pane, _)) = ui_state.navigation_history.last() {
                    if *pane != ActivePane::Disassembly {
                        ui_state.navigation_history.pop();
                    } else {
                        break;
                    }
                }

                if let Some((pane, target)) = ui_state.navigation_history.pop() {
                    if pane == ActivePane::Disassembly {
                        use crate::ui_state::NavigationTarget;
                        match target {
                            NavigationTarget::Address(addr) => {
                                // Delegate to shared jump logic (no history push)
                                crate::ui::menu::perform_jump_to_address_no_history(
                                    app_state, ui_state, addr,
                                );
                                ui_state.set_status_message("Navigated back");
                            }
                            NavigationTarget::Index(idx) => {
                                if idx < app_state.disassembly.len() {
                                    ui_state.cursor_index = idx;
                                    ui_state.set_status_message("Navigated back");
                                } else {
                                    ui_state.set_status_message("History invalid");
                                }
                            }
                        }
                    }
                } else {
                    ui_state.set_status_message("No history");
                }
                WidgetResult::Handled
            }

            KeyCode::Char('l') if key.modifiers.is_empty() => action_set_label(app_state, ui_state),
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
            KeyCode::Char('p') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::PetsciiText)
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ScreencodeText)
            }
            KeyCode::Char('?')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(MenuAction::Undefined)
            }
            KeyCode::Char('<')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(MenuAction::SetLoHiAddress)
            }
            KeyCode::Char('>')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(MenuAction::SetHiLoAddress)
            }
            KeyCode::Char(',') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetLoHiWord)
            }
            KeyCode::Char('.') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetHiLoWord)
            }
            KeyCode::Char('|')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(MenuAction::ToggleSplitter)
            }
            KeyCode::Char(';')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(MenuAction::SideComment)
            }
            KeyCode::Char(':')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
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
            KeyCode::Char('[') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::PackLoHiAddress)
            }
            KeyCode::Char(']') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::PackHiLoAddress)
            }
            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(MenuAction::PreviousImmediateFormat)
            }

            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(MenuAction::ToggleCollapsedBlock)
            }
            _ => {
                // Check if modifiers contain CONTROL
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && let KeyCode::Char('b') = key.code
                {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        return WidgetResult::Action(MenuAction::ListBookmarks);
                    } else {
                        return WidgetResult::Action(MenuAction::ToggleBookmark);
                    }
                }

                if key.modifiers.contains(KeyModifiers::ALT)
                    && let KeyCode::Char('b') = key.code
                {
                    return WidgetResult::Action(MenuAction::ListBookmarks);
                }
                WidgetResult::Ignored
            }
        }
    }
}

pub fn action_set_label(app_state: &AppState, ui_state: &mut UIState) -> WidgetResult {
    if !app_state.raw_data.is_empty()
        && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
    {
        let mut target_addr = line.address;
        let mut current_sub_index = 0;
        let mut found = false;

        if line.bytes.is_empty() {
            if let Some(addr) = line.external_label_address {
                target_addr = addr;
            } else {
                // Header or empty line in external section -> Ignore 'l'
                return WidgetResult::Ignored;
            }
        } else if line.bytes.len() > 1 {
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
        ui_state.active_dialog = Some(Box::new(crate::ui::dialog_label::LabelDialog::new(
            text,
            target_addr,
        )));
        ui_state.set_status_message("Enter Label");
        WidgetResult::Handled
    } else {
        WidgetResult::Ignored
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;
    use crate::state::{AppState, Label, LabelKind, LabelType};

    #[test]
    fn test_get_sub_index_for_address() {
        let mut app_state = AppState::default();

        // Add label at $C001
        let label = Label {
            name: "test_label".to_string(),
            label_type: LabelType::UserDefined,
            kind: LabelKind::User,
        };
        app_state.labels.insert(0xC001, vec![label]);

        // Create line at $C000 with 2 bytes
        let line = DisassemblyLine {
            address: 0xC000,
            bytes: vec![0xA9, 0x00],
            mnemonic: "LDA".to_string(),
            operand: "#$00".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // Case 1: Target $C000 (instruction)
        let idx_main = DisassemblyView::get_sub_index_for_address(&line, &app_state, 0xC000);
        // Should point to instruction (index 1) because there is 1 label line before it
        assert_eq!(
            idx_main, 1,
            "Should point to instruction (index 1) because there is 1 label line before it"
        );

        // Case 2: Target $C001 (label)
        let idx_label = DisassemblyView::get_sub_index_for_address(&line, &app_state, 0xC001);
        assert_eq!(idx_label, 0, "Should point to label (index 0)");
    }

    #[test]
    fn test_handle_input_shifted_keys() {
        let mut app_state = AppState::default();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let mut view = DisassemblyView;

        let keys = vec!['?', '<', '>', '|', ';', ':'];
        let actions = vec![
            MenuAction::Undefined,
            MenuAction::SetLoHiAddress,
            MenuAction::SetHiLoAddress,
            MenuAction::ToggleSplitter,
            MenuAction::SideComment,
            MenuAction::LineComment,
        ];

        for (c, expected_action) in keys.into_iter().zip(actions.into_iter()) {
            // Test without SHIFT
            let key_no_shift = KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            };
            let result = view.handle_input(key_no_shift, &mut app_state, &mut ui_state);
            assert_eq!(result, WidgetResult::Action(expected_action.clone()));

            // Test with SHIFT (Windows behavior)
            let key_with_shift = KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            };
            let result = view.handle_input(key_with_shift, &mut app_state, &mut ui_state);
            assert_eq!(result, WidgetResult::Action(expected_action));
        }
    }
}
