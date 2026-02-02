use crate::state::{AppState, HexdumpViewMode};
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

pub struct HexDumpView;

impl Navigable for HexDumpView {
    fn len(&self, app_state: &AppState) -> usize {
        let bytes_per_row = 16;
        let padding = (app_state.origin as usize) % bytes_per_row;
        (app_state.raw_data.len() + padding).div_ceil(bytes_per_row)
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.hex_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.hex_selection_start.is_none() {
                ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
            }
        } else {
            ui_state.hex_selection_start = None;
        }
        let total = self.len(app_state);
        if total == 0 {
            return;
        }
        ui_state.hex_cursor_index =
            (ui_state.hex_cursor_index + amount).min(total.saturating_sub(1));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.hex_selection_start.is_none() {
                ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
            }
        } else {
            ui_state.hex_selection_start = None;
        }
        ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 10);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 10);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = self.len(app_state);
        ui_state.hex_cursor_index = index.min(total.saturating_sub(1));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let total = self.len(app_state);
        let target = if input == 0 {
            total.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(total.saturating_sub(1))
        };
        ui_state.hex_cursor_index = target;
    }

    fn item_name(&self) -> &str {
        "row"
    }
}

impl Widget for HexDumpView {
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

        let click_row = (mouse.row - inner_area.y) as usize;
        let visible_height = inner_area.height as usize;
        let context_lines = visible_height / 2;
        let offset = ui_state.hex_cursor_index.saturating_sub(context_lines);

        let row_index = offset + click_row;
        let total_rows = self.len(app_state);

        if row_index < total_rows {
            ui_state.hex_cursor_index = row_index;
            if ui_state.is_visual_mode {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                }
            } else {
                ui_state.hex_selection_start = None;
            }
            return WidgetResult::Handled;
        }

        WidgetResult::Ignored
    }

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
            .title(match ui_state.hexdump_view_mode {
                HexdumpViewMode::PETSCIIUnshifted => " Hex Dump (PETSCII Unshifted) ",
                HexdumpViewMode::PETSCIIShifted => " Hex Dump (PETSCII Shifted) ",
                HexdumpViewMode::ScreencodeUnshifted => " Hex Dump (Screencode Unshifted) ",
                HexdumpViewMode::ScreencodeShifted => " Hex Dump (Screencode Shifted) ",
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
                        let char_to_render = match ui_state.hexdump_view_mode {
                            HexdumpViewMode::PETSCIIShifted => {
                                crate::utils::petscii_to_unicode(b, true)
                            }
                            HexdumpViewMode::PETSCIIUnshifted => {
                                crate::utils::petscii_to_unicode(b, false)
                            }
                            HexdumpViewMode::ScreencodeShifted => {
                                let petscii = crate::utils::screencode_to_petscii(b);
                                crate::utils::petscii_to_unicode(petscii, true)
                            }
                            HexdumpViewMode::ScreencodeUnshifted => {
                                let petscii = crate::utils::screencode_to_petscii(b);
                                crate::utils::petscii_to_unicode(petscii, false)
                            }
                        };
                        ascii_part.push(char_to_render);
                    } else {
                        // Padding
                        hex_part.push_str("   ");
                        ascii_part.push(' ');
                    }

                    if j == 7 {
                        hex_part.push(' '); // Extra space after 8 bytes
                    }
                }

                let is_selected = if let Some(sel_start) = ui_state.hex_selection_start {
                    let (start, end) = if sel_start < ui_state.hex_cursor_index {
                        (sel_start, ui_state.hex_cursor_index)
                    } else {
                        (ui_state.hex_cursor_index, sel_start)
                    };
                    row_index >= start && row_index <= end
                } else {
                    row_index == ui_state.hex_cursor_index
                };

                let style = if is_selected {
                    Style::default().bg(ui_state.theme.selection_bg)
                } else {
                    Style::default()
                };

                let row_end_addr = row_start_addr + bytes_per_row;
                // Determine the intersection of the current row and the valid data range
                let intersect_start = row_start_addr.max(origin);
                let intersect_end = row_end_addr.min(origin + app_state.raw_data.len());

                let entropy_val = if intersect_start < intersect_end {
                    let start_idx = intersect_start - origin;
                    let end_idx = intersect_end - origin;
                    crate::utils::calculate_entropy(&app_state.raw_data[start_idx..end_idx])
                } else {
                    0.0
                };

                let (entropy_char, entropy_color) = if entropy_val < 2.0 {
                    (' ', ui_state.theme.comment)
                } else if entropy_val < 4.0 {
                    ('░', ui_state.theme.mnemonic)
                } else if entropy_val < 6.0 {
                    ('▒', ui_state.theme.label)
                } else if entropy_val < 7.5 {
                    ('▓', ui_state.theme.sprite_multicolor_1)
                } else {
                    ('█', ui_state.theme.error_fg)
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
                    Span::styled(" ", Style::default()),
                    Span::styled(entropy_char.to_string(), Style::default().fg(entropy_color)),
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
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            // Escape cancels visual mode / selection
            KeyCode::Esc => {
                if ui_state.hex_selection_start.is_some() || ui_state.is_visual_mode {
                    ui_state.hex_selection_start = None;
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
                        ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.hex_selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            // Shift+Down for selection
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.hex_selection_start;
                if saved_selection.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                }
                let selection_to_keep = ui_state.hex_selection_start;
                // Move cursor (this may clear selection if not in visual mode)
                let total = self.len(app_state);
                if total > 0 {
                    ui_state.hex_cursor_index =
                        (ui_state.hex_cursor_index + 1).min(total.saturating_sub(1));
                }
                // Restore selection for shift+arrow mode
                ui_state.hex_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Up for selection
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.hex_selection_start;
                if saved_selection.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                }
                let selection_to_keep = ui_state.hex_selection_start;
                // Move cursor
                ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(1);
                // Restore selection for shift+arrow mode
                ui_state.hex_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::HexdumpViewModeNext)
            }
            KeyCode::Char('M') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(MenuAction::HexdumpViewModePrev)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                // Convert selected rows or current row to bytes block (16 bytes per row)
                let origin = app_state.origin as usize;
                let bytes_per_row = 16;
                let alignment_padding = origin % bytes_per_row;
                let aligned_origin = origin - alignment_padding;

                // Determine row range based on selection
                let (start_row, end_row) = if let Some(sel_start) = ui_state.hex_selection_start {
                    if sel_start < ui_state.hex_cursor_index {
                        (sel_start, ui_state.hex_cursor_index)
                    } else {
                        (ui_state.hex_cursor_index, sel_start)
                    }
                } else {
                    (ui_state.hex_cursor_index, ui_state.hex_cursor_index)
                };

                let row_start_addr = aligned_origin + (start_row * bytes_per_row);
                let row_end_addr = aligned_origin + ((end_row + 1) * bytes_per_row) - 1;

                // Calculate the byte offset range within raw_data
                let start_offset = row_start_addr.saturating_sub(origin);
                let end_offset = (row_end_addr.saturating_sub(origin))
                    .min(app_state.raw_data.len().saturating_sub(1));

                // Clear selection after action
                ui_state.hex_selection_start = None;
                ui_state.is_visual_mode = false;

                if start_offset < app_state.raw_data.len() {
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
                let alignment_padding = origin % 16;
                let aligned_origin = origin - alignment_padding;
                // Hex cursor index is row index
                let row_addr = aligned_origin + ui_state.hex_cursor_index * 16;

                // If this row contains the origin, jump to origin instead of the aligned boundary
                let target_addr = if origin >= row_addr && origin < row_addr + 16 {
                    origin as u16
                } else {
                    row_addr as u16
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
