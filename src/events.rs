pub mod input;
pub mod menu;

use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{Terminal, backend::Backend};
use std::io;

use input::handle_global_input;
use menu::{execute_menu_action, handle_menu_action};

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app_state: AppState,
    mut ui_state: UIState,
) -> io::Result<()> {
    loop {
        // Update menu availability based on current state
        ui_state.menu.update_availability(
            &app_state,
            ui_state.cursor_index,
            ui_state.search_dialog.last_search.is_empty(),
            ui_state.active_pane,
        );

        if ui_state.active_pane == ActivePane::Disassembly
            && ui_state.right_pane == crate::ui_state::RightPane::Blocks
            && app_state.system_config.sync_blocks_view
            && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
            && let Some(idx) = app_state.get_block_index_for_address(line.address)
        {
            ui_state.blocks_list_state.select(Some(idx));
        }

        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .map_err(|e| io::Error::other(e.to_string()))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            ui_state.dismiss_logo = true;
            if ui_state.jump_to_address_dialog.active {
                crate::dialog_jump_to_address::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.jump_to_line_dialog.active {
                crate::dialog_jump_to_line::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.save_as_dialog.active {
                crate::dialog_save_as::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.export_as_dialog.active {
                crate::dialog_export_as::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.label_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.label_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        // Get address from dialog state
                        if let Some(address) = ui_state.label_dialog.address {
                            let label_name = ui_state.label_dialog.input.trim().to_string();

                            if label_name.is_empty() {
                                // Remove label
                                let old_label = app_state.labels.get(&address).cloned();

                                let command = crate::commands::Command::SetLabel {
                                    address,
                                    new_label: None,
                                    old_label,
                                };

                                command.apply(&mut app_state);
                                app_state.push_command(command);

                                ui_state.set_status_message("Label removed");
                                app_state.disassemble();
                                ui_state.label_dialog.close();
                            } else {
                                // Check for duplicates (exclude current address in case of rename/edit)
                                let exists = app_state.labels.iter().any(|(addr, label_vec)| {
                                    *addr != address
                                        && label_vec.iter().any(|l| l.name == label_name)
                                });

                                if exists {
                                    ui_state.set_status_message(format!(
                                        "Error: Label '{}' already exists",
                                        label_name
                                    ));
                                    // Do not close dialog, let user correct it
                                } else {
                                    let old_label_vec = app_state.labels.get(&address).cloned();

                                    let mut new_label_vec =
                                        old_label_vec.clone().unwrap_or_default();

                                    let new_label_entry = crate::state::Label {
                                        name: label_name,
                                        kind: crate::state::LabelKind::User,
                                        label_type: crate::state::LabelType::UserDefined,
                                    };

                                    // If vector has items, we assume we are editing the first one (as that's what we showed).
                                    // If we want to SUPPORT multiple, we need a better UI.
                                    // For now, replace 0 or push.
                                    if !new_label_vec.is_empty() {
                                        new_label_vec[0] = new_label_entry;
                                    } else {
                                        new_label_vec.push(new_label_entry);
                                    }

                                    let command = crate::commands::Command::SetLabel {
                                        address,
                                        new_label: Some(new_label_vec),
                                        old_label: old_label_vec,
                                    };

                                    command.apply(&mut app_state);
                                    app_state.push_command(command);

                                    ui_state.set_status_message("Label set");
                                    app_state.disassemble();
                                    ui_state.label_dialog.close();
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.label_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        ui_state.label_dialog.input.push(c);
                    }
                    _ => {}
                }
            } else if ui_state.comment_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.comment_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                            let address = line.comment_address.unwrap_or(line.address);
                            let new_comment = ui_state.comment_dialog.input.trim().to_string();
                            let new_comment_opt = if new_comment.is_empty() {
                                None
                            } else {
                                Some(new_comment)
                            };

                            let command = match ui_state.comment_dialog.comment_type {
                                crate::dialog_comment::CommentType::Side => {
                                    let old_comment =
                                        app_state.user_side_comments.get(&address).cloned();
                                    crate::commands::Command::SetUserSideComment {
                                        address,
                                        new_comment: new_comment_opt,
                                        old_comment,
                                    }
                                }
                                crate::dialog_comment::CommentType::Line => {
                                    let old_comment =
                                        app_state.user_line_comments.get(&address).cloned();
                                    crate::commands::Command::SetUserLineComment {
                                        address,
                                        new_comment: new_comment_opt,
                                        old_comment,
                                    }
                                }
                            };

                            command.apply(&mut app_state);
                            app_state.push_command(command);

                            ui_state.set_status_message("Comment set");
                            app_state.disassemble();
                            ui_state.comment_dialog.close();
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.comment_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        ui_state.comment_dialog.input.push(c);
                    }
                    _ => {}
                }
            } else if ui_state.open_dialog.active {
                crate::dialog_open::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.search_dialog.active {
                crate::dialog_search::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.menu.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.menu.active = false;
                        ui_state.menu.selected_item = None;
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Right => {
                        ui_state.menu.next_category();
                    }
                    KeyCode::Left => {
                        ui_state.menu.previous_category();
                    }
                    KeyCode::Down => {
                        ui_state.menu.next_item();
                    }
                    KeyCode::Up => {
                        ui_state.menu.previous_item();
                    }
                    KeyCode::Enter => {
                        if let Some(item_idx) = ui_state.menu.selected_item {
                            let category_idx = ui_state.menu.selected_category;
                            let item = &ui_state.menu.categories[category_idx].items[item_idx];

                            if !item.disabled {
                                let action = item.action.clone();
                                if let Some(action) = action {
                                    handle_menu_action(&mut app_state, &mut ui_state, action);
                                    // Close menu after valid action
                                    ui_state.menu.active = false;
                                    ui_state.menu.selected_item = None;
                                }
                            } else {
                                // Optional: Feedback that it's disabled
                                ui_state.set_status_message("Item is disabled");
                            }
                        } else {
                            // Enter on category -> open first item?
                            // ui_state.menu.selected_item = Some(0);
                            ui_state.menu.select_first_enabled_item();
                        }
                    }
                    _ => {}
                }
            } else if ui_state.about_dialog.active {
                crate::dialog_about::handle_input(key, &mut ui_state);
            } else if ui_state.shortcuts_dialog.active {
                crate::dialog_keyboard_shortcut::handle_input(key, &mut ui_state);
            } else if ui_state.confirmation_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.confirmation_dialog.close();
                        ui_state.set_status_message("Action cancelled");
                    }
                    KeyCode::Enter | KeyCode::Char('y') => {
                        if let Some(action) = ui_state.confirmation_dialog.action_on_confirm.take()
                        {
                            ui_state.confirmation_dialog.close();
                            // Use the local helper that delegates to menu module
                            execute_menu_action(&mut app_state, &mut ui_state, action);
                        }
                    }
                    KeyCode::Char('n') => {
                        ui_state.confirmation_dialog.close();
                        ui_state.set_status_message("Action cancelled");
                    }
                    _ => {}
                }
            } else if ui_state.settings_dialog.active {
                crate::dialog_document_settings::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.system_settings_dialog.active {
                crate::dialog_settings::handle_input(key, &mut app_state, &mut ui_state);
            } else if ui_state.origin_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.origin_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        if let Ok(new_origin) =
                            u16::from_str_radix(&ui_state.origin_dialog.input, 16)
                        {
                            let size = app_state.raw_data.len();
                            // Check for overflow
                            if (new_origin as usize) + size <= 0x10000 {
                                let old_origin = app_state.origin;
                                let command = crate::commands::Command::ChangeOrigin {
                                    new_origin,
                                    old_origin,
                                };
                                command.apply(&mut app_state);
                                app_state.push_command(command);

                                app_state.disassemble();
                                ui_state.set_status_message(format!(
                                    "Origin changed to ${:04X}",
                                    new_origin
                                ));
                                ui_state.origin_dialog.close();
                            } else {
                                ui_state.set_status_message("Error: Origin + Size exceeds $FFFF");
                            }
                        } else {
                            ui_state.set_status_message("Invalid Hex Address");
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.origin_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        if c.is_ascii_hexdigit() && ui_state.origin_dialog.input.len() < 4 {
                            ui_state.origin_dialog.input.push(c.to_ascii_uppercase());
                        }
                    }
                    _ => {}
                }
            } else if ui_state.vim_search_active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.vim_search_active = false;
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        ui_state.search_dialog.last_search = ui_state.vim_search_input.clone();
                        ui_state.vim_search_active = false;
                        crate::dialog_search::perform_search(&mut app_state, &mut ui_state, true);
                    }
                    KeyCode::Backspace => {
                        ui_state.vim_search_input.pop();
                    }
                    KeyCode::Char(c) => {
                        ui_state.vim_search_input.push(c);
                    }
                    _ => {}
                }
            } else {
                if ui_state.active_pane == ActivePane::Disassembly {
                    use crate::view_disassembly::InputResult;
                    match crate::view_disassembly::handle_input(key, &mut app_state, &mut ui_state)
                    {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            handle_menu_action(&mut app_state, &mut ui_state, action);
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::HexDump {
                    use crate::view_hexdump::InputResult;
                    match crate::view_hexdump::handle_input(key, &mut app_state, &mut ui_state) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            handle_menu_action(&mut app_state, &mut ui_state, action);
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Sprites {
                    use crate::view_sprites::InputResult;
                    match crate::view_sprites::handle_input(key, &mut app_state, &mut ui_state) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            handle_menu_action(&mut app_state, &mut ui_state, action);
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Charset {
                    use crate::view_charset::InputResult;
                    match crate::view_charset::handle_input(key, &mut app_state, &mut ui_state) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            handle_menu_action(&mut app_state, &mut ui_state, action);
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                if ui_state.active_pane == ActivePane::Blocks {
                    use crate::view_blocks::InputResult;
                    match crate::view_blocks::handle_input(key, &mut app_state, &mut ui_state) {
                        InputResult::Handled => continue,
                        InputResult::Action(action) => {
                            handle_menu_action(&mut app_state, &mut ui_state, action);
                            continue;
                        }
                        InputResult::Ignored => {}
                    }
                }

                handle_global_input(key, &mut app_state, &mut ui_state);
            }

            if ui_state.should_quit {
                return Ok(());
            }
        }
    }
}
