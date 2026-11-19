use crate::state::AppState;
use crate::ui::view_disassembly::DisassemblyView;
use crate::ui::widget::WidgetResult;
use crate::ui_state::{AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

/// Shared cursor-navigation contract implemented by all scrollable/grid TUI views
/// (`DisassemblyView`, `HexDumpView`, `SpritesView`, `CharsetView`, `BitmapView`, `BlocksView`).
pub trait Navigable {
    fn len(&self, app_state: &AppState) -> usize;

    fn len_with_ui(&self, app_state: &AppState, _ui_state: &UIState) -> usize {
        self.len(app_state)
    }

    fn current_index(&self, app_state: &AppState, ui_state: &UIState) -> usize;

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize);
    fn move_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize);

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 10);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 10);
    }

    // Jump to absolute index (e.g. from Home/End or computed target)
    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize);

    // Jump based on 1-based user input (e.g. from `[N]G`, where `0` jumps to the last item)
    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let total = self.len_with_ui(app_state, ui_state);
        let target = if input == 0 {
            total.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(total.saturating_sub(1))
        };
        self.jump_to(app_state, ui_state, target);
    }

    fn item_name(&self) -> &str;
}

pub use regenerator2000_core::navigation::AlignedGridRange;

/// Mirrors a right-pane cursor address and optional selection anchor into the disassembly view.
///
/// When `row_floor_raw_addr` is `Some(row_addr)`, this function checks whether the containing
/// instruction starts before `row_addr`. If so, it snaps forward to the first content instruction
/// at or after `row_addr`, or falls back to the containing block.
pub fn sync_pane_to_disassembly(
    app_state: &AppState,
    ui_state: &mut UIState,
    grid: &AlignedGridRange,
    cursor_raw_addr: usize,
    selection_raw_addr: Option<usize>,
    row_floor_raw_addr: Option<usize>,
) {
    if app_state.raw_data.is_empty() || app_state.disassembly.is_empty() {
        return;
    }
    let Some(cursor_addr) = grid.clamped_addr(cursor_raw_addr) else {
        return;
    };

    let containing = app_state.get_line_index_containing_address(cursor_addr);
    let inst_idx = if let Some(row_raw) = row_floor_raw_addr
        && let Some(row_addr) = grid.clamped_addr(row_raw)
    {
        containing
            .filter(|&idx| app_state.disassembly[idx].address >= row_addr.0)
            .or_else(|| {
                app_state
                    .get_line_index_for_address(row_addr)
                    .filter(|&idx| !app_state.disassembly[idx].bytes.is_empty())
                    .or(containing)
            })
    } else {
        containing
    };

    if let Some(inst_idx) = inst_idx {
        ui_state.cursor_index = inst_idx;
        let counts =
            DisassemblyView::get_visual_line_counts(&app_state.disassembly[inst_idx], app_state);
        ui_state.sub_cursor_index = counts.opcode_sub_index();

        ui_state.selection_start = selection_raw_addr
            .and_then(|raw| grid.clamped_addr(raw))
            .and_then(|anchor_addr| app_state.get_line_index_containing_address(anchor_addr));
    }
}

/// Syncs a grid pane cursor and optional selection into `DisassemblyView`.
///
/// - **No selection (`selection_start == None`):** Moves `ui_state.cursor_index` to the item's
///   first byte and leaves `ui_state.selection_start` as `None`. `DisassemblyView` highlights
///   the focused item via [`AlignedGridRange::active_pane_item_addr_range`].
/// - **Active selection (`selection_start == Some(anchor)`):** Resolves the clamped byte range
///   via [`AlignedGridRange::clamped_item_addr_range`]. Places `ui_state.cursor_index` at
///   `end_addr` when `cursor_index > anchor`, or at `start_addr` otherwise. Clears
///   `ui_state.selection_start` if the range lies in alignment padding before `origin`.
pub fn sync_grid_item_to_disassembly(
    app_state: &AppState,
    ui_state: &mut UIState,
    grid: &AlignedGridRange,
    cursor_index: usize,
    selection_start: Option<usize>,
) {
    if app_state.raw_data.is_empty() || app_state.disassembly.is_empty() {
        return;
    }

    if let Some(anchor_index) = selection_start {
        let Some((start_addr, end_addr)) = grid.clamped_item_addr_range(anchor_index, cursor_index)
        else {
            // Item lies in alignment padding before `origin`. Clear the disassembly selection.
            sync_pane_to_disassembly(
                app_state,
                ui_state,
                grid,
                grid.item_addr(cursor_index),
                None,
                None,
            );
            return;
        };

        // Place the disassembly cursor on the moving edge so the viewport scrolls with it.
        let (cursor_raw, anchor_raw) = if cursor_index > anchor_index {
            (usize::from(end_addr.0), usize::from(start_addr.0))
        } else {
            (usize::from(start_addr.0), usize::from(end_addr.0))
        };
        sync_pane_to_disassembly(
            app_state,
            ui_state,
            grid,
            cursor_raw,
            Some(anchor_raw),
            None,
        );
    } else {
        // Leave `selection_start` as `None` during passive focus so `Tab` carries no sticky selection.
        sync_pane_to_disassembly(
            app_state,
            ui_state,
            grid,
            grid.item_addr(cursor_index),
            None,
            None,
        );
    }
}

