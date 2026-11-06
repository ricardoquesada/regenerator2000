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

/// Floor-aligned memory grid geometry for right-pane views (`HexDumpView`, `SpritesView`,
/// `CharsetView`, and `BitmapView`).
///
/// Each view aligns its first grid slot to the floor multiple of `alignment` at or below
/// `app_state.origin`. It pads bytes before `origin` with zeros to preserve hardware boundaries
/// (`$2000` bitmaps, `$0400` charsets, `$0040` sprites, and `$0008`/`$0010` hex rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignedGridRange {
    pub origin: usize,
    pub aligned_origin: usize,
    pub end_address: usize,
    pub stride: usize,
    pub item_bytes: usize,
}

impl AlignedGridRange {
    /// Computes the floor-aligned grid range for `app_state` with explicit `alignment`,
    /// per-item address `stride`, and per-item payload size `item_bytes`.
    #[must_use]
    pub fn new(app_state: &AppState, alignment: usize, stride: usize, item_bytes: usize) -> Self {
        let origin = app_state.origin.0 as usize;
        let aligned_origin = (origin / alignment) * alignment;
        let end_address = (origin + app_state.raw_data.len()).min(0x10000);
        Self {
            origin,
            aligned_origin,
            end_address,
            stride,
            item_bytes,
        }
    }

    /// Computes a uniform floor-aligned grid range where `alignment == stride == item_bytes`.
    #[must_use]
    pub fn uniform(app_state: &AppState, stride: usize) -> Self {
        Self::new(app_state, stride, stride, stride)
    }

    /// Returns the total number of grid items spanning `[aligned_origin, end_address)`.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.end_address
            .saturating_sub(self.aligned_origin)
            .div_ceil(self.stride)
    }

    /// Returns the starting 16-bit memory address of grid item `index`.
    #[must_use]
    pub fn item_addr(&self, index: usize) -> usize {
        self.aligned_origin + index * self.stride
    }

    /// Reads the byte at `addr` from `raw_data`, returning `0` when `addr` falls in floor-alignment
    /// padding (`< origin`) or beyond `end_address`.
    #[inline]
    #[must_use]
    pub fn byte_at(&self, raw_data: &[u8], addr: usize) -> u8 {
        if addr >= self.origin && addr < self.end_address {
            raw_data[addr - self.origin]
        } else {
            0
        }
    }

    /// Clamps `raw_addr` to the loaded binary range `[origin, end_address - 1]`, returning `None`
    /// when `raw_data` is empty.
    #[must_use]
    pub fn clamped_addr(&self, raw_addr: usize) -> Option<crate::state::Addr> {
        (self.end_address > self.origin)
            .then(|| crate::state::Addr(raw_addr.max(self.origin).min(self.end_address - 1) as u16))
    }

    /// Returns the clamped starting address of grid item `index` within `[origin, end_address - 1]`.
    #[must_use]
    pub fn clamped_item_addr(&self, index: usize) -> Option<crate::state::Addr> {
        self.clamped_addr(self.item_addr(index))
    }

    /// Converts an inclusive `[start_addr, end_addr]` address range into a clamped
    /// `(start_offset, end_offset)` byte-offset pair within `raw_data`. Returns `None` when the
    /// range lies outside `[origin, end_address)`.
    #[must_use]
    pub fn clamped_byte_offsets(
        &self,
        start_addr: usize,
        end_addr: usize,
    ) -> Option<(usize, usize)> {
        if self.end_address <= self.origin
            || end_addr < self.origin
            || start_addr >= self.end_address
        {
            return None;
        }
        let clamped_start = start_addr.max(self.origin) - self.origin;
        let clamped_end = end_addr.min(self.end_address - 1) - self.origin;
        (clamped_start <= clamped_end).then_some((clamped_start, clamped_end))
    }

    /// Converts an inclusive `(first_item, last_item)` grid index range (in either order) into a
    /// clamped `(start_offset, end_offset)` byte-offset pair within `raw_data`.
    #[must_use]
    pub fn item_range_byte_offsets(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        let (first, last) = if a <= b { (a, b) } else { (b, a) };
        let start_addr = self.item_addr(first);
        let end_addr = self.item_addr(last) + self.item_bytes.saturating_sub(1);
        self.clamped_byte_offsets(start_addr, end_addr)
    }
}

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

