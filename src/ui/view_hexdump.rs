use crate::state::{AppState, PetsciiMode};
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct HexDumpView;

impl Widget for HexDumpView {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::HexDump;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(match ui_state.petscii_mode {
                PetsciiMode::Shifted => " Hex Dump (Shifted) ",
                PetsciiMode::Unshifted => " Hex Dump (Unshifted) ",
            })
            .style(
                Style::default()
                    .bg(ui_state.theme.background)
                    .fg(ui_state.theme.foreground),
            );
        let inner_area = block.inner(area);

        let visible_height = inner_area.height as usize;
        // Each row is 16 bytes
        let bytes_per_row = 16;
        let origin = app_state.origin as usize;
        let alignment_padding = origin % bytes_per_row;
        let aligned_origin = origin - alignment_padding;

        let total_len = app_state.raw_data.len() + alignment_padding;
        let total_rows = total_len.div_ceil(bytes_per_row);

        let context_lines = visible_height / 2;
        let offset = ui_state.hex_cursor_index.saturating_sub(context_lines);

        let items: Vec<ListItem> = (0..visible_height)
            .map(|i| {
                let row_index = offset + i;
                if row_index >= total_rows {
                    return ListItem::new("");
                }

                let row_start_addr = aligned_origin + (row_index * bytes_per_row);

                let mut hex_part = String::with_capacity(3 * 16);
                let mut ascii_part = String::with_capacity(16);

                for j in 0..bytes_per_row {
                    let current_addr = row_start_addr + j;

                    if current_addr >= origin && current_addr < origin + app_state.raw_data.len() {
                        let data_idx = current_addr - origin;
                        let b = app_state.raw_data[data_idx];

                        hex_part.push_str(&format!("{:02X} ", b));
                        let is_shifted = ui_state.petscii_mode == PetsciiMode::Shifted;
                        ascii_part.push(crate::utils::petscii_to_unicode(b, is_shifted));
                    } else {
                        // Padding
                        hex_part.push_str("   ");
                        ascii_part.push(' ');
                    }

                    if j == 7 {
                        hex_part.push(' '); // Extra space after 8 bytes
                    }
                }

                let is_selected = if let Some(selection_start) = ui_state.selection_start {
                    let (start, end) = if selection_start < ui_state.cursor_index {
                        (selection_start, ui_state.cursor_index)
                    } else {
                        (ui_state.cursor_index, selection_start)
                    };
                    row_index >= start && row_index <= end
                } else {
                    false
                };

                let style = if row_index == ui_state.hex_cursor_index {
                    Style::default().bg(ui_state.theme.selection_bg)
                } else if is_selected {
                    Style::default()
                        .bg(ui_state.theme.selection_bg)
                        .fg(ui_state.theme.selection_fg)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{:04X}  ", row_start_addr),
                        Style::default().fg(ui_state.theme.address),
                    ),
                    Span::styled(
                        format!("{:<49}", hex_part),
                        Style::default().fg(ui_state.theme.hex_bytes),
                    ), // 49 = 16*3 + 1 extra space
                    Span::styled(
                        format!("| {}", ascii_part),
                        Style::default().fg(ui_state.theme.hex_ascii),
                    ),
                ]);

                ListItem::new(line).style(style)
            })
            .collect();

        let list = List::new(items).block(block);

        // We handle scrolling manually via offset, so no ListState needed for scrolling,
        // but useful if we wanted ratatui to handle it.
        // However, similar to render_disassembly, we render what's visible.
        f.render_widget(list, area);
        ui_state.hex_scroll_index = offset;
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let bytes_per_row = 16;
        let padding = (app_state.origin as usize) % bytes_per_row;
        let total_rows = (app_state.raw_data.len() + padding).div_ceil(bytes_per_row);

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
                if key.code == KeyCode::Down || key.modifiers.is_empty() =>
            {
                ui_state.input_buffer.clear();
                if ui_state.hex_cursor_index < total_rows.saturating_sub(1) {
                    ui_state.hex_cursor_index += 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if key.code == KeyCode::Up || key.modifiers.is_empty() =>
            {
                ui_state.input_buffer.clear();
                if ui_state.hex_cursor_index > 0 {
                    ui_state.hex_cursor_index -= 1;
                }
                WidgetResult::Handled
            }
            KeyCode::PageDown => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index =
                    (ui_state.hex_cursor_index + 10).min(total_rows.saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index =
                    (ui_state.hex_cursor_index + 10).min(total_rows.saturating_sub(1));
                WidgetResult::Handled
            }
            KeyCode::PageUp => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(10);
                WidgetResult::Handled
            }
            KeyCode::Home => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index = 0;
                WidgetResult::Handled
            }
            KeyCode::End => {
                ui_state.input_buffer.clear();
                ui_state.hex_cursor_index = total_rows.saturating_sub(1);
                WidgetResult::Handled
            }
            KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
                let is_buffer_empty = ui_state.input_buffer.is_empty();
                ui_state.input_buffer.clear();

                let target_row = if is_buffer_empty {
                    total_rows
                } else {
                    entered_number
                };

                let new_cursor = if target_row == 0 {
                    total_rows.saturating_sub(1)
                } else {
                    target_row
                        .saturating_sub(1)
                        .min(total_rows.saturating_sub(1))
                };

                ui_state
                    .navigation_history
                    .push((ui_state.active_pane, ui_state.hex_cursor_index));
                ui_state.hex_cursor_index = new_cursor;
                ui_state.set_status_message(format!("Jumped to row {}", target_row));
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::TogglePetsciiMode)
            }
            _ => WidgetResult::Ignored,
        }
    }
}