/// Adjusts `scroll_offset` so `cursor_row` stays within `[scroll_offset + margin,
/// scroll_offset + visible_rows - margin)`, then clamps to `total_rows.saturating_sub(visible_rows)`.
#[must_use]
pub fn compute_scroll_offset(
    mut scroll_offset: usize,
    cursor_row: usize,
    visible_rows: usize,
    total_rows: usize,
    max_margin: usize,
) -> usize {
    let margin = max_margin.min(visible_rows / 3);
    if cursor_row < scroll_offset + margin {
        scroll_offset = cursor_row.saturating_sub(margin);
    }
    if cursor_row >= scroll_offset + visible_rows.saturating_sub(margin) {
        scroll_offset =
            cursor_row.saturating_sub(visible_rows.saturating_sub(margin).saturating_sub(1));
    }
    scroll_offset.min(total_rows.saturating_sub(visible_rows))
}

/// Returns `true` if `item_index` falls within the inclusive selection range
/// `[selection_start, cursor_index]` (or equals `cursor_index` when no selection is active).
#[must_use]
pub fn is_grid_item_selected(
    item_index: usize,
    cursor_index: usize,
    selection_start: Option<usize>,
) -> bool {
    if let Some(sel_start) = selection_start {
        let (start, end) = if sel_start <= cursor_index {
            (sel_start, cursor_index)
        } else {
            (cursor_index, sel_start)
        };
        item_index >= start && item_index <= end
    } else {
        item_index == cursor_index
    }
}

/// Returns the 1-cell-inset inner area of `ui_state.right_pane_area` if `mouse` falls inside it.
#[must_use]
pub fn right_pane_mouse_inner_area(ui_state: &UIState, mouse: &MouseEvent) -> Option<Rect> {
    let area = ui_state.right_pane_area;
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    (mouse.column >= inner.x
        && mouse.column < inner.x + inner.width
        && mouse.row >= inner.y
        && mouse.row < inner.y + inner.height)
        .then_some(inner)
}

/// Returns `true` when a mouse press/drag event should extend the current selection
/// (mouse drag, `Shift+Click`, or active visual mode).
#[must_use]
pub fn is_mouse_selection_extend(mouse: &MouseEvent, is_visual_mode: bool) -> bool {
    matches!(mouse.kind, MouseEventKind::Drag(_))
        || mouse.modifiers.contains(KeyModifiers::SHIFT)
        || is_visual_mode
}

/// Applies a left-click or drag hit-test result to a 1D grid cursor and optional selection anchor.
pub fn apply_grid_mouse_click(
    mouse: &MouseEvent,
    is_visual_mode: bool,
    cursor_index: &mut usize,
    selection_start: &mut Option<usize>,
    hit_index: Option<usize>,
) {
    if let Some(idx) = hit_index {
        if is_mouse_selection_extend(mouse, is_visual_mode) {
            if selection_start.is_none() {
                *selection_start = Some(*cursor_index);
            }
        } else {
            *selection_start = None;
        }
        *cursor_index = idx;
    }
}

