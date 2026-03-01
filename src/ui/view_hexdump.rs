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
use crate::ui::view_disassembly::DisassemblyView;

/// Immediately mirror the hex cursor / selection into the disassembly view's
/// cursor_index / selection_start.  Called eagerly from every input handler so
/// both views are consistent within the same render frame.
fn sync_hex_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    if app_state.raw_data.is_empty() || app_state.disassembly.is_empty() {
        return;
    }
    let origin = app_state.origin as usize;
    let bytes_per_row = 16usize;
    let aligned_origin = origin - origin % bytes_per_row;

    let col = ui_state.hex_col_cursor.min(bytes_per_row - 1);
    let cursor_byte_addr = (aligned_origin + ui_state.hex_cursor_index * bytes_per_row + col)
        .min(origin + app_state.raw_data.len().saturating_sub(1))
        .max(origin) as u16;

    let row_start_addr =
        (aligned_origin + ui_state.hex_cursor_index * bytes_per_row).max(origin) as u16;

    // Find the instruction containing this specific byte.  If that instruction
    // starts *before* the current hex row (i.e. a multi-byte instruction whose
    // first byte is in a previous row), snap forward to the first instruction
    // that starts at or after the row boundary instead.  This prevents the
    // disassembly cursor from jumping backwards when the user presses Down.
    let inst_idx = app_state
        .get_line_index_containing_address(cursor_byte_addr)
        .filter(|&idx| app_state.disassembly[idx].address >= row_start_addr)
        .or_else(|| app_state.get_line_index_for_address(row_start_addr));

    if let Some(inst_idx) = inst_idx {
        ui_state.cursor_index = inst_idx;
        // Point sub_cursor at the opcode line, which comes after any labels and
        // line-comments that occupy lower sub-indices.
        let counts =
            DisassemblyView::get_visual_line_counts(&app_state.disassembly[inst_idx], app_state);
        ui_state.sub_cursor_index = counts.labels + counts.comments;

        ui_state.selection_start = ui_state.hex_selection_start.and_then(|sel_row| {
            let anchor_addr = (aligned_origin
                + sel_row * bytes_per_row
                + ui_state.hex_selection_start_col.min(bytes_per_row - 1))
            .min(origin + app_state.raw_data.len().saturating_sub(1))
                as u16;
            app_state.get_line_index_containing_address(anchor_addr)
        });
    }
}

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
                ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
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
                ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
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
        // Use the scroll offset stored by the last render so clicks match what's on screen
        let offset = ui_state.hex_scroll_index;

        let row_index = offset + click_row;
        let total_rows = self.len(app_state);

        if row_index < total_rows {
            ui_state.hex_cursor_index = row_index;

            // Determine which byte column was clicked.
            // Layout per row (starting at inner_area.x):
            //   [0–6]   address "$XXXX  " (7 chars)
            //   [7–55]  hex bytes (49 chars = 16×3 + 1 gap)
            //             bytes 0–7 : positions 0–23 within hex area
            //             gap       : position 24
            //             bytes 8–15: positions 25–48
            //   [56–57] separator "| " (2 chars)
            //   [58–73] ASCII chars (16 chars, one per byte)
            let click_col = (mouse.column as usize).saturating_sub(inner_area.x as usize);
            let byte_col = if (7..56).contains(&click_col) {
                let hex_rel = click_col - 7;
                if hex_rel < 24 {
                    hex_rel / 3 // bytes 0–7
                } else if hex_rel == 24 {
                    7 // gap between halves → snap to byte 7
                } else {
                    ((hex_rel - 25) / 3 + 8).min(15) // bytes 8–15
                }
            } else if (58..74).contains(&click_col) {
                click_col - 58 // ASCII area: direct column index
            } else {
                ui_state.hex_col_cursor // outside hex/ascii area → no change
            };
            ui_state.hex_col_cursor = byte_col;

            if ui_state.is_visual_mode {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                    ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                }
            } else {
                ui_state.hex_selection_start = None;
            }
            sync_hex_to_disassembly(app_state, ui_state);
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

        let is_hex_active = ui_state.active_pane == ActivePane::HexDump;

        // --- Cross-pane Sync ---
        if is_hex_active {
            // Hex is active: eagerly push hex cursor into the disassembly cursor so
            // the disassembly view is up to date regardless of render order.
            sync_hex_to_disassembly(app_state, ui_state);
        } else if total_rows > 0 {
            // Hex is passive: follow the disassembly cursor for scrolling.
            if let Some(dline) = app_state.disassembly.get(ui_state.cursor_index) {
                let addr = dline.address as usize;
                if addr >= aligned_origin {
                    let row = (addr - aligned_origin) / bytes_per_row;
                    ui_state.hex_cursor_index = row.min(total_rows.saturating_sub(1));
                    ui_state.hex_col_cursor = (addr - aligned_origin) % bytes_per_row;
                }
            }
        }

        // --- Scrolloff Logic Start ---
        let margin = 5.min(visible_height / 3);
        let mut offset = ui_state.hex_scroll_index;

        // Constraint 1: Cursor must be visible (at least margin from top)
        if ui_state.hex_cursor_index < offset + margin {
            offset = ui_state.hex_cursor_index.saturating_sub(margin);
        }

        // Constraint 2: Cursor must be visible (at least margin from bottom)
        if ui_state.hex_cursor_index >= offset + visible_height.saturating_sub(margin) {
            offset = ui_state
                .hex_cursor_index
                .saturating_sub(visible_height.saturating_sub(margin).saturating_sub(1));
        }

        // Final bounds check
        let max_offset = total_rows.saturating_sub(visible_height);
        offset = offset.min(max_offset);
        // --- Scrolloff Logic End ---

        // Compute the highlight address range for per-byte colouring.
        //
        // When the hex pane is active the range is:
        //   • no selection → single byte at (hex_cursor_index, hex_col_cursor)
        //   • row selection (visual mode) → all bytes covered by the selected rows
        // When another pane is active the range mirrors the bytes of the current
        // disassembly instruction so the user can see exactly which bytes it uses.
        let (highlight_start_addr, highlight_end_addr): (usize, usize) = if is_hex_active {
            if let Some(sel_start_row) = ui_state.hex_selection_start {
                // Byte-level selection: anchor at (sel_start_row, sel_start_col),
                // moving end at (hex_cursor_index, hex_col_cursor).
                let anchor_addr = aligned_origin
                    + sel_start_row * bytes_per_row
                    + ui_state.hex_selection_start_col.min(bytes_per_row - 1);
                let cursor_addr = aligned_origin
                    + ui_state.hex_cursor_index * bytes_per_row
                    + ui_state.hex_col_cursor.min(bytes_per_row - 1);
                if anchor_addr <= cursor_addr {
                    (anchor_addr, cursor_addr)
                } else {
                    (cursor_addr, anchor_addr)
                }
            } else {
                let col = ui_state.hex_col_cursor.min(bytes_per_row - 1);
                let addr = aligned_origin + ui_state.hex_cursor_index * bytes_per_row + col;
                (addr, addr)
            }
        } else if let Some(dline) = app_state.disassembly.get(ui_state.cursor_index) {
            // If there is an active selection in the disassembly view, highlight all
            // bytes covered by the selected instruction range rather than just the
            // instruction under the cursor.
            if let Some(sel_start) = ui_state.selection_start {
                let (s_idx, e_idx) = if sel_start <= ui_state.cursor_index {
                    (sel_start, ui_state.cursor_index)
                } else {
                    (ui_state.cursor_index, sel_start)
                };
                let start_addr = app_state
                    .disassembly
                    .get(s_idx)
                    .map(|l| l.address as usize)
                    .unwrap_or(dline.address as usize);
                let end_addr = app_state
                    .disassembly
                    .get(e_idx)
                    .map(|l| l.address as usize + l.bytes.len().saturating_sub(1))
                    .unwrap_or(dline.address as usize);
                (start_addr, end_addr)
            } else {
                let addr = dline.address as usize;
                let end = addr + dline.bytes.len().saturating_sub(1);
                (addr, end)
            }
        } else {
            (usize::MAX, 0) // nothing to highlight
        };

        let items: Vec<ListItem> = (0..visible_height)
            .map(|i| {
                let row_index = offset + i;
                if row_index >= total_rows {
                    return ListItem::new("");
                }

                let row_start_addr = aligned_origin + (row_index * bytes_per_row);

                // Build row as individual per-byte spans so we can highlight
                // a specific byte (or range) without colouring the whole row.
                let mut spans: Vec<Span> = Vec::with_capacity(2 + bytes_per_row * 2 + 4);

                spans.push(Span::styled(
                    format!("${:04X}  ", row_start_addr),
                    Style::default().fg(ui_state.theme.address),
                ));

                let mut ascii_spans: Vec<Span> = Vec::with_capacity(bytes_per_row);

                for j in 0..bytes_per_row {
                    let current_addr = row_start_addr + j;
                    let is_highlighted =
                        current_addr >= highlight_start_addr && current_addr <= highlight_end_addr;

                    if current_addr >= origin && current_addr < origin + app_state.raw_data.len() {
                        let data_idx = current_addr - origin;
                        let b = app_state.raw_data[data_idx];

                        let hex_style = if is_highlighted {
                            Style::default()
                                .bg(ui_state.theme.selection_bg)
                                .fg(ui_state.theme.hex_bytes)
                        } else {
                            Style::default().fg(ui_state.theme.hex_bytes)
                        };
                        spans.push(Span::styled(format!("{:02X} ", b), hex_style));

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
                        let ascii_style = if is_highlighted {
                            Style::default()
                                .bg(ui_state.theme.selection_bg)
                                .fg(ui_state.theme.hex_ascii)
                        } else {
                            Style::default().fg(ui_state.theme.hex_ascii)
                        };
                        ascii_spans.push(Span::styled(char_to_render.to_string(), ascii_style));
                    } else {
                        // Padding (before origin or after end of data)
                        spans.push(Span::styled(
                            "   ",
                            Style::default().fg(ui_state.theme.hex_bytes),
                        ));
                        ascii_spans.push(Span::styled(
                            " ",
                            Style::default().fg(ui_state.theme.hex_ascii),
                        ));
                    }

                    if j == 7 {
                        // Extra separator space between the two 8-byte halves
                        spans.push(Span::styled(
                            " ",
                            Style::default().fg(ui_state.theme.hex_bytes),
                        ));
                    }
                }

                // ASCII column: separator + per-byte chars
                spans.push(Span::styled(
                    "| ",
                    Style::default().fg(ui_state.theme.hex_ascii),
                ));
                spans.extend(ascii_spans);

                // Calculate entropy using a larger window (512 bytes before + 512 bytes after)
                // providing a more reliable value than just the current 16 bytes.
                let window_size = 512;
                let entropy_start_addr = row_start_addr.saturating_sub(window_size);
                let entropy_end_addr = row_start_addr + window_size;

                let effective_start = entropy_start_addr.max(origin);
                let effective_end = entropy_end_addr.min(origin + app_state.raw_data.len());

                let entropy_val = if effective_start < effective_end {
                    let start_idx = effective_start - origin;
                    let end_idx = effective_end - origin;
                    crate::utils::calculate_entropy(&app_state.raw_data[start_idx..end_idx])
                } else {
                    0.0
                };

                let entropy_char = if entropy_val < 2.0 {
                    ' '
                } else if entropy_val < 4.0 {
                    '░'
                } else if entropy_val < 6.0 {
                    '▒'
                } else if entropy_val < 7.5 {
                    '▓'
                } else {
                    '█'
                };

                spans.push(Span::styled(" ", Style::default()));
                spans.push(Span::styled(
                    entropy_char.to_string(),
                    Style::default().fg(ui_state.theme.hex_ascii),
                ));

                ListItem::new(Line::from(spans))
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
            sync_hex_to_disassembly(app_state, ui_state);
            return WidgetResult::Handled;
        }

        let result = match key.code {
            // Byte-level column navigation (Left/Right / vim h/l)
            KeyCode::Left if key.modifiers.is_empty() => {
                if ui_state.hex_col_cursor > 0 {
                    ui_state.hex_col_cursor -= 1;
                } else if ui_state.hex_cursor_index > 0 {
                    ui_state.hex_cursor_index -= 1;
                    ui_state.hex_col_cursor = 15;
                }
                WidgetResult::Handled
            }
            KeyCode::Right if key.modifiers.is_empty() => {
                let total = self.len(app_state);
                if ui_state.hex_col_cursor < 15 {
                    ui_state.hex_col_cursor += 1;
                } else if ui_state.hex_cursor_index + 1 < total {
                    ui_state.hex_cursor_index += 1;
                    ui_state.hex_col_cursor = 0;
                }
                WidgetResult::Handled
            }
            KeyCode::Char('h') if key.modifiers.is_empty() => {
                if ui_state.hex_col_cursor > 0 {
                    ui_state.hex_col_cursor -= 1;
                } else if ui_state.hex_cursor_index > 0 {
                    ui_state.hex_cursor_index -= 1;
                    ui_state.hex_col_cursor = 15;
                }
                WidgetResult::Handled
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                let total = self.len(app_state);
                if ui_state.hex_col_cursor < 15 {
                    ui_state.hex_col_cursor += 1;
                } else if ui_state.hex_cursor_index + 1 < total {
                    ui_state.hex_cursor_index += 1;
                    ui_state.hex_col_cursor = 0;
                }
                WidgetResult::Handled
            }
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
                        ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.hex_selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            // Shift+Left/Right: extend selection by one byte at a time
            KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                    ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                }
                if ui_state.hex_col_cursor > 0 {
                    ui_state.hex_col_cursor -= 1;
                } else if ui_state.hex_cursor_index > 0 {
                    ui_state.hex_cursor_index -= 1;
                    ui_state.hex_col_cursor = 15;
                }
                WidgetResult::Handled
            }
            KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                    ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                }
                let total = self.len(app_state);
                if ui_state.hex_col_cursor < 15 {
                    ui_state.hex_col_cursor += 1;
                } else if ui_state.hex_cursor_index + 1 < total {
                    ui_state.hex_cursor_index += 1;
                    ui_state.hex_col_cursor = 0;
                }
                WidgetResult::Handled
            }
            // Shift+Down for selection (row at a time, preserves column)
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                    ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                }
                let selection_to_keep = ui_state.hex_selection_start;
                let total = self.len(app_state);
                if total > 0 {
                    ui_state.hex_cursor_index =
                        (ui_state.hex_cursor_index + 1).min(total.saturating_sub(1));
                }
                ui_state.hex_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Up for selection (row at a time, preserves column)
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                if ui_state.hex_selection_start.is_none() {
                    ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                    ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                }
                let selection_to_keep = ui_state.hex_selection_start;
                ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(1);
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
                // Convert selected bytes (or current row if no selection) to a bytes block.
                let origin = app_state.origin as usize;
                let bytes_per_row = 16;
                let alignment_padding = origin % bytes_per_row;
                let aligned_origin = origin - alignment_padding;

                let (start_offset, end_offset) =
                    if let Some(sel_start_row) = ui_state.hex_selection_start {
                        // Byte-level selection: use anchor and cursor byte addresses.
                        let anchor_addr = aligned_origin
                            + sel_start_row * bytes_per_row
                            + ui_state.hex_selection_start_col.min(bytes_per_row - 1);
                        let cursor_addr = aligned_origin
                            + ui_state.hex_cursor_index * bytes_per_row
                            + ui_state.hex_col_cursor.min(bytes_per_row - 1);
                        let (s_addr, e_addr) = if anchor_addr <= cursor_addr {
                            (anchor_addr, cursor_addr)
                        } else {
                            (cursor_addr, anchor_addr)
                        };
                        (
                            s_addr.saturating_sub(origin),
                            e_addr
                                .saturating_sub(origin)
                                .min(app_state.raw_data.len().saturating_sub(1)),
                        )
                    } else {
                        // No selection: convert only the single byte under the cursor.
                        let cursor_addr = aligned_origin
                            + ui_state.hex_cursor_index * bytes_per_row
                            + ui_state.hex_col_cursor.min(bytes_per_row - 1);
                        let off = cursor_addr
                            .saturating_sub(origin)
                            .min(app_state.raw_data.len().saturating_sub(1));
                        (off, off)
                    };

                // Clear selection after action (keep disassembly in sync)
                ui_state.hex_selection_start = None;
                ui_state.is_visual_mode = false;
                ui_state.selection_start = None;

                // Move hex cursor to the end of the newly-created bytes block so
                // that sync_hex_to_disassembly agrees with the cursor re-anchor done
                // in the menu handler (otherwise sync would pull cursor_index back to
                // the wrong position on the next render).
                let end_abs_addr = origin + end_offset;
                ui_state.hex_cursor_index = (end_abs_addr - aligned_origin) / bytes_per_row;
                ui_state.hex_col_cursor = (end_abs_addr - aligned_origin) % bytes_per_row;

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
                let row_addr = aligned_origin + ui_state.hex_cursor_index * 16;
                // Jump to the specific byte under the column cursor, clamped to valid data
                let cursor_addr = row_addr + ui_state.hex_col_cursor.min(15);
                let target_addr = cursor_addr
                    .max(origin)
                    .min(origin + app_state.raw_data.len().saturating_sub(1))
                    as u16;

                crate::ui::navigable::jump_to_disassembly_at_address(
                    app_state,
                    ui_state,
                    target_addr,
                )
            }
            _ => WidgetResult::Ignored,
        };

        if matches!(result, WidgetResult::Handled) {
            sync_hex_to_disassembly(app_state, ui_state);
        }
        result
    }
}
