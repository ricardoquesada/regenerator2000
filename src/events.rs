use crate::state::AppState;
use crate::ui::ui;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use std::io;

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut state: AppState) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if state.file_picker.active {
                match key.code {
                    KeyCode::Esc => {
                        state.file_picker.close();
                        state.status_message = "Ready".to_string();
                    }
                    KeyCode::Down => state.file_picker.next(),
                    KeyCode::Up => state.file_picker.previous(),
                    KeyCode::Backspace => {
                        // Go to parent dir
                        if let Some(parent) = state
                            .file_picker
                            .current_dir
                            .parent()
                            .map(|p| p.to_path_buf())
                        {
                            state.file_picker.current_dir = parent;
                            state.file_picker.refresh_files();
                            state.file_picker.selected_index = 0;
                        }
                    }
                    KeyCode::Enter => {
                        if !state.file_picker.files.is_empty() {
                            let selected_path =
                                state.file_picker.files[state.file_picker.selected_index].clone();
                            if selected_path.is_dir() {
                                state.file_picker.current_dir = selected_path;
                                state.file_picker.refresh_files();
                                state.file_picker.selected_index = 0;
                            } else {
                                // Load file
                                if let Err(e) = state.load_file(selected_path.clone()) {
                                    state.status_message = format!("Error loading file: {}", e);
                                } else {
                                    state.status_message = format!("Loaded: {:?}", selected_path);
                                    state.file_picker.close();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if state.menu.active {
                match key.code {
                    KeyCode::Esc => {
                        state.menu.active = false;
                        state.menu.selected_item = None;
                        state.status_message = "Ready".to_string();
                    }
                    KeyCode::Right => {
                        state.menu.next_category();
                    }
                    KeyCode::Left => {
                        state.menu.previous_category();
                    }
                    KeyCode::Down => {
                        state.menu.next_item();
                    }
                    KeyCode::Up => {
                        state.menu.previous_item();
                    }
                    KeyCode::Enter => {
                        if let Some(item_idx) = state.menu.selected_item {
                            let category_idx = state.menu.selected_category;
                            let action_name = state.menu.categories[category_idx].items[item_idx]
                                .name
                                .clone();
                            handle_menu_action(&mut state, &action_name);
                            // Start with closing menu after action? Or keep it open?
                            // Usually valid action closes menu.
                            state.menu.active = false;
                            state.menu.selected_item = None;
                        } else {
                            // Enter on category -> open first item?
                            state.menu.selected_item = Some(0);
                        }
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        state.should_quit = true;
                    }
                    KeyCode::F(10) => {
                        state.menu.active = true;
                        state.status_message = "Menu Active".to_string();
                    }
                    // Global Shortcuts
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(&mut state, "New")
                    }
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(&mut state, "Open")
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(&mut state, "Save As");
                        } else {
                            handle_menu_action(&mut state, "Save");
                        }
                    }
                    KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(&mut state, "Redo");
                        } else {
                            handle_menu_action(&mut state, "Undo");
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        handle_menu_action(&mut state, "Zoom In")
                    }
                    KeyCode::Char('-') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(&mut state, "Zoom Out")
                    }
                    KeyCode::Char('0') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(&mut state, "Reset Zoom")
                    }

                    // Normal Navigation
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.cursor_index < state.disassembly.len().saturating_sub(1) {
                            state.cursor_index += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if state.cursor_index > 0 {
                            state.cursor_index -= 1;
                        }
                    }
                    KeyCode::PageDown => {
                        state.cursor_index = (state.cursor_index + 10)
                            .min(state.disassembly.len().saturating_sub(1));
                    }
                    KeyCode::PageUp => {
                        state.cursor_index = state.cursor_index.saturating_sub(10);
                    }
                    KeyCode::Home => {
                        state.cursor_index = 0;
                    }
                    KeyCode::End => {
                        state.cursor_index = state.disassembly.len().saturating_sub(1);
                    }
                    _ => {}
                }
            }

            if state.should_quit {
                return Ok(());
            }
        }
    }
}

fn handle_menu_action(state: &mut AppState, action: &str) {
    state.status_message = format!("Action: {}", action);
    match action {
        "Exit" => state.should_quit = true,
        "New" => {
            // Placeholder
        }
        "Open" => {
            state.file_picker.open();
            state.status_message = "Select a file to open".to_string();
        }
        "Save" => {
            // Placeholder
        }
        "Save As" => {
            // Placeholder
        }
        "Undo" => {}
        "Redo" => {}
        "Zoom In" => {}
        "Zoom Out" => {}
        "Reset Zoom" => {}
        _ => {}
    }
}