/// Moves a grid cursor forward by `delta` and updates `selection_start`.
///
/// Keeps or sets `selection_start` when `extend_selection` or `is_visual_mode` is `true`, and
/// clears it otherwise. If `total == 0`, returns without setting `selection_start`.
pub fn move_grid_cursor_down(
    cursor_index: &mut usize,
    selection_start: &mut Option<usize>,
    is_visual_mode: bool,
    extend_selection: bool,
    delta: usize,
    total: usize,
) {
    if total == 0 {
        if !extend_selection && !is_visual_mode {
            *selection_start = None;
        }
        return;
    }
    if extend_selection || is_visual_mode {
        if selection_start.is_none() {
            *selection_start = Some(*cursor_index);
        }
    } else {
        *selection_start = None;
    }
    *cursor_index = (*cursor_index + delta).min(total.saturating_sub(1));
}

/// Moves a grid cursor backward by `delta` and updates `selection_start`.
///
/// Keeps or sets `selection_start` when `extend_selection` or `is_visual_mode` is `true`, and
/// clears it otherwise. Callers must check `total > 0` before passing `extend_selection = true`.
pub fn move_grid_cursor_up(
    cursor_index: &mut usize,
    selection_start: &mut Option<usize>,
    is_visual_mode: bool,
    extend_selection: bool,
    delta: usize,
) {
    if extend_selection || is_visual_mode {
        if selection_start.is_none() {
            *selection_start = Some(*cursor_index);
        }
    } else {
        *selection_start = None;
    }
    *cursor_index = cursor_index.saturating_sub(delta);
}

/// Jumps a grid cursor to `index` (clamped to `total - 1`).
///
/// Keeps or sets `selection_start` in visual mode, and clears non-visual `Shift+Arrow`
/// selections. If `total == 0`, returns without setting `selection_start`.
pub fn jump_grid_cursor(
    cursor_index: &mut usize,
    selection_start: &mut Option<usize>,
    is_visual_mode: bool,
    index: usize,
    total: usize,
) {
    if total == 0 {
        if !is_visual_mode {
            *selection_start = None;
        }
        return;
    }
    if is_visual_mode {
        if selection_start.is_none() {
            *selection_start = Some(*cursor_index);
        }
    } else {
        *selection_start = None;
    }
    *cursor_index = index.min(total.saturating_sub(1));
}

