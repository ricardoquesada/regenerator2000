use crate::state::AppState;
use crate::ui::view_disassembly::DisassemblyView;
use crate::ui_state::{ActivePane, AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

/// Immediately mirror the charset cursor / selection into the disassembly
/// view's `cursor_index` / `selection_start`.  Called eagerly so both views
/// are consistent within the same render frame.
fn sync_charset_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    if app_state.raw_data.is_empty() || app_state.disassembly.is_empty() {
        return;
    }
    let origin = app_state.origin.0 as usize;
    let base_alignment = 0x400;
    let aligned_start_addr = (origin / base_alignment) * base_alignment;

    let cursor_addr = (aligned_start_addr + ui_state.charset_cursor_index * 8)
        .max(origin)
        .min(origin + app_state.raw_data.len().saturating_sub(1));

    if let Some(inst_idx) =
        app_state.get_line_index_containing_address(crate::state::Addr(cursor_addr as u16))
    {
        ui_state.cursor_index = inst_idx;
        let counts =
            DisassemblyView::get_visual_line_counts(&app_state.disassembly[inst_idx], app_state);
        ui_state.sub_cursor_index = counts.labels + counts.comments;

        ui_state.selection_start = ui_state.charset_selection_start.and_then(|sel_idx| {
            let anchor_addr = (aligned_start_addr + sel_idx * 8)
                .max(origin)
                .min(origin + app_state.raw_data.len().saturating_sub(1));
            app_state.get_line_index_containing_address(crate::state::Addr(anchor_addr as u16))
        });
    }
}

pub struct CharsetView;

impl Navigable for CharsetView {
    fn len(&self, app_state: &AppState) -> usize {
        let origin = app_state.origin.0 as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let end_addr = origin + app_state.raw_data.len();
        (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8)
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.charset_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.charset_selection_start.is_none() {
                ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
            }
        } else {
            ui_state.charset_selection_start = None;
        }
        let max_char_index = self.len(app_state);
        let grid_cols = charset_grid_cols(ui_state);
        // Move Down by grid_cols (one visual row)
        if ui_state.charset_cursor_index + (amount * grid_cols) < max_char_index {
            ui_state.charset_cursor_index += amount * grid_cols;
        } else {
            ui_state.charset_cursor_index = max_char_index.saturating_sub(1);
        }
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.charset_selection_start.is_none() {
                ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
            }
        } else {
            ui_state.charset_selection_start = None;
        }
        let grid_cols = charset_grid_cols(ui_state);
        // Move Up by grid_cols (one visual row)
        ui_state.charset_cursor_index = ui_state
            .charset_cursor_index
            .saturating_sub(amount * grid_cols);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        let max_char_index = self.len(app_state);
        let grid_cols = charset_grid_cols(ui_state);
        // Advance by 10 lines (10 rows × grid_cols characters)
        ui_state.charset_cursor_index =
            (ui_state.charset_cursor_index + 10 * grid_cols).min(max_char_index.saturating_sub(1));
    }

    fn page_up(&self, _app_state: &AppState, ui_state: &mut UIState) {
        let grid_cols = charset_grid_cols(ui_state);
        // Move back by 10 lines (10 rows × grid_cols characters)
        ui_state.charset_cursor_index =
            ui_state.charset_cursor_index.saturating_sub(10 * grid_cols);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let max_char_index = self.len(app_state);
        ui_state.charset_cursor_index = index.min(max_char_index.saturating_sub(1));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let max_char_index = self.len(app_state);
        let target_char = if input == 0 {
            max_char_index.saturating_sub(1)
        } else {
            input
                .saturating_sub(1)
                .min(max_char_index.saturating_sub(1))
        };
        ui_state.charset_cursor_index = target_char;
    }

    fn item_name(&self) -> &'static str {
        "char"
    }
}

/// Returns the number of character columns for the current charset view mode.
#[must_use]
fn charset_grid_cols(ui_state: &UIState) -> usize {
    if ui_state.right_pane == crate::ui_state::RightPane::Charset4Col {
        4
    } else {
        8
    }
}

impl CharsetView {
    /// Grid layout constants (must stay in sync with `render`).
    /// Note: `GRID_COLS` is now dynamic — use [`charset_grid_cols`] instead.
    const CHAR_RENDER_WIDTH: usize = 8;
    const COL_SPACING: usize = 1;
    const ROW_SPACING: usize = 1;
    const ITEM_WIDTH: usize = Self::CHAR_RENDER_WIDTH + Self::COL_SPACING; // 9
    const ITEM_HEIGHT: usize = 4 + Self::ROW_SPACING; // 5

