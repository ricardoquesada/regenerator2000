//! Right-pane hexadecimal and PETSCII/screencode byte inspector widget.

use crate::state::{AppState, HexdumpViewMode};
use crate::ui::navigable::{
    AlignedGridRange, Navigable, SelectionExtend, VisualMode, compute_scroll_offset,
    handle_nav_input, is_mouse_selection_extend, jump_to_disassembly_at_address,
    right_pane_mouse_inner_area, sync_pane_to_disassembly,
};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::{ActivePane, AppAction, UIState};
use crate::utils::{self, PetsciiCharset};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};
use regenerator2000_core::navigation::disassembly_selection_addr_span;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepDirection {
    Backward,
    Forward,
}

/// Interactive right-pane hex dump and PETSCII/screencode viewer.
pub struct HexDumpView;

impl HexDumpView {
    #[must_use]
    fn grid_range(app_state: &AppState, ui_state: &UIState) -> AlignedGridRange {
        AlignedGridRange::hex_dump(app_state, ui_state.right_pane)
    }

    /// Sets or clears `(hex_selection_start, hex_selection_start_col)` before cursor movement.
    ///
    /// Sets the anchor when `extend` is [`SelectionExtend::Extend`] or `ui_state.is_visual_mode`
    /// is `true` and `total_rows > 0`. Clears both row and column anchors otherwise.
    fn prepare_selection(ui_state: &mut UIState, total_rows: usize, extend: SelectionExtend) {
        let keep_selection = extend == SelectionExtend::Extend || ui_state.is_visual_mode;
        if total_rows == 0 {
            if !keep_selection {
                ui_state.hex_selection_start = None;
                ui_state.hex_selection_start_col = 0;
            }
            return;
        }
        if keep_selection {
            if ui_state.hex_selection_start.is_none() {
                ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
            }
        } else {
            ui_state.hex_selection_start = None;
            ui_state.hex_selection_start_col = 0;
        }
    }

    fn step_byte(
        ui_state: &mut UIState,
        total_rows: usize,
        max_col: usize,
        direction: StepDirection,
        extend: SelectionExtend,
    ) {
        Self::prepare_selection(ui_state, total_rows, extend);
        if total_rows == 0 {
            return;
        }
        match direction {
            StepDirection::Forward => {
                if ui_state.hex_col_cursor < max_col {
                    ui_state.hex_col_cursor += 1;
                } else if ui_state.hex_cursor_index + 1 < total_rows {
                    ui_state.hex_cursor_index += 1;
                    ui_state.hex_col_cursor = 0;
                }
            }
            StepDirection::Backward => {
                if ui_state.hex_col_cursor > 0 {
                    ui_state.hex_col_cursor -= 1;
                } else if ui_state.hex_cursor_index > 0 {
                    ui_state.hex_cursor_index -= 1;
                    ui_state.hex_col_cursor = max_col;
                }
            }
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
        let grid = Self::grid_range(app_state, ui_state);
        let total = grid.item_count();
        Self::prepare_selection(ui_state, total, SelectionExtend::Replace);
        if total == 0 {
            return;
        }
        ui_state.hex_cursor_index =
            (ui_state.hex_cursor_index + amount).min(total.saturating_sub(1));
        ui_state.hex_col_cursor = ui_state.hex_col_cursor.min(grid.stride - 1);
    }

    fn move_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let grid = Self::grid_range(app_state, ui_state);
        let total = grid.item_count();
        Self::prepare_selection(ui_state, total, SelectionExtend::Replace);
        if total == 0 {
            return;
        }
        ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(amount);
        ui_state.hex_col_cursor = ui_state.hex_col_cursor.min(grid.stride - 1);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = Self::grid_range(app_state, ui_state).item_count();
        Self::prepare_selection(ui_state, total, SelectionExtend::Replace);
        if total == 0 {
            return;
        }
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
                total_rows,
                is_mouse_selection_extend(&mouse, VisualMode::from_active(ui_state.is_visual_mode)),
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
        // Sync the hex cursor only when no Disassembly selection is active.
        // This preserves sub-instruction hex anchors across `Tab` switches.
        if is_hex_active {
            sync_hex_to_disassembly(app_state, ui_state);
        } else if total_rows > 0
            && app_state.system_config.sync_hex_dump
            && ui_state.selection_start.is_none()
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
        } else {
            disassembly_selection_addr_span(
                app_state,
                ui_state.selection_start,
                ui_state.cursor_index,
            )
            .map_or((usize::MAX, 0), |span| (span.start, span.end))
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
                                utils::petscii_to_unicode(b, PetsciiCharset::Shifted)
                            }
                            HexdumpViewMode::PETSCIIUnshifted => {
                                utils::petscii_to_unicode(b, PetsciiCharset::Unshifted)
                            }
                            HexdumpViewMode::ScreencodeShifted => {
                                let petscii = utils::screencode_to_petscii(b);
                                utils::petscii_to_unicode(petscii, PetsciiCharset::Shifted)
                            }
                            HexdumpViewMode::ScreencodeUnshifted => {
                                let petscii = utils::screencode_to_petscii(b);
                                utils::petscii_to_unicode(petscii, PetsciiCharset::Unshifted)
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
            if !matches!(key.code, KeyCode::Char(c) if c.is_ascii_digit()) {
                sync_hex_to_disassembly(app_state, ui_state);
            }
            return WidgetResult::Handled;
        }