/// Handles shared selection and navigation keys for `SpritesView`, `CharsetView`, and
/// `BitmapView`.
///
/// - `Esc`: Clears visual mode, all pane selections, and `input_buffer`.
/// - `V` / `Shift+V`: Toggles visual selection mode.
/// - `Shift+Up` / `Shift+Down`: Extends the selection by one row (`grid_cols` items).
/// - `Left` / `Right` / `h` / `l` (when `grid_cols > 1`): Moves the cursor by one item.
/// - `Shift+Left` / `Shift+Right` (when `grid_cols > 1`): Extends the selection by one item.
/// - `b`: Converts the selected or focused items to `DataByte` and clears selections. Ignores
///   items that lie only in alignment padding before `origin`.
/// - `Enter`: Jumps focus to `DisassemblyView` at the current item's clamped address.
#[allow(clippy::too_many_arguments)]
pub fn handle_grid_selection_input(
    key: KeyEvent,
    app_state: &AppState,
    ui_state: &mut UIState,
    grid: &AlignedGridRange,
    grid_cols: usize,
    mut cursor: usize,
    mut selection: Option<usize>,
    set_state: impl FnOnce(&mut UIState, usize, Option<usize>),
) -> WidgetResult {
    let has_items = !app_state.raw_data.is_empty() && grid.item_count() > 0;
    match key.code {
        KeyCode::Esc if key.modifiers.is_empty() => {
            if selection.is_some() || ui_state.is_visual_mode {
                selection = None;
                set_state(ui_state, cursor, selection);
                // Clear selections in all panes and reset any `[N]G` count prefix.
                ui_state.core.clear_all_selections();
                ui_state.input_buffer.clear();
                ui_state.set_status_message("");
                WidgetResult::Handled
            } else {
                WidgetResult::Ignored
            }
        }
        KeyCode::Char('V') if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            if has_items {
                ui_state.is_visual_mode = !ui_state.is_visual_mode;
                if ui_state.is_visual_mode {
                    selection = Some(cursor);
                    ui_state.set_status_message("Visual Mode");
                } else {
                    selection = None;
                    ui_state.set_status_message("");
                }
            }
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
            if has_items {
                move_grid_cursor_down(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ true,
                    grid_cols,
                    grid.item_count(),
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
            if has_items {
                move_grid_cursor_up(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ true,
                    grid_cols,
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        KeyCode::Left if key.modifiers == KeyModifiers::SHIFT && grid_cols > 1 => {
            if has_items {
                move_grid_cursor_up(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ true,
                    1,
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        KeyCode::Right if key.modifiers == KeyModifiers::SHIFT && grid_cols > 1 => {
            if has_items {
                move_grid_cursor_down(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ true,
                    1,
                    grid.item_count(),
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        // Guard `Left`/`Right`/`h`/`l` with `key.modifiers.is_empty()` so modified arrow keys
        // pass through to global shortcuts.
        KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() && grid_cols > 1 => {
            ui_state.input_buffer.clear();
            if has_items {
                move_grid_cursor_up(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ false,
                    1,
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() && grid_cols > 1 => {
            ui_state.input_buffer.clear();
            if has_items {
                move_grid_cursor_down(
                    &mut cursor,
                    &mut selection,
                    ui_state.is_visual_mode,
                    /* extend_selection= */ false,
                    1,
                    grid.item_count(),
                );
                set_state(ui_state, cursor, selection);
            }
            WidgetResult::Handled
        }
        KeyCode::Char('b') if key.modifiers.is_empty() => {
            let end_item = selection.unwrap_or(cursor);
            // Ignore padding-only slots before dispatching `AppAction::Byte` so active selections stay intact.
            if grid.item_range_byte_offsets(cursor, end_item).is_none() {
                return WidgetResult::Ignored;
            }
            WidgetResult::Action(AppAction::Byte)
        }
        KeyCode::Enter if key.modifiers.is_empty() => grid
            .clamped_item_addr(cursor)
            .map_or(WidgetResult::Ignored, |addr| {
                jump_to_disassembly_at_address(app_state, ui_state, addr)
            }),
        _ => WidgetResult::Ignored,
    }
}

/// Processes standard navigation keys (`Up`/`Down`/`j`/`k`, `PageUp`/`PageDown`, `Ctrl+u`/`Ctrl+d`,
/// `Home`/`End`, numeric prefixes `0`–`9`, and `[N]G`) for any [`Navigable`] view.
///
/// This function guards `Up` and `Down` with `key.modifiers.is_empty()` so `Shift` and `Alt`
/// arrow keys pass through to the view handler.
pub fn handle_nav_input<T: Navigable>(
    target: &T,
    key: KeyEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> WidgetResult {
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
        KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
            ui_state.input_buffer.clear();
            target.move_down(app_state, ui_state, 1);
            WidgetResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
            ui_state.input_buffer.clear();
            target.move_up(app_state, ui_state, 1);
            WidgetResult::Handled
        }
        KeyCode::PageDown => {
            ui_state.input_buffer.clear();
            target.page_down(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
            ui_state.input_buffer.clear();
            target.page_down(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::PageUp => {
            ui_state.input_buffer.clear();
            target.page_up(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
            ui_state.input_buffer.clear();
            target.page_up(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Home => {
            ui_state.input_buffer.clear();
            target.jump_to(app_state, ui_state, 0);
            WidgetResult::Handled
        }
        KeyCode::End => {
            ui_state.input_buffer.clear();
            let len = target.len_with_ui(app_state, ui_state);
            target.jump_to(app_state, ui_state, len.saturating_sub(1));
            WidgetResult::Handled
        }
        // Accept both `KeyModifiers::NONE` and `KeyModifiers::SHIFT` for uppercase `'G'` across
        // different terminal backends while rejecting `Ctrl+Shift+G` / `Alt+Shift+G` (`JumpToLine`).
        KeyCode::Char('G') if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
            let is_buffer_empty = ui_state.input_buffer.is_empty();
            let active_pane = ui_state.active_pane;
            let current_idx = target.current_index(app_state, ui_state);

            ui_state.input_buffer.clear();

            // Push history before jumping
            ui_state.core.navigation_history.push((
                active_pane,
                crate::ui_state::NavigationTarget::Index(current_idx),
            ));

            if is_buffer_empty {
                let len = target.len_with_ui(app_state, ui_state);
                let end_idx = len.saturating_sub(1);
                target.jump_to(app_state, ui_state, end_idx);
                ui_state.set_status_message("Jumped to end");
            } else {
                target.jump_to_user_input(app_state, ui_state, entered_number);
                ui_state.set_status_message(format!(
                    "Jumped to {} {}",
                    target.item_name(),
                    entered_number
                ));
            }
            WidgetResult::Handled
        }
        _ => WidgetResult::Ignored,
    }
}

/// Switches focus to [`crate::ui_state::ActivePane::Disassembly`] at the line containing
/// `target_addr` and records the caller pane and index in `navigation_history`.
pub fn jump_to_disassembly_at_address(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) -> WidgetResult {
    regenerator2000_core::navigation::perform_jump_to_address(
        app_state,
        &mut ui_state.core,
        target_addr,
    );
    ui_state.sync_status_message();
    WidgetResult::Handled
}

/// Switches focus to [`crate::ui_state::ActivePane::Disassembly`] at `line_idx` (if `Some`),
/// recording the previous pane's cursor position in `navigation_history` for `Backspace` navigation.
pub fn jump_to_disassembly_at_line_idx(
    ui_state: &mut UIState,
    line_idx: Option<usize>,
    target_addr: crate::state::Addr,
) -> WidgetResult {
    if let Some(line_idx) = line_idx {
        let active_pane = ui_state.active_pane;
        let target = match active_pane {
            crate::ui_state::ActivePane::Disassembly => {
                crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index)
            }
            crate::ui_state::ActivePane::HexDump => {
                crate::ui_state::NavigationTarget::Index(ui_state.hex_cursor_index)
            }
            crate::ui_state::ActivePane::Sprites => {
                crate::ui_state::NavigationTarget::Index(ui_state.sprites_cursor_index)
            }
            crate::ui_state::ActivePane::Charset => {
                crate::ui_state::NavigationTarget::Index(ui_state.charset_cursor_index)
            }
            crate::ui_state::ActivePane::Bitmap => {
                crate::ui_state::NavigationTarget::Index(ui_state.bitmap_cursor_index)
            }
            crate::ui_state::ActivePane::Blocks => crate::ui_state::NavigationTarget::Index(
                ui_state.blocks_selected_index.unwrap_or(0),
            ),
            crate::ui_state::ActivePane::Debugger => {
                crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index)
            }
        };
        ui_state.core.navigation_history.push((active_pane, target));
        ui_state.cursor_index = line_idx;
        ui_state.active_pane = crate::ui_state::ActivePane::Disassembly;
        ui_state.sub_cursor_index = 0;

        // Also reset scroll if not visible
        let is_visible = ui_state.disassembly_viewport_height > 0
            && line_idx >= ui_state.scroll_index
            && line_idx < ui_state.scroll_index + ui_state.disassembly_viewport_height;

        if !is_visible {
            ui_state.scroll_index = line_idx;
            ui_state.scroll_sub_index = 0;
        }

        ui_state.set_status_message(format!("Jumped to ${target_addr:04X}"));
    } else {
        ui_state.set_status_message(format!("Address ${target_addr:04X} not found"));
    }
    WidgetResult::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crate::ui_state::ActivePane;

    #[test]
    fn test_jump_to_disassembly_at_address_skips_splitter_and_sets_sub_cursor() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA; 8];
        app_state.block_types = vec![BlockType::Code; 8];
        app_state.splitters.insert(Addr(0x1004));
        app_state.annotations.update(Addr(0x1004), |e| {
            e.user_line_comment = Some("line1\nline2".to_string());
        });
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::HexDump;
        ui_state.hex_cursor_index = 3;

        let res = jump_to_disassembly_at_address(&app_state, &mut ui_state, Addr(0x1004));
        assert_eq!(res, WidgetResult::Handled);
        assert_eq!(ui_state.active_pane, ActivePane::Disassembly);

        let landed = &app_state.disassembly[ui_state.cursor_index];
        assert_eq!(landed.address, Addr(0x1004));
        assert!(
            !landed.bytes.is_empty(),
            "Jump must advance past zero-byte splitter header to the instruction line"
        );
        assert!(
            ui_state.sub_cursor_index > 0,
            "Jump must set sub_cursor_index past line comments"
        );
        assert_eq!(
            ui_state.core.navigation_history,
            vec![(
                ActivePane::HexDump,
                crate::ui_state::NavigationTarget::Index(3)
            )]
        );
    }

    #[test]
    fn test_aligned_grid_range_clamping_and_item_offsets() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 128];

        let grid = AlignedGridRange::uniform(&app_state, 64);
        assert_eq!(grid.origin, 0x0801);
        assert_eq!(grid.aligned_origin, 0x0800);
        assert_eq!(grid.end_address, 0x0881);
        assert_eq!(grid.item_count(), 3);

        // Byte at $0800 is in alignment padding (returns 0), $0801 is loaded (returns 0xEA).
        assert_eq!(grid.byte_at(&app_state.raw_data, 0x0800), 0);
        assert_eq!(grid.byte_at(&app_state.raw_data, 0x0801), 0xEA);
        assert_eq!(grid.clamped_item_addr(0), Some(Addr(0x0801)));

        // Item 0 covers [$0800..=$083F]; clamped to origin $0801 it yields offsets (0, 0x3E).
        assert_eq!(grid.item_range_byte_offsets(0, 0), Some((0, 0x3E)));
        // Reversed anchor/cursor order produces the same inclusive offset range.
        assert_eq!(grid.item_range_byte_offsets(1, 0), Some((0, 0x7E)));
    }

    #[test]
    fn test_sync_grid_item_to_disassembly_updates_cursor_and_selection() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA; 24];
        app_state.block_types = vec![BlockType::Code; 24];
        app_state.disassemble();

        let grid = AlignedGridRange::new(&app_state, 0x400, 8, 8);
        let mut ui_state = UIState::new(Theme::default());

        // Forward selection: item 0..=2 covers $1000..=$1017. Cursor leads at $1017.
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &grid, 2, Some(0));
        assert_eq!(ui_state.selection_start, Some(0));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x1017)
        );

        // Backward selection: item 2..=0 covers $1000..=$1017. Cursor leads at $1000 and anchor stays at $1017.
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &grid, 0, Some(2));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x1000)
        );
        let selection_index = ui_state
            .selection_start
            .expect("backward selection must set selection_start");
        assert_eq!(app_state.disassembly[selection_index].address, Addr(0x1017));
    }

    #[test]
    fn test_sync_grid_item_to_disassembly_tier1_tier2_and_padding() {
        // Setup A1: 256 bytes at $0900 for Sprites (64B) and Charset (8B).
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0900);
        app_state.raw_data = vec![0xEA; 256];
        app_state.block_types = vec![BlockType::Code; 256];
        app_state.disassemble();

        let sprites_grid = AlignedGridRange::sprites(&app_state);
        let charset_grid = AlignedGridRange::charset(&app_state);
        let mut ui_state = UIState::new(Theme::default());

        // Tier 1 passive focus (selection_start == None): cursor at item start, no selection_start.
        ui_state.selection_start = Some(99);
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &sprites_grid, 1, None);
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0940)
        );
        assert_eq!(ui_state.selection_start, None);

        // Tier 2 1-item selection (1, Some(1)): cursor at $0940, anchor at $097F (64 bytes).
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &sprites_grid, 1, Some(1));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0940)
        );
        assert_eq!(
            app_state.disassembly[ui_state
                .selection_start
                .expect("1-item sprite selection must set selection_start")]
            .address,
            Addr(0x097F)
        );

        // Tier 2 2-item forward (2, Some(1)) and backward (1, Some(2)) cover $0940..=$09BF (128 bytes).
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &sprites_grid, 2, Some(1));
        assert_eq!(
            app_state.disassembly[ui_state
                .selection_start
                .expect("forward sprite selection must set selection_start")]
            .address,
            Addr(0x0940)
        );
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x09BF)
        );

        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &sprites_grid, 1, Some(2));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0940)
        );
        assert_eq!(
            app_state.disassembly[ui_state
                .selection_start
                .expect("backward sprite selection must set selection_start")]
            .address,
            Addr(0x09BF)
        );

        // Charset 1-item (33, Some(33)) covers $0908..=$090F (8 bytes) since Charset aligns to $0400 ($0800).
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &charset_grid, 33, Some(33));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0908)
        );
        assert_eq!(
            app_state.disassembly[ui_state
                .selection_start
                .expect("1-item charset selection must set selection_start")]
            .address,
            Addr(0x090F)
        );

        // Charset item 1 ($0808..=$080F) lies in pre-origin padding (< $0900) and clears selection.
        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &charset_grid, 1, Some(1));
        assert_eq!(ui_state.selection_start, None);
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0900)
        );

        // Setup A2: 3-byte JSR $0920 straddling item 0 / item 1 boundary at $093E..=$0940.
        let mut straddle_state = AppState::new();
        straddle_state.origin = Addr(0x0900);
        let mut data = vec![0xEA; 256];
        data[0x3E] = 0x20;
        data[0x3F] = 0x20;
        data[0x40] = 0x09;
        straddle_state.raw_data = data;
        straddle_state.block_types = vec![BlockType::Code; 256];
        straddle_state.disassemble();
        let straddle_grid = AlignedGridRange::sprites(&straddle_state);
        sync_grid_item_to_disassembly(&straddle_state, &mut ui_state, &straddle_grid, 0, Some(0));
        assert_eq!(
            straddle_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0900)
        );
        assert_eq!(
            straddle_state.disassembly[ui_state
                .selection_start
                .expect("straddling instruction selection must set selection_start")]
            .address,
            Addr(0x093E)
        );

        // Setup B: Bitmap at $0000 (16,384 bytes) maps (0, Some(0)) to $0000..=$1F3F.
        let mut bitmap_state = AppState::new();
        bitmap_state.origin = Addr(0x0000);
        bitmap_state.raw_data = vec![0xEA; 16384];
        bitmap_state.block_types = vec![BlockType::Undefined; 16384];
        bitmap_state.disassemble();
        let bitmap_grid = AlignedGridRange::bitmap(&bitmap_state);
        sync_grid_item_to_disassembly(&bitmap_state, &mut ui_state, &bitmap_grid, 0, Some(0));
        assert_eq!(
            bitmap_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0000)
        );
        assert_eq!(
            bitmap_state.disassembly[ui_state
                .selection_start
                .expect("1-item bitmap selection must set selection_start")]
            .address,
            Addr(0x1F3F)
        );

        // Setup C: Pre-origin alignment padding only (origin = $0809, Charset slot 0 = $0800..=$0807).
        let mut padding_state = AppState::new();
        padding_state.origin = Addr(0x0809);
        padding_state.raw_data = vec![0xEA; 32];
        padding_state.block_types = vec![BlockType::Code; 32];
        padding_state.disassemble();
        let padding_grid = AlignedGridRange::charset(&padding_state);
        ui_state.selection_start = Some(5);
        sync_grid_item_to_disassembly(&padding_state, &mut ui_state, &padding_grid, 0, Some(0));
        assert_eq!(ui_state.selection_start, None);
        assert_eq!(
            padding_state.disassembly[ui_state.cursor_index].address,
            Addr(0x0809)
        );
    }

    #[test]
    fn test_handle_grid_selection_input_empty_clamping_2d_padding_and_esc() {
        // 1. Empty binary guard (raw_data = vec![]).
        let empty_state = AppState::new();
        let empty_grid = AlignedGridRange::sprites(&empty_state);
        let mut ui_state = UIState::new(Theme::default());
        let mut cursor = 0usize;
        let mut selection = None;

        for (code, modifiers) in [
            (KeyCode::Down, KeyModifiers::SHIFT),
            (KeyCode::Up, KeyModifiers::SHIFT),
            (KeyCode::Left, KeyModifiers::SHIFT),
            (KeyCode::Right, KeyModifiers::SHIFT),
            (KeyCode::Char('V'), KeyModifiers::SHIFT),
        ] {
            let result = handle_grid_selection_input(
                KeyEvent::new(code, modifiers),
                &empty_state,
                &mut ui_state,
                &empty_grid,
                2,
                cursor,
                selection,
                |_, new_cursor, new_selection| {
                    cursor = new_cursor;
                    selection = new_selection;
                },
            );
            assert_eq!(result, WidgetResult::Handled);
            assert_eq!(selection, None);
            assert_eq!(cursor, 0);
            assert!(!ui_state.is_visual_mode);
        }

        // 2. Shift+Cursor clamping at 0 and len - 1 (1-slot bitmap, total = 1).
        let mut bitmap_state = AppState::new();
        bitmap_state.origin = Addr(0x2000);
        bitmap_state.raw_data = vec![0xEA; 8000];
        bitmap_state.block_types = vec![BlockType::Undefined; 8000];
        bitmap_state.disassemble();
        let bitmap_grid = AlignedGridRange::bitmap(&bitmap_state);
        assert_eq!(bitmap_grid.item_count(), 1);

        cursor = 0;
        selection = None;
        let up_result = handle_grid_selection_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT),
            &bitmap_state,
            &mut ui_state,
            &bitmap_grid,
            1,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(up_result, WidgetResult::Handled);
        assert_eq!(selection, Some(0));
        assert_eq!(cursor, 0);

        selection = None;
        let down_result = handle_grid_selection_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
            &bitmap_state,
            &mut ui_state,
            &bitmap_grid,
            1,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(down_result, WidgetResult::Handled);
        assert_eq!(selection, Some(0));
        assert_eq!(cursor, 0);

        // A 1-column grid (`grid_cols == 1`) ignores `Shift+Left` and `Shift+Right`.
        for code in [KeyCode::Left, KeyCode::Right] {
            assert_eq!(
                handle_grid_selection_input(
                    KeyEvent::new(code, KeyModifiers::SHIFT),
                    &bitmap_state,
                    &mut ui_state,
                    &bitmap_grid,
                    1,
                    cursor,
                    selection,
                    |_, _, _| {},
                ),
                WidgetResult::Ignored
            );
        }

        // 3. 2D grid partial last row (Charset8Col, grid_cols = 8, total = 10).
        let mut charset_state = AppState::new();
        charset_state.origin = Addr(0x0800);
        charset_state.raw_data = vec![0xEA; 80];
        charset_state.block_types = vec![BlockType::Undefined; 80];
        charset_state.disassemble();
        let charset_grid = AlignedGridRange::charset(&charset_state);
        assert_eq!(charset_grid.item_count(), 10);

        cursor = 5;
        selection = None;
        handle_grid_selection_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
            &charset_state,
            &mut ui_state,
            &charset_grid,
            8,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(cursor, 9);
        assert_eq!(selection, Some(5));

        cursor = 1;
        selection = None;
        handle_grid_selection_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT),
            &charset_state,
            &mut ui_state,
            &charset_grid,
            8,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(cursor, 0);
        assert_eq!(selection, Some(1));

        // 4. 'b' on pure pre-origin alignment padding preserves active selection & returns Ignored.
        let mut padding_state = AppState::new();
        padding_state.origin = Addr(0x0809);
        padding_state.raw_data = vec![0xEA; 32];
        padding_state.block_types = vec![BlockType::Code; 32];
        padding_state.disassemble();
        let padding_grid = AlignedGridRange::charset(&padding_state);
        cursor = 0;
        selection = Some(0);
        ui_state.is_visual_mode = true;
        ui_state.selection_start = Some(0);
        let byte_result = handle_grid_selection_input(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &padding_state,
            &mut ui_state,
            &padding_grid,
            8,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(byte_result, WidgetResult::Ignored);
        assert_eq!(selection, Some(0));
        assert!(ui_state.is_visual_mode);
        assert_eq!(ui_state.selection_start, Some(0));

        // 5. Esc clears input_buffer and all companion selection anchors when active, ignores otherwise.
        ui_state.input_buffer = "42".to_string();
        ui_state.hex_selection_start = Some(2);
        ui_state.hex_selection_start_col = 5;
        ui_state.sprites_selection_start = Some(1);
        ui_state.bitmap_selection_start = Some(0);
        let escape_result = handle_grid_selection_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &padding_state,
            &mut ui_state,
            &padding_grid,
            8,
            cursor,
            selection,
            |_, new_cursor, new_selection| {
                cursor = new_cursor;
                selection = new_selection;
            },
        );
        assert_eq!(escape_result, WidgetResult::Handled);
        assert_eq!(selection, None);
        assert!(!ui_state.core.has_any_selection());
        assert_eq!(ui_state.hex_selection_start_col, 0);
        assert!(ui_state.input_buffer.is_empty());

        let idle_escape_result = handle_grid_selection_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &padding_state,
            &mut ui_state,
            &padding_grid,
            8,
            cursor,
            selection,
            |_, _, _| {},
        );
        assert_eq!(idle_escape_result, WidgetResult::Ignored);
    }
}