    /// Compute the scroll row using scrolloff (scroll-margin) logic.
    ///
    /// The cursor can move freely within the viewport without scrolling;
    /// the viewport only shifts when the cursor enters the margin zone
    /// near the top or bottom edge.
    ///
    /// Updates `ui_state.charset_scroll_row` in place and returns the value.
    fn compute_scroll_row(ui_state: &mut UIState, rows_fit: usize) -> usize {
        let grid_cols = charset_grid_cols(ui_state);
        let cursor_grid_row = ui_state.charset_cursor_index / grid_cols;
        let margin = 1_usize.min(rows_fit / 3);

        let mut scroll_row = ui_state.charset_scroll_row;

        // Cursor too close to the bottom → scroll down
        if cursor_grid_row >= scroll_row + rows_fit.saturating_sub(margin) {
            scroll_row =
                cursor_grid_row.saturating_sub(rows_fit.saturating_sub(margin).saturating_sub(1));
        }
        // Cursor too close to the top → scroll up
        if cursor_grid_row < scroll_row + margin {
            scroll_row = cursor_grid_row.saturating_sub(margin);
        }

        ui_state.charset_scroll_row = scroll_row;
        scroll_row
    }

    /// Map a mouse position (column, row) to a character index, replicating the
    /// render loop's grid layout (scroll offset, header rows, grid geometry).
    ///
    /// Returns `None` if the position doesn't land on a character cell.
    #[must_use]
    fn hit_test_char_index(
        &self,
        mouse_col: u16,
        mouse_row: u16,
        app_state: &AppState,
        ui_state: &UIState,
    ) -> Option<usize> {
        let area = ui_state.right_pane_area;
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if app_state.raw_data.is_empty() {
            return None;
        }

        let origin = app_state.origin.0 as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let end_address = origin + app_state.raw_data.len();
        let total_chars = (end_address.saturating_sub(aligned_start_addr)).div_ceil(8);
        let grid_cols = charset_grid_cols(ui_state);

        let visible_rows = inner_area.height as usize;
        let rows_fit = visible_rows.div_ceil(Self::ITEM_HEIGHT);

        // Use the stored scroll row (already computed by the last render frame)
        let scroll_row = ui_state.charset_scroll_row;
        let end_row = scroll_row + rows_fit + 1;

        // Walk the render layout to find which char_idx the click lands on
        let mut y_offset: usize = 0;
        let click_rel_y = (mouse_row - inner_area.y) as usize;
        // +1 margin matches render's `x_pos = inner_area.x + (col_idx * item_width) as u16 + 1`
        let click_rel_x = (mouse_col as usize).checked_sub(inner_area.x as usize + 1)?;

        for row_idx in scroll_row..end_row {
            if y_offset >= visible_rows {
                break;
            }

            let charset_address = aligned_start_addr + (row_idx * grid_cols * 8);
            // Header row (every 2048 bytes)
            if charset_address.is_multiple_of(2048) {
                if click_rel_y == y_offset {
                    // Clicked on a header row — not a char cell
                    return None;
                }
                y_offset += 1;
                if y_offset >= visible_rows {
                    break;
                }
            }

            // Check if click_rel_y falls within this grid row's 4-line char area
            if click_rel_y >= y_offset && click_rel_y < y_offset + 4 {
                // Determine column
                let col_idx = click_rel_x / Self::ITEM_WIDTH;
                // Check if the click is in the gap between columns
                let col_offset = click_rel_x % Self::ITEM_WIDTH;
                if col_idx >= grid_cols || col_offset >= Self::CHAR_RENDER_WIDTH {
                    return None; // In the spacing gap or beyond grid
                }

                let char_idx = row_idx * grid_cols + col_idx;
                if char_idx < total_chars {
                    return Some(char_idx);
                }
                return None;
            }

            // Check if click is in the row_spacing gap below the char cells
            if click_rel_y >= y_offset + 4 && click_rel_y < y_offset + Self::ITEM_HEIGHT {
                return None; // In the vertical gap between rows
            }

            y_offset += Self::ITEM_HEIGHT;
        }

        None
    }
}

impl Widget for CharsetView {
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let area = ui_state.right_pane_area;
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

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                WidgetResult::Handled
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                let is_drag = matches!(mouse.kind, MouseEventKind::Drag(_));
                let shift_held = mouse.modifiers.contains(KeyModifiers::SHIFT);

