use crate::state::AppState;
use crate::ui_state::{ActivePane, AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::navigable::{
    AlignedGridRange, Navigable, apply_grid_mouse_click, compute_scroll_offset,
    handle_grid_selection_input, handle_nav_input, is_grid_item_selected, move_grid_cursor_down,
    move_grid_cursor_up, right_pane_mouse_inner_area, sync_grid_item_to_disassembly,
};
use crate::ui::widget::{Widget, WidgetResult};

/// Immediately mirror the charset cursor / selection into the disassembly
/// view's `cursor_index` / `selection_start`.  Called eagerly so both views
/// are consistent within the same render frame.
fn sync_charset_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    let grid = CharsetView::grid_range(app_state);
    sync_grid_item_to_disassembly(
        app_state,
        ui_state,
        &grid,
        ui_state.charset_cursor_index,
        ui_state.charset_selection_start,
    );
}

pub struct CharsetView;

impl Navigable for CharsetView {
    fn len(&self, app_state: &AppState) -> usize {
        Self::grid_range(app_state).item_count()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.charset_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let total = self.len(app_state);
        let delta = amount * charset_grid_cols(ui_state);
        move_grid_cursor_down(
            &mut ui_state.core.charset_cursor_index,
            &mut ui_state.core.charset_selection_start,
            ui_state.core.is_visual_mode,
            false,
            delta,
            total,
        );
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let delta = amount * charset_grid_cols(ui_state);
        move_grid_cursor_up(
            &mut ui_state.core.charset_cursor_index,
            &mut ui_state.core.charset_selection_start,
            ui_state.core.is_visual_mode,
            false,
            delta,
        );
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let max_char_index = self.len(app_state);
        ui_state.charset_cursor_index = index.min(max_char_index.saturating_sub(1));
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
    const BASE_ALIGNMENT: usize = 0x400;
    const CHAR_BYTES: usize = 8;

    /// Grid layout constants (must stay in sync with `render`).
    /// Note: `GRID_COLS` is now dynamic — use [`charset_grid_cols`] instead.
    const CHAR_RENDER_WIDTH: usize = 8;
    const COL_SPACING: usize = 1;
    const ROW_SPACING: usize = 1;
    const ITEM_WIDTH: usize = Self::CHAR_RENDER_WIDTH + Self::COL_SPACING; // 9
    const ITEM_HEIGHT: usize = 4 + Self::ROW_SPACING; // 5

    #[must_use]
    fn grid_range(app_state: &AppState) -> AlignedGridRange {
        AlignedGridRange::new(
            app_state,
            Self::BASE_ALIGNMENT,
            Self::CHAR_BYTES,
            Self::CHAR_BYTES,
        )
    }

    /// Map a mouse position (column, row) inside `inner_area` to a character index,
    /// replicating the render loop's grid layout (scroll offset, header rows, grid geometry).
    ///
    /// Returns `None` if the position doesn't land on a character cell.
    #[must_use]
    fn hit_test_char_index(
        &self,
        mouse_col: u16,
        mouse_row: u16,
        inner_area: Rect,
        app_state: &AppState,
        ui_state: &UIState,
    ) -> Option<usize> {
        if app_state.raw_data.is_empty() {
            return None;
        }

        let grid = Self::grid_range(app_state);
        let total_chars = grid.item_count();
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

            let charset_address = grid.item_addr(row_idx * grid_cols);
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
                return (char_idx < total_chars).then_some(char_idx);
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
        let Some(inner_area) = right_pane_mouse_inner_area(ui_state, &mouse) else {
            return WidgetResult::Ignored;
        };

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                sync_charset_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                sync_charset_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                let hit = self.hit_test_char_index(
                    mouse.column,
                    mouse.row,
                    inner_area,
                    app_state,
                    ui_state,
                );
                apply_grid_mouse_click(
                    &mouse,
                    ui_state.core.is_visual_mode,
                    &mut ui_state.core.charset_cursor_index,
                    &mut ui_state.core.charset_selection_start,
                    hit,
                );
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

        let grid = Self::grid_range(app_state);

        let cursor_info = if app_state.raw_data.is_empty() {
            String::new()
        } else {
            let cursor_char_addr = grid.item_addr(ui_state.charset_cursor_index);
            let cursor_char_idx = (cursor_char_addr / Self::CHAR_BYTES) % 256;
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

        let total_chars = grid.item_count();
        let grid_cols = charset_grid_cols(ui_state);
        let total_rows = total_chars.div_ceil(grid_cols);

        // Scroll-margin logic: shift viewport only when cursor enters the margin zone.
        let rows_fit = visible_rows.div_ceil(Self::ITEM_HEIGHT);
        let scroll_row = compute_scroll_offset(
            ui_state.charset_scroll_row,
            ui_state.charset_cursor_index / grid_cols,
            rows_fit,
            total_rows,
            1,
        );
        ui_state.charset_scroll_row = scroll_row;

        let end_row = scroll_row + rows_fit + 1; // Render a bit extra

        let mut y_offset = 0;

        for row_idx in scroll_row..end_row {
            if y_offset >= visible_rows {
                break;
            }

            let charset_address = grid.item_addr(row_idx * grid_cols);
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

                let char_addr = grid.item_addr(char_idx);

                // Render Char
                let x_pos = inner_area.x + (col_idx * Self::ITEM_WIDTH) as u16 + 1; // +1 margin
                let y_pos = inner_area.y + y_offset as u16;

                let is_selected = is_grid_item_selected(
                    char_idx,
                    ui_state.charset_cursor_index,
                    ui_state.charset_selection_start,
                );

                // Draw 4 lines of half-blocks
                for line in 0..4 {
                    if y_offset + line >= visible_rows {
                        break;
                    }

                    let byte_top = grid.byte_at(&app_state.raw_data, char_addr + line * 2);
                    let byte_bot = grid.byte_at(&app_state.raw_data, char_addr + line * 2 + 1);

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
                    // Draw top border only when a gap row exists above this cell.
                    if !charset_address.is_multiple_of(2048) && y_pos > inner_area.y {
                        f.render_widget(
                            Paragraph::new(" ".repeat(10)).style(gap_style),
                            Rect::new(x_pos - 1, y_pos - 1, 10, 1),
                        );
                    }
                    // Draw bottom border only when it fits inside the inner viewport.
                    if y_pos + 4 < inner_area.bottom() {
                        f.render_widget(
                            Paragraph::new(" ".repeat(10)).style(gap_style),
                            Rect::new(x_pos - 1, y_pos + 4, 10, 1),
                        );
                    }
                    // Clamp left and right border height to the inner viewport.
                    let h = 4.min(inner_area.bottom().saturating_sub(y_pos));
                    if h > 0 {
                        let side_lines: Vec<Line> =
                            (0..usize::from(h)).map(|_| Line::from(" ")).collect();
                        f.render_widget(
                            Paragraph::new(side_lines.clone()).style(gap_style),
                            Rect::new(x_pos - 1, y_pos, 1, h),
                        );
                        f.render_widget(
                            Paragraph::new(side_lines).style(gap_style),
                            Rect::new(x_pos + 8, y_pos, 1, h),
                        );
                    }
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
        if key.code == KeyCode::Char('m') && key.modifiers.is_empty() {
            return WidgetResult::Action(AppAction::ToggleCharsetMulticolor);
        }

        let grid = Self::grid_range(app_state);
        let cols = charset_grid_cols(ui_state);
        let cursor = ui_state.charset_cursor_index;
        let selection = ui_state.charset_selection_start;
        let result = handle_grid_selection_input(
            key,
            app_state,
            ui_state,
            &grid,
            cols,
            cursor,
            selection,
            |ui, new_cursor, new_sel| {
                ui.charset_cursor_index = new_cursor;
                ui.charset_selection_start = new_sel;
            },
        );
        if matches!(result, WidgetResult::Handled) {
            sync_charset_to_disassembly(app_state, ui_state);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_charset_view_selecting_char_0_preserves_bank_header_row() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0xAA; 64];
        app_state.block_types = vec![BlockType::Code; 64];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Charset;
        ui_state.charset_cursor_index = 0;

        let area = Rect::new(0, 0, 80, 12);
        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).unwrap();

        let view = CharsetView;
        terminal
            .draw(|f| {
                view.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let header_row: String = (1..79)
            .map(|x| {
                buffer
                    .cell((x, 1))
                    .map_or(String::new(), |c| c.symbol().to_string())
            })
            .collect();
        assert!(
            header_row.contains("Charset  4 / ($04) @ $2000"),
            "Selecting character 0 must not overwrite the bank header row, got: '{header_row}'"
        );
    }

    #[test]
    fn test_charset_view_b_on_alignment_padding_before_origin_is_ignored() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0809);
        app_state.raw_data = vec![0xEA; 32];
        app_state.block_types = vec![BlockType::Code; 32];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Charset;
        // Character 0 spans $0800..=$0807, which is strictly before origin $0809.
        ui_state.charset_cursor_index = 0;

        let mut view = CharsetView;
        let res = view.handle_input(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Ignored);
        assert_eq!(app_state.block_types[0], BlockType::Code);
    }
}
