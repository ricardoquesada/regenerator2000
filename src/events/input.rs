use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regenerator_core::Core;

use crate::ui::menu::handle_menu_action;

pub fn handle_global_input(key: KeyEvent, core: &mut Core, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Exit);
        }

        // VICE Debugger begin
        // VICE keyboard shortcuts are global, at least the "F" keys, since they can be used from different views.
        KeyCode::F(2) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceToggleBreakpoint,
            );
        }
        KeyCode::F(2) if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceBreakpointDialog,
            );
        }
        KeyCode::F(6) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceToggleWatchpoint,
            );
        }
        KeyCode::F(4) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceRunToCursor,
            );
        }
        KeyCode::F(7) if key.modifiers.is_empty() => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::ViceStep);
        }
        KeyCode::F(8) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceStepOver,
            );
        }
        KeyCode::F(8) if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceStepOut,
            );
        }
        KeyCode::F(9) if key.modifiers.is_empty() => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ViceContinue,
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
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::GoToSymbol);
        }
        // Global Shortcuts
        KeyCode::Char('o') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Open);
        }
        KeyCode::Char('o')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::OpenRecent);
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Analyze);
        }
        KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Save);
        }
        KeyCode::Char('s')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::SaveAs);
        }
        KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ExportProject,
            );
        }
        KeyCode::Char('e')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ExportProjectAs,
            );
        }

        KeyCode::Char(',') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::SystemSettings,
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::ALT => {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::SystemSettings,
            );
        }

        KeyCode::Char('d')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::DocumentSettings,
            );
        }

        KeyCode::Char('u') if key.modifiers.is_empty() => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Undo);
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::Redo);
        }
        KeyCode::Char('1')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleBlocksView,
            );
        }
        KeyCode::Char('2')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleHexDump,
            );
        }
        KeyCode::Char('3')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleSpritesView,
            );
        }
        KeyCode::Char('4')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleCharsetView,
            );
        }
        KeyCode::Char('5')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleBitmapView,
            );
        }
        KeyCode::Char('6')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::ToggleDebuggerView,
            );
        }
        KeyCode::Char('g')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                core,
                ui_state,
                crate::state::actions::AppAction::JumpToAddress,
            );
        }
        KeyCode::Char('g')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) =>
        {
            handle_menu_action(core, ui_state, crate::state::actions::AppAction::JumpToLine);
        }
        KeyCode::Tab => {
            ui_state.active_pane = match ui_state.active_pane {
                ActivePane::Disassembly => match ui_state.right_pane {
                    crate::ui_state::RightPane::None => ActivePane::Disassembly,
                    crate::ui_state::RightPane::HexDump => ActivePane::HexDump,
                    crate::ui_state::RightPane::Sprites => ActivePane::Sprites,
                    crate::ui_state::RightPane::Charset => ActivePane::Charset,
                    crate::ui_state::RightPane::Bitmap => ActivePane::Bitmap,
                    crate::ui_state::RightPane::Blocks => ActivePane::Blocks,
                    crate::ui_state::RightPane::Debugger => ActivePane::Debugger,
                },
                ActivePane::HexDump
                | ActivePane::Sprites
                | ActivePane::Charset
                | ActivePane::Bitmap
                | ActivePane::Blocks
                | ActivePane::Debugger => ActivePane::Disassembly,
            };
        }
        KeyCode::Esc => {
            ui_state.input_buffer.clear();
            if ui_state.is_visual_mode {
                ui_state.is_visual_mode = false;
                ui_state.selection_start = None;
                ui_state.set_status_message("Visual Mode Exited");
            } else if ui_state.selection_start.is_some() {
                ui_state.selection_start = None;
                ui_state.set_status_message("Selection cleared");
            }
        }
        _ => {}
    }
}
