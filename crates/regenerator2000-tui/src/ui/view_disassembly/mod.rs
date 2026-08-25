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

use crate::state::actions::AppAction;
use crate::ui::navigable::{self, Navigable};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
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
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = navigable::handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j')
                if key.code == KeyCode::Down || key.modifiers.is_empty() =>
            {
                if key.modifiers == KeyModifiers::SHIFT || ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }
                navigation::move_cursor_down(app_state, ui_state, 1);
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if key.code == KeyCode::Up || key.modifiers.is_empty() =>
            {
                if key.modifiers == KeyModifiers::SHIFT || ui_state.is_visual_mode {
                    if ui_state.selection_start.is_none() {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                    }
                } else {
                    ui_state.selection_start = None;
                }
                navigation::move_cursor_up(app_state, ui_state, 1);
                WidgetResult::Handled
            }
            KeyCode::F(3) => {
                if key.modifiers == KeyModifiers::SHIFT {
                    WidgetResult::Action(AppAction::FindPrevious)
                } else if key.modifiers.is_empty() {
                    WidgetResult::Action(AppAction::FindNext)
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Char('/') if key.modifiers.is_empty() => {
                ui_state.vim_search_active = true;
                ui_state.vim_search_input.clear();
                WidgetResult::Handled
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                crate::ui::dialog_search::perform_search(app_state, ui_state, true);
                WidgetResult::Handled
            }
            KeyCode::Char('N') if key.modifiers == KeyModifiers::SHIFT => {
                crate::ui::dialog_search::perform_search(app_state, ui_state, false);
                WidgetResult::Handled
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(AppAction::Search)
            }
            KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                if !app_state.raw_data.is_empty() {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        ui_state.selection_start = Some(ui_state.cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Backspace if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::NavigateBack)
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::SetLabel)
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => WidgetResult::Action(AppAction::Code),
            KeyCode::Char('r')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::Scope)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => WidgetResult::Action(AppAction::Byte),
            KeyCode::Char('w') if key.modifiers.is_empty() => WidgetResult::Action(AppAction::Word),
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::Address)
            }
            KeyCode::Char('p') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::PetsciiText)
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::ScreencodeText)
            }
            KeyCode::Char('?')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::Undefined)
            }
            KeyCode::Char('<')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::SetLoHiAddress)
            }
            KeyCode::Char('>')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::SetHiLoAddress)
            }
            KeyCode::Char(',') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::SetLoHiWord)
            }
            KeyCode::Char('.') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::SetHiLoWord)
            }
            KeyCode::Char('|')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::ToggleSplitter)
            }
            KeyCode::Char(';')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::SideComment)
            }
            KeyCode::Char(':')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                WidgetResult::Action(AppAction::LineComment)
            }
            KeyCode::Char('e') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::ApplyEnumDialog)
            }
            KeyCode::Char('E') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(AppAction::SetExternalFile)
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::JumpToOperand)
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::DisassembleAddress)
            }
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::NextImmediateFormat)
            }
            KeyCode::Char('I') if key.modifiers == KeyModifiers::SHIFT => {
                WidgetResult::Action(AppAction::PreviousImmediateFormat)
            }
            KeyCode::Char('[') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::PackLoHiAddress)
            }
            KeyCode::Char(']') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::PackHiLoAddress)
            }
            KeyCode::Up if key.modifiers == KeyModifiers::ALT => {
                WidgetResult::Action(AppAction::NudgeScopeBoundary { expand: false })
            }
            KeyCode::Down if key.modifiers == KeyModifiers::ALT => {
                WidgetResult::Action(AppAction::NudgeScopeBoundary { expand: true })
            }
            KeyCode::Delete if key.modifiers.is_empty() => {
                if !app_state.raw_data.is_empty()
                    && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
                {
                    // Priority 1: External address exclusion
                    if line.external_label_address.is_some() {
                        return WidgetResult::Action(AppAction::ExcludeExternalAddress);
                    }

                    // Priority 2: Scope removal (existing behavior)
                    let formatter = app_state.get_formatter();
                    let is_start = formatter
                        .format_scope_start(None)
                        .map(|(_, m, _)| m == line.mnemonic)
                        .unwrap_or(false);
                    let is_end = formatter
                        .format_scope_end()
                        .map(|m| m == line.mnemonic)
                        .unwrap_or(false);

                    if is_start || is_end {
                        return WidgetResult::Action(AppAction::RemoveScope);
                    }
                }
                WidgetResult::Ignored
            }
            KeyCode::Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                WidgetResult::Action(AppAction::ToggleCollapsedBlock)
            }
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && let KeyCode::Char('b') = key.code
                {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        return WidgetResult::Action(AppAction::ListBookmarks);
                    }
                    return WidgetResult::Action(AppAction::ToggleBookmark);
                }

                if key.modifiers.contains(KeyModifiers::ALT)
                    && let KeyCode::Char('b') = key.code
                {
                    return WidgetResult::Action(AppAction::ListBookmarks);
                }
                WidgetResult::Ignored
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_input_enter_jumps_to_operand() {
        let mut app_state = AppState::default();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let mut view = DisassemblyView;

        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        };

        let result = view.handle_input(key, &mut app_state, &mut ui_state);
        assert_eq!(result, WidgetResult::Action(AppAction::JumpToOperand));
    }

    #[test]
    fn test_handle_input_shifted_keys() {
        let mut app_state = AppState::default();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let mut view = DisassemblyView;

        let keys = vec!['?', '<', '>', '|', ';', ':'];
        let actions = vec![
            AppAction::Undefined,
            AppAction::SetLoHiAddress,
            AppAction::SetHiLoAddress,
            AppAction::ToggleSplitter,
            AppAction::SideComment,
            AppAction::LineComment,
        ];

        for (c, expected_action) in keys.into_iter().zip(actions) {
            // Test without SHIFT
            let key_no_shift = KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::empty(),
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            };
            let result = view.handle_input(key_no_shift, &mut app_state, &mut ui_state);
            assert_eq!(result, WidgetResult::Action(expected_action.clone()));

            // Test with SHIFT (Windows behavior)
            let key_with_shift = KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            };
            let result = view.handle_input(key_with_shift, &mut app_state, &mut ui_state);
            assert_eq!(result, WidgetResult::Action(expected_action));
        }
    }
}