/// Mirrors a grid-indexed right-pane cursor (`cursor_index` and `selection_start`) into the
/// disassembly view using `grid` geometry.
pub fn sync_grid_item_to_disassembly(
    app_state: &AppState,
    ui_state: &mut UIState,
    grid: &AlignedGridRange,
    cursor_index: usize,
    selection_start: Option<usize>,
) {
    sync_pane_to_disassembly(
        app_state,
        ui_state,
        grid,
        grid.item_addr(cursor_index),
        selection_start.map(|sel| grid.item_addr(sel)),
        None,
    );
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

/// Moves a 1D grid cursor forward by `delta`, preserving or clearing `selection_start`
/// according to `is_visual_mode` or `extend_selection`.
pub fn move_grid_cursor_down(
    cursor_index: &mut usize,
    selection_start: &mut Option<usize>,
    is_visual_mode: bool,
    extend_selection: bool,
    delta: usize,
    total: usize,
) {
    if extend_selection || is_visual_mode {
        if selection_start.is_none() {
            *selection_start = Some(*cursor_index);
        }
    } else {
        *selection_start = None;
    }
    if total > 0 {
        *cursor_index = (*cursor_index + delta).min(total.saturating_sub(1));
    }
}

/// Moves a 1D grid cursor backward by `delta`, preserving or clearing `selection_start`
/// according to `is_visual_mode` or `extend_selection`.
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

/// Handles shared keyboard shortcuts for grid-based right-pane views (`SpritesView` and
/// `CharsetView`):
/// - `Esc`: Clears active visual mode and selection anchor.
/// - `V` / `Shift+V`: Toggles visual selection mode.
/// - `Shift+Up` / `Shift+Down`: Extends selection by one visual row (`grid_cols` items).
/// - `Left` / `Right` / `h` / `l` and `Shift+Left` / `Shift+Right` (when `grid_cols > 1`):
///   Moves or extends selection horizontally by 1 item.
/// - `b`: Converts the selected item range (or current item) to a `DataByte` block and clears
///   selection state.
/// - `Enter`: Jumps focus to the Disassembly view at the current item's clamped address.
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
    match key.code {
        KeyCode::Esc if key.modifiers.is_empty() => {
            if selection.is_some() || ui_state.is_visual_mode {
                selection = None;
                ui_state.is_visual_mode = false;
                ui_state.set_status_message("");
                set_state(ui_state, cursor, selection);
                WidgetResult::Handled
            } else {
                WidgetResult::Ignored
            }
        }
        KeyCode::Char('V') if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            if !app_state.raw_data.is_empty() {
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
            move_grid_cursor_down(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                true,
                grid_cols,
                grid.item_count(),
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
            move_grid_cursor_up(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                true,
                grid_cols,
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Left if key.modifiers == KeyModifiers::SHIFT && grid_cols > 1 => {
            move_grid_cursor_up(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                true,
                1,
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Right if key.modifiers == KeyModifiers::SHIFT && grid_cols > 1 => {
            move_grid_cursor_down(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                true,
                1,
                grid.item_count(),
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        // Guard `Left`/`Right`/`h`/`l` with `key.modifiers.is_empty()` so modified arrow keys
        // pass through to global shortcuts.
        KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() && grid_cols > 1 => {
            ui_state.input_buffer.clear();
            move_grid_cursor_up(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                false,
                1,
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() && grid_cols > 1 => {
            ui_state.input_buffer.clear();
            move_grid_cursor_down(
                &mut cursor,
                &mut selection,
                ui_state.is_visual_mode,
                false,
                1,
                grid.item_count(),
            );
            set_state(ui_state, cursor, selection);
            WidgetResult::Handled
        }
        KeyCode::Char('b') if key.modifiers.is_empty() => {
            let end_item = selection.unwrap_or(cursor);
            // Clear selection state before dispatching `SetBytesBlockByOffset` so `handle_menu_action`
            // syncs the cleared flags into `core.view`.
            selection = None;
            ui_state.is_visual_mode = false;
            ui_state.selection_start = None;
            set_state(ui_state, cursor, selection);
            grid.item_range_byte_offsets(cursor, end_item).map_or(
                WidgetResult::Ignored,
                |(start, end)| {
                    WidgetResult::Action(AppAction::SetBytesBlockByOffset { start, end })
                },
            )
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

        sync_grid_item_to_disassembly(&app_state, &mut ui_state, &grid, 2, Some(0));
        assert_eq!(ui_state.selection_start, Some(0));
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x1010)
        );
    }
}