        let grid = Self::grid_range(app_state, ui_state);
        let bytes_per_row = grid.stride;
        let max_col = bytes_per_row - 1;
        let total = grid.item_count();

        let result = match key.code {
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                ui_state.input_buffer.clear();
                Self::step_byte(
                    ui_state,
                    total,
                    max_col,
                    StepDirection::Backward,
                    SelectionExtend::Replace,
                );
                WidgetResult::Handled
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                ui_state.input_buffer.clear();
                Self::step_byte(
                    ui_state,
                    total,
                    max_col,
                    StepDirection::Forward,
                    SelectionExtend::Replace,
                );
                WidgetResult::Handled
            }
            KeyCode::Esc if key.modifiers.is_empty() => {
                if ui_state.has_any_selection() {
                    // Clear selections in all panes and reset any `[N]G` count prefix.
                    ui_state.core.clear_all_selections();
                    ui_state.input_buffer.clear();
                    ui_state.set_status_message("");
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Char('V')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                if total > 0 {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        ui_state.hex_selection_start = Some(ui_state.hex_cursor_index);
                        ui_state.hex_selection_start_col = ui_state.hex_col_cursor;
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.hex_selection_start = None;
                        ui_state.hex_selection_start_col = 0;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => {
                Self::step_byte(
                    ui_state,
                    total,
                    max_col,
                    StepDirection::Backward,
                    SelectionExtend::Extend,
                );
                WidgetResult::Handled
            }
            KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => {
                Self::step_byte(
                    ui_state,
                    total,
                    max_col,
                    StepDirection::Forward,
                    SelectionExtend::Extend,
                );
                WidgetResult::Handled
            }
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                Self::prepare_selection(ui_state, total, SelectionExtend::Extend);
                if total > 0 {
                    ui_state.hex_cursor_index =
                        (ui_state.hex_cursor_index + 1).min(total.saturating_sub(1));
                }
                WidgetResult::Handled
            }
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                Self::prepare_selection(ui_state, total, SelectionExtend::Extend);
                if total > 0 {
                    ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(1);
                }
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
                // Ignore padding-only bytes before dispatching `AppAction::Byte` so active selections stay intact.
                // Query `grid` directly so widget unit tests do not depend on `ui_state.active_pane`.
                let selection = ui_state
                    .hex_selection_start
                    .map(|row| (row, ui_state.hex_selection_start_col));
                if grid
                    .hex_byte_offset_range(
                        ui_state.hex_cursor_index,
                        ui_state.hex_col_cursor,
                        selection,
                    )
                    .is_none()
                {
                    return WidgetResult::Ignored;
                }
                WidgetResult::Action(AppAction::Byte)
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
    use crate::theme::Theme;
    use crate::ui::widget::{Widget, WidgetResult};
    use crate::ui_state::{ActivePane, RightPane, UIState};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use regenerator2000_core::state::project::Label;
    use regenerator2000_core::state::{Addr, AppState, Assembler, BlockType, LabelKind, LabelType};
    use regenerator2000_core::view_state::CoreViewState;

    /// Regression test for: hex-dump right-arrow at the last column of a row
    /// that sits inside a large multi-byte block (e.g. `.fill`) causes the
    /// disassembly cursor to jump to the "external labels" section when
    /// "Display External Labels at top" (`settings.all_labels`) is enabled.
    ///
    /// Root cause: `sync_hex_to_disassembly` filtered out the containing block
    /// (it starts before the current hex row) then fell back to
    /// `line_index_for_address`, whose old "first address >= target" pass
    /// found an external-label definition line (prepended at the top of the
    /// `disassembly` array) with a large address, before the real content line.
    #[test]
    fn test_sync_hex_to_disassembly_fill_block_row_boundary_with_all_labels() {
        // --- Build AppState ---
        // Origin $3A00; 48 bytes of identical data → emitted as one .fill block.
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x3A00);
        app_state.raw_data = vec![0x98u8; 48];
        app_state.block_types = vec![BlockType::DataByte; 48];

        // An external label at $989A — a high address that appears before the
        // main disassembly body in the array when all_labels = true, and whose
        // address ($989A) satisfies `address >= $3A10`, making it the old buggy
        // winner of the "first address >= target" third pass.
        app_state
            .labels
            .entry(Addr(0x989A))
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
        let theme = Theme::default();
        let mut ui_state = UIState::new(theme);
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
            .load_binary(Addr(0x1000), vec![0xEA; 64])
            .expect("loading test binary should succeed");
        let mut ui_state = UIState::new(Theme::default());
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
            .load_binary(Addr(0x0801), vec![0xEA; 16])
            .expect("loading alignment test binary should succeed");
        let mut ui_state = UIState::new(Theme::default());
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

    #[test]
    fn test_hexdump_padding_b_preserves_selection_and_esc_clears_all() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x0801), vec![0xEA; 16])
            .expect("load 16-byte binary at $0801");
        let mut ui_state = UIState::new(Theme::default());
        let mut view = HexDumpView;

