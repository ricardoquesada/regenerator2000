use crate::state::{AppState, HexdumpViewMode};
use crate::ui_state::{ActivePane, AppAction, RightPane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::navigable::{
    AlignedGridRange, Navigable, compute_scroll_offset, handle_nav_input,
    is_mouse_selection_extend, jump_to_disassembly_at_address, right_pane_mouse_inner_area,
    sync_pane_to_disassembly,
};
use crate::ui::widget::{Widget, WidgetResult};

/// Returns the number of bytes per hex-dump row based on the current right-pane
/// mode: 16 for `HexDump16`, 8 for `HexDump8`, and 16 as a fallback.
#[must_use]
fn hex_bytes_per_row(ui_state: &UIState) -> usize {
    match ui_state.right_pane {
        RightPane::HexDump8 => 8,
        _ => 16,
    }
}

/// Immediately mirror the hex cursor / selection into the disassembly view's
/// `cursor_index` / `selection_start`.  Called eagerly from every input handler so
/// both views are consistent within the same render frame.
fn sync_hex_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    let grid = HexDumpView::grid_range(app_state, ui_state);
    let max_col = grid.stride - 1;
    let row_addr = grid.item_addr(ui_state.hex_cursor_index);
    let cursor_addr = row_addr + ui_state.hex_col_cursor.min(max_col);
    let selection_addr = ui_state
        .hex_selection_start
        .map(|sel_row| grid.item_addr(sel_row) + ui_state.hex_selection_start_col.min(max_col));
    sync_pane_to_disassembly(
        app_state,
        ui_state,
        &grid,
        cursor_addr,
        selection_addr,
        Some(row_addr),
    );
}

pub struct HexDumpView;

impl HexDumpView {
    #[must_use]
    fn grid_range(app_state: &AppState, ui_state: &UIState) -> AlignedGridRange {
        AlignedGridRange::uniform(app_state, hex_bytes_per_row(ui_state))
    }

    fn prepare_selection(ui_state: &mut UIState, extend: bool) {
        if extend || ui_state.is_visual_mode {
            if ui_state.hex_selection_start.is_none() {
                ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
            }
        } else {
            ui_state.hex_selection_start = None;
        }
    }

    fn step_byte(
        ui_state: &mut UIState,
        total_rows: usize,
        max_col: usize,
        forward: bool,
        extend: bool,
    ) {
        Self::prepare_selection(ui_state, extend);
        if forward {
            if ui_state.hex_col_cursor < max_col {
                ui_state.hex_col_cursor += 1;
            } else if ui_state.hex_cursor_index + 1 < total_rows {
                ui_state.hex_cursor_index += 1;
                ui_state.hex_col_cursor = 0;
            }
        } else if ui_state.hex_col_cursor > 0 {
            ui_state.hex_col_cursor -= 1;
        } else if ui_state.hex_cursor_index > 0 {
            ui_state.hex_cursor_index -= 1;
            ui_state.hex_col_cursor = max_col;
        }
    }
}

impl Navigable for HexDumpView {
    fn len(&self, app_state: &AppState) -> usize {
        AlignedGridRange::uniform(app_state, 16).item_count()
    }

    fn len_with_ui(&self, app_state: &AppState, ui_state: &UIState) -> usize {
        Self::grid_range(app_state, ui_state).item_count()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.hex_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        Self::prepare_selection(ui_state, false);
        let grid = Self::grid_range(app_state, ui_state);
        let total = grid.item_count();
        if total == 0 {
            return;
        }
        ui_state.hex_cursor_index =
            (ui_state.hex_cursor_index + amount).min(total.saturating_sub(1));
        ui_state.hex_col_cursor = ui_state.hex_col_cursor.min(grid.stride - 1);
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        Self::prepare_selection(ui_state, false);
        ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(amount);
        let max_col = hex_bytes_per_row(ui_state) - 1;
        ui_state.hex_col_cursor = ui_state.hex_col_cursor.min(max_col);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = Self::grid_range(app_state, ui_state).item_count();
        ui_state.hex_cursor_index = index.min(total.saturating_sub(1));
    }

