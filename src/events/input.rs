use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::menu::handle_menu_action;

pub fn handle_global_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Exit);
        }
        KeyCode::F(10) => {
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
        }
        // Global Shortcuts
        KeyCode::Char('o') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Open)
        }
        KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Analyze);
        }
        KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Save);
        }
        KeyCode::Char('S') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SaveAs);
        }
        KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ExportProject,
            );
        }
        KeyCode::Char('E') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
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

        KeyCode::Char('u') if key.modifiers.is_empty() => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Undo);
        }
        KeyCode::Char('r') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Redo);
        }
        KeyCode::Char('2') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleHexDump,
            );
        }
        KeyCode::Char('3') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleSpritesView,
            );
        }
        KeyCode::Char('4') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleCharsetView,
            );
        }
        KeyCode::Char('5') if key.modifiers == KeyModifiers::CONTROL => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleBlocksView,
            );
        }
        KeyCode::Char('g') if key.modifiers.is_empty() => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::JumpToAddress,
            );
        }
        KeyCode::Char('G') if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::JumpToLine);
        }

        // Input Buffer for Numbers
        KeyCode::Char(c)
            if c.is_ascii_digit()
                && !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
        {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::HexDump
                || ui_state.active_pane == ActivePane::Sprites
                || ui_state.active_pane == ActivePane::Charset
                || ui_state.active_pane == ActivePane::Blocks
            {
                // Only append if it's a valid number sequence (avoid overflow though usize is large)
                if ui_state.input_buffer.len() < 10 {
                    ui_state.input_buffer.push(c);
                    ui_state.set_status_message(format!(":{}", ui_state.input_buffer));
                }
            }
        }

        KeyCode::Tab => {
            ui_state.active_pane = match ui_state.active_pane {
                ActivePane::Disassembly => match ui_state.right_pane {
                    crate::ui_state::RightPane::None => ActivePane::Disassembly,
                    crate::ui_state::RightPane::HexDump => ActivePane::HexDump,
                    crate::ui_state::RightPane::Sprites => ActivePane::Sprites,
                    crate::ui_state::RightPane::Charset => ActivePane::Charset,
                    crate::ui_state::RightPane::Blocks => ActivePane::Blocks,
                },
                ActivePane::HexDump
                | ActivePane::Sprites
                | ActivePane::Charset
                | ActivePane::Blocks => ActivePane::Disassembly,
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
