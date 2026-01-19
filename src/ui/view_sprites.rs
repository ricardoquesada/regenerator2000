use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct SpritesView;

impl Widget for SpritesView {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Sprites;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let title = if ui_state.sprite_multicolor_mode {
            " Sprites (Multicolor) "
        } else {
            " Sprites (Single Color) "
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
        let padding = (64 - (origin % 64)) % 64;

        if app_state.raw_data.len() <= padding {
            return;
        }

        let usable_len = app_state.raw_data.len() - padding;
        let total_sprites = usable_len.div_ceil(64);

        let sprite_height = 22; // 21 lines + 1 separator
        let visible_rows = inner_area.height as usize;
        let num_sprites_fit = visible_rows.div_ceil(sprite_height); // Approximation

        let start_index = if ui_state.sprites_cursor_index > num_sprites_fit / 2 {
            ui_state
                .sprites_cursor_index
                .saturating_sub(num_sprites_fit / 2)
        } else {
            0
        };

        let end_index = (start_index + num_sprites_fit + 1).min(total_sprites);

        let mut y_offset = 0;
        for i in start_index..end_index {
            if y_offset >= visible_rows {
                break;
            }

            let sprite_offset_in_data = padding + i * 64;
            let sprite_address = origin + sprite_offset_in_data;

            if sprite_offset_in_data >= app_state.raw_data.len() {
                break;
            }

            // Draw Sprite Header/Index
            let is_selected = i == ui_state.sprites_cursor_index;
            let style = if is_selected {
                Style::default()
                    .fg(ui_state.theme.highlight_fg)
                    .bg(ui_state.theme.highlight_bg)
            } else {
                Style::default()
            };

            // Sprite number calculation: (Address / 64) % 256
            let sprite_num = (sprite_address / 64) % 256;

            if y_offset < visible_rows {
                f.render_widget(
                    Paragraph::new(format!(
                        "Sprite  {:03} / ${:02X} @ ${:04X}",
                        sprite_num, sprite_num, sprite_address
                    ))
                    .style(style),
                    Rect::new(
                        inner_area.x,
                        inner_area.y + y_offset as u16,
                        inner_area.width,
                        1,
                    ),
                );
                y_offset += 1;
            }

            // Draw Sprite Data (21 lines)
            for row in 0..21 {
                if y_offset >= visible_rows {
                    break;
                }

                let row_offset = sprite_offset_in_data + row * 3;
                // 3 bytes per row = 24 bits
                if row_offset + 2 < app_state.raw_data.len() {
                    let bytes = &app_state.raw_data[row_offset..row_offset + 3];

                    if ui_state.sprite_multicolor_mode {
                        // Multicolor Mode: 12 pixels per row, 2 bits per pixel
                        // Pixel width = 2 chars
                        let mut spans = Vec::with_capacity(12);
                        for b in bytes {
                            for pair in (0..4).rev() {
                                let bits = (b >> (pair * 2)) & 0b11;
                                let (char_str, fg_color) = match bits {
                                    0b00 => ("..", ui_state.theme.foreground), // Background (transparent-ish)
                                    0b01 => ("██", ui_state.theme.foreground), // Shared color 1 (Foreground/Highlight?) - standard is sprite color
                                    0b10 => ("██", ui_state.theme.sprite_multicolor_1), // MC 1
                                    0b11 => ("██", ui_state.theme.sprite_multicolor_2), // MC 2
                                    _ => unreachable!(),
                                };

                                // For 00 (background), we might want to be dim or just dots
                                let style = if bits == 0b00 {
                                    Style::default().fg(Color::DarkGray) // Dim dots
                                } else {
                                    Style::default().fg(fg_color)
                                };
                                spans.push(Span::styled(char_str, style));
                            }
                        }
                        f.render_widget(
                            Paragraph::new(Line::from(spans)),
                            Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1),
                        );
                    } else {
                        // Single Color Mode: 24 pixels per row, 1 bit per pixel
                        let mut line_str = String::with_capacity(24);
                        for b in bytes {
                            for bit in (0..8).rev() {
                                if (b >> bit) & 1 == 1 {
                                    line_str.push('█');
                                } else {
                                    line_str.push('.'); // Use dot for empty to see grid better, or space
                                }
                            }
                        }
                        f.render_widget(
                            Paragraph::new(line_str),
                            Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1), // Indent
                        );
                    }
                } else {
                    // Partial padding?
                    f.render_widget(
                        Paragraph::new("                        "),
                        Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1),
                    );
                }

                y_offset += 1;
            }
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let origin = app_state.origin as usize;
        let padding = (64 - (origin % 64)) % 64;
        let usable_len = app_state.raw_data.len().saturating_sub(padding);
        let total_sprites = usable_len.div_ceil(64);

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
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j')
                if key.modifiers.is_empty() || key.code == KeyCode::Down =>
            {
                ui_state.input_buffer.clear();
                if ui_state.sprites_cursor_index < total_sprites.saturating_sub(1) {
                    ui_state.sprites_cursor_index += 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if key.modifiers.is_empty() || key.code == KeyCode::Up =>
            {
                ui_state.input_buffer.clear();
                if ui_state.sprites_cursor_index > 0 {
                    ui_state.sprites_cursor_index -= 1;
                }
                WidgetResult::Handled
            }
            KeyCode::PageDown => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index =
                    (ui_state.sprites_cursor_index + 10).min(total_sprites.saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index =
                    (ui_state.sprites_cursor_index + 10).min(total_sprites.saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::PageUp => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Home => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index = 0;
                WidgetResult::Handled
            }
            KeyCode::End => {
                ui_state.input_buffer.clear();
                ui_state.sprites_cursor_index = total_sprites.saturating_sub(1);
                WidgetResult::Handled
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
                let is_buffer_empty = ui_state.input_buffer.is_empty();
                ui_state.input_buffer.clear();

                let target_sprite = if is_buffer_empty {
                    total_sprites
                } else {
                    entered_number
                };

                let new_cursor = if target_sprite == 0 {
                    total_sprites.saturating_sub(1)
                } else {
                    target_sprite
                        .saturating_sub(1)
                        .min(total_sprites.saturating_sub(1))
                };

                ui_state
                    .navigation_history
                    .push((ui_state.active_pane, ui_state.sprites_cursor_index));
                ui_state.sprites_cursor_index = new_cursor;
                ui_state.set_status_message(format!("Jumped to sprite {}", target_sprite));
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ToggleSpriteMulticolor)
            }
            _ => WidgetResult::Ignored,
        }
    }
}
