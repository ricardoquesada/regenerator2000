use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

pub struct SpritesView;

impl Navigable for SpritesView {
    fn len(&self, app_state: &AppState) -> usize {
        let origin = app_state.origin as usize;
        let aligned_origin = (origin / 64) * 64;
        let end_address = origin + app_state.raw_data.len();
        let total_bytes = end_address.saturating_sub(aligned_origin);
        total_bytes.div_ceil(64)
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.sprites_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.sprites_selection_start.is_none() {
                ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
            }
        } else {
            ui_state.sprites_selection_start = None;
        }
        let total = self.len(app_state);
        if total == 0 {
            return;
        }
        ui_state.sprites_cursor_index =
            (ui_state.sprites_cursor_index + amount).min(total.saturating_sub(1));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.sprites_selection_start.is_none() {
                ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
            }
        } else {
            ui_state.sprites_selection_start = None;
        }
        ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 10);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 10);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = self.len(app_state);
        ui_state.sprites_cursor_index = index.min(total.saturating_sub(1));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let total = self.len(app_state);
        let target = if input == 0 {
            total.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(total.saturating_sub(1))
        };
        ui_state.sprites_cursor_index = target;
    }

    fn item_name(&self) -> &str {
        "sprite"
    }
}

impl Widget for SpritesView {
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
            _ => WidgetResult::Ignored,
        }
    }

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
        let aligned_origin = (origin / 64) * 64;
        let end_address = origin + app_state.raw_data.len();
        let total_bytes = end_address.saturating_sub(aligned_origin);
        let total_sprites = total_bytes.div_ceil(64);

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

            let sprite_addr_start = aligned_origin + i * 64;
            let sprite_address = sprite_addr_start;

            if sprite_addr_start >= end_address {
                break;
            }

            // Draw Sprite Header/Index
            let is_selected = if let Some(sel_start) = ui_state.sprites_selection_start {
                let (start, end) = if sel_start < ui_state.sprites_cursor_index {
                    (sel_start, ui_state.sprites_cursor_index)
                } else {
                    (ui_state.sprites_cursor_index, sel_start)
                };
                i >= start && i <= end
            } else {
                i == ui_state.sprites_cursor_index
            };
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

                let row_addr_start = sprite_addr_start + row * 3;

                // Fetch 3 bytes for the row, handling alignment/padding
                let mut bytes = [0u8; 3];
                for (b_idx, b) in bytes.iter_mut().enumerate() {
                    let addr = row_addr_start + b_idx;
                    if addr >= origin && addr < end_address {
                        *b = app_state.raw_data[addr - origin];
                    }
                }

                if row_addr_start < end_address {
                    let bytes = &bytes;

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
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            // Escape cancels visual mode / selection
            KeyCode::Esc => {
                if ui_state.sprites_selection_start.is_some() || ui_state.is_visual_mode {
                    ui_state.sprites_selection_start = None;
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
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.sprites_selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            // Shift+Down for selection
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.sprites_selection_start;
                if saved_selection.is_none() {
                    ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                }
                let selection_to_keep = ui_state.sprites_selection_start;
                // Move cursor
                let total = self.len(app_state);
                if total > 0 {
                    ui_state.sprites_cursor_index =
                        (ui_state.sprites_cursor_index + 1).min(total.saturating_sub(1));
                }
                // Restore selection for shift+arrow mode
                ui_state.sprites_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Up for selection
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.sprites_selection_start;
                if saved_selection.is_none() {
                    ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                }
                let selection_to_keep = ui_state.sprites_selection_start;
                // Move cursor
                ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(1);
                // Restore selection for shift+arrow mode
                ui_state.sprites_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ToggleSpriteMulticolor)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                // Convert selected sprites or current sprite to bytes block (64 bytes per sprite)
                let origin = app_state.origin as usize;
                let aligned_origin = (origin / 64) * 64;
                let end_address = origin + app_state.raw_data.len();

                // Determine sprite range based on selection
                let (start_sprite, end_sprite) =
                    if let Some(sel_start) = ui_state.sprites_selection_start {
                        if sel_start < ui_state.sprites_cursor_index {
                            (sel_start, ui_state.sprites_cursor_index)
                        } else {
                            (ui_state.sprites_cursor_index, sel_start)
                        }
                    } else {
                        (ui_state.sprites_cursor_index, ui_state.sprites_cursor_index)
                    };

                let start_addr = aligned_origin + start_sprite * 64;
                let end_addr = aligned_origin + (end_sprite + 1) * 64 - 1;

                // Clamp to actual data range
                let start_offset = start_addr.saturating_sub(origin);

                let end_offset_abs = end_addr.min(end_address.saturating_sub(1));
                let end_offset = end_offset_abs.saturating_sub(origin);

                // Clear selection after action
                ui_state.sprites_selection_start = None;
                ui_state.is_visual_mode = false;

                if start_offset < app_state.raw_data.len() && start_offset <= end_offset {
                    WidgetResult::Action(MenuAction::SetBytesBlockByOffset {
                        start: start_offset,
                        end: end_offset,
                    })
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Enter => {
                let origin = app_state.origin as usize;
                let aligned_origin = (origin / 64) * 64;
                let sprite_offset = ui_state.sprites_cursor_index * 64;
                let sprite_addr = aligned_origin + sprite_offset;

                // If this sprite contains the origin, jump to origin instead of the aligned boundary
                let target_addr = if origin >= sprite_addr && origin < sprite_addr + 64 {
                    origin as u16
                } else {
                    sprite_addr as u16
                };

                crate::ui::navigable::jump_to_disassembly_at_address(
                    app_state,
                    ui_state,
                    target_addr,
                )
            }
            _ => WidgetResult::Ignored,
        }
    }
}