        // Cell (row 0, col 0) is $0800 (pre-origin alignment padding).
        ui_state.hex_cursor_index = 0;
        ui_state.hex_col_cursor = 0;
        ui_state.hex_selection_start = Some(0);
        ui_state.hex_selection_start_col = 0;
        ui_state.is_visual_mode = true;
        ui_state.selection_start = Some(0);

        let byte_result = view.handle_input(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(byte_result, WidgetResult::Ignored);
        assert_eq!(ui_state.hex_selection_start, Some(0));
        assert!(ui_state.is_visual_mode);
        assert_eq!(ui_state.selection_start, Some(0));

        // Esc clears input_buffer and all companion selection anchors.
        ui_state.input_buffer = "42".to_string();
        ui_state.hex_selection_start = Some(0);
        ui_state.hex_selection_start_col = 3;
        ui_state.sprites_selection_start = Some(1);
        ui_state.charset_selection_start = Some(2);
        ui_state.bitmap_selection_start = Some(0);
        ui_state.is_visual_mode = true;
        let escape_result = view.handle_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(escape_result, WidgetResult::Handled);
        assert!(!ui_state.has_any_selection());
        assert_eq!(ui_state.hex_selection_start_col, 0);
        assert!(ui_state.input_buffer.is_empty());
    }

    #[test]
    fn test_hexdump_empty_binary_ignores_shift_arrow_selection() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::HexDump;
        ui_state.right_pane = RightPane::HexDump16;
        let mut view = HexDumpView;

