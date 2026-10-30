//! Global keyboard shortcut fallback handler (Layer 1 of the keyboard shortcut architecture).
//!
//! Invoked by `events::handle_key_event_active_view` whenever the active pane's widget returns
//! [`crate::ui::widget::WidgetResult::Ignored`]. Handles application-wide shortcuts (`Ctrl+*`,
//! `Alt+*`, `F1`–`F10`, `Tab`, `Esc`, and global `u` Undo) across all 7 [`crate::ui_state::ActivePane`]s.

use crate::events::AppEvent;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regenerator2000_core::Core;

use crate::ui::menu::handle_menu_action;

/// Processes application-wide keyboard shortcuts that were not consumed by a modal dialog,
/// active menu bar, inline Vim search bar, or the focused view's [`crate::ui::widget::Widget::handle_input`].
///
/// # Cross-Terminal Shifted Modifier Resilience
/// Terminal emulators and keyboard protocols differ in how they report shifted modifier
/// combinations (e.g. legacy ANSI `ESC G` vs. the Kitty keyboard protocol): some report
/// `KeyCode::Char('g')` with `KeyModifiers::SHIFT`, while others report uppercase
/// `KeyCode::Char('G')` with `KeyModifiers::SHIFT`. All `Ctrl+Shift+*` and `Alt+Shift+*` arms
/// therefore match both lowercase and uppercase `KeyCode::Char` variants.
pub fn handle_global_input(
    key: KeyEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) {
    match key.code {
        KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Exit,
                event_tx,
            );
        }

        // VICE Debugger begin
        // VICE keyboard shortcuts are global, at least the "F" keys, since they can be used from different views.
        KeyCode::F(2) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceToggleBreakpoint,
                event_tx,
            );
        }
        KeyCode::F(2) if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceBreakpointDialog,
                event_tx,
            );
        }
        KeyCode::F(6) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceToggleWatchpoint,
                event_tx,
            );
        }
        KeyCode::F(4) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceRunToCursor,
                event_tx,
            );
        }
        KeyCode::F(7) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceStep,
                event_tx,
            );
        }
        KeyCode::F(8) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceStepOver,
                event_tx,
            );
        }
        KeyCode::F(8) if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceStepOut,
                event_tx,
            );
        }
        KeyCode::F(9) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceContinue,
                event_tx,
            );
        }
        KeyCode::F(10) if key.modifiers.is_empty() => {
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
        }
        // VICE Debugger end
        KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
            ui_state.menu.selected_category = 0; // File is index 0
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
        }
        KeyCode::Char('h') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "Help")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('j') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "Jump")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('v') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "View")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "Search")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('t') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "Edit")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('u') if key.modifiers == KeyModifiers::ALT => {
            if let Some(pos) = ui_state
                .menu
                .categories
                .iter()
                .position(|c| c.name == "Debugger")
            {
                ui_state.menu.selected_category = pos;
                ui_state.menu.active = true;
                ui_state.menu.select_first_enabled_item();
                ui_state.set_status_message("Menu Active");
            }
        }
        KeyCode::Char('x') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::FindReferences,
                event_tx,
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::GoToSymbol,
                event_tx,
            );
        }
        // Global Shortcuts
        // Match both lowercase and uppercase characters on `Ctrl+Shift+*` / `Alt+Shift+*`
        // so shortcuts work regardless of whether the terminal sends `'o'` or `'O'` with `SHIFT`.
        KeyCode::Char('o') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Open,
                event_tx,
            );
        }
        KeyCode::Char('o' | 'O')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::OpenRecent,
                event_tx,
            );
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Analyze,
                event_tx,
            );
        }
        KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Save,
                event_tx,
            );
        }
        KeyCode::Char('s' | 'S')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::SaveAs,
                event_tx,
            );
        }
        KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ExportAsm,
                event_tx,
            );
        }
        KeyCode::Char('e' | 'E')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ExportAsmAs,
                event_tx,
            );
        }

        KeyCode::Char(',') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::SystemSettings,
                event_tx,
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::ALT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::SystemSettings,
                event_tx,
            );
        }

        KeyCode::Char('d' | 'D')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::DocumentSettings,
                event_tx,
            );
        }

        // Bookmark shortcuts (`Ctrl+B` and `Alt+B` / `Ctrl+Shift+B`) are handled globally
        // so that users can toggle or list bookmarks from any pane (and never accidentally
        // trigger view-local `'b'` byte conversions in `BitmapView`, `HexDumpView`, etc.).
        KeyCode::Char('b') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleBookmark,
                event_tx,
            );
        }
        KeyCode::Char('b' | 'B')
            if key.modifiers == KeyModifiers::ALT
                || key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT)
                || key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ListBookmarks,
                event_tx,
            );
        }

        KeyCode::Char('u') if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Undo,
                event_tx,
            );
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Redo,
                event_tx,
            );
        }
        KeyCode::Char('1')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleBlocksView,
                event_tx,
            );
        }
        KeyCode::Char('2')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleHexDump,
                event_tx,
            );
        }
        KeyCode::Char('3')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleSpritesView,
                event_tx,
            );
        }
        KeyCode::Char('4')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleCharsetView,
                event_tx,
            );
        }
        KeyCode::Char('5')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleBitmapView,
                event_tx,
            );
        }
        KeyCode::Char('6')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleDebuggerView,
                event_tx,
            );
        }
        KeyCode::Char('g')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::JumpToAddress,
                event_tx,
            );
        }
        KeyCode::Char('g' | 'G')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::JumpToLine,
                event_tx,
            );
        }
        KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::JumpNextUnexplored,
                event_tx,
            );
        }
        KeyCode::Char('n' | 'N')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::JumpPrevUnexplored,
                event_tx,
            );
        }
        KeyCode::Tab => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::CyclePane,
                event_tx,
            );
        }
        KeyCode::Esc => {
            ui_state.input_buffer.clear();
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::Cancel,
                event_tx,
            );
        }
        _ => {}
    }
}
