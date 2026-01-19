use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub enum InputResult {
    Ignored,
    Handled,
    Action(MenuAction),
}

pub fn render(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Charset;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let title = if ui_state.charset_multicolor_mode {
        " Charset (Multicolor) "
    } else {
        " Charset (Single Color) "
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
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app_state.raw_data.is_empty() {
        return;
    }

    let origin = app_state.origin as usize;
    // Align origin to next multiple of $400 as per user request (and consistent with events.rs)
    let base_alignment = 0x400;
    let aligned_start_addr = (origin / base_alignment) * base_alignment;

    // Grid Constants
    // Char is 8x8 pixels. Rendered as 8x4 text cells (half blocks).
    let char_render_width = 8;
    // let char_render_height = 4;
    let grid_cols = 8;
    let col_spacing = 1;
    let row_spacing = 1;

    // Width of one grid item including spacing
    let item_width = char_render_width + col_spacing;
    // Height of one grid item including spacing
    let item_height = 4 + row_spacing;

    let visible_rows = inner_area.height as usize;

    // We navigate by "Character Index" (0..N).
    // Mapping Index -> (GridRow, GridCol)
    // GridRow = Index / grid_cols
    // GridCol = Index % grid_cols

    let end_address = origin + app_state.raw_data.len();
    let total_chars = (end_address.saturating_sub(aligned_start_addr)).div_ceil(8);

    // Scroll Logic
    // We want the cursor row to be visible.
    let cursor_grid_row = ui_state.charset_cursor_index / grid_cols;

    // items fit vertically
    let rows_fit = visible_rows.div_ceil(item_height);

    // Calculate scroll offset (in grid rows)
    // If cursor is not in view/center, adjust scroll.

    let scroll_row = if cursor_grid_row > rows_fit / 2 {
        cursor_grid_row.saturating_sub(rows_fit / 2)
    } else {
        0
    };

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
                    "Charset  {} / (${:02X}) @ ${:04X}",
                    charset_num, charset_num, charset_address
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
            let x_pos = inner_area.x + (col_idx * item_width) as u16 + 1; // +1 margin
            let y_pos = inner_area.y + y_offset as u16;

            let is_selected = char_idx == ui_state.charset_cursor_index;

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
        }
        y_offset += item_height;
    }
}

pub fn handle_input(
    key: KeyEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> InputResult {
    let origin = app_state.origin as usize;
    let base_alignment = 0x400;
    let aligned_start_addr = (origin / base_alignment) * base_alignment;
    let end_addr = origin + app_state.raw_data.len();
    let max_char_index = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

    match key.code {
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
            InputResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j')
            if key.modifiers.is_empty() || key.code == KeyCode::Down =>
        {
            ui_state.input_buffer.clear();
            // Move Down by 8 (one row)
            if ui_state.charset_cursor_index + 8 < max_char_index {
                ui_state.charset_cursor_index += 8;
            } else {
                ui_state.charset_cursor_index = max_char_index.saturating_sub(1);
            }
            InputResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() || key.code == KeyCode::Up => {
            ui_state.input_buffer.clear();
            // Move Up by 8 (one row)
            ui_state.charset_cursor_index = ui_state.charset_cursor_index.saturating_sub(8);
            InputResult::Handled
        }
        KeyCode::Left | KeyCode::Char('h')
            if key.modifiers.is_empty() || key.code == KeyCode::Left =>
        {
            ui_state.input_buffer.clear();
            if ui_state.charset_cursor_index > 0 {
                ui_state.charset_cursor_index -= 1;
            }
            InputResult::Handled
        }
        KeyCode::Right | KeyCode::Char('l')
            if key.modifiers.is_empty() || key.code == KeyCode::Right =>
        {
            ui_state.input_buffer.clear();
            if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                ui_state.charset_cursor_index += 1;
            }
            InputResult::Handled
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            ui_state.input_buffer.clear();
            ui_state.charset_cursor_index =
                (ui_state.charset_cursor_index + 10).min(max_char_index.saturating_sub(1));
            InputResult::Handled
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            ui_state.input_buffer.clear();
            ui_state.charset_cursor_index = ui_state.charset_cursor_index.saturating_sub(10);
            InputResult::Handled
        }
        KeyCode::Home => {
            ui_state.input_buffer.clear();
            ui_state.charset_cursor_index = 0;
            InputResult::Handled
        }
        KeyCode::End => {
            ui_state.input_buffer.clear();
            ui_state.charset_cursor_index = max_char_index.saturating_sub(1);
            InputResult::Handled
        }
        KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
            let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
            let is_buffer_empty = ui_state.input_buffer.is_empty();
            ui_state.input_buffer.clear();

            let target_char = if is_buffer_empty {
                max_char_index
            } else {
                entered_number
            };

            let new_cursor = if target_char == 0 {
                max_char_index.saturating_sub(1)
            } else {
                target_char
                    .saturating_sub(1)
                    .min(max_char_index.saturating_sub(1))
            };

            ui_state
                .navigation_history
                .push((ui_state.active_pane, ui_state.charset_cursor_index));
            ui_state.charset_cursor_index = new_cursor;
            ui_state.set_status_message(format!("Jumped to char {}", target_char));
            InputResult::Handled
        }
        KeyCode::Char('m') if key.modifiers.is_empty() => {
            InputResult::Action(MenuAction::ToggleCharsetMulticolor)
        }
        _ => InputResult::Ignored,
    }
}
