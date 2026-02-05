use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

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
            let counts = Self::get_visual_line_counts(line, app_state);
            let instruction_sub_idx = counts.labels + counts.comments;

            if ui_state.sub_cursor_index < instruction_sub_idx {
                // If we are currently on a label or comment (e.g. mouse click), jump to the instruction
                ui_state.sub_cursor_index = instruction_sub_idx;
            } else {
                // Move to next line, skipping metadata lines
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
                    let next_line = &app_state.disassembly[ui_state.cursor_index];
                    let next_counts = Self::get_visual_line_counts(next_line, app_state);
                    // Always land on the instruction, skipping labels and comments
                    ui_state.sub_cursor_index = next_counts.labels + next_counts.comments;
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
            // Unlike Down, Up always takes us to the previous line's instruction
            // regardless of where we are in the current line (instruction, comment, or label).
            if ui_state.cursor_index > 0 {
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

                // Check if the found prev_idx is valid (it might be 0 and valid, or 0 and invalid if file starts with metadata)
                let prev_line = &app_state.disassembly[prev_idx];
                if !prev_line.bytes.is_empty()
                    || prev_line.is_collapsed
                    || prev_line.label.is_some()
                    || !prev_line.mnemonic.is_empty()
                {
                    ui_state.cursor_index = prev_idx;
                    let prev_counts = Self::get_visual_line_counts(prev_line, app_state);
                    ui_state.sub_cursor_index = prev_counts.labels + prev_counts.comments;
                }
            } else if ui_state.sub_cursor_index > 0 {
                // Optimization/Edge-case: If we are at index 0 but sub-index > 0 (comment/label at file start),
                // do nothing as per existing logic analysis.
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
        // PageDown logic: flat 30 lines jump
        ui_state.cursor_index =
            (ui_state.cursor_index + 30).min(self.len(app_state).saturating_sub(1));
    }

    fn page_up(&self, _app_state: &AppState, ui_state: &mut UIState) {
        // PageUp logic: flat 10 lines jump
        ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let max = self.len(app_state).saturating_sub(1);
        ui_state.cursor_index = index.min(max);
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
        let context_lines = visible_height / 2;
        let start_index = ui_state.cursor_index.saturating_sub(context_lines);

        let mut current_y = 0;

        for (i, line) in app_state.disassembly.iter().enumerate().skip(start_index) {
            let counts = Self::get_visual_line_counts(line, app_state);
            let height = counts.total();

            if click_row < current_y + height {
                ui_state.cursor_index = i;
                ui_state.sub_cursor_index = click_row - current_y;

                if ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }
                return WidgetResult::Handled;
            }

            current_y += height;
            if current_y >= visible_height {
                break;
            }
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
                // If we have an opcode, use shared logic to decide if we should draw arrow
                if let Some(opcode) = &line.opcode {
                    if !opcode.is_flow_control_with_target() {
                        continue;
                    }
                } else if line.mnemonic.eq_ignore_ascii_case("JMP") && line.operand.contains('(') {
                    // Fallback check if opcode struct is missing but mnemonic is textual
                    // (Though line.opcode should usually be present for documented ops)
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

                let is_down = arrow.start < arrow.end;
                let mut passes_through = (current_line > low
                    && current_line < high
                    && arrow.start_visible
                    && arrow.end_visible)
                    || (current_line == arrow.start && arrow.end < arrow.start)
                    || (current_line == arrow.end && arrow.start < arrow.end);

                // Add tip logic for partial arrows to match get_arrow_str
                if !passes_through {
                    if arrow.start_visible && !arrow.end_visible {
                        if is_down
                            && (current_line == arrow.start
                                || current_line == arrow.start.saturating_add(1))
                        {
                            // If we are at start, comments are ABOVE, so no line.
                            // If we are at start+1, comments are ABOVE (between start and start+1), so draw line to connect with ▼
                            if current_line == arrow.start.saturating_add(1) {
                                passes_through = true;
                            }
                        } else if !is_down && current_line == arrow.start {
                            // Comments of start line for upward jump are ABOVE, so draw line
                            passes_through = true;
                        }
                    } else if !arrow.start_visible && arrow.end_visible {
                        if is_down && current_line == arrow.end {
                            // Comments of end line for downward jump are ABOVE, so draw line
                            passes_through = true;
                        } else if !is_down
                            && (current_line == arrow.end
                                || current_line == arrow.end.saturating_add(1))
                        {
                            // If we are at end, comments are ABOVE, so no line.
                            // If we are at end+1, comments are ABOVE (between end and end+1), so draw line to connect with │
                            if current_line == arrow.end.saturating_add(1) {
                                passes_through = true;
                            }
                        }
                    }
                }

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
                current_line_num += Self::get_visual_line_count_for_instruction(line, app_state);
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
                                if !refs.is_empty() && app_state.settings.max_xref_count > 0 {
                                    format!(
                                        "{} {}",
                                        formatter.comment_prefix(),
                                        crate::disassembler::format_cross_references(
                                            refs,
                                            app_state.settings.max_xref_count,
                                        )
                                    )
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
                                    Span::styled("                   ".to_string(), line_style),
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
                    for comment_part in line_comment.lines() {
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
                            Span::styled("                   ".to_string(), line_style),
                            Span::styled(
                                format!("{} {}", formatter.comment_prefix(), comment_part),
                                line_style.fg(ui_state.theme.comment),
                            ),
                        ]));
                        current_line_num += 1;
                        current_sub_index += 1;
                    }
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

                let show_address =
                    !line.bytes.is_empty() || line.is_collapsed || line.label.is_some();
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

                let mut spans = vec![
                    Span::styled(
                        format!("{:5} ", current_line_num),
                        line_style.fg(ui_state.theme.bytes),
                    ),
                    Span::styled(
                        format!("{:<width$} ", arrow_padding, width = arrow_width),
                        line_style.fg(ui_state.theme.arrow),
                    ),
                    Span::styled(
                        address_str,
                        if let Some(next_line) = app_state.disassembly.get(i + 1)
                            && app_state.splitters.contains(&next_line.address)
                            && show_address
                        {
                            line_style
                                .fg(ui_state.theme.address)
                                .add_modifier(Modifier::UNDERLINED)
                        } else if app_state.splitters.contains(&line.address) && show_address {
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
                ];

                if is_collapsed {
                    spans.push(Span::styled(
                        line.mnemonic.to_string(),
                        line_style
                            .fg(ui_state.theme.collapsed_block)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!("{: <16}", label_text),
                        line_style
                            .fg(ui_state.theme.label_def)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        format!("{: <4} ", line.mnemonic),
                        line_style
                            .fg(ui_state.theme.mnemonic)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        format!("{: <15}", line.operand),
                        line_style.fg(ui_state.theme.operand),
                    ));
                    spans.push(Span::styled(
                        if line.comment.is_empty() {
                            String::new()
                        } else {
                            format!("{} {}", formatter.comment_prefix(), line.comment)
                        },
                        line_style.fg(ui_state.theme.comment),
                    ));
                }

                item_lines.push(Line::from(spans));
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
            KeyCode::Char('t') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::SetLoHiWord)
            }
            KeyCode::Char('T')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
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
            _ => WidgetResult::Ignored,
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
