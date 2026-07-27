//! TUI disassembly view component for rendering and interacting with MOS 6502 disassembly.
//!
//! This module organizes the disassembly view widget into specialized submodules:
//! - [`layout`]: Visual line height calculation, line mapping, and layout geometry breakdown.
//! - [`navigation`]: Cursor movement, scrolling, paging, and jump commands.
//! - [`mouse`]: Mouse event handling, scroll wheel actions, and click/drag range selection.
//! - [`render`]: Widget rendering logic using Ratatui.

pub mod layout;
pub mod mouse;
pub mod navigation;
pub mod render;

use crate::ui::navigable::Navigable;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::MouseEvent;
use ratatui::{Frame, layout::Rect};
use regenerator2000_core::state::AppState;

pub use layout::VisualLineCounts;

/// Interactive Ratatui [`Widget`] representing the MOS 6502 Disassembly View pane.
pub struct DisassemblyView;

impl DisassemblyView {
    /// Breakdown of visual lines consumed by a single instruction line.
    ///
    /// # Arguments
    ///
    /// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
    /// * `app_state` - Reference to application state [`AppState`].
    ///
    /// # Returns
    ///
    /// A [`VisualLineCounts`] breakdown detailing label lines, comment lines, and instruction lines.
    #[must_use]
    pub fn get_visual_line_counts(
        line: &regenerator2000_core::disassembler::DisassemblyLine,
        app_state: &AppState,
    ) -> VisualLineCounts {
        layout::get_visual_line_counts(line, app_state)
    }

    /// Total visual line height required for an instruction line.
    ///
    /// # Arguments
    ///
    /// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
    /// * `app_state` - Reference to application state [`AppState`].
    ///
    /// # Returns
    ///
    /// `usize` total visual line count.
    #[must_use]
    #[allow(dead_code)]
    pub fn get_visual_line_count_for_instruction(
        line: &regenerator2000_core::disassembler::DisassemblyLine,
        app_state: &AppState,
    ) -> usize {
        layout::get_visual_line_count_for_instruction(line, app_state)
    }

    /// Resolves target visual line number to instruction index in disassembly.
    ///
    /// # Arguments
    ///
    /// * `app_state` - Reference to application state [`AppState`].
    /// * `target_line` - 1-indexed visual line number to resolve.
    ///
    /// # Returns
    ///
    /// `Some(usize)` disassembly line index, or `None` if out of bounds.
    #[must_use]
    pub fn get_index_for_visual_line(app_state: &AppState, target_line: usize) -> Option<usize> {
        layout::get_index_for_visual_line(app_state, target_line)
    }

    /// Resolves sub-cursor index for address within a multi-line instruction.
    ///
    /// # Arguments
    ///
    /// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
    /// * `app_state` - Reference to application state [`AppState`].
    /// * `target_addr` - Target 16-bit memory address value.
    ///
    /// # Returns
    ///
    /// Sub-cursor offset line index `usize`.
    #[must_use]
    pub fn get_sub_index_for_address(
        line: &regenerator2000_core::disassembler::DisassemblyLine,
        app_state: &AppState,
        target_addr: u16,
    ) -> usize {
        layout::get_sub_index_for_address(line, app_state, target_addr)
    }
}

impl Navigable for DisassemblyView {
    fn len(&self, app_state: &AppState) -> usize {
        app_state.disassembly.len()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.selection_start.is_none() {
                ui_state.selection_start = Some(ui_state.cursor_index);
            }
        } else {
            ui_state.selection_start = None;
        }
        navigation::move_cursor_down(app_state, ui_state, amount);
    }

    fn move_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.selection_start.is_none() {
                ui_state.selection_start = Some(ui_state.cursor_index);
            }
        } else {
            ui_state.selection_start = None;
        }
        navigation::move_cursor_up(app_state, ui_state, amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        navigation::page_down(app_state, ui_state, self.len(app_state));
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        navigation::page_up(app_state, ui_state);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        navigation::jump_to(ui_state, index, self.len(app_state));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        navigation::jump_to_user_input(app_state, ui_state, input, self.len(app_state));
    }

    fn item_name(&self) -> &'static str {
        "visual line"
    }
}

impl Widget for DisassemblyView {
    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        crate::ui::navigable::handle_nav_input(self, key, app_state, ui_state)
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        mouse::handle_mouse_events(mouse, app_state, ui_state)
    }

    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        render::render_disassembly(f, area, app_state, ui_state);
    }
}
