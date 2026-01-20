use crate::state::{AppState, HexdumpViewMode};
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
        let total = self.len(app_state);
        if total == 0 {
            return;
        }
        ui_state.hex_cursor_index =
            (ui_state.hex_cursor_index + amount).min(total.saturating_sub(1));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
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

                let is_selected = if let Some(selection_start) = ui_state.selection_start {
                    let (start, end) = if selection_start < ui_state.cursor_index {
                        (selection_start, ui_state.cursor_index)
                    } else {
                        (ui_state.cursor_index, selection_start)
                    };
                    // Note: selection logic here seemingly refers to `ui_state.cursor_index`?
                    // But HexDump uses `hex_cursor_index`.
                    // The original code used `ui_state.cursor_index` in loop?
                    // Wait, let's check original code.
                    // "let (start, end) = if selection_start < ui_state.cursor_index ..."
                    // This seems to link HexDump selection to Disassembly cursor? That sounds wrong or I misread.
                    // In `view_hexdump.rs` line 108: `ui_state.cursor_index`.
                    // BUT render uses `ui_state.hex_cursor_index` for current row style.
                    // Using `selection_start < ui_state.cursor_index` looks like a bug copy-pasted from disassembly,
                    // OR hex view selection interacts with disassembly cursor?
                    // Given I am refactoring input, I should probably leave render logic alone unless it's clearly broken.
                    // However, `selection_start` is usually for disassembly.
                    // Hexdump doesn't seem to have its own selection start in UIState?
                    // Check UIState later.

                    // Actually, let's keep it as is to minimize regression risk, but this looks suspicious.
                    // Original code: `selection_start < ui_state.cursor_index`
                    // But `row_index` is checked against `start` and `end`.
                    // If `ui_state.cursor_index` (disasm) is used to define range for HexDump, that implies they are synced?
                    // `ui_state.hex_cursor_index` is used for styling the current row.

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
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::HexdumpViewModeNext)
            }
            KeyCode::Char('M') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(MenuAction::HexdumpViewModePrev)
            }
            _ => WidgetResult::Ignored,
        }
    }
}
