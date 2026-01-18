use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use std::io;

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

                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Exit,
                        );
                    }
                    KeyCode::Char('/') if key.modifiers.is_empty() => {
                        ui_state.vim_search_active = true;
                        ui_state.vim_search_input.clear();
                    }
                    KeyCode::Char('n') if key.modifiers.is_empty() => {
                        crate::dialog_search::perform_search(&mut app_state, &mut ui_state, true);
                    }
                    KeyCode::Char('N')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        crate::dialog_search::perform_search(&mut app_state, &mut ui_state, false);
                    }
                    KeyCode::F(10) => {
                        ui_state.menu.active = true;
                        ui_state.menu.select_first_enabled_item();
                        ui_state.set_status_message("Menu Active");
                    }
                    // Global Shortcuts
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Open,
                        )
                    }
                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Search,
                        );
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Analyze,
                        );
                    }
                    KeyCode::F(3) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::FindPrevious,
                            );
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::FindNext,
                            );
                        }
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SaveAs,
                            );
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Save,
                            );
                        }
                    }
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ExportProjectAs,
                            );
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ExportProject,
                            );
                        }
                    }

                    KeyCode::Char(',') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::SystemSettings,
                        );
                    }

                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                            }
                            ActivePane::HexDump => {
                                ui_state.hex_cursor_index =
                                    ui_state.hex_cursor_index.saturating_sub(10);
                            }
                            ActivePane::Sprites => {
                                ui_state.sprites_cursor_index =
                                    ui_state.sprites_cursor_index.saturating_sub(10);
                            }
                            ActivePane::Charset => {}
                            ActivePane::Blocks => {
                                ui_state.blocks_list_state.select(Some(
                                    ui_state
                                        .blocks_list_state
                                        .selected()
                                        .unwrap_or(0)
                                        .saturating_sub(10),
                                ));
                            }
                        }
                    }

                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::DocumentSettings,
                            );
                        } else {
                            match ui_state.active_pane {
                                ActivePane::Disassembly => {
                                    ui_state.cursor_index = (ui_state.cursor_index + 10)
                                        .min(app_state.disassembly.len().saturating_sub(1));
                                }

                                ActivePane::HexDump => {}
                                ActivePane::Sprites => {}
                                ActivePane::Charset => {}
                                ActivePane::Blocks => {}
                            }
                        }
                    }

                    KeyCode::Char('u') if key.modifiers.is_empty() => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Undo,
                        );
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Redo,
                        );
                    }
                    KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ToggleHexDump,
                        );
                    }
                    KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ToggleSpritesView,
                        );
                    }
                    KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ToggleCharsetView,
                        );
                    }
                    KeyCode::Char('5') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ToggleBlocksView,
                        );
                    }

                    KeyCode::Char('g') => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                handle_menu_action(
                                    &mut app_state,
                                    &mut ui_state,
                                    crate::ui_state::MenuAction::JumpToLine,
                                );
                            }
                        } else if key.modifiers.is_empty() {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::JumpToAddress,
                            );
                        }
                    }

                    KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
                        // Vim-like G command (Shift+g)
                        let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
                        let is_buffer_empty = ui_state.input_buffer.is_empty();
                        ui_state.input_buffer.clear();

                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                let target_line = if is_buffer_empty {
                                    app_state.disassembly.len()
                                } else {
                                    entered_number
                                };

                                let new_cursor = if target_line == 0 {
                                    app_state.disassembly.len().saturating_sub(1)
                                } else {
                                    target_line
                                        .saturating_sub(1)
                                        .min(app_state.disassembly.len().saturating_sub(1))
                                };

                                // Handle Visual Mode
                                if ui_state.is_visual_mode && ui_state.selection_start.is_none() {
                                    ui_state.selection_start = Some(ui_state.cursor_index);
                                }

                                ui_state
                                    .navigation_history
                                    .push((ui_state.active_pane, ui_state.cursor_index));
                                ui_state.cursor_index = new_cursor;
                                ui_state
                                    .set_status_message(format!("Jumped to line {}", target_line));
                            }

                            ActivePane::HexDump => {}
                            ActivePane::Sprites => {}
                            ActivePane::Charset => {}
                            ActivePane::Blocks => {}
                        }
                    }

                    // Only handle Enter for Jump to Operand if NO modifiers (to avoid conflict)
                    KeyCode::Enter if key.modifiers.is_empty() => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::JumpToOperand,
                        );
                    }

                    KeyCode::Backspace => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            // Pop until we find a Disassembly entry or run out of history
                            while let Some((pane, _)) = ui_state.navigation_history.last() {
                                if *pane != ActivePane::Disassembly {
                                    ui_state.navigation_history.pop();
                                } else {
                                    break;
                                }
                            }

                            if let Some((pane, idx)) = ui_state.navigation_history.pop() {
                                // Double check it is Disassembly (should be guaranteed by loop above)
                                if pane == ActivePane::Disassembly {
                                    if idx < app_state.disassembly.len() {
                                        ui_state.cursor_index = idx;
                                        ui_state.active_pane = ActivePane::Disassembly; // Ensure focus remains
                                        ui_state.set_status_message("Navigated back");
                                    } else {
                                        ui_state.set_status_message("History invalid");
                                    }
                                }
                            } else {
                                ui_state.set_status_message("No history");
                            }
                        }
                    }

                    // Data Conversion Shortcuts
                    KeyCode::Char('c') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Code,
                            )
                        }
                    }
                    KeyCode::Char('b') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Byte,
                            )
                        }
                    }
                    KeyCode::Char('w') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Word,
                            )
                        }
                    }
                    KeyCode::Char('a') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Address,
                            )
                        }
                    }
                    KeyCode::Char('t') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Text,
                            )
                        }
                    }
                    KeyCode::Char('s') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Screencode,
                            )
                        }
                    }
                    // p moved to m
                    KeyCode::Char('?')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Undefined,
                            )
                        }
                    }
                    // Handle Shift+/ as ?
                    KeyCode::Char('?') if key.modifiers == KeyModifiers::SHIFT => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Undefined,
                            )
                        }
                    }
                    KeyCode::Char('<')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetLoHi,
                            )
                        }
                    }
                    // Handle Shift+, as <
                    KeyCode::Char('<') if key.modifiers == KeyModifiers::SHIFT => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetLoHi,
                            )
                        }
                    }
                    KeyCode::Char('>')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetHiLo,
                            )
                        }
                    }
                    // Handle Shift+. as >
                    KeyCode::Char('>') if key.modifiers == KeyModifiers::SHIFT => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetHiLo,
                            )
                        }
                    }
                    KeyCode::Char(';') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SideComment,
                            )
                        }
                    }
                    // Handle Shift+\ as |
                    KeyCode::Char('|') if key.modifiers == KeyModifiers::SHIFT => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ToggleSplitter,
                            );
                        }
                    }
                    KeyCode::Char('|')
                        if !key.modifiers.intersects(
                            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                        ) =>
                    {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::Blocks
                        {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ToggleSplitter,
                            );
                        } else {
                            ui_state.set_status_message("No open document");
                        }
                    }

                    KeyCode::Char(':')
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::LineComment,
                            )
                        }
                    }
                    // Handle Shift+; as :
                    KeyCode::Char(':') if key.modifiers == KeyModifiers::SHIFT => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::LineComment,
                            )
                        }
                    }

                    // Label
                    KeyCode::Char('l') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Charset {
                            let origin = app_state.origin as usize;
                            let base_alignment = 0x400;
                            let aligned_start_addr = (origin / base_alignment) * base_alignment;
                            let end_addr = origin + app_state.raw_data.len();
                            let max_char_index =
                                (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                            if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                                ui_state.charset_cursor_index += 1;
                            }
                        } else if !app_state.raw_data.is_empty() {
                            if !ui_state.menu.active
                                && !ui_state.jump_to_address_dialog.active
                                && !ui_state.jump_to_line_dialog.active
                                && !ui_state.save_as_dialog.active
                                && !ui_state.export_as_dialog.active
                                && !ui_state.open_dialog.active
                                && ui_state.active_pane == ActivePane::Disassembly
                                && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
                            {
                                let mut target_addr = line.address;
                                let mut current_sub_index = 0;
                                let mut found = false;

                                // Check relative labels (to match UI rendering)
                                if line.bytes.len() > 1 {
                                    for offset in 1..line.bytes.len() {
                                        let mid_addr = line.address.wrapping_add(offset as u16);
                                        if let Some(labels) = app_state.labels.get(&mid_addr) {
                                            for _label in labels {
                                                if current_sub_index == ui_state.sub_cursor_index {
                                                    target_addr = mid_addr;
                                                    found = true;
                                                    break;
                                                }
                                                current_sub_index += 1;
                                            }
                                        }
                                        if found {
                                            break;
                                        }
                                    }
                                }

                                // Check line comment if not found
                                if !found && line.line_comment.is_some() {
                                    // Line comments are associated with the main address line visually,
                                    // but occupy a sub-index.
                                    // current_sub_index += 1; // Unused
                                }

                                // If we haven't found a relative label match, target_addr remains line.address,
                                // which is correct for both the Line Comment and the Main Line.

                                let text = app_state
                                    .labels
                                    .get(&target_addr)
                                    .and_then(|v| v.first())
                                    .map(|l| l.name.as_str());
                                ui_state.label_dialog.open(text, target_addr);
                                ui_state.set_status_message("Enter Label");
                            }
                        } else if ui_state.active_pane == ActivePane::Disassembly {
                            ui_state.set_status_message("No open document");
                        }
                    }

                    // Visual Mode Toggle
                    KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                        if !app_state.raw_data.is_empty() {
                            if ui_state.active_pane == ActivePane::Disassembly {
                                ui_state.is_visual_mode = !ui_state.is_visual_mode;
                                if ui_state.is_visual_mode {
                                    if ui_state.selection_start.is_none() {
                                        ui_state.selection_start = Some(ui_state.cursor_index);
                                    }
                                    ui_state.set_status_message("Visual Mode");
                                } else {
                                    ui_state.selection_start = None;
                                    ui_state.set_status_message("Visual Mode Exited");
                                }
                            }
                        } else if ui_state.active_pane == ActivePane::Disassembly {
                            ui_state.set_status_message("No open document");
                        }
                    }

                    KeyCode::Char('d') if key.modifiers.is_empty() => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::NextImmediateFormat,
                        );
                    }

                    // Previous Immediate Format (Shift+d)
                    KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::PreviousImmediateFormat,
                        );
                    }

                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ToggleCollapsedBlock,
                        );
                    }

                    // External File
                    KeyCode::Char('e') if key.modifiers.is_empty() => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::SetExternalFile,
                        );
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

                    // Normal Navigation
                    KeyCode::Down | KeyCode::Char('j')
                        if key.code == KeyCode::Down || key.modifiers.is_empty() =>
                    {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Blocks => {}
                            ActivePane::Disassembly => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || ui_state.is_visual_mode
                                {
                                    if ui_state.selection_start.is_none() {
                                        ui_state.selection_start = Some(ui_state.cursor_index);
                                    }
                                } else {
                                    ui_state.selection_start = None;
                                }

                                let line = &app_state.disassembly[ui_state.cursor_index];
                                let mut sub_count = 1; // Main line

                                // Add line comment if it exists (rendered above)
                                if app_state.user_line_comments.contains_key(&line.address) {
                                    sub_count += 1;
                                }

                                // Add relative labels (rendered above instruction)
                                if line.bytes.len() > 1 {
                                    for offset in 1..line.bytes.len() {
                                        let mid_addr = line.address.wrapping_add(offset as u16);
                                        if let Some(labels) = app_state.labels.get(&mid_addr) {
                                            sub_count += labels.len();
                                        }
                                    }
                                }

                                if ui_state.sub_cursor_index < sub_count - 1 {
                                    ui_state.sub_cursor_index += 1;
                                } else if ui_state.cursor_index
                                    < app_state.disassembly.len().saturating_sub(1)
                                {
                                    ui_state.cursor_index += 1;
                                    ui_state.sub_cursor_index = 0;
                                }
                            }
                            ActivePane::Sprites => {}
                            ActivePane::Charset => {}
                            ActivePane::HexDump => {}
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k')
                        if key.code == KeyCode::Up || key.modifiers.is_empty() =>
                    {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || ui_state.is_visual_mode
                                {
                                    if ui_state.selection_start.is_none() {
                                        ui_state.selection_start = Some(ui_state.cursor_index);
                                    }
                                } else {
                                    ui_state.selection_start = None;
                                }

                                if ui_state.sub_cursor_index > 0 {
                                    ui_state.sub_cursor_index -= 1;
                                } else if ui_state.cursor_index > 0 {
                                    ui_state.cursor_index -= 1;
                                    // Calculate max sub_index for the new line
                                    let line = &app_state.disassembly[ui_state.cursor_index];
                                    let mut sub_count = 1; // Main line

                                    // Add line comment if it exists (rendered above)
                                    if app_state.user_line_comments.contains_key(&line.address) {
                                        sub_count += 1;
                                    }

                                    // Add relative labels (rendered above instruction)
                                    if line.bytes.len() > 1 {
                                        for offset in 1..line.bytes.len() {
                                            let mid_addr = line.address.wrapping_add(offset as u16);
                                            if let Some(labels) = app_state.labels.get(&mid_addr) {
                                                sub_count += labels.len();
                                            }
                                        }
                                    }
                                    ui_state.sub_cursor_index = sub_count - 1;
                                }
                            }
                            ActivePane::HexDump => {}
                            ActivePane::Sprites => {}
                            ActivePane::Charset => {}
                            ActivePane::Blocks => {}
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
                    KeyCode::PageDown => {
                        ui_state.input_buffer.clear();
                        if ui_state.active_pane == ActivePane::Disassembly {
                            ui_state.cursor_index = (ui_state.cursor_index + 30)
                                .min(app_state.disassembly.len().saturating_sub(1));
                        } else if ui_state.active_pane == ActivePane::Blocks {
                            ui_state.blocks_list_state.select(Some(
                                ui_state
                                    .blocks_list_state
                                    .selected()
                                    .unwrap_or(0)
                                    .saturating_add(30)
                                    .min(app_state.get_compressed_blocks().len().saturating_sub(1)),
                            ));
                        } else if ui_state.active_pane == ActivePane::HexDump {
                        } else if ui_state.active_pane == ActivePane::Sprites {
                            let origin = app_state.origin as usize;
                            let padding = (64 - (origin % 64)) % 64;
                            let usable_len = app_state.raw_data.len().saturating_sub(padding);
                            let total_sprites = usable_len.div_ceil(64);
                            ui_state.sprites_cursor_index = (ui_state.sprites_cursor_index + 10)
                                .min(total_sprites.saturating_sub(1));
                        } else if ui_state.active_pane == ActivePane::Charset {
                            let origin = app_state.origin as usize;
                            let base_alignment = 0x400;
                            let aligned_start_addr = (origin / base_alignment) * base_alignment;
                            let end_addr = origin + app_state.raw_data.len();
                            let max_char_index =
                                (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                            ui_state.charset_cursor_index = (ui_state.charset_cursor_index + 256)
                                .min(max_char_index.saturating_sub(1));
                        }
                    }
                    KeyCode::PageUp => {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                            }
                            ActivePane::HexDump => {
                                ui_state.hex_cursor_index =
                                    ui_state.hex_cursor_index.saturating_sub(10);
                            }
                            ActivePane::Sprites => {
                                ui_state.sprites_cursor_index =
                                    ui_state.sprites_cursor_index.saturating_sub(10);
                            }
                            ActivePane::Charset => {
                                ui_state.charset_cursor_index =
                                    ui_state.charset_cursor_index.saturating_sub(256);
                            }
                            ActivePane::Blocks => {}
                        }
                    }
                    KeyCode::Home => {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => ui_state.cursor_index = 0,
                            ActivePane::HexDump => {}
                            ActivePane::Sprites => {}
                            ActivePane::Charset => {}
                            ActivePane::Blocks => {}
                        }
                    }
                    KeyCode::End => {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index =
                                    app_state.disassembly.len().saturating_sub(1)
                            }
                            ActivePane::Blocks => {}
                            ActivePane::HexDump => {}
                            ActivePane::Sprites => {}
                            ActivePane::Charset => {}
                        }
                    }
                    KeyCode::Char('m') if key.modifiers.is_empty() => {
                        if ui_state.active_pane == ActivePane::Sprites {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ToggleSpriteMulticolor,
                            )
                        } else if ui_state.active_pane == ActivePane::Charset {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ToggleCharsetMulticolor,
                            )
                        }
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if ui_state.active_pane == ActivePane::Charset
                            && ui_state.charset_cursor_index > 0
                        {
                            ui_state.charset_cursor_index -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if ui_state.active_pane == ActivePane::Charset {
                            let origin = app_state.origin as usize;
                            let base_alignment = 0x400;
                            let aligned_start_addr = (origin / base_alignment) * base_alignment;
                            let end_addr = origin + app_state.raw_data.len();
                            let max_char_index =
                                (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                            if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                                ui_state.charset_cursor_index += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if ui_state.should_quit {
                return Ok(());
            }
        }
    }
}

