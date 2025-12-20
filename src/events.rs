use crate::state::AppState;
use crate::ui::ui;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use std::io;

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut state: AppState) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if state.jump_dialog.active {
                match key.code {
                    KeyCode::Esc => {
                        state.jump_dialog.close();
                        state.status_message = "Ready".to_string();
                    }
                    KeyCode::Enter => {
                        let input = &state.jump_dialog.input;
                        if let Ok(addr) = u16::from_str_radix(input, 16) {
                            // Find closest address in disassembly
                            // Since disassembly is sorted by address (mostly), we can find it.
                            // But map is easier.
                            // Better: convert address to index.
                            // We need to find the line that contains this address.

                            // Simple linear search for now or binary search if vector is sorted.
                            // It is sorted by definition of disassembly.

                            // We need to account for origin.
                            let target_addr = addr;
                            // We iterate to find the line that covers this address
                            let mut found_idx = None;
                            for (i, line) in state.disassembly.iter().enumerate() {
                                if line.address == target_addr {
                                    found_idx = Some(i);
                                    break;
                                } else if line.address > target_addr {
                                    // We went past it, give the previous one (closest)
                                    if i > 0 {
                                        found_idx = Some(i - 1);
                                    } else {
                                        found_idx = Some(0);
                                    }
                                    break;
                                }
                            }

                            if let Some(idx) = found_idx {
                                state.navigation_history.push(state.cursor_index);
                                state.cursor_index = idx;
                                state.status_message = format!("Jumped to ${:04X}", target_addr);
                            } else {
                                // Address might be beyond the last line
                                if !state.disassembly.is_empty() {
                                    state.navigation_history.push(state.cursor_index);
                                    state.cursor_index = state.disassembly.len() - 1;
                                    state.status_message = "Jumped to end".to_string();
                                }
                            }
                            state.jump_dialog.close();
                        } else {
                            state.status_message = "Invalid Hex Address".to_string();
                        }
                    }
                    KeyCode::Backspace => {
                        state.jump_dialog.input.pop();
                    }
                    KeyCode::Char(c) => {
                        if c.is_ascii_hexdigit() && state.jump_dialog.input.len() < 4 {
                            state.jump_dialog.input.push(c.to_ascii_uppercase());
                        }
                    }
                    _ => {}
                }
            } else if state.file_picker.active {
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

                    KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        handle_menu_action(&mut state, "Jump to address");
                    }

                    // Only handle Enter for Jump to Operand if NO modifiers (to avoid conflict)
                    KeyCode::Enter if key.modifiers.is_empty() => {
                        handle_menu_action(&mut state, "Jump to operand");
                    }

                    KeyCode::Backspace => {
                        if let Some(prev_idx) = state.navigation_history.pop() {
                            // Verify index is still valid
                            if prev_idx < state.disassembly.len() {
                                state.cursor_index = prev_idx;
                                state.status_message = "Navigated back".to_string();
                            } else {
                                state.status_message = "History invalid".to_string();
                            }
                        } else {
                            state.status_message = "No history".to_string();
                        }
                    }

                    // Data Conversion Shortcuts
                    KeyCode::Char('c') => handle_menu_action(&mut state, "Code"),
                    KeyCode::Char('b') => handle_menu_action(&mut state, "Byte"),
                    KeyCode::Char('w') => handle_menu_action(&mut state, "Word"),
                    KeyCode::Char('p') => handle_menu_action(&mut state, "Pointer"),

                    // Normal Navigation
                    KeyCode::Down | KeyCode::Char('j') => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            if state.selection_start.is_none() {
                                state.selection_start = Some(state.cursor_index);
                            }
                        } else {
                            state.selection_start = None;
                        }

                        if state.cursor_index < state.disassembly.len().saturating_sub(1) {
                            state.cursor_index += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            if state.selection_start.is_none() {
                                state.selection_start = Some(state.cursor_index);
                            }
                        } else {
                            state.selection_start = None;
                        }

                        if state.cursor_index > 0 {
                            state.cursor_index -= 1;
                        }
                    }
                    KeyCode::Esc => {
                        if state.selection_start.is_some() {
                            state.selection_start = None;
                            state.status_message = "Selection cleared".to_string();
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

    // Helper to get range, returns Option
    let get_range = |state: &AppState| -> Option<(usize, usize)> {
        if let Some(selection_start) = state.selection_start {
            let (s, e) = if selection_start < state.cursor_index {
                (selection_start, state.cursor_index)
            } else {
                (state.cursor_index, selection_start)
            };

            if let (Some(start_line), Some(end_line)) =
                (state.disassembly.get(s), state.disassembly.get(e))
            {
                let start_addr = start_line.address;
                let end_addr_inclusive = end_line.address + end_line.bytes.len() as u16 - 1;

                let start_idx = (start_addr.wrapping_sub(state.origin)) as usize;
                let end_idx = (end_addr_inclusive.wrapping_sub(state.origin)) as usize;

                Some((start_idx, end_idx))
            } else {
                None
            }
        } else {
            // Single line action
            if let Some(line) = state.disassembly.get(state.cursor_index) {
                let start_addr = line.address;
                let end_addr_inclusive = line.address + line.bytes.len() as u16 - 1;

                let start_idx = (start_addr.wrapping_sub(state.origin)) as usize;
                let end_idx = (end_addr_inclusive.wrapping_sub(state.origin)) as usize;
                Some((start_idx, end_idx))
            } else {
                None
            }
        }
    };

    // Helper to update range
    let mut update_type = |new_type: crate::state::AddressType| {
        if let Some((start, end)) = get_range(state) {
            // Boundary check
            let max_len = state.address_types.len();
            if start < max_len {
                let valid_end = end.min(max_len - 1);
                for i in start..=valid_end {
                    state.address_types[i] = new_type;
                }
                // Clear selection after action
                state.selection_start = None;
                state.disassemble();
            }
        }
    };

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
        "Code" => update_type(crate::state::AddressType::Code),
        "Byte" => update_type(crate::state::AddressType::DataByte),
        "Word" => update_type(crate::state::AddressType::DataWord),
        "Pointer" => update_type(crate::state::AddressType::DataPtr),
        "Jump to address" => {
            state.jump_dialog.open();
            state.status_message = "Enter address (Hex)".to_string();
        }
        "Jump to operand" => {
            if let Some(line) = state.disassembly.get(state.cursor_index) {
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
                        for (i, l) in state.disassembly.iter().enumerate() {
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
                            state.navigation_history.push(state.cursor_index);
                            state.cursor_index = idx;
                            state.status_message = format!("Jumped to ${:04X}", addr);
                        } else {
                            // Maybe valid address but not in loaded range?
                            // Or at end
                            if !state.disassembly.is_empty() {
                                if addr >= state.disassembly.last().unwrap().address {
                                    state.navigation_history.push(state.cursor_index);
                                    state.cursor_index = state.disassembly.len() - 1;
                                    state.status_message = "Jumped to end".to_string();
                                } else {
                                    state.status_message =
                                        format!("Address ${:04X} not found", addr);
                                }
                            }
                        }
                    } else {
                        state.status_message = "No target address".to_string();
                    }
                } else {
                    // Maybe it is a .WORD or .PTR?
                    // Not specified in requirements, but "Jump to operand" generally implies instruction operands.
                }
            }
        }
        _ => {}
    }
}
