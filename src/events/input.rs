use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::menu::handle_menu_action;

pub fn handle_global_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Exit);
        }
        KeyCode::F(2) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ViceToggleBreakpoint,
            );
        }
        KeyCode::F(4) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ViceRunToCursor,
            );
        }
        KeyCode::F(7) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::ViceStep);
        }
        KeyCode::F(8) if key.modifiers.is_empty() => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ViceStepOver,
            );
        }
        KeyCode::F(9) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ViceContinue,
            );
        }
        KeyCode::F(10) => {
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
        }
        KeyCode::F(8) if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ViceStepOut,
            );
        }
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
                app_state,
                ui_state,
                crate::ui_state::MenuAction::FindReferences,
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::GoToSymbol);
        }
        // Global Shortcuts
        KeyCode::Char('o') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Open)
        }
        KeyCode::Char('o')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::OpenRecent);
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Analyze);
        }
        KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Save);
        }
        KeyCode::Char('s')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SaveAs);
        }
        KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ExportProject,
            );
        }
        KeyCode::Char('e')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ExportProjectAs,
            );
        }

        KeyCode::Char(',') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::SystemSettings,
            );
        }
        KeyCode::Char('p') if key.modifiers == KeyModifiers::ALT => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::SystemSettings,
            );
        }

        KeyCode::Char('d')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::DocumentSettings,
            );
        }

        KeyCode::Char('u') if key.modifiers.is_empty() => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Undo);
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Redo);
        }
        KeyCode::Char('1')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleDebuggerView,
            );
        }
        KeyCode::Char('2')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleHexDump,
            );
        }
        KeyCode::Char('3')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleSpritesView,
            );
        }
        KeyCode::Char('4')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleCharsetView,
            );
        }
        KeyCode::Char('5')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleBitmapView,
            );
        }
        KeyCode::Char('6')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleBlocksView,
            );
        }
        KeyCode::Char('g')
            if key.modifiers == KeyModifiers::CONTROL || key.modifiers == KeyModifiers::ALT =>
        {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::JumpToAddress,
            );
        }
        KeyCode::Char('g')
            if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT)
                || key.modifiers == (KeyModifiers::ALT | KeyModifiers::SHIFT) =>
        {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::JumpToLine);
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