                if let Some(char_idx) =
                    self.hit_test_char_index(mouse.column, mouse.row, app_state, ui_state)
                {
                    // Drag or Shift+Click or visual mode: anchor selection
                    if is_drag || shift_held || ui_state.is_visual_mode {
                        if ui_state.charset_selection_start.is_none() {
                            ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                        }
                    } else {
                        // Plain click: clear selection
                        ui_state.charset_selection_start = None;
                    }

                    ui_state.charset_cursor_index = char_idx;
                }
                sync_charset_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }

    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Charset;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let origin = app_state.origin.0 as usize;
        // Align origin to next multiple of $400 as per user request (and consistent with events.rs)
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;

        let cursor_info = if app_state.raw_data.is_empty() {
            String::new()
        } else {
            let cursor_char_addr = aligned_start_addr + ui_state.charset_cursor_index * 8;
            let cursor_char_idx = (cursor_char_addr / 8) % 256;
            format!(" - Char #{cursor_char_idx}, Addr: ${cursor_char_addr:04X}")
        };

        let mode_label = if ui_state.charset_multicolor_mode {
            "Multicolor"
        } else {
            "Single Color"
        };

        let title = format!(" Charset{cursor_info} - {mode_label} ");

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title.as_str())
            .style(
                Style::default()
                    .bg(ui_state.theme.background)
                    .fg(ui_state.theme.foreground),
            );
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        if app_state.raw_data.is_empty() {
            return;
        }

        let visible_rows = inner_area.height as usize;

        let end_address = origin + app_state.raw_data.len();
        let total_chars = (end_address.saturating_sub(aligned_start_addr)).div_ceil(8);
        let grid_cols = charset_grid_cols(ui_state);

        // Scrolloff logic: cursor can move freely within the viewport;
        // viewport only shifts when cursor enters the margin zone.
        let rows_fit = visible_rows.div_ceil(Self::ITEM_HEIGHT);
        let scroll_row = Self::compute_scroll_row(ui_state, rows_fit);

        let end_row = scroll_row + rows_fit + 1; // Render a bit extra

        let mut y_offset = 0;

        for row_idx in scroll_row..end_row {
            if y_offset >= visible_rows {
                break;
            }

            let charset_address = aligned_start_addr + (row_idx * grid_cols * 8);
            // Header every 2048 bytes (address-aligned)
            if charset_address.is_multiple_of(2048) {
                // There can only be at most 8 charsets per VIC-II bank (16K per bank)
                let charset_num = charset_address / 2048 % 8;

                f.render_widget(
                    Paragraph::new(format!(
                        "Charset  {charset_num} / (${charset_num:02X}) @ ${charset_address:04X}"
                    ))
                    .style(Style::default().fg(ui_state.theme.comment)),
                    Rect::new(
                        inner_area.x,
                        inner_area.y + y_offset as u16,
                        inner_area.width,
                        1,
                    ),
                );
                y_offset += 1;
                if y_offset >= visible_rows {
                    break;
                }
            }

            for col_idx in 0..grid_cols {
                let char_idx = row_idx * grid_cols + col_idx;
                if char_idx >= total_chars {
                    continue;
                }

                let char_offset = char_idx * 8;
                let char_addr = aligned_start_addr + char_offset;

                // Render Char
                let x_pos = inner_area.x + (col_idx * Self::ITEM_WIDTH) as u16 + 1; // +1 margin
                let y_pos = inner_area.y + y_offset as u16;

                let is_selected = if let Some(sel_start) = ui_state.charset_selection_start {
                    let (start, end) = if sel_start < ui_state.charset_cursor_index {
                        (sel_start, ui_state.charset_cursor_index)
                    } else {
                        (ui_state.charset_cursor_index, sel_start)
                    };
                    char_idx >= start && char_idx <= end
                } else {
                    char_idx == ui_state.charset_cursor_index
                };

                // Draw 4 lines of half-blocks
                for line in 0..4 {
                    if y_offset + line >= visible_rows {
                        break;
                    }

                    let row_addr_top = char_addr + line * 2;
                    let byte_top = if row_addr_top >= origin && row_addr_top < end_address {
                        app_state.raw_data[row_addr_top - origin]
                    } else {
                        0
                    };

                    let row_addr_bot = char_addr + line * 2 + 1;
                    let byte_bot = if row_addr_bot >= origin && row_addr_bot < end_address {
                        app_state.raw_data[row_addr_bot - origin]
                    } else {
                        0
                    };

                    // Different rendering for multicolor vs standard
                    if ui_state.charset_multicolor_mode {
                        // Multicolor: 4 pixels width, double wide (2 chars per pixel)
                        // 2 bits per pixel.
                        // 00=bg, 01=fg, 10=mc1, 11=mc2
                        let mut spans = Vec::with_capacity(4);

                        for pixel_idx in (0..4).rev() {
                            let shift = pixel_idx * 2;

                            // Get 2 bits for top and bottom
                            let bits_top = (byte_top >> shift) & 0b11;
                            let bits_bot = (byte_bot >> shift) & 0b11;

                            let color_top = match bits_top {
                                0b00 => ui_state.theme.background, // Or explicit BG
                                0b01 => ui_state.theme.foreground,
                                0b10 => ui_state.theme.charset_multicolor_1,
                                0b11 => ui_state.theme.charset_multicolor_2,
                                _ => unreachable!(),
                            };

                            let color_bot = match bits_bot {
                                0b00 => ui_state.theme.background,
                                0b01 => ui_state.theme.foreground,
                                0b10 => ui_state.theme.charset_multicolor_1,
                                0b11 => ui_state.theme.charset_multicolor_2,
                                _ => unreachable!(),
                            };

                            let mut style = Style::default().fg(color_top).bg(color_bot);

                            // Apply selection - tricky with multicolor.
                            // If selected, force background 00 to be selection_bg?
                            if is_selected {
                                if bits_top == 0b00 {
                                    style = style.fg(ui_state.theme.selection_bg);
                                }
                                if bits_bot == 0b00 {
                                    style = style.bg(ui_state.theme.selection_bg);
                                }
                            }

                            // Double wide pixel
                            spans.push(Span::styled("▀▀", style));
                        }
                        f.render_widget(
                            Paragraph::new(Line::from(spans)),
                            Rect::new(x_pos, y_pos + line as u16, 8, 1),
                        );
                    } else {
                        let mut line_str = String::with_capacity(8);
                        for bit in (0..8).rev() {
                            let t = (byte_top >> bit) & 1;
                            let b = (byte_bot >> bit) & 1;

                            let c = match (t, b) {
                                (0, 0) => ' ',
                                (1, 0) => '▀',
                                (0, 1) => '▄',
                                (1, 1) => '█',
                                _ => unreachable!(),
                            };
                            line_str.push(c);
                        }

                        let fg_style = if is_selected {
                            Style::default()
                                .fg(ui_state.theme.selection_fg)
                                .bg(ui_state.theme.selection_bg)
                        } else {
                            Style::default().fg(ui_state.theme.foreground)
                        };

                        f.render_widget(
                            Paragraph::new(line_str).style(fg_style),
                            Rect::new(x_pos, y_pos + line as u16, 8, 1),
                        );
                    }
                }

                // Highlight: fill all four gap sides with selection_bg to form a
                // clear rectangular frame around the selected char cell.
                if is_selected {
                    let gap_style = Style::default()
                        .bg(ui_state.theme.selection_bg)
                        .fg(ui_state.theme.selection_bg);
                    // Top row (only when there is a gap row above)
                    if y_pos > inner_area.y {
                        f.render_widget(
                            Paragraph::new(" ".repeat(10)).style(gap_style),
                            Rect::new(x_pos - 1, y_pos - 1, 10, 1),
                        );
                    }
                    // Bottom row
                    f.render_widget(
                        Paragraph::new(" ".repeat(10)).style(gap_style),
                        Rect::new(x_pos - 1, y_pos + 4, 10, 1),
                    );
                    // Left col
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(" "),
                            Line::from(" "),
                            Line::from(" "),
                            Line::from(" "),
                        ])
                        .style(gap_style),
                        Rect::new(x_pos - 1, y_pos, 1, 4),
                    );
                    // Right col
                    f.render_widget(
                        Paragraph::new(vec![
                            Line::from(" "),
                            Line::from(" "),
                            Line::from(" "),
                            Line::from(" "),
                        ])
                        .style(gap_style),
                        Rect::new(x_pos + 8, y_pos, 1, 4),
                    );
                }
            }
            y_offset += Self::ITEM_HEIGHT;
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            sync_charset_to_disassembly(app_state, ui_state);
            return WidgetResult::Handled;
        }

        // Recalculate max_char_index for local logic (h/l)
        // Or could we extract it?
        // Let's rely on self.len() helper but it needs AppState.
        // It's cleaner to just recalc here or use the helper if I made it public/extractable,
        // but Navigable::len takes &self.
        // So I can call self.len(app_state).

        let result = match key.code {
            // Escape cancels visual mode / selection
            KeyCode::Esc => {
                if ui_state.charset_selection_start.is_some() || ui_state.is_visual_mode {
                    ui_state.charset_selection_start = None;
                    ui_state.is_visual_mode = false;
                    ui_state.set_status_message("");
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            // Visual mode toggle
            KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                if !app_state.raw_data.is_empty() {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.charset_selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            // Shift+Down for selection
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.charset_selection_start;
                if saved_selection.is_none() {
                    ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                }
                let selection_to_keep = ui_state.charset_selection_start;
                // Move cursor down by grid_cols (one visual row)
                let max_char_index = self.len(app_state);
                let grid_cols = charset_grid_cols(ui_state);
                if ui_state.charset_cursor_index + grid_cols < max_char_index {
                    ui_state.charset_cursor_index += grid_cols;
                } else {
                    ui_state.charset_cursor_index = max_char_index.saturating_sub(1);
                }
                // Restore selection for shift+arrow mode
                ui_state.charset_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Up for selection
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.charset_selection_start;
                if saved_selection.is_none() {
                    ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                }
                let selection_to_keep = ui_state.charset_selection_start;
                // Move cursor up by grid_cols (one visual row)
                let grid_cols = charset_grid_cols(ui_state);
                ui_state.charset_cursor_index =
                    ui_state.charset_cursor_index.saturating_sub(grid_cols);
                // Restore selection for shift+arrow mode
                ui_state.charset_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Left for selection
            KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.charset_selection_start;
                if saved_selection.is_none() {
                    ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                }
                let selection_to_keep = ui_state.charset_selection_start;
                if ui_state.charset_cursor_index > 0 {
                    ui_state.charset_cursor_index -= 1;
                }
                ui_state.charset_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Right for selection
            KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.charset_selection_start;
                if saved_selection.is_none() {
                    ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                }
                let selection_to_keep = ui_state.charset_selection_start;
                let max_char_index = self.len(app_state);
                if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                    ui_state.charset_cursor_index += 1;
                }
                ui_state.charset_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            KeyCode::Left | KeyCode::Char('h')
                if key.modifiers.is_empty() || key.code == KeyCode::Left =>
            {
                ui_state.input_buffer.clear();
                if ui_state.is_visual_mode {
                    if ui_state.charset_selection_start.is_none() {
                        ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                    }
                } else {
                    ui_state.charset_selection_start = None;
                }
                if ui_state.charset_cursor_index > 0 {
                    ui_state.charset_cursor_index -= 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Right | KeyCode::Char('l')
                if key.modifiers.is_empty() || key.code == KeyCode::Right =>
            {
                ui_state.input_buffer.clear();
                if ui_state.is_visual_mode {
                    if ui_state.charset_selection_start.is_none() {
                        ui_state.charset_selection_start = Some(ui_state.charset_cursor_index);
                    }
                } else {
                    ui_state.charset_selection_start = None;
                }
                let max_char_index = self.len(app_state);
                if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                    ui_state.charset_cursor_index += 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::ToggleCharsetMulticolor)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                // Convert selected characters or current character to bytes block (8 bytes per character)
                let origin = app_state.origin.0 as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;

                // Determine character range based on selection
                let (start_char, end_char) =
                    if let Some(sel_start) = ui_state.charset_selection_start {
                        if sel_start < ui_state.charset_cursor_index {
                            (sel_start, ui_state.charset_cursor_index)
                        } else {
                            (ui_state.charset_cursor_index, sel_start)
                        }
                    } else {
                        (ui_state.charset_cursor_index, ui_state.charset_cursor_index)
                    };

                let start_char_addr = aligned_start_addr + (start_char * 8);
                let end_char_addr = aligned_start_addr + ((end_char + 1) * 8) - 1;

                // Calculate the byte offset range within raw_data
                let start_offset = start_char_addr.saturating_sub(origin);
                let end_offset = (end_char_addr.saturating_sub(origin))
                    .min(app_state.raw_data.len().saturating_sub(1));

                // Clear selection after action
                ui_state.charset_selection_start = None;
                ui_state.is_visual_mode = false;

                if start_offset < app_state.raw_data.len() {
                    WidgetResult::Action(AppAction::SetBytesBlockByOffset {
                        start: start_offset,
                        end: end_offset,
                    })
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Enter => {
                let origin = app_state.origin.0 as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                let char_offset = ui_state.charset_cursor_index * 8;
                let char_addr = aligned_start_addr + char_offset;

                // If this char contains the origin, jump to origin instead of the aligned boundary
                let target_addr = if origin >= char_addr && origin < char_addr + 8 {
                    origin as u16
                } else {
                    char_addr as u16
                };

                crate::ui::navigable::jump_to_disassembly_at_address(
                    app_state,
                    ui_state,
                    crate::state::Addr(target_addr),
                )
            }
            _ => WidgetResult::Ignored,
        };

        if matches!(result, WidgetResult::Handled) {
            sync_charset_to_disassembly(app_state, ui_state);
        }
        result
    }
}
