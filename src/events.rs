use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, SaveDialogMode, UIState};
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
        );

        terminal
            .draw(|f| ui(f, &app_state, &mut ui_state))
            .map_err(|e| io::Error::other(e.to_string()))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != event::KeyEventKind::Press {
                continue;
            }
            ui_state.dismiss_logo = true;
            if ui_state.jump_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.jump_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        let input = &ui_state.jump_dialog.input;
                        match ui_state.jump_dialog.mode {
                            crate::ui_state::JumpDialogMode::Address => {
                                if let Ok(addr) = u16::from_str_radix(input, 16) {
                                    let target_addr = addr;

                                    match ui_state.active_pane {
                                        ActivePane::Disassembly => {
                                            // Find closest address in disassembly
                                            let mut found_idx = None;
                                            for (i, line) in
                                                app_state.disassembly.iter().enumerate()
                                            {
                                                if line.address == target_addr {
                                                    found_idx = Some(i);
                                                    break;
                                                } else if line.address > target_addr {
                                                    if i > 0 {
                                                        found_idx = Some(i - 1);
                                                    } else {
                                                        found_idx = Some(0);
                                                    }
                                                    break;
                                                }
                                            }

                                            if let Some(idx) = found_idx {
                                                ui_state.navigation_history.push((
                                                    crate::ui_state::ActivePane::Disassembly,
                                                    ui_state.cursor_index,
                                                ));
                                                ui_state.cursor_index = idx;
                                                ui_state.set_status_message(format!(
                                                    "Jumped to ${:04X}",
                                                    target_addr
                                                ));
                                            } else if !app_state.disassembly.is_empty() {
                                                ui_state.navigation_history.push((
                                                    crate::ui_state::ActivePane::Disassembly,
                                                    ui_state.cursor_index,
                                                ));
                                                ui_state.cursor_index =
                                                    app_state.disassembly.len() - 1;
                                                ui_state.set_status_message("Jumped to end");
                                            }
                                        }
                                        ActivePane::HexDump => {
                                            let origin = app_state.origin as usize;
                                            let target = target_addr as usize;
                                            let data_len = app_state.raw_data.len();
                                            let end_addr = origin + data_len;

                                            if target >= origin && target < end_addr {
                                                ui_state.navigation_history.push((
                                                    crate::ui_state::ActivePane::HexDump,
                                                    ui_state.hex_cursor_index,
                                                ));
                                                let alignment_padding = origin % 16;
                                                let aligned_origin = origin - alignment_padding;
                                                let offset = target - aligned_origin;
                                                let row = offset / 16;
                                                ui_state.hex_cursor_index = row;
                                                ui_state.set_status_message(format!(
                                                    "Jumped to ${:04X}",
                                                    target_addr
                                                ));
                                            } else {
                                                ui_state.set_status_message("Address out of range");
                                            }
                                        }
                                    }

                                    ui_state.jump_dialog.close();
                                } else {
                                    ui_state.set_status_message("Invalid Hex Address");
                                }
                            }
                            crate::ui_state::JumpDialogMode::Line => {
                                if let Ok(line_num) = input.parse::<usize>() {
                                    if line_num > 0 && line_num <= app_state.disassembly.len() {
                                        ui_state.navigation_history.push((
                                            crate::ui_state::ActivePane::Disassembly,
                                            ui_state.cursor_index,
                                        ));
                                        ui_state.cursor_index = line_num - 1;
                                        ui_state.set_status_message(format!(
                                            "Jumped to line {}",
                                            line_num
                                        ));
                                        ui_state.jump_dialog.close();
                                    } else {
                                        ui_state.set_status_message("Line number out of range");
                                    }
                                } else {
                                    ui_state.set_status_message("Invalid Line Number");
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.jump_dialog.input.pop();
                    }
                    KeyCode::Char(c) => match ui_state.jump_dialog.mode {
                        crate::ui_state::JumpDialogMode::Address => {
                            if c.is_ascii_hexdigit() && ui_state.jump_dialog.input.len() < 4 {
                                ui_state.jump_dialog.input.push(c.to_ascii_uppercase());
                            }
                        }
                        crate::ui_state::JumpDialogMode::Line => {
                            if c.is_ascii_digit() && ui_state.jump_dialog.input.len() < 10 {
                                ui_state.jump_dialog.input.push(c);
                            }
                        }
                    },
                    _ => {}
                }
            } else if ui_state.save_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.save_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        let filename = ui_state.save_dialog.input.clone();
                        if !filename.is_empty() {
                            let mut path = ui_state.file_picker.current_dir.join(filename);
                            if ui_state.save_dialog.mode == SaveDialogMode::Project {
                                if path.extension().is_none() {
                                    path.set_extension("regen2000proj");
                                }
                                app_state.project_path = Some(path);
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
                                    // Make sure it's within valid range
                                    // The cursor is at the start of the row
                                    let addr = aligned_origin + row_start_offset;
                                    // Check if this address is somewhat valid (it might be padding)
                                    // But we just want to restore the row, so saving the row start address is fine.
                                    Some(addr as u16)
                                } else {
                                    None
                                };

                                if let Err(e) = app_state.save_project(cursor_addr, hex_addr) {
                                    ui_state.set_status_message(format!("Error saving: {}", e));
                                } else {
                                    ui_state.set_status_message("Project saved");
                                    ui_state.save_dialog.close();
                                }
                            } else {
                                // Export ASM
                                if path.extension().is_none() {
                                    path.set_extension("asm");
                                }
                                app_state.export_path = Some(path.clone());
                                if let Err(e) = crate::exporter::export_asm(&app_state, &path) {
                                    ui_state.set_status_message(format!("Error exporting: {}", e));
                                } else {
                                    ui_state.set_status_message("Project Exported");
                                    ui_state.save_dialog.close();
                                }
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.save_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        ui_state.save_dialog.input.push(c);
                    }
                    _ => {}
                }
            } else if ui_state.label_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.label_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        // Get current address
                        if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                            let address = line.address;
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
                                crate::ui_state::CommentType::Side => {
                                    let old_comment =
                                        app_state.user_side_comments.get(&address).cloned();
                                    crate::commands::Command::SetUserSideComment {
                                        address,
                                        new_comment: new_comment_opt,
                                        old_comment,
                                    }
                                }
                                crate::ui_state::CommentType::Line => {
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
            } else if ui_state.file_picker.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.file_picker.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Down => ui_state.file_picker.next(),
                    KeyCode::Up => ui_state.file_picker.previous(),
                    KeyCode::Backspace => {
                        // Go to parent dir
                        if let Some(parent) = ui_state
                            .file_picker
                            .current_dir
                            .parent()
                            .map(|p| p.to_path_buf())
                        {
                            ui_state.file_picker.current_dir = parent;
                            ui_state.file_picker.refresh_files();
                            ui_state.file_picker.selected_index = 0;
                        }
                    }
                    KeyCode::Enter => {
                        if !ui_state.file_picker.files.is_empty() {
                            let selected_path = ui_state.file_picker.files
                                [ui_state.file_picker.selected_index]
                                .clone();
                            if selected_path.is_dir() {
                                ui_state.file_picker.current_dir = selected_path;
                                ui_state.file_picker.refresh_files();
                                ui_state.file_picker.selected_index = 0;
                            } else {
                                // Load file
                                match app_state.load_file(selected_path.clone()) {
                                    Err(e) => {
                                        ui_state.set_status_message(format!(
                                            "Error loading file: {}",
                                            e
                                        ));
                                    }
                                    Ok((loaded_cursor, loaded_hex_cursor)) => {
                                        ui_state.set_status_message(format!(
                                            "Loaded: {:?}",
                                            selected_path
                                        ));
                                        ui_state.file_picker.close();

                                        // Auto-analyze if it's a binary file (not json)
                                        let is_project = selected_path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .map(|e| e.eq_ignore_ascii_case("regen2000proj"))
                                            .unwrap_or(false);

                                        if !is_project {
                                            app_state.perform_analysis();
                                        }

                                        // Move cursor
                                        if let Some(cursor_addr) = loaded_cursor {
                                            if let Some(idx) =
                                                app_state.get_line_index_for_address(cursor_addr)
                                            {
                                                ui_state.cursor_index = idx;
                                            }
                                        } else {
                                            // Default to origin
                                            if let Some(idx) = app_state
                                                .get_line_index_for_address(app_state.origin)
                                            {
                                                ui_state.cursor_index = idx;
                                            }
                                        }

                                        // Restore Hex Cursor
                                        if let Some(hex_addr) = loaded_hex_cursor
                                            && !app_state.raw_data.is_empty()
                                        {
                                            let origin = app_state.origin as usize;
                                            let alignment_padding = origin % 16;
                                            let aligned_origin = origin - alignment_padding;
                                            let target = hex_addr as usize;

                                            if target >= aligned_origin {
                                                let offset = target - aligned_origin;
                                                let row = offset / 16;
                                                // Ensure row is within bounds
                                                // Max rows calculation:
                                                let total_len =
                                                    app_state.raw_data.len() + alignment_padding;
                                                let max_rows = total_len.div_ceil(16);

                                                if row < max_rows {
                                                    ui_state.hex_cursor_index = row;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if ui_state.search_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.search_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        ui_state.search_dialog.last_search = ui_state.search_dialog.input.clone();
                        ui_state.search_dialog.close();
                        perform_search(&mut app_state, &mut ui_state, true);
                    }
                    KeyCode::Backspace => {
                        ui_state.search_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        ui_state.search_dialog.input.push(c);
                    }
                    _ => {}
                }
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
                if let KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_) = key.code {
                    ui_state.about_dialog.close();
                    ui_state.set_status_message("Ready");
                }
            } else if ui_state.shortcuts_dialog.active {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        ui_state.shortcuts_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Down => ui_state.shortcuts_dialog.scroll_down(),
                    KeyCode::Up => ui_state.shortcuts_dialog.scroll_up(),
                    _ => {}
                }
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
                match key.code {
                    KeyCode::Esc => {
                        if ui_state.settings_dialog.is_selecting_platform {
                            ui_state.settings_dialog.is_selecting_platform = false;
                        } else if ui_state.settings_dialog.is_selecting_assembler {
                            ui_state.settings_dialog.is_selecting_assembler = false;
                        } else if ui_state.settings_dialog.is_editing_xref_count {
                            ui_state.settings_dialog.is_editing_xref_count = false;
                            // Reset input to current value
                            ui_state.settings_dialog.xref_count_input.clear();
                        } else if ui_state.settings_dialog.is_editing_arrow_columns {
                            ui_state.settings_dialog.is_editing_arrow_columns = false;
                            ui_state.settings_dialog.arrow_columns_input.clear();
                        } else if ui_state.settings_dialog.is_editing_text_char_limit {
                            ui_state.settings_dialog.is_editing_text_char_limit = false;
                            ui_state.settings_dialog.text_char_limit_input.clear();
                        } else {
                            ui_state.settings_dialog.close();
                            ui_state.set_status_message("Ready");
                            app_state.load_system_assets();
                            app_state.perform_analysis();
                            app_state.disassemble(); // Disassemble on close to apply all settings
                        }
                    }
                    KeyCode::Up => {
                        if ui_state.settings_dialog.is_selecting_platform {
                            // Cycle platforms backwards
                            let platforms = crate::state::Platform::all();
                            let current_idx = platforms
                                .iter()
                                .position(|p| *p == app_state.settings.platform)
                                .unwrap_or(0);
                            let new_idx = if current_idx == 0 {
                                platforms.len() - 1
                            } else {
                                current_idx - 1
                            };
                            app_state.settings.platform = platforms[new_idx];
                        } else if ui_state.settings_dialog.is_selecting_assembler {
                            // Cycle assemblers backwards
                            let assemblers = crate::state::Assembler::all();
                            let current_idx = assemblers
                                .iter()
                                .position(|a| *a == app_state.settings.assembler)
                                .unwrap_or(0);
                            let new_idx = if current_idx == 0 {
                                assemblers.len() - 1
                            } else {
                                current_idx - 1
                            };
                            app_state.settings.assembler = assemblers[new_idx];
                        } else if !ui_state.settings_dialog.is_editing_xref_count
                            && !ui_state.settings_dialog.is_editing_arrow_columns
                            && !ui_state.settings_dialog.is_editing_text_char_limit
                        {
                            ui_state.settings_dialog.previous();
                        }
                    }
                    KeyCode::Left => {
                        if !ui_state.settings_dialog.is_editing_xref_count
                            && !ui_state.settings_dialog.is_editing_arrow_columns
                            && !ui_state.settings_dialog.is_editing_text_char_limit
                        {
                            match ui_state.settings_dialog.selected_index {
                                7 => {
                                    app_state.settings.max_xref_count =
                                        app_state.settings.max_xref_count.saturating_sub(1);
                                }
                                8 => {
                                    app_state.settings.max_arrow_columns =
                                        app_state.settings.max_arrow_columns.saturating_sub(1);
                                }
                                9 => {
                                    app_state.settings.text_char_limit =
                                        app_state.settings.text_char_limit.saturating_sub(1);
                                }
                                _ => {}
                            }
                        }
                    }
                    KeyCode::Right => {
                        if !ui_state.settings_dialog.is_editing_xref_count
                            && !ui_state.settings_dialog.is_editing_arrow_columns
                            && !ui_state.settings_dialog.is_editing_text_char_limit
                        {
                            match ui_state.settings_dialog.selected_index {
                                7 => {
                                    app_state.settings.max_xref_count =
                                        app_state.settings.max_xref_count.saturating_add(1);
                                }
                                8 => {
                                    app_state.settings.max_arrow_columns =
                                        app_state.settings.max_arrow_columns.saturating_add(1);
                                }
                                9 => {
                                    app_state.settings.text_char_limit =
                                        app_state.settings.text_char_limit.saturating_add(1);
                                }
                                _ => {}
                            }
                        }
                    }
                    KeyCode::Down => {
                        if ui_state.settings_dialog.is_selecting_platform {
                            // Cycle platforms forwards
                            let platforms = crate::state::Platform::all();
                            let current_idx = platforms
                                .iter()
                                .position(|p| *p == app_state.settings.platform)
                                .unwrap_or(0);
                            let new_idx = (current_idx + 1) % platforms.len();
                            app_state.settings.platform = platforms[new_idx];
                        } else if ui_state.settings_dialog.is_selecting_assembler {
                            // Cycle assemblers forwards
                            let assemblers = crate::state::Assembler::all();
                            let current_idx = assemblers
                                .iter()
                                .position(|a| *a == app_state.settings.assembler)
                                .unwrap_or(0);
                            let new_idx = (current_idx + 1) % assemblers.len();
                            app_state.settings.assembler = assemblers[new_idx];
                        } else if !ui_state.settings_dialog.is_editing_xref_count
                            && !ui_state.settings_dialog.is_editing_arrow_columns
                            && !ui_state.settings_dialog.is_editing_text_char_limit
                        {
                            ui_state.settings_dialog.next();
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if ui_state.settings_dialog.is_selecting_platform {
                            ui_state.settings_dialog.is_selecting_platform = false;
                        } else if ui_state.settings_dialog.is_selecting_assembler {
                            ui_state.settings_dialog.is_selecting_assembler = false;
                        } else if ui_state.settings_dialog.is_editing_xref_count {
                            // Commit value
                            if let Ok(val) =
                                ui_state.settings_dialog.xref_count_input.parse::<usize>()
                            {
                                app_state.settings.max_xref_count = val;
                                ui_state.settings_dialog.is_editing_xref_count = false;
                            }
                        } else if ui_state.settings_dialog.is_editing_arrow_columns {
                            // Commit value
                            if let Ok(val) = ui_state
                                .settings_dialog
                                .arrow_columns_input
                                .parse::<usize>()
                            {
                                app_state.settings.max_arrow_columns = val;
                                ui_state.settings_dialog.is_editing_arrow_columns = false;
                            }
                        } else if ui_state.settings_dialog.is_editing_text_char_limit {
                            // Commit value
                            if let Ok(val) = ui_state
                                .settings_dialog
                                .text_char_limit_input
                                .parse::<usize>()
                            {
                                app_state.settings.text_char_limit = val;
                                ui_state.settings_dialog.is_editing_text_char_limit = false;
                            }
                        } else {
                            // Toggle checkbox or enter mode
                            match ui_state.settings_dialog.selected_index {
                                0 => app_state.settings.all_labels = !app_state.settings.all_labels,
                                1 => {
                                    app_state.settings.preserve_long_bytes =
                                        !app_state.settings.preserve_long_bytes;
                                }
                                2 => {
                                    app_state.settings.brk_single_byte =
                                        !app_state.settings.brk_single_byte;
                                    if app_state.settings.brk_single_byte {
                                        app_state.settings.patch_brk = false;
                                    }
                                }
                                3 => {
                                    if !app_state.settings.brk_single_byte {
                                        app_state.settings.patch_brk =
                                            !app_state.settings.patch_brk;
                                    }
                                }
                                4 => {
                                    app_state.settings.use_illegal_opcodes =
                                        !app_state.settings.use_illegal_opcodes;
                                }
                                5 => {
                                    ui_state.settings_dialog.is_selecting_platform = true;
                                }
                                6 => {
                                    ui_state.settings_dialog.is_selecting_assembler = true;
                                }
                                7 => {
                                    ui_state.settings_dialog.is_editing_xref_count = true;
                                    ui_state.settings_dialog.xref_count_input =
                                        app_state.settings.max_xref_count.to_string();
                                }
                                8 => {
                                    ui_state.settings_dialog.is_editing_arrow_columns = true;
                                    ui_state.settings_dialog.arrow_columns_input =
                                        app_state.settings.max_arrow_columns.to_string();
                                }
                                9 => {
                                    ui_state.settings_dialog.is_editing_text_char_limit = true;
                                    ui_state.settings_dialog.text_char_limit_input =
                                        app_state.settings.text_char_limit.to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if ui_state.settings_dialog.is_editing_xref_count {
                            ui_state.settings_dialog.xref_count_input.pop();
                        } else if ui_state.settings_dialog.is_editing_arrow_columns {
                            ui_state.settings_dialog.arrow_columns_input.pop();
                        } else if ui_state.settings_dialog.is_editing_text_char_limit {
                            ui_state.settings_dialog.text_char_limit_input.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if ui_state.settings_dialog.is_editing_xref_count && c.is_ascii_digit() {
                            ui_state.settings_dialog.xref_count_input.push(c);
                        } else if ui_state.settings_dialog.is_editing_arrow_columns
                            && c.is_ascii_digit()
                        {
                            ui_state.settings_dialog.arrow_columns_input.push(c);
                        } else if ui_state.settings_dialog.is_editing_text_char_limit
                            && c.is_ascii_digit()
                        {
                            ui_state.settings_dialog.text_char_limit_input.push(c);
                        }
                    }
                    _ => {}
                }
            } else if ui_state.system_settings_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        if ui_state.system_settings_dialog.is_selecting_theme {
                            ui_state.system_settings_dialog.is_selecting_theme = false;
                        } else {
                            ui_state.system_settings_dialog.close();
                            ui_state.set_status_message("Ready");
                        }
                    }
                    KeyCode::Up => {
                        if ui_state.system_settings_dialog.is_selecting_theme {
                            // Cycle themes
                            let themes = crate::theme::Theme::all_names();
                            let current = app_state.system_config.theme.as_str();
                            let idx = themes.iter().position(|t| *t == current).unwrap_or(0);
                            let new_idx = if idx == 0 { themes.len() - 1 } else { idx - 1 };
                            let new_theme = themes[new_idx].to_string();
                            app_state.system_config.theme = new_theme.clone();
                            ui_state.theme = crate::theme::Theme::from_name(&new_theme);
                        } else {
                            ui_state.system_settings_dialog.selected_index = ui_state
                                .system_settings_dialog
                                .selected_index
                                .saturating_sub(1);
                        }
                    }
                    KeyCode::Down => {
                        if ui_state.system_settings_dialog.is_selecting_theme {
                            // Cycle themes
                            let themes = crate::theme::Theme::all_names();
                            let current = app_state.system_config.theme.as_str();
                            let idx = themes.iter().position(|t| *t == current).unwrap_or(0);
                            let new_idx = (idx + 1) % themes.len();
                            let new_theme = themes[new_idx].to_string();
                            app_state.system_config.theme = new_theme.clone();
                            ui_state.theme = crate::theme::Theme::from_name(&new_theme);
                        } else {
                            // Limit to 1 (2 items)
                            if ui_state.system_settings_dialog.selected_index < 1 {
                                ui_state.system_settings_dialog.selected_index += 1;
                            }
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if ui_state.system_settings_dialog.is_selecting_theme {
                            ui_state.system_settings_dialog.is_selecting_theme = false;
                            let _ = app_state.system_config.save();
                        } else if ui_state.system_settings_dialog.selected_index == 0 {
                            app_state.system_config.open_last_project =
                                !app_state.system_config.open_last_project;
                            let _ = app_state.system_config.save();
                        } else if ui_state.system_settings_dialog.selected_index == 1 {
                            ui_state.system_settings_dialog.is_selecting_theme = true;
                        }
                    }
                    _ => {}
                }
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
                        perform_search(&mut app_state, &mut ui_state, true);
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
                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Exit,
                        );
                    }
                    KeyCode::Char('/') => {
                        ui_state.vim_search_active = true;
                        ui_state.vim_search_input.clear();
                    }
                    KeyCode::Char('n') => {
                        perform_search(&mut app_state, &mut ui_state, true);
                    }
                    KeyCode::Char('N') => {
                        perform_search(&mut app_state, &mut ui_state, false);
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
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::DocumentSettings,
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
                        }
                    }
                    KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetPetsciiShifted,
                            );
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetPetsciiUnshifted,
                            );
                        }
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index = (ui_state.cursor_index + 10)
                                    .min(app_state.disassembly.len().saturating_sub(1));
                            }
                            ActivePane::HexDump => {
                                let bytes_per_row = 16;
                                let padding = (app_state.origin as usize) % bytes_per_row;
                                let total_rows =
                                    (app_state.raw_data.len() + padding).div_ceil(bytes_per_row);
                                ui_state.hex_cursor_index = (ui_state.hex_cursor_index + 10)
                                    .min(total_rows.saturating_sub(1));
                            }
                        }
                    }
                    KeyCode::Char('u') => {
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

                    KeyCode::Char('g') => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                handle_menu_action(
                                    &mut app_state,
                                    &mut ui_state,
                                    crate::ui_state::MenuAction::JumpToLine,
                                );
                            }
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::JumpToAddress,
                            );
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
                        if let Some((pane, idx)) = ui_state.navigation_history.pop() {
                            ui_state.active_pane = pane;
                            match pane {
                                ActivePane::Disassembly => {
                                    if idx < app_state.disassembly.len() {
                                        ui_state.cursor_index = idx;
                                        ui_state.set_status_message("Navigated back");
                                    } else {
                                        ui_state.set_status_message("History invalid");
                                    }
                                }
                                ActivePane::HexDump => {
                                    // Basic bounds check might be hard here without recalculating rows
                                    // For now assume it's valid if it was pushed
                                    ui_state.hex_cursor_index = idx;
                                    ui_state.set_status_message("Navigated back");
                                }
                            }
                        } else {
                            ui_state.set_status_message("No history");
                        }
                    }

                    // Data Conversion Shortcuts
                    KeyCode::Char('c') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Code,
                            )
                        }
                    }
                    KeyCode::Char('b') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Byte,
                            )
                        }
                    }
                    KeyCode::Char('w') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Word,
                            )
                        }
                    }
                    KeyCode::Char('a') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Address,
                            )
                        }
                    }
                    KeyCode::Char('t') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Text,
                            )
                        }
                    }
                    KeyCode::Char('s') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Screencode,
                            )
                        }
                    }
                    KeyCode::Char('U') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::Undefined,
                            )
                        }
                    }
                    KeyCode::Char('<') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetLoHi,
                            )
                        }
                    }
                    KeyCode::Char('>') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SetHiLo,
                            )
                        }
                    }
                    KeyCode::Char(';') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::SideComment,
                            )
                        }
                    }
                    KeyCode::Char(':') => {
                        if ui_state.active_pane == ActivePane::Disassembly {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::LineComment,
                            )
                        }
                    }

                    // Label
                    KeyCode::Char('l') => {
                        if !app_state.raw_data.is_empty() {
                            if !ui_state.menu.active
                                && !ui_state.jump_dialog.active
                                && !ui_state.save_dialog.active
                                && !ui_state.file_picker.active
                                && ui_state.active_pane == ActivePane::Disassembly
                                && let Some(line) = app_state.disassembly.get(ui_state.cursor_index)
                            {
                                let addr = line.address;
                                let text = app_state
                                    .labels
                                    .get(&addr)
                                    .and_then(|v| v.first())
                                    .map(|l| l.name.as_str());
                                ui_state.label_dialog.open(text);
                                ui_state.set_status_message("Enter Label");
                            }
                        } else if ui_state.active_pane == ActivePane::Disassembly {
                            ui_state.set_status_message("No open document");
                        }
                    }

                    // Visual Mode Toggle
                    KeyCode::Char('V') => {
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

                    KeyCode::Char('d') => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::NextImmediateFormat,
                        );
                    }

                    KeyCode::Char('D') => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::PreviousImmediateFormat,
                        );
                    }

                    // Vim-like G command
                    KeyCode::Char('G') => {
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
                            ActivePane::HexDump => {
                                let padding = (app_state.origin as usize) % 16;
                                let total_rows = (app_state.raw_data.len() + padding).div_ceil(16);
                                let target_row = if is_buffer_empty {
                                    total_rows
                                } else {
                                    entered_number
                                };

                                let new_cursor = if target_row == 0 {
                                    total_rows.saturating_sub(1)
                                } else {
                                    target_row
                                        .saturating_sub(1)
                                        .min(total_rows.saturating_sub(1))
                                };

                                ui_state
                                    .navigation_history
                                    .push((ui_state.active_pane, ui_state.hex_cursor_index));
                                ui_state.hex_cursor_index = new_cursor;
                                ui_state
                                    .set_status_message(format!("Jumped to row {}", target_row));
                            }
                        }
                    }

                    // Input Buffer for Numbers
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        if ui_state.active_pane == ActivePane::Disassembly
                            || ui_state.active_pane == ActivePane::HexDump
                        {
                            // Only append if it's a valid number sequence (avoid overflow though usize is large)
                            if ui_state.input_buffer.len() < 10 {
                                ui_state.input_buffer.push(c);
                                ui_state.set_status_message(format!(":{}", ui_state.input_buffer));
                            }
                        }
                    }

                    // Normal Navigation
                    KeyCode::Down | KeyCode::Char('j') => {
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

                                if ui_state.cursor_index
                                    < app_state.disassembly.len().saturating_sub(1)
                                {
                                    ui_state.cursor_index += 1;
                                }
                            }
                            ActivePane::HexDump => {
                                let bytes_per_row = 16;
                                let padding = (app_state.origin as usize) % bytes_per_row;
                                let total_rows =
                                    (app_state.raw_data.len() + padding).div_ceil(bytes_per_row);
                                if ui_state.hex_cursor_index < total_rows.saturating_sub(1) {
                                    ui_state.hex_cursor_index += 1;
                                }
                            }
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
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

                                if ui_state.cursor_index > 0 {
                                    ui_state.cursor_index -= 1;
                                }
                            }
                            ActivePane::HexDump => {
                                if ui_state.hex_cursor_index > 0 {
                                    ui_state.hex_cursor_index -= 1;
                                }
                            }
                        }
                    }
                    KeyCode::Tab => {
                        ui_state.active_pane = match ui_state.active_pane {
                            ActivePane::Disassembly => ActivePane::HexDump,
                            ActivePane::HexDump => ActivePane::Disassembly,
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
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index = (ui_state.cursor_index + 10)
                                    .min(app_state.disassembly.len().saturating_sub(1));
                            }
                            ActivePane::HexDump => {
                                let bytes_per_row = 16;
                                let padding = (app_state.origin as usize) % bytes_per_row;
                                let total_rows =
                                    (app_state.raw_data.len() + padding).div_ceil(bytes_per_row);
                                ui_state.hex_cursor_index = (ui_state.hex_cursor_index + 10)
                                    .min(total_rows.saturating_sub(1));
                            }
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
                        }
                    }
                    KeyCode::Home => {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => ui_state.cursor_index = 0,
                            ActivePane::HexDump => ui_state.hex_cursor_index = 0,
                        }
                    }
                    KeyCode::End => {
                        ui_state.input_buffer.clear();
                        match ui_state.active_pane {
                            ActivePane::Disassembly => {
                                ui_state.cursor_index =
                                    app_state.disassembly.len().saturating_sub(1)
                            }
                            ActivePane::HexDump => {
                                let bytes_per_row = 16;
                                let padding = (app_state.origin as usize) % bytes_per_row;
                                let total_rows =
                                    (app_state.raw_data.len() + padding).div_ceil(bytes_per_row);
                                ui_state.hex_cursor_index = total_rows.saturating_sub(1);
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
            ui_state.file_picker.open();
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

                if let Err(e) = app_state.save_project(cursor_addr, hex_addr) {
                    ui_state.set_status_message(format!("Error saving: {}", e));
                } else {
                    ui_state.set_status_message("Project saved");
                }
            } else {
                ui_state.save_dialog.open(SaveDialogMode::Project);
                ui_state.set_status_message("Enter Project filename");
            }
        }
        MenuAction::SaveAs => {
            ui_state.save_dialog.open(SaveDialogMode::Project);
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
                ui_state.save_dialog.open(SaveDialogMode::ExportProject);
                ui_state.set_status_message("Enter .asm filename");
            }
        }
        MenuAction::ExportProjectAs => {
            ui_state.save_dialog.open(SaveDialogMode::ExportProject);
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
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
        MenuAction::Address => {
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
            ui_state
                .jump_dialog
                .open(crate::ui_state::JumpDialogMode::Address);
            ui_state.status_message = "Enter address (Hex)".to_string();
        }
        MenuAction::JumpToLine => {
            ui_state
                .jump_dialog
                .open(crate::ui_state::JumpDialogMode::Line);
            ui_state.status_message = "Enter Line Number (Dec)".to_string();
        }
        MenuAction::Search => {
            ui_state.search_dialog.open();
            ui_state.set_status_message("Search...");
        }
        MenuAction::FindNext => {
            perform_search(app_state, ui_state, true);
        }
        MenuAction::FindPrevious => {
            perform_search(app_state, ui_state, false);
        }
        MenuAction::JumpToOperand => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                // Try to extract address from operand.
                // We utilize the opcode mode if available.
                if let Some(opcode) = &line.opcode {
                    use crate::cpu::AddressingMode;
                    let target = match opcode.mode {
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
                                Some(line.address.wrapping_add(2).wrapping_add(offset as u16))
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
                    };

                    if let Some(addr) = target {
                        // Perform Jump
                        let mut found_idx = None;
                        for (i, l) in app_state.disassembly.iter().enumerate() {
                            if l.address == addr {
                                found_idx = Some(i);
                                break;
                            } else if l.address > addr {
                                // Closest before
                                if i > 0 {
                                    found_idx = Some(i - 1);
                                } else {
                                    found_idx = Some(0);
                                }
                                break;
                            }
                        }

                        if let Some(idx) = found_idx {
                            ui_state.navigation_history.push((
                                crate::ui_state::ActivePane::Disassembly,
                                ui_state.cursor_index,
                            ));
                            ui_state.cursor_index = idx;
                            ui_state.status_message = format!("Jumped to ${:04X}", addr);
                        } else {
                            // Maybe valid address but not in loaded range?
                            // Or at end
                            if !app_state.disassembly.is_empty() {
                                if addr >= app_state.disassembly.last().unwrap().address {
                                    ui_state.navigation_history.push((
                                        crate::ui_state::ActivePane::Disassembly,
                                        ui_state.cursor_index,
                                    ));
                                    ui_state.cursor_index = app_state.disassembly.len() - 1;
                                    ui_state.status_message = "Jumped to end".to_string();
                                } else {
                                    ui_state.status_message =
                                        format!("Address ${:04X} not found", addr);
                                }
                            }
                        }
                    } else {
                        ui_state.status_message = "No target address".to_string();
                    }
                } else {
                    // Maybe it is a .WORD or .PTR?
                    // Not specified in requirements, but "Jump to operand" generally implies instruction operands.
                }
            }
        }
        MenuAction::About => {
            ui_state.about_dialog.open();
            ui_state.status_message = "About Regenerator 2000".to_string();
        }
        MenuAction::SetPetsciiUnshifted => {
            ui_state.petscii_mode = crate::ui_state::PetsciiMode::Unshifted;
            ui_state.set_status_message("PETSCII Mode: Unshifted");
        }
        MenuAction::SetPetsciiShifted => {
            ui_state.petscii_mode = crate::ui_state::PetsciiMode::Shifted;
            ui_state.set_status_message("PETSCII Mode: Shifted");
        }
        MenuAction::SetLoHi => {
            if let Some(start_index) = ui_state.selection_start {
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
            if let Some(start_index) = ui_state.selection_start {
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
                    .open(current_comment, crate::ui_state::CommentType::Side);
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
                    .open(current_comment, crate::ui_state::CommentType::Line);
                ui_state.set_status_message(format!("Edit Line Comment at ${:04X}", address));
            }
        }
        MenuAction::ToggleHexDump => {
            ui_state.show_hex_dump = !ui_state.show_hex_dump;
            if ui_state.show_hex_dump {
                ui_state.set_status_message("Hex Dump View Shown");
            } else {
                ui_state.set_status_message("Hex Dump View Hidden");
                // If we were in Hex view, switch to Disassembly
                if ui_state.active_pane == ActivePane::HexDump {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
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
    }
}

fn perform_search(app_state: &mut crate::state::AppState, ui_state: &mut UIState, forward: bool) {
    let query = &ui_state.search_dialog.last_search;
    if query.is_empty() {
        ui_state.set_status_message("No search query");
        return;
    }

    let query_lower = query.to_lowercase();
    let disassembly_len = app_state.disassembly.len();
    if disassembly_len == 0 {
        return;
    }

    let start_idx = if forward {
        // Start from next line
        if ui_state.cursor_index + 1 < disassembly_len {
            ui_state.cursor_index + 1
        } else {
            0 // Wrap
        }
    } else if ui_state.cursor_index > 0 {
        ui_state.cursor_index - 1
    } else {
        disassembly_len.saturating_sub(1)
    };

    let mut found_idx = None;

    // Iterate len times to cover the whole buffer if we wrap.
    // We implement a simple linear search from start_idx.

    for i in 0..disassembly_len {
        let idx = if forward {
            (start_idx + i) % disassembly_len
        } else {
            // backward wrap
            if i <= start_idx {
                start_idx - i
            } else {
                disassembly_len - (i - start_idx)
            }
        };

        if let Some(line) = app_state.disassembly.get(idx)
            && match_search(line, &query_lower)
        {
            found_idx = Some(idx);
            break;
        }
    }

    if let Some(idx) = found_idx {
        ui_state
            .navigation_history
            .push((ui_state.active_pane, ui_state.cursor_index));
        ui_state.cursor_index = idx;
        ui_state.set_status_message(format!("Found '{}'", query));
    } else {
        ui_state.set_status_message(format!("'{}' not found", query));
    }
}

fn match_search(line: &crate::disassembler::DisassemblyLine, query_lower: &str) -> bool {
    // Free search: address, bytes, mnemonic, operand, comments, labels

    // Address
    if format!("{:04x}", line.address).contains(query_lower) {
        return true;
    }

    // Bytes (hex string)
    // We can format bytes as hex string and check.
    // e.g. "A9 00" -> "a900" or "a9 00"
    // Let's check hex string without spaces for robust matching of byte sequences
    let bytes_hex = line
        .bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    if bytes_hex
        .match_indices(query_lower)
        .any(|(idx, _)| idx % 2 == 0)
    {
        return true;
    }

    // Also with spaces?
    let bytes_hex_spaces = line
        .bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ");
    if bytes_hex_spaces.contains(query_lower) {
        return true;
    }

    if line.mnemonic.to_lowercase().contains(query_lower) {
        return true;
    }

    if line.operand.to_lowercase().contains(query_lower) {
        return true;
    }

    if line.comment.to_lowercase().contains(query_lower) {
        return true;
    }

    if let Some(lc) = &line.line_comment
        && lc.to_lowercase().contains(query_lower)
    {
        return true;
    }

    if let Some(lbl) = &line.label
        && lbl.to_lowercase().contains(query_lower)
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;

    #[test]
    fn test_match_search_bytes_alignment() {
        let line = DisassemblyLine {
            address: 0x1000,
            bytes: vec![0x8D, 0x02, 0x08], // 8d0208
            mnemonic: "STA".to_string(),
            operand: "$0802".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            comment_address: None,
        };

        // "d020" is in "8d0208" starting at index 1 -> Should FAIL
        assert!(!match_search(&line, "d020"));

        // "8d02" is in "8d0208" starting at index 0 -> Should PASS
        assert!(match_search(&line, "8d02"));

        // "0208" is in "8d0208" starting at index 2 -> Should PASS
        assert!(match_search(&line, "0208"));

        // "d0" is in "8d0208" starting at index 1 -> Should FAIL
        assert!(!match_search(&line, "d0"));

        // "02" is in "8d0208" starting at index 2 -> Should PASS
        assert!(match_search(&line, "02"));
    }
}