    fn item_name(&self) -> &'static str {
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
                sync_hex_to_disassembly(app_state, ui_state);
                return WidgetResult::Handled;
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                sync_hex_to_disassembly(app_state, ui_state);
                return WidgetResult::Handled;
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {}
            _ => return WidgetResult::Ignored,
        }

        let Some(inner_area) = right_pane_mouse_inner_area(ui_state, &mouse) else {
            return WidgetResult::Ignored;
        };

        let click_row = (mouse.row - inner_area.y) as usize;
        let row_index = ui_state.hex_scroll_index + click_row;
        let grid = Self::grid_range(app_state, ui_state);
        let total_rows = grid.item_count();

        if row_index < total_rows {
            let bytes_per_row = grid.stride;
            let max_col = bytes_per_row - 1;
            Self::prepare_selection(
                ui_state,
                is_mouse_selection_extend(&mouse, ui_state.is_visual_mode),
            );
            ui_state.hex_cursor_index = row_index;

            // Determine which byte column was clicked.
            // Layout per row (starting at inner_area.x):
            //   [0–6]   address "$XXXX  " (7 chars)
            //   Hex bytes with 4-byte group separators:
            //     16-col: 4 groups × 12 chars + 3 gaps = 51 chars (positions 7–57)
            //      8-col: 2 groups × 12 chars + 1 gap  = 25 chars (positions 7–31)
            //   Then 2-space separator, then bytes_per_row ASCII chars.
            let num_groups = bytes_per_row / 4;
            let hex_area_end = 7 + num_groups * 12 + num_groups.saturating_sub(1);
            let ascii_area_start = hex_area_end + 2;
            let ascii_area_end = ascii_area_start + bytes_per_row;
            let click_col = (mouse.column as usize).saturating_sub(inner_area.x as usize);
            let byte_col = if click_col >= 7 && click_col < hex_area_end {
                let hex_rel = click_col - 7;
                let group = hex_rel / 13;
                let within_group = hex_rel % 13;
                if within_group == 12 {
                    (group * 4 + 3).min(max_col)
                } else {
                    (group * 4 + within_group / 3).min(max_col)
                }
            } else if click_col >= ascii_area_start && click_col < ascii_area_end {
                click_col - ascii_area_start
            } else {
                ui_state.hex_col_cursor
            };
            ui_state.hex_col_cursor = byte_col.min(max_col);

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
        let grid = Self::grid_range(app_state, ui_state);
        let bytes_per_row = grid.stride;
        let max_col = bytes_per_row - 1;
        let origin = grid.origin;
        let aligned_origin = grid.aligned_origin;
        let total_rows = grid.item_count();

        // Clamp column cursor when switching from 16-col to 8-col mode
        ui_state.hex_col_cursor = ui_state.hex_col_cursor.min(max_col);

        let is_hex_active = ui_state.active_pane == ActivePane::HexDump;

        // --- Cross-pane Sync ---
        if is_hex_active {
            sync_hex_to_disassembly(app_state, ui_state);
        } else if total_rows > 0
            && app_state.system_config.sync_hex_dump
            && let Some(dline) = app_state.disassembly.get(ui_state.cursor_index)
            && dline.external_label_address.is_none()
        {
            let addr = dline.address.0 as usize;
            if addr >= aligned_origin {
                let row = (addr - aligned_origin) / bytes_per_row;
                ui_state.hex_cursor_index = row.min(total_rows.saturating_sub(1));
                ui_state.hex_col_cursor = (addr - aligned_origin) % bytes_per_row;
            }
        }

        let offset = compute_scroll_offset(
            ui_state.hex_scroll_index,
            ui_state.hex_cursor_index,
            visible_height,
            total_rows,
            5,
        );

        let (highlight_start_addr, highlight_end_addr): (usize, usize) = if is_hex_active {
            let cursor_addr =
                grid.item_addr(ui_state.hex_cursor_index) + ui_state.hex_col_cursor.min(max_col);
            if let Some(sel_start_row) = ui_state.hex_selection_start {
                let anchor_addr =
                    grid.item_addr(sel_start_row) + ui_state.hex_selection_start_col.min(max_col);
                if anchor_addr <= cursor_addr {
                    (anchor_addr, cursor_addr)
                } else {
                    (cursor_addr, anchor_addr)
                }
            } else {
                (cursor_addr, cursor_addr)
            }
        } else if let Some(dline) = app_state.disassembly.get(ui_state.cursor_index) {
            if let Some(sel_start) = ui_state.selection_start {
                let (s_idx, e_idx) = if sel_start <= ui_state.cursor_index {
                    (sel_start, ui_state.cursor_index)
                } else {
                    (ui_state.cursor_index, sel_start)
                };
                let start_addr = app_state
                    .disassembly
                    .get(s_idx)
                    .map_or(dline.address.0 as usize, |l| l.address.0 as usize);
                let end_addr = app_state
                    .disassembly
                    .get(e_idx)
                    .map_or(dline.address.0 as usize, |l| {
                        l.address.0 as usize + l.bytes.len().saturating_sub(1)
                    });
                (start_addr, end_addr)
            } else {
                let addr = dline.address.0 as usize;
                let end = addr + dline.bytes.len().saturating_sub(1);
                (addr, end)
            }
        } else {
            (usize::MAX, 0)
        };

        let items: Vec<ListItem> = (0..visible_height)
            .map(|i| {
                let row_index = offset + i;
                if row_index >= total_rows {
                    return ListItem::new("");
                }

                let row_start_addr = grid.item_addr(row_index);
                let mut spans: Vec<Span> = Vec::with_capacity(2 + bytes_per_row * 2 + 4);

                spans.push(Span::styled(
                    format!("${row_start_addr:04X}  "),
                    Style::default().fg(ui_state.theme.address),
                ));

                let mut ascii_spans: Vec<Span> = Vec::with_capacity(bytes_per_row);

                for j in 0..bytes_per_row {
                    let current_addr = row_start_addr + j;
                    let is_highlighted =
                        current_addr >= highlight_start_addr && current_addr <= highlight_end_addr;

                    if current_addr >= origin && current_addr < grid.end_address {
                        let b = app_state.raw_data[current_addr - origin];

                        let palette_idx = if b == 0x00 {
                            0
                        } else if b == 0xFF {
                            17
                        } else {
                            (b >> 4) as usize + 1
                        };
                        let palette_color = ui_state.theme.hex_color_palette[palette_idx];

                        let byte_style = if is_highlighted {
                            Style::default()
                                .bg(ui_state.theme.selection_bg)
                                .fg(palette_color)
                        } else {
                            Style::default().fg(palette_color)
                        };
                        spans.push(Span::styled(format!("{b:02X} "), byte_style));

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
                        ascii_spans.push(Span::styled(char_to_render.to_string(), byte_style));
                    } else {
                        spans.push(Span::styled(
                            "   ",
                            Style::default().fg(ui_state.theme.hex_bytes),
                        ));
                        ascii_spans.push(Span::styled(
                            " ",
                            Style::default().fg(ui_state.theme.hex_ascii),
                        ));
                    }

                    if j % 4 == 3 && j + 1 < bytes_per_row {
                        spans.push(Span::styled(
                            " ",
                            Style::default().fg(ui_state.theme.hex_bytes),
                        ));
                    }
                }

                spans.push(Span::styled(
                    " ",
                    Style::default().fg(ui_state.theme.hex_bytes),
                ));
                spans.extend(ascii_spans);

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).block(block);
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

        let grid = Self::grid_range(app_state, ui_state);
        let bytes_per_row = grid.stride;
        let max_col = bytes_per_row - 1;
        let total = grid.item_count();

        let result = match key.code {
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                ui_state.input_buffer.clear();
                Self::step_byte(ui_state, total, max_col, false, false);
                WidgetResult::Handled
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                ui_state.input_buffer.clear();
                Self::step_byte(ui_state, total, max_col, true, false);
                WidgetResult::Handled
            }
            KeyCode::Esc if key.modifiers.is_empty() => {
                if ui_state.hex_selection_start.is_some() || ui_state.is_visual_mode {
                    ui_state.hex_selection_start = None;
                    ui_state.is_visual_mode = false;
                    ui_state.set_status_message("");
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Char('V')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
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
            KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => {
                Self::step_byte(ui_state, total, max_col, false, true);
                WidgetResult::Handled
            }
            KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => {
                Self::step_byte(ui_state, total, max_col, true, true);
                WidgetResult::Handled
            }
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                Self::prepare_selection(ui_state, true);
                if total > 0 {
                    ui_state.hex_cursor_index =
                        (ui_state.hex_cursor_index + 1).min(total.saturating_sub(1));
                }
                WidgetResult::Handled
            }
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                Self::prepare_selection(ui_state, true);
                ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(1);
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::HexdumpViewModeNext)
            }
            KeyCode::Char('M')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::HexdumpViewModePrev)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                let cursor_addr = grid.item_addr(ui_state.hex_cursor_index)
                    + ui_state.hex_col_cursor.min(max_col);
                let (start_addr, end_addr) =
                    if let Some(sel_start_row) = ui_state.hex_selection_start {
                        let anchor_addr = grid.item_addr(sel_start_row)
                            + ui_state.hex_selection_start_col.min(max_col);
                        if anchor_addr <= cursor_addr {
                            (anchor_addr, cursor_addr)
                        } else {
                            (cursor_addr, anchor_addr)
                        }
                    } else {
                        (cursor_addr, cursor_addr)
                    };

                ui_state.hex_selection_start = None;
                ui_state.is_visual_mode = false;
                ui_state.selection_start = None;

                let Some((start, end)) = grid.clamped_byte_offsets(start_addr, end_addr) else {
                    return WidgetResult::Ignored;
                };

                // Move the hex cursor to the end of the new bytes block.
                let end_abs_addr = grid.origin + end;
                ui_state.hex_cursor_index = (end_abs_addr - grid.aligned_origin) / bytes_per_row;
                ui_state.hex_col_cursor = (end_abs_addr - grid.aligned_origin) % bytes_per_row;

                WidgetResult::Action(AppAction::SetBytesBlockByOffset { start, end })
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                let cursor_addr = grid.item_addr(ui_state.hex_cursor_index)
                    + ui_state.hex_col_cursor.min(max_col);
                grid.clamped_addr(cursor_addr)
                    .map_or(WidgetResult::Ignored, |addr| {
                        jump_to_disassembly_at_address(app_state, ui_state, addr)
                    })
            }
            _ => WidgetResult::Ignored,
        };

        if matches!(result, WidgetResult::Handled) {
            sync_hex_to_disassembly(app_state, ui_state);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{HexDumpView, sync_hex_to_disassembly};
    use crate::ui::widget::{Widget, WidgetResult};
    use crate::ui_state::RightPane;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use regenerator2000_core::{
        state::{AppState, BlockType, LabelKind, LabelType},
        view_state::CoreViewState,
    };

    /// Regression test for: hex-dump right-arrow at the last column of a row
    /// that sits inside a large multi-byte block (e.g. `.fill`) causes the
    /// disassembly cursor to jump to the "external labels" section when
    /// "Display External Labels at top" (`settings.all_labels`) is enabled.
    ///
    /// Root cause: `sync_hex_to_disassembly` filtered out the containing block
    /// (it starts before the current hex row) then fell back to
    /// `get_line_index_for_address`, whose old "first address >= target" pass
    /// found an external-label definition line (prepended at the top of the
    /// `disassembly` array) with a large address, before the real content line.
    #[test]
    fn test_sync_hex_to_disassembly_fill_block_row_boundary_with_all_labels() {
        use regenerator2000_core::state::project::Label;

        // --- Build AppState ---
        // Origin $3A00; 48 bytes of identical data → emitted as one .fill block.
        let mut app_state = AppState::new();
        app_state.origin = regenerator2000_core::state::Addr(0x3A00);
        app_state.raw_data = vec![0x98u8; 48];
        app_state.block_types = vec![BlockType::DataByte; 48];

        // An external label at $989A — a high address that appears before the
        // main disassembly body in the array when all_labels = true, and whose
        // address ($989A) satisfies `address >= $3A10`, making it the old buggy
        // winner of the "first address >= target" third pass.
        app_state
            .labels
            .entry(regenerator2000_core::state::Addr(0x989A))
            .or_default()
            .push(Label {
                name: "s_external_routine".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::ExternalJump,
            });
        app_state.settings.all_labels = true;
        app_state.disassemble();

        // --- Build UIState (just the core view fields we need) ---
        // Simulate hex cursor at $3A0F (last byte of the row starting at $3A00).
        // hex_cursor_index = ($3A0F - $3A00) / 16 = 0, hex_col_cursor = 15.
        // After a right-arrow the cursor would be at ($3A10, col 0), i.e. index 1
        // col 0 — but sync_hex_to_disassembly is called with the *post-move*
        // state, so we set it to the state after the move: row 1, col 0 → $3A10.
        let mut core = CoreViewState::new();
        core.hex_cursor_index = 1; // row index 1 = addresses $3A10..$3A1F
        core.hex_col_cursor = 0; // column 0 → byte address $3A10
        core.cursor_index = 999; // sentinel: must be overwritten

        // We need a UIState, but sync_hex_to_disassembly only touches the
        // CoreViewState fields — so we can create a minimal UIState via Default
        // on the TUI wrapper and replace its core.
        //
        // Since UIState::new requires a Theme (a lot of colours), we instead
        // call sync_hex_to_disassembly with a thin stand-in: a UIState whose
        // fields are all defaulted via a helper that calls CoreViewState::new()
        // for the non-TUI parts.  The function only reads/writes core fields.
        //
        // Because this test lives in the same module as sync_hex_to_disassembly
        // we can call it with a real UIState too, but constructing one requires
        // a Theme.  Use Theme::default() if available, otherwise build manually.
        let theme = crate::theme::Theme::default();
        let mut ui_state = crate::ui_state::UIState::new(theme);
        // Overwrite the core view state with our carefully crafted one.
        ui_state.core = core;

        // --- Call the function under test ---
        sync_hex_to_disassembly(&app_state, &mut ui_state);

        // --- Assertions ---
        let landed_idx = ui_state.cursor_index;
        assert_ne!(
            landed_idx, 999,
            "sync_hex_to_disassembly must update cursor_index"
        );

        let landed_line = app_state
            .disassembly
            .get(landed_idx)
            .expect("cursor_index must be a valid disassembly index");

        assert!(
            !landed_line.bytes.is_empty(),
            "Disassembly cursor must land on a content line (has bytes), \
             not on an external-label definition (bytes empty). \
             Landed at address ${:04X} (index {landed_idx})",
            landed_line.address.0,
        );

        assert!(
            landed_line.address.0 <= 0x3A10,
            "Landed line address (${:04X}) must not be above the hex cursor \
             address ($3A10); it should be the fill block at $3A00",
            landed_line.address.0,
        );
    }

    #[test]
    fn test_hexdump8_end_and_g_keys_jump_to_final_8_byte_row() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(regenerator2000_core::state::Addr(0x1000), vec![0xEA; 64])
            .unwrap();
        let mut ui_state = crate::ui_state::UIState::new(crate::theme::Theme::default());
        ui_state.right_pane = RightPane::HexDump8;

        let mut view = HexDumpView;
        let res = view.handle_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Handled);
        // 64 bytes in 8-column mode = 8 rows, so last row is index 7.
        assert_eq!(ui_state.hex_cursor_index, 7);

        // Reset to 0 via Home, then jump to end via 'G' (with SHIFT modifier).
        view.handle_input(
            KeyEvent::new(KeyCode::Home, KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.hex_cursor_index, 0);

        view.handle_input(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.hex_cursor_index, 7);

        // Numeric prefix "6G" in HexDump8 jumps to 1-based row 6 (index 5, which is > 16-col len 4).
        view.handle_input(
            KeyEvent::new(KeyCode::Char('6'), KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        view.handle_input(
            KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.hex_cursor_index, 5);
    }

    #[test]
    fn test_hexdump_left_right_clears_selection_and_b_ignores_alignment_padding() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(regenerator2000_core::state::Addr(0x0801), vec![0xEA; 16])
            .unwrap();
        let mut ui_state = crate::ui_state::UIState::new(crate::theme::Theme::default());
        let mut view = HexDumpView;

        // Cell (row 0, col 0) is $0800, which is alignment padding before origin $0801.
        ui_state.hex_cursor_index = 0;
        ui_state.hex_col_cursor = 0;
        let res_b = view.handle_input(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res_b, WidgetResult::Ignored);

        // Extend a Shift+Right selection, then verify plain Right clears it.
        ui_state.hex_col_cursor = 1;
        let _ = view.handle_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.hex_selection_start, Some(0));

        let _ = view.handle_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.hex_selection_start, None);
    }
}