        for code in [KeyCode::Down, KeyCode::Up, KeyCode::Left, KeyCode::Right] {
            let _ = view.handle_input(
                KeyEvent::new(code, KeyModifiers::SHIFT),
                &mut app_state,
                &mut ui_state,
            );
            assert_eq!(
                ui_state.hex_selection_start, None,
                "Shift+{code:?} on empty binary must not anchor hex selection"
            );
        }
    }

    #[test]
    fn test_hexdump_render_collapsed_block_and_scope_closing_brace_highlights() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x2000), vec![0xEA; 48])
            .expect("load 48-byte binary at $2000");
        app_state.block_types = vec![BlockType::Code; 48];
        app_state.settings.assembler = Assembler::Kick;
        // Collapsed block covers $2000..=$200F, and scope covers $2010..=$201F (closing `}` at $2020).
        app_state.collapsed_blocks.push((0, 15));
        app_state.annotations.update(Addr(0x2010), |entry| {
            entry.scope = Some(Addr(0x201F));
        });
        app_state.disassemble();

        let collapsed_index = app_state
            .disassembly
            .iter()
            .position(|line| line.is_collapsed && line.address == Addr(0x2000))
            .expect("collapsed block line at $2000");
        let scope_2010_index = app_state
            .disassembly
            .iter()
            .position(|line| line.address == Addr(0x2010) && !line.bytes.is_empty())
            .expect("code line at $2010");
        let close_brace_index = app_state
            .disassembly
            .iter()
            .position(|line| {
                line.address == Addr(0x2020) && line.mnemonic == "}" && line.bytes.is_empty()
            })
            .expect("closing brace scope delimiter at $2020");

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Disassembly;
        ui_state.right_pane = RightPane::HexDump16;
        let view = HexDumpView;
        let area = Rect::new(0, 0, 78, 8);
        let selection_bg = ui_state.theme.selection_bg;
        let byte_cell_x = |col: usize| -> u16 { (8 + col * 3 + col / 4) as u16 };

        // 1. Collapsed block at $2000..=$200F highlights all 16 bytes on row 0 without spilling.
        ui_state.selection_start = None;
        ui_state.cursor_index = collapsed_index;
        let mut terminal = Terminal::new(TestBackend::new(78, 8)).expect("create 78x8 terminal");
        terminal
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .expect("draw collapsed block highlight frame");
        let collapsed_buf = terminal.backend().buffer();
        for col in 0..16 {
            assert_eq!(
                collapsed_buf
                    .cell((byte_cell_x(col), 1))
                    .expect("row 0 byte cell")
                    .bg,
                selection_bg,
                "collapsed block byte at col {col} ($2000+{col}) must highlight"
            );
        }
        assert_ne!(
            collapsed_buf
                .cell((byte_cell_x(0), 2))
                .expect("$2010 cell")
                .bg,
            selection_bg,
            "$2010 must not highlight for collapsed block $2000..=$200F"
        );

        // 2. Selection ending on closing `}` at $2020 highlights $2010..=$201F and stops before $2020.
        ui_state.selection_start = Some(scope_2010_index);
        ui_state.cursor_index = close_brace_index;
        terminal
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .expect("draw scoped selection highlight frame");
        let scoped_buf = terminal.backend().buffer();
        for col in 0..16 {
            assert_eq!(
                scoped_buf
                    .cell((byte_cell_x(col), 2))
                    .expect("row 1 byte cell")
                    .bg,
                selection_bg,
                "scoped byte at col {col} ($2010+{col}) must highlight"
            );
        }
        assert_ne!(
            scoped_buf.cell((byte_cell_x(0), 3)).expect("$2020 cell").bg,
            selection_bg,
            "closing brace at $2020 must not spill highlight to $2020"
        );
    }

    #[test]
    fn test_hexdump_render_preserves_sub_instruction_selection_across_tab() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(
                Addr(0x0800),
                vec![0xEA, 0xEA, 0xEA, 0xEA, 0x20, 0x10, 0x08, 0x60],
            )
            .expect("load 8-byte binary at $0800");
        app_state.block_types = vec![BlockType::Code; 8];
        app_state.system_config.sync_hex_dump = true;
        app_state.disassemble();

        let line_for_0804 = app_state
            .line_index_containing_address(Addr(0x0804))
            .expect("line index for $0804");

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Disassembly;
        ui_state.right_pane = RightPane::HexDump16;
        ui_state.hex_selection_start = Some(0);
        ui_state.hex_selection_start_col = 4;
        ui_state.hex_cursor_index = 0;
        ui_state.hex_col_cursor = 5;
        ui_state.cursor_index = line_for_0804;
        ui_state.selection_start = Some(line_for_0804);

        let view = HexDumpView;
        let mut terminal = Terminal::new(TestBackend::new(78, 12)).expect("create 78x12 terminal");
        terminal
            .draw(|f| view.render(f, Rect::new(0, 0, 78, 12), &app_state, &mut ui_state))
            .expect("draw hexdump frame across Tab");

        assert_eq!(ui_state.hex_cursor_index, 0);
        assert_eq!(ui_state.hex_col_cursor, 5);
        assert_eq!(ui_state.hex_selection_start, Some(0));
        assert_eq!(ui_state.hex_selection_start_col, 4);
    }
}
