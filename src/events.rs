use crate::state::AppState;
use crate::ui::ui;
use crate::ui_state::{ActivePane, SaveDialogMode, UIState};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use std::io;

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app_state: AppState,
    mut ui_state: UIState,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app_state, &mut ui_state))?;

        if let Event::Key(key) = event::read()? {
            ui_state.dismiss_logo = true;
            if ui_state.jump_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        ui_state.jump_dialog.close();
                        ui_state.set_status_message("Ready");
                    }
                    KeyCode::Enter => {
                        let input = &ui_state.jump_dialog.input;
                        if let Ok(addr) = u16::from_str_radix(input, 16) {
                            // Find closest address in disassembly
                            let target_addr = addr;
                            let mut found_idx = None;
                            for (i, line) in app_state.disassembly.iter().enumerate() {
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
                                ui_state.navigation_history.push(ui_state.cursor_index);
                                ui_state.cursor_index = idx;
                                ui_state
                                    .set_status_message(format!("Jumped to ${:04X}", target_addr));
                            } else {
                                if !app_state.disassembly.is_empty() {
                                    ui_state.navigation_history.push(ui_state.cursor_index);
                                    ui_state.cursor_index = app_state.disassembly.len() - 1;
                                    ui_state.set_status_message("Jumped to end");
                                }
                            }
                            ui_state.jump_dialog.close();
                        } else {
                            ui_state.set_status_message("Invalid Hex Address");
                        }
                    }
                    KeyCode::Backspace => {
                        ui_state.jump_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        if c.is_ascii_hexdigit() && ui_state.jump_dialog.input.len() < 4 {
                            ui_state.jump_dialog.input.push(c.to_ascii_uppercase());
                        }
                    }
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
                                if let Err(e) = app_state.save_project(cursor_addr) {
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
                                if let Err(e) = crate::exporter::export_asm(&app_state, &path) {
                                    ui_state.set_status_message(format!("Error exporting: {}", e));
                                } else {
                                    ui_state.set_status_message("ASM Exported");
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
                                app_state.undo_stack.push(command);

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
                                        refs: Vec::new(),
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
                                    app_state.undo_stack.push(command);

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
                                    Ok(loaded_cursor) => {
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
                                    }
                                }
                            }
                        }
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
                            let action = ui_state.menu.categories[category_idx].items[item_idx]
                                .action
                                .clone();
                            if let Some(action) = action {
                                handle_menu_action(&mut app_state, &mut ui_state, action);
                                // Close menu after valid action
                                ui_state.menu.active = false;
                                ui_state.menu.selected_item = None;
                            }
                        } else {
                            // Enter on category -> open first item?
                            ui_state.menu.selected_item = Some(0);
                        }
                    }
                    _ => {}
                }
            } else if ui_state.about_dialog.active {
                if let KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_) = key.code {
                    ui_state.about_dialog.close();
                    ui_state.set_status_message("Ready");
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
                        } else {
                            ui_state.settings_dialog.close();
                            ui_state.set_status_message("Ready");
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
                        } else if !ui_state.settings_dialog.is_editing_xref_count {
                            ui_state.settings_dialog.previous();
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
                        } else if !ui_state.settings_dialog.is_editing_xref_count {
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
                            } else {
                                // Invalid input, maybe keep editing or reset?
                                // For now, keep editing.
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
                                    ui_state.settings_dialog.is_selecting_platform = true;
                                }
                                5 => {
                                    ui_state.settings_dialog.is_selecting_assembler = true;
                                }
                                6 => {
                                    ui_state.settings_dialog.is_editing_xref_count = true;
                                    ui_state.settings_dialog.xref_count_input =
                                        app_state.settings.max_xref_count.to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if ui_state.settings_dialog.is_editing_xref_count {
                            ui_state.settings_dialog.xref_count_input.pop();
                        }
                    }
                    KeyCode::Char(c) => {
                        if ui_state.settings_dialog.is_editing_xref_count {
                            if c.is_ascii_digit() {
                                ui_state.settings_dialog.xref_count_input.push(c);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        ui_state.should_quit = true;
                    }
                    KeyCode::F(10) => {
                        ui_state.menu.active = true;
                        ui_state.menu.selected_item = Some(0);
                        ui_state.set_status_message("Menu Active");
                    }
                    // Global Shortcuts
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::New,
                        )
                    }
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::Open,
                        )
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
                                crate::ui_state::MenuAction::ExportAsmAs,
                            );
                        } else {
                            handle_menu_action(
                                &mut app_state,
                                &mut ui_state,
                                crate::ui_state::MenuAction::ExportAsm,
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
                        ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        ui_state.cursor_index = (ui_state.cursor_index + 10)
                            .min(app_state.disassembly.len().saturating_sub(1));
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
                    KeyCode::Char('+') | KeyCode::Char('=')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ZoomIn,
                        )
                    }
                    KeyCode::Char('-') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ZoomOut,
                        )
                    }
                    KeyCode::Char('0') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::ResetZoom,
                        )
                    }

                    KeyCode::Char('g') => {
                        handle_menu_action(
                            &mut app_state,
                            &mut ui_state,
                            crate::ui_state::MenuAction::JumpToAddress,
                        );
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
                        if let Some(prev_idx) = ui_state.navigation_history.pop() {
                            // Verify index is still valid
                            if prev_idx < app_state.disassembly.len() {
                                ui_state.cursor_index = prev_idx;
                                ui_state.set_status_message("Navigated back");
                            } else {
                                ui_state.set_status_message("History invalid");
                            }
                        } else {
                            ui_state.set_status_message("No history");
                        }
                    }

                    // Data Conversion Shortcuts
                    KeyCode::Char('c') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Code,
                    ),
                    KeyCode::Char('b') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Byte,
                    ),
                    KeyCode::Char('w') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Word,
                    ),
                    KeyCode::Char('a') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Address,
                    ),
                    KeyCode::Char('t') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Text,
                    ),
                    KeyCode::Char('s') => handle_menu_action(
                        &mut app_state,
                        &mut ui_state,
                        crate::ui_state::MenuAction::Screencode,
                    ),

                    // Label
                    KeyCode::Char('l') => {
                        if !ui_state.menu.active
                            && !ui_state.jump_dialog.active
                            && !ui_state.save_dialog.active
                            && !ui_state.file_picker.active
                        {
                            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                                let addr = line.address;
                                let text = app_state
                                    .labels
                                    .get(&addr)
                                    .and_then(|v| v.first())
                                    .map(|l| l.name.as_str());
                                ui_state.label_dialog.open(text);
                                ui_state.set_status_message("Enter Label");
                            }
                        }
                    }

                    // Normal Navigation
                    KeyCode::Down | KeyCode::Char('j') => match ui_state.active_pane {
                        ActivePane::Disassembly => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                if ui_state.selection_start.is_none() {
                                    ui_state.selection_start = Some(ui_state.cursor_index);
                                }
                            } else {
                                ui_state.selection_start = None;
                            }

                            if ui_state.cursor_index < app_state.disassembly.len().saturating_sub(1)
                            {
                                ui_state.cursor_index += 1;
                            }
                        }
                        ActivePane::Hex => {
                            let bytes_per_row = 16;
                            let total_rows =
                                (app_state.raw_data.len() + bytes_per_row - 1) / bytes_per_row;
                            if ui_state.hex_cursor_index < total_rows.saturating_sub(1) {
                                ui_state.hex_cursor_index += 1;
                            }
                        }
                    },
                    KeyCode::Up | KeyCode::Char('k') => match ui_state.active_pane {
                        ActivePane::Disassembly => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
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
                        ActivePane::Hex => {
                            if ui_state.hex_cursor_index > 0 {
                                ui_state.hex_cursor_index -= 1;
                            }
                        }
                    },
                    KeyCode::Tab => {
                        ui_state.active_pane = match ui_state.active_pane {
                            ActivePane::Disassembly => ActivePane::Hex,
                            ActivePane::Hex => ActivePane::Disassembly,
                        };
                    }
                    KeyCode::Esc => {
                        if ui_state.selection_start.is_some() {
                            ui_state.selection_start = None;
                            ui_state.set_status_message("Selection cleared");
                        }
                    }
                    KeyCode::PageDown => match ui_state.active_pane {
                        ActivePane::Disassembly => {
                            ui_state.cursor_index = (ui_state.cursor_index + 10)
                                .min(app_state.disassembly.len().saturating_sub(1));
                        }
                        ActivePane::Hex => {
                            let bytes_per_row = 16;
                            let total_rows =
                                (app_state.raw_data.len() + bytes_per_row - 1) / bytes_per_row;
                            ui_state.hex_cursor_index =
                                (ui_state.hex_cursor_index + 10).min(total_rows.saturating_sub(1));
                        }
                    },
                    KeyCode::PageUp => match ui_state.active_pane {
                        ActivePane::Disassembly => {
                            ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                        }
                        ActivePane::Hex => {
                            ui_state.hex_cursor_index =
                                ui_state.hex_cursor_index.saturating_sub(10);
                        }
                    },
                    KeyCode::Home => match ui_state.active_pane {
                        ActivePane::Disassembly => ui_state.cursor_index = 0,
                        ActivePane::Hex => ui_state.hex_cursor_index = 0,
                    },
                    KeyCode::End => match ui_state.active_pane {
                        ActivePane::Disassembly => {
                            ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1)
                        }
                        ActivePane::Hex => {
                            let bytes_per_row = 16;
                            let total_rows =
                                (app_state.raw_data.len() + bytes_per_row - 1) / bytes_per_row;
                            ui_state.hex_cursor_index = total_rows.saturating_sub(1);
                        }
                    },
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
    ui_state.set_status_message(format!("Action: {:?}", action));

    use crate::ui_state::MenuAction;

    match action {
        MenuAction::Exit => ui_state.should_quit = true,
        MenuAction::New => {
            // Placeholder
        }
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
                if let Err(e) = app_state.save_project(cursor_addr) {
                    ui_state.set_status_message(format!("Error saving: {}", e));
                } else {
                    ui_state.set_status_message("Project saved");
                }
            } else {
                ui_state.save_dialog.open(SaveDialogMode::Project);
                ui_state.set_status_message("Enter filename");
            }
        }
        MenuAction::SaveAs => {
            ui_state.save_dialog.open(SaveDialogMode::Project);
            ui_state.set_status_message("Enter filename");
        }
        MenuAction::ExportAsm => {
            ui_state.save_dialog.open(SaveDialogMode::ExportAsm);
            ui_state.set_status_message("Enter filename for ASM");
        }
        MenuAction::ExportAsmAs => {
            ui_state.save_dialog.open(SaveDialogMode::ExportAsm);
            ui_state.set_status_message("Enter filename for ASM");
        }
        MenuAction::DocumentSettings => {
            ui_state.settings_dialog.open();
            ui_state.set_status_message("Document Settings");
        }
        MenuAction::Analyze => {
            ui_state.set_status_message(app_state.perform_analysis());
            // Move cursor to origin
            if let Some(idx) = app_state.get_line_index_for_address(app_state.origin) {
                ui_state.cursor_index = idx;
            }
        }
        MenuAction::Undo => {
            ui_state.set_status_message(app_state.undo_last_command());
        }
        MenuAction::Redo => {
            ui_state.set_status_message(app_state.redo_last_command());
        }
        MenuAction::ZoomIn => {}
        MenuAction::ZoomOut => {}
        MenuAction::ResetZoom => {}
        MenuAction::Code => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::Code,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::Byte => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::DataByte,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::Word => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::DataWord,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::Address => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::Address,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::Text => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::Text,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::Screencode => {
            let new_cursor = ui_state
                .selection_start
                .map(|start| start.min(ui_state.cursor_index));

            app_state.set_block_type_region(
                crate::state::BlockType::Screencode,
                ui_state.selection_start,
                ui_state.cursor_index,
            );
            if let Some(idx) = new_cursor {
                ui_state.cursor_index = idx;
            }
            ui_state.selection_start = None;
        }
        MenuAction::JumpToAddress => {
            ui_state.jump_dialog.open();
            ui_state.status_message = "Enter address (Hex)".to_string();
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
                            ui_state.navigation_history.push(ui_state.cursor_index);
                            ui_state.cursor_index = idx;
                            ui_state.status_message = format!("Jumped to ${:04X}", addr);
                        } else {
                            // Maybe valid address but not in loaded range?
                            // Or at end
                            if !app_state.disassembly.is_empty() {
                                if addr >= app_state.disassembly.last().unwrap().address {
                                    ui_state.navigation_history.push(ui_state.cursor_index);
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
            ui_state.status_message = "About Regenerator2000".to_string();
        }
    }
}
