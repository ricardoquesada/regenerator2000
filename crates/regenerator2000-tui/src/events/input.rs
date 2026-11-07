//! Global keyboard shortcut fallback handler (Layer 1 of the keyboard shortcut architecture).
//!
//! `events::handle_key_event_active_view` calls [`handle_global_input`] when the active pane
//! returns [`crate::ui::widget::WidgetResult::Ignored`]. This module handles application-wide
//! shortcuts (`Ctrl+*`, `Alt+*`, `F2`–`F10`, `Tab`, `Esc`, and `u`) across all 7 panes.

use crate::events::AppEvent;
use crate::state::actions::AppAction;
use crate::ui::menu::handle_menu_action;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regenerator2000_core::Core;

/// Processes application-wide keyboard shortcuts when the active pane ignores a key event.
///
/// # Cross-Terminal Shifted Modifier Resilience
///
/// Terminal emulators report shifted modifier combinations differently. Legacy ANSI terminals
/// send `KeyCode::Char('g')` with `KeyModifiers::SHIFT`, while the Kitty protocol sends
/// `KeyCode::Char('G')` with `KeyModifiers::SHIFT`. Every `Ctrl+Shift+*` and `Alt+Shift+*`
/// arm matches both lowercase and uppercase `KeyCode::Char` variants.
pub fn handle_global_input(
    key: KeyEvent,
    core: &mut Core,
    ui_state: &mut UIState,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) {
    match key.code {
        KeyCode::F(10) if key.modifiers.is_empty() => {
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
            return;
        }
        KeyCode::Char(c)
            if key.modifiers == KeyModifiers::ALT
                && ui_state.menu.select_category_by_accelerator(c) =>
        {
            ui_state.menu.active = true;
            ui_state.set_status_message("Menu Active");
            return;
        }
        KeyCode::Esc => {
            ui_state.input_buffer.clear();
            handle_menu_action(core, ui_state, AppAction::Cancel, event_tx);
            return;
        }
        _ => {}
    }

    if let Some(action) = global_action_for_key(key) {
        handle_menu_action(core, ui_state, action, event_tx);
    }
}

/// Maps a global shortcut [`KeyEvent`] to its corresponding [`AppAction`].
#[must_use]
fn global_action_for_key(key: KeyEvent) -> Option<AppAction> {
    let ctrl = key.modifiers == KeyModifiers::CONTROL;
    let alt = key.modifiers == KeyModifiers::ALT;
    let shift = key.modifiers == KeyModifiers::SHIFT;
    let ctrl_or_alt = ctrl || alt;
    let ctrl_shift = key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    let alt_shift = key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT);
    let no_mod = key.modifiers.is_empty();

    match key.code {
        KeyCode::Char('q') if ctrl => Some(AppAction::Exit),

        // VICE debugger function keys operate globally across all panes.
        KeyCode::F(2) if no_mod => Some(AppAction::ViceToggleBreakpoint),
        KeyCode::F(2) if shift => Some(AppAction::ViceBreakpointDialog),
        KeyCode::F(4) if no_mod => Some(AppAction::ViceRunToCursor),
        KeyCode::F(6) if no_mod => Some(AppAction::ViceToggleWatchpoint),
        KeyCode::F(7) if no_mod => Some(AppAction::ViceStep),
        KeyCode::F(8) if no_mod => Some(AppAction::ViceStepOver),
        KeyCode::F(8) if shift => Some(AppAction::ViceStepOut),
        KeyCode::F(9) if no_mod => Some(AppAction::ViceContinue),

        KeyCode::Char('x') if ctrl => Some(AppAction::FindReferences),
        KeyCode::Char('p') if ctrl => Some(AppAction::GoToSymbol),
        KeyCode::Char('o') if ctrl => Some(AppAction::Open),
        KeyCode::Char('o' | 'O') if ctrl_shift || alt => Some(AppAction::OpenRecent),
        KeyCode::Char('a') if ctrl => Some(AppAction::Analyze),
        KeyCode::Char('s') if ctrl => Some(AppAction::Save),
        KeyCode::Char('s' | 'S') if ctrl_shift || alt => Some(AppAction::SaveAs),
        KeyCode::Char('e') if ctrl => Some(AppAction::ExportAsm),
        KeyCode::Char('e' | 'E') if ctrl_shift || alt => Some(AppAction::ExportAsmAs),
        KeyCode::Char(',') if ctrl => Some(AppAction::SystemSettings),
        KeyCode::Char('p') if alt => Some(AppAction::SystemSettings),
        KeyCode::Char('d' | 'D') if ctrl_shift || alt => Some(AppAction::DocumentSettings),

        // Global bookmark shortcuts prevent view-local `'b'` bindings from intercepting them.
        KeyCode::Char('b') if ctrl => Some(AppAction::ToggleBookmark),
        KeyCode::Char('b' | 'B') if alt || alt_shift || ctrl_shift => {
            Some(AppAction::ListBookmarks)
        }

        KeyCode::Char('u') if no_mod => Some(AppAction::Undo),
        KeyCode::Char('r') if ctrl => Some(AppAction::Redo),

        KeyCode::Char('1') if ctrl_or_alt => Some(AppAction::ToggleBlocksView),
        KeyCode::Char('2') if ctrl_or_alt => Some(AppAction::ToggleHexDump),
        KeyCode::Char('3') if ctrl_or_alt => Some(AppAction::ToggleSpritesView),
        KeyCode::Char('4') if ctrl_or_alt => Some(AppAction::ToggleCharsetView),
        KeyCode::Char('5') if ctrl_or_alt => Some(AppAction::ToggleBitmapView),
        KeyCode::Char('6') if ctrl_or_alt => Some(AppAction::ToggleDebuggerView),

        KeyCode::Char('g') if ctrl_or_alt => Some(AppAction::JumpToAddress),
        KeyCode::Char('g' | 'G') if ctrl_shift || alt_shift => Some(AppAction::JumpToLine),
        KeyCode::Char('n') if ctrl => Some(AppAction::JumpNextUnexplored),
        KeyCode::Char('n' | 'N') if ctrl_shift => Some(AppAction::JumpPrevUnexplored),

        KeyCode::Tab if no_mod => Some(AppAction::CyclePane),
        _ => None,
    }
}