fn handle_menu_action(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    action: crate::ui_state::MenuAction,
) {
    if action.requires_document() && app_state.raw_data.is_empty() {
        ui_state.set_status_message("No open document");
        return;
    }

    // Check for changes on destructive actions
    let is_destructive = matches!(
        action,
        crate::ui_state::MenuAction::Exit | crate::ui_state::MenuAction::Open
    );

    if is_destructive && app_state.is_dirty() {
        ui_state.confirmation_dialog.open(
            "Unsaved Changes",
            "You have unsaved changes. Proceed?",
            action,
        );
        return;
    }

    execute_menu_action(app_state, ui_state, action);
}

fn execute_menu_action(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    action: crate::ui_state::MenuAction,
) {
    ui_state.set_status_message(format!("Action: {:?}", action));

    use crate::ui_state::MenuAction;

    match action {
        MenuAction::Exit => ui_state.should_quit = true,

        MenuAction::Open => {
            ui_state.open_dialog.open();
            ui_state.set_status_message("Select a file to open");
        }
        MenuAction::Save => {
            if app_state.project_path.is_some() {
                let cursor_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|l| l.address);

                // Calculate hex cursor address
                let hex_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let row_start_offset = ui_state.hex_cursor_index * 16;
                    let addr = aligned_origin + row_start_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                // Calculate sprites cursor address
                let sprites_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    let sprite_offset = ui_state.sprites_cursor_index * 64;
                    let addr = origin + padding + sprite_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                let charset_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    let char_offset = ui_state.charset_cursor_index * 8;
                    let addr = aligned_start_addr + char_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                let right_pane_str = format!("{:?}", ui_state.right_pane);

                if let Err(e) = app_state.save_project(
                    crate::state::ProjectSaveContext {
                        cursor_address: cursor_addr,
                        hex_dump_cursor_address: hex_addr,
                        sprites_cursor_address: sprites_addr,
                        right_pane_visible: Some(right_pane_str),
                        charset_cursor_address: charset_addr,
                        sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
                        charset_multicolor_mode: ui_state.charset_multicolor_mode,
                        petscii_mode: ui_state.petscii_mode,
                        splitters: app_state.splitters.clone(),
                        blocks_view_cursor: ui_state.blocks_list_state.selected(),
                    },
                    true,
                ) {
                    ui_state.set_status_message(format!("Error saving: {}", e));
                } else {
                    ui_state.set_status_message("Project saved");
                }
            } else {
                ui_state.save_as_dialog.open();
                ui_state.set_status_message("Enter Project filename");
            }
        }
        MenuAction::SaveAs => {
            ui_state.save_as_dialog.open();
            ui_state.set_status_message("Enter Project filename");
        }
        MenuAction::ExportProject => {
            if let Some(path) = &app_state.export_path {
                if let Err(e) = crate::exporter::export_asm(app_state, path) {
                    ui_state.set_status_message(format!("Error exporting: {}", e));
                } else {
                    ui_state.set_status_message("Project Exported");
                }
            } else {
                ui_state.export_as_dialog.open();
                ui_state.set_status_message("Enter .asm filename");
            }
        }
        MenuAction::ExportProjectAs => {
            ui_state.export_as_dialog.open();
            ui_state.set_status_message("Enter .asm filename");
        }
        MenuAction::DocumentSettings => {
            ui_state.settings_dialog.open();
            ui_state.set_status_message("Document Settings");
        }
        MenuAction::Analyze => {
            // Capture current address
            let current_addr = app_state
                .disassembly
                .get(ui_state.cursor_index)
                .map(|l| l.address);

            ui_state.set_status_message(app_state.perform_analysis());

            // Restore cursor
            if let Some(addr) = current_addr {
                if let Some(idx) = app_state.get_line_index_containing_address(addr) {
                    ui_state.cursor_index = idx;
                } else if let Some(idx) = app_state.get_line_index_for_address(addr) {
                    // Fallback
                    ui_state.cursor_index = idx;
                } else {
                    // Fallback to origin if address lost
                    if let Some(idx) = app_state.get_line_index_for_address(app_state.origin) {
                        ui_state.cursor_index = idx;
                    }
                }
            } else {
                // If we didn't have a valid cursor (empty?), go to origin
                if let Some(idx) = app_state.get_line_index_for_address(app_state.origin) {
                    ui_state.cursor_index = idx;
                }
            }
        }
        MenuAction::Undo => {
            ui_state.set_status_message(app_state.undo_last_command());
        }
        MenuAction::Redo => {
            ui_state.set_status_message(app_state.redo_last_command());
        }

        MenuAction::Code => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::Code,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Code");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(crate::state::BlockType::Code, Some(start), end);
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::Code,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Byte => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::DataByte,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Byte");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(
                    crate::state::BlockType::DataByte,
                    Some(start),
                    end,
                );
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::DataByte,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Word => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::DataWord,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Word");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(
                    crate::state::BlockType::DataWord,
                    Some(start),
                    end,
                );
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::DataWord,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::SetExternalFile => {
            if ui_state.active_pane == ActivePane::Blocks {
                // Not supported/No specific action on block yet
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(
                    crate::state::BlockType::ExternalFile,
                    Some(start),
                    end,
                );
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::ExternalFile,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Address => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::Address,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Address");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(crate::state::BlockType::Address, Some(start), end);
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::Address,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Text => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::Text,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Text");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(crate::state::BlockType::Text, Some(start), end);
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::Text,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Screencode => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::Screencode,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Screencode");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(
                    crate::state::BlockType::Screencode,
                    Some(start),
                    end,
                );
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::Screencode,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::Undefined => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let start_idx = start as usize;
                    let end_idx = end as usize;
                    app_state.set_block_type_region(
                        crate::state::BlockType::Undefined,
                        Some(start_idx),
                        end_idx,
                    );
                    ui_state.set_status_message("Set block type to Undefined");
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);

                let target_address = if let Some(line) = app_state.disassembly.get(end) {
                    line.address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1)
                } else {
                    0
                };

                app_state.set_block_type_region(
                    crate::state::BlockType::Undefined,
                    Some(start),
                    end,
                );
                ui_state.selection_start = None;
                ui_state.is_visual_mode = false;

                if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                    ui_state.cursor_index = idx;
                }
            } else {
                app_state.set_block_type_region(
                    crate::state::BlockType::Undefined,
                    ui_state.selection_start,
                    ui_state.cursor_index,
                );
            }
        }
        MenuAction::JumpToAddress => {
            ui_state.jump_to_address_dialog.open();
            ui_state.status_message = "Enter address (Hex)".to_string();
        }
        MenuAction::JumpToLine => {
            ui_state.jump_to_line_dialog.open();
            ui_state.status_message = "Enter Line Number (Dec)".to_string();
        }
        MenuAction::Search => {
            ui_state.search_dialog.open();
            ui_state.set_status_message("Search...");
        }
        MenuAction::FindNext => {
            crate::dialog_search::perform_search(app_state, ui_state, true);
        }
        MenuAction::FindPrevious => {
            crate::dialog_search::perform_search(app_state, ui_state, false);
        }
        MenuAction::JumpToOperand => {
            let target_addr = match ui_state.active_pane {
                ActivePane::Disassembly => {
                    if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                        // Try to extract address from operand.
                        // We utilize the opcode mode if available.
                        if let Some(opcode) = &line.opcode {
                            use crate::cpu::AddressingMode;
                            match opcode.mode {
                                AddressingMode::Absolute
                                | AddressingMode::AbsoluteX
                                | AddressingMode::AbsoluteY => {
                                    if line.bytes.len() >= 3 {
                                        Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Indirect => {
                                    // JMP ($1234) -> target is $1234
                                    if line.bytes.len() >= 3 {
                                        Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Relative => {
                                    // Branch
                                    if line.bytes.len() >= 2 {
                                        let offset = line.bytes[1] as i8;
                                        Some(
                                            line.address
                                                .wrapping_add(2)
                                                .wrapping_add(offset as u16),
                                        )
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::ZeroPage
                                | AddressingMode::ZeroPageX
                                | AddressingMode::ZeroPageY
                                | AddressingMode::IndirectX
                                | AddressingMode::IndirectY => {
                                    if line.bytes.len() >= 2 {
                                        Some(line.bytes[1] as u16)
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                ActivePane::HexDump => {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    Some((aligned_origin + ui_state.hex_cursor_index * 16) as u16)
                }
                ActivePane::Sprites => {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    Some((origin + padding + ui_state.sprites_cursor_index * 64) as u16)
                }
                ActivePane::Charset => {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    Some((aligned_start_addr + ui_state.charset_cursor_index * 8) as u16)
                }
                ActivePane::Blocks => {
                    // Jump to start of selected block
                    let blocks = app_state.get_blocks_view_items();
                    let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                    if idx < blocks.len() {
                        match blocks[idx] {
                            crate::state::BlockItem::Block { start, .. } => {
                                let offset = start;
                                Some(app_state.origin.wrapping_add(offset))
                            }
                            crate::state::BlockItem::Splitter(addr) => Some(addr),
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(addr) = target_addr {
                // Perform Jump
                if let Some(idx) = app_state.get_line_index_containing_address(addr) {
                    ui_state
                        .navigation_history
                        .push((ActivePane::Disassembly, ui_state.cursor_index));
                    ui_state.cursor_index = idx;
                    ui_state.active_pane = ActivePane::Disassembly;
                    ui_state.sub_cursor_index = 0; // Reset sub-line selection
                    ui_state.set_status_message(format!("Jumped to ${:04X}", addr));
                } else {
                    ui_state.set_status_message(format!("Address ${:04X} not found", addr));
                }
            } else if ui_state.active_pane == ActivePane::Disassembly {
                ui_state.set_status_message("No target address");
            }
        }
        MenuAction::About => {
            ui_state.about_dialog.open();
            ui_state.status_message = "About Regenerator 2000".to_string();
        }
        MenuAction::TogglePetsciiMode => {
            let new_mode = match ui_state.petscii_mode {
                crate::state::PetsciiMode::Unshifted => crate::state::PetsciiMode::Shifted,
                crate::state::PetsciiMode::Shifted => crate::state::PetsciiMode::Unshifted,
            };
            ui_state.petscii_mode = new_mode;
            let status = match new_mode {
                crate::state::PetsciiMode::Shifted => "Shifted",
                crate::state::PetsciiMode::Unshifted => "Unshifted",
            };
            ui_state.set_status_message(format!("Hex Dump: {} PETSCII", status));
        }
        MenuAction::ToggleSplitter => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    // If it's a splitter, toggle it (remove it).
                    // If it's a block, do we allow adding a splitter at the START?
                    // Or maybe we don't support adding splitters from blocks view except by selecting a splitter to remove it.
                    // Actually, if we select a splitter and hit '|', we should remove it.
                    && let crate::state::BlockItem::Splitter(addr) = blocks[idx]
                {
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(app_state);
                    app_state.push_command(command);
                    ui_state.set_status_message(format!("Removed splitter at ${:04X}", addr));
                }
            } else if ui_state.active_pane == ActivePane::Disassembly {
                let addr_to_toggle = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|line| line.address);

                if let Some(addr) = addr_to_toggle {
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(app_state);
                    app_state.push_command(command);
                    ui_state.set_status_message(format!("Toggled splitter at ${:04X}", addr));
                }
            }
        }
        MenuAction::ToggleSpriteMulticolor => {
            ui_state.sprite_multicolor_mode = !ui_state.sprite_multicolor_mode;
            if ui_state.sprite_multicolor_mode {
                ui_state.set_status_message("Sprites: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Sprites: Single Color Mode");
            }
        }
        MenuAction::ToggleCharsetMulticolor => {
            ui_state.charset_multicolor_mode = !ui_state.charset_multicolor_mode;
            if ui_state.charset_multicolor_mode {
                ui_state.set_status_message("Charset: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Charset: Single Color Mode");
            }
        }
        MenuAction::SetLoHi => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let len = (end as usize) - (start as usize) + 1;
                    if !len.is_multiple_of(2) {
                        ui_state.set_status_message("Error: LoHi requires even number of bytes");
                    } else {
                        let start_idx = start as usize;
                        let end_idx = end as usize;
                        app_state.set_block_type_region(
                            crate::state::BlockType::LoHi,
                            Some(start_idx),
                            end_idx,
                        );
                        ui_state.set_status_message("Set block type to LoHi");
                    }
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);
                let len = end - start + 1;

                if len % 2 != 0 {
                    ui_state.set_status_message("Error: LoHi requires even number of bytes");
                } else {
                    // Calculate target address (last byte of the selection)
                    let target_address = if let Some(line) = app_state.disassembly.get(end) {
                        line.address
                            .wrapping_add(line.bytes.len() as u16)
                            .wrapping_sub(1)
                    } else {
                        0
                    };

                    app_state.set_block_type_region(
                        crate::state::BlockType::LoHi,
                        Some(start),
                        end,
                    );
                    ui_state.selection_start = None;
                    ui_state.is_visual_mode = false;

                    // Move cursor to the line containing target_address
                    if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                        ui_state.cursor_index = idx;
                    }

                    ui_state.set_status_message("Set block type to Lo/Hi Address");
                }
            } else {
                // Single byte is NOT allowed for LoHi as it's odd (length 1)
                ui_state.set_status_message("Error: LoHi requires even number of bytes");
            }
        }
        MenuAction::SetHiLo => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
                {
                    let len = (end as usize) - (start as usize) + 1;
                    if !len.is_multiple_of(2) {
                        ui_state.set_status_message("Error: HiLo requires even number of bytes");
                    } else {
                        let start_idx = start as usize;
                        let end_idx = end as usize;
                        app_state.set_block_type_region(
                            crate::state::BlockType::HiLo,
                            Some(start_idx),
                            end_idx,
                        );
                        ui_state.set_status_message("Set block type to HiLo");
                    }
                }
            } else if let Some(start_index) = ui_state.selection_start {
                let start = start_index.min(ui_state.cursor_index);
                let end = start_index.max(ui_state.cursor_index);
                let len = end - start + 1;

                if len % 2 != 0 {
                    ui_state.set_status_message("Error: HiLo requires even number of bytes");
                } else {
                    // Calculate target address (last byte of the selection)
                    let target_address = if let Some(line) = app_state.disassembly.get(end) {
                        line.address
                            .wrapping_add(line.bytes.len() as u16)
                            .wrapping_sub(1)
                    } else {
                        0
                    };

                    app_state.set_block_type_region(
                        crate::state::BlockType::HiLo,
                        Some(start),
                        end,
                    );
                    ui_state.selection_start = None;
                    ui_state.is_visual_mode = false;

                    // Move cursor to the line containing target_address
                    if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
                        ui_state.cursor_index = idx;
                    }

                    ui_state.set_status_message("Set block type to Hi/Lo Address");
                }
            } else {
                // Single byte is NOT allowed for HiLo as it's odd (length 1)
                ui_state.set_status_message("Error: HiLo requires even number of bytes");
            }
        }
        MenuAction::SideComment => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let address = line.address;
                let current_comment = app_state
                    .user_side_comments
                    .get(&address)
                    .map(|s| s.as_str());
                ui_state
                    .comment_dialog
                    .open(current_comment, crate::dialog_comment::CommentType::Side);
                ui_state.set_status_message(format!("Edit Side Comment at ${:04X}", address));
            }
        }
        MenuAction::LineComment => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let address = line.address;
                let current_comment = app_state
                    .user_line_comments
                    .get(&address)
                    .map(|s| s.as_str());
                ui_state
                    .comment_dialog
                    .open(current_comment, crate::dialog_comment::CommentType::Line);
                ui_state.set_status_message(format!("Edit Line Comment at ${:04X}", address));
            }
        }
        MenuAction::ToggleHexDump => {
            if ui_state.right_pane == crate::ui_state::RightPane::HexDump {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Hex Dump View Hidden");
                if ui_state.active_pane == ActivePane::HexDump {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::HexDump;
                ui_state.active_pane = ActivePane::HexDump;
                ui_state.set_status_message("Hex Dump View Shown");
            }
        }
        MenuAction::ToggleSpritesView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Sprites {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Sprites View Hidden");
                if ui_state.active_pane == ActivePane::Sprites {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Sprites;
                ui_state.active_pane = ActivePane::Sprites;
                ui_state.set_status_message("Sprites View Shown");
            }
        }
        MenuAction::ToggleCharsetView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Charset {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Charset View Hidden");
                if ui_state.active_pane == ActivePane::Charset {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Charset;
                ui_state.active_pane = ActivePane::Charset;
                ui_state.set_status_message("Charset View Shown");
            }
        }
        MenuAction::ToggleBlocksView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Blocks {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Blocks View Hidden");
                if ui_state.active_pane == ActivePane::Blocks {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Blocks;
                ui_state.active_pane = ActivePane::Blocks;
                ui_state.set_status_message("Blocks View Shown");
            }
        }
        MenuAction::KeyboardShortcuts => {
            ui_state.shortcuts_dialog.open();
            ui_state.set_status_message("Keyboard Shortcuts");
        }
        MenuAction::ChangeOrigin => {
            ui_state.origin_dialog.open(app_state.origin);
            ui_state.set_status_message("Enter new origin (Hex)");
        }
        MenuAction::SystemSettings => {
            ui_state.system_settings_dialog.open();
            ui_state.set_status_message("System Settings");
        }
        MenuAction::NextImmediateFormat => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let has_immediate = if let Some(opcode) = &line.opcode {
                    opcode.mode == crate::cpu::AddressingMode::Immediate
                } else {
                    false
                };

                if has_immediate {
                    let val = line.bytes.get(1).copied().unwrap_or(0);
                    let current_fmt = app_state
                        .immediate_value_formats
                        .get(&line.address)
                        .copied()
                        .unwrap_or(crate::state::ImmediateFormat::Hex);

                    let next_fmt = match current_fmt {
                        crate::state::ImmediateFormat::Hex => {
                            crate::state::ImmediateFormat::InvertedHex
                        }
                        crate::state::ImmediateFormat::InvertedHex => {
                            crate::state::ImmediateFormat::Decimal
                        }
                        crate::state::ImmediateFormat::Decimal => {
                            if val <= 128 {
                                crate::state::ImmediateFormat::Binary
                            } else {
                                crate::state::ImmediateFormat::NegativeDecimal
                            }
                        }
                        crate::state::ImmediateFormat::NegativeDecimal => {
                            crate::state::ImmediateFormat::Binary
                        }
                        crate::state::ImmediateFormat::Binary => {
                            crate::state::ImmediateFormat::InvertedBinary
                        }
                        crate::state::ImmediateFormat::InvertedBinary => {
                            crate::state::ImmediateFormat::Hex
                        }
                    };

                    let command = crate::commands::Command::SetImmediateFormat {
                        address: line.address,
                        new_format: Some(next_fmt),
                        old_format: Some(current_fmt),
                    };
                    command.apply(app_state);
                    app_state.undo_stack.push(command);
                    app_state.disassemble();
                }
            }
        }
        MenuAction::PreviousImmediateFormat => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let has_immediate = if let Some(opcode) = &line.opcode {
                    opcode.mode == crate::cpu::AddressingMode::Immediate
                } else {
                    false
                };

                if has_immediate {
                    let val = line.bytes.get(1).copied().unwrap_or(0);
                    let current_fmt = app_state
                        .immediate_value_formats
                        .get(&line.address)
                        .copied()
                        .unwrap_or(crate::state::ImmediateFormat::Hex);

                    let next_fmt = match current_fmt {
                        crate::state::ImmediateFormat::Hex => {
                            crate::state::ImmediateFormat::InvertedBinary
                        }
                        crate::state::ImmediateFormat::InvertedBinary => {
                            crate::state::ImmediateFormat::Binary
                        }
                        crate::state::ImmediateFormat::Binary => {
                            if val <= 128 {
                                crate::state::ImmediateFormat::Decimal
                            } else {
                                crate::state::ImmediateFormat::NegativeDecimal
                            }
                        }
                        crate::state::ImmediateFormat::NegativeDecimal => {
                            crate::state::ImmediateFormat::Decimal
                        }
                        crate::state::ImmediateFormat::Decimal => {
                            crate::state::ImmediateFormat::InvertedHex
                        }
                        crate::state::ImmediateFormat::InvertedHex => {
                            crate::state::ImmediateFormat::Hex
                        }
                    };

                    let command = crate::commands::Command::SetImmediateFormat {
                        address: line.address,
                        new_format: Some(next_fmt),
                        old_format: Some(current_fmt),
                    };
                    command.apply(app_state);
                    app_state.undo_stack.push(command);
                    app_state.disassemble();
                }
            }
        }
        MenuAction::ToggleCollapsedBlock => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected() {
                    if let Some(crate::state::BlockItem::Block { start, end, .. }) = blocks.get(idx)
                    {
                        let start_offset = *start as usize;
                        let end_offset = *end as usize;

                        let current_cursor_addr = app_state
                            .disassembly
                            .get(ui_state.cursor_index)
                            .map(|line| line.address);

                        // Check if already collapsed
                        if let Some(&range) = app_state
                            .collapsed_blocks
                            .iter()
                            .find(|(s, e)| *s == start_offset && *e == end_offset)
                        {
                            // Uncollapse
                            let command = crate::commands::Command::UncollapseBlock { range };
                            command.apply(app_state);
                            app_state.undo_stack.push(command);
                            app_state.disassemble();
                            ui_state.set_status_message("Block Uncollapsed");
                        } else {
                            // Collapse
                            let command = crate::commands::Command::CollapseBlock {
                                range: (start_offset, end_offset),
                            };
                            command.apply(app_state);
                            app_state.undo_stack.push(command);
                            app_state.disassemble();
                            ui_state.set_status_message("Block Collapsed");
                        }

                        // Restore cursor to the same address if possible
                        if let Some(addr) = current_cursor_addr {
                            if let Some(new_idx) = app_state.get_line_index_containing_address(addr)
                            {
                                ui_state.cursor_index = new_idx;
                            } else {
                                // Fallback: try to find nearest address or clamp?
                                // get_line_index_containing_address usually handles clamping or nearest logic inside?
                                // Actually it usually finds the specific line.
                                // If the line disappeared (e.g. it was inside the block we just collapsed),
                                // we might want to point to the start of the collapsed block.
                                // If we just collapsed a block, `get_line_index_containing_address` for an address INSIDE the block
                                // might return the index of the collapsed placeholder line?
                                // Let's trust `get_line_index_containing_address` behavior or default to keeping index if not found.
                            }
                        }
                    } else {
                        ui_state.set_status_message("Selected item is not a block");
                    }
                }
            } else {
                let cursor_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|line| line.address)
                    .unwrap_or(0);

                // First check if we are ON a collapsed block placeholder (Uncollapse case)
                if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                    let offset = (line.address as usize).wrapping_sub(app_state.origin as usize);
                    if let Some(&range) = app_state
                        .collapsed_blocks
                        .iter()
                        .find(|(s, _)| *s == offset)
                    {
                        let command = crate::commands::Command::UncollapseBlock { range };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);
                        app_state.disassemble();
                        ui_state.set_status_message("Block Uncollapsed");
                        return;
                    }
                }

                // If not uncollapsing, try to Collapse
                if let Some((start_addr, end_addr)) = app_state.get_block_range(cursor_addr) {
                    let start_offset =
                        (start_addr as usize).wrapping_sub(app_state.origin as usize);
                    let end_offset = (end_addr as usize).wrapping_sub(app_state.origin as usize);

                    // Check if already collapsed (redundant check if uncollapse logic above is correct,
                    // but safety against duplicate ranges or different lookup method)
                    if let Some(&range) = app_state
                        .collapsed_blocks
                        .iter()
                        .find(|(s, e)| *s == start_offset && *e == end_offset)
                    {
                        // Should have been caught above if cursor is at start, but maybe cursor is middle?
                        // If collapsed, it's just one line, so cursor must be at start.
                        // Just uncollapse to be safe.
                        let command = crate::commands::Command::UncollapseBlock { range };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);
                        app_state.disassemble();
                        ui_state.set_status_message("Block Uncollapsed");
                    } else {
                        // Collapse
                        let command = crate::commands::Command::CollapseBlock {
                            range: (start_offset, end_offset),
                        };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);

                        ui_state.selection_start = None; // clear selection if any
                        ui_state.is_visual_mode = false;
                        app_state.disassemble();
                        ui_state.set_status_message("Block Collapsed");

                        // Move cursor to start of collapsed block
                        if let Some(idx) = app_state.get_line_index_containing_address(start_addr) {
                            ui_state.cursor_index = idx;
                        }
                    }
                } else {
                    ui_state.set_status_message("No block found at cursor");
                }
            }
        }
    }
}
