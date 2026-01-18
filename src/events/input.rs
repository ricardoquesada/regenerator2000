use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::menu::handle_menu_action;

pub fn handle_global_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Exit);
        }
        KeyCode::Char('/') if key.modifiers.is_empty() => {
            ui_state.vim_search_active = true;
            ui_state.vim_search_input.clear();
        }
        KeyCode::Char('n') if key.modifiers.is_empty() => {
            crate::dialog_search::perform_search(app_state, ui_state, true);
        }
        KeyCode::Char('N')
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            crate::dialog_search::perform_search(app_state, ui_state, false);
        }
        KeyCode::F(10) => {
            ui_state.menu.active = true;
            ui_state.menu.select_first_enabled_item();
            ui_state.set_status_message("Menu Active");
        }
        // Global Shortcuts
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Open)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Search);
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Analyze);
        }
        KeyCode::F(3) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                handle_menu_action(
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::FindPrevious,
                );
            } else {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::FindNext);
            }
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SaveAs);
            } else {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Save);
            }
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                handle_menu_action(
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::ExportProjectAs,
                );
            } else {
                handle_menu_action(
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::ExportProject,
                );
            }
        }

        KeyCode::Char(',') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::SystemSettings,
            );
        }

        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            match ui_state.active_pane {
                ActivePane::Disassembly => {
                    ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                }
                ActivePane::HexDump => {
                    ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(10);
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
                    app_state,
                    ui_state,
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
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Undo);
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Redo);
        }
        KeyCode::Char('2') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleHexDump,
            );
        }
        KeyCode::Char('3') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleSpritesView,
            );
        }
        KeyCode::Char('4') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleCharsetView,
            );
        }
        KeyCode::Char('5') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleBlocksView,
            );
        }

        KeyCode::Char('g') => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    handle_menu_action(
                        app_state,
                        ui_state,
                        crate::ui_state::MenuAction::JumpToLine,
                    );
                }
            } else if key.modifiers.is_empty() {
                handle_menu_action(
                    app_state,
                    ui_state,
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
                    ui_state.set_status_message(format!("Jumped to line {}", target_line));
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
                app_state,
                ui_state,
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
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Code)
            }
        }
        KeyCode::Char('b') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Byte)
            }
        }
        KeyCode::Char('w') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Word)
            }
        }
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Address)
            }
        }
        KeyCode::Char('t') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Text)
            }
        }
        KeyCode::Char('s') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Screencode)
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
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Undefined)
            }
        }
        // Handle Shift+/ as ?
        KeyCode::Char('?') if key.modifiers == KeyModifiers::SHIFT => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::Undefined)
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
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SetLoHi)
            }
        }
        // Handle Shift+, as <
        KeyCode::Char('<') if key.modifiers == KeyModifiers::SHIFT => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SetLoHi)
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
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SetHiLo)
            }
        }
        // Handle Shift+. as >
        KeyCode::Char('>') if key.modifiers == KeyModifiers::SHIFT => {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(app_state, ui_state, crate::ui_state::MenuAction::SetHiLo)
            }
        }
        KeyCode::Char(';') if key.modifiers.is_empty() => {
            if ui_state.active_pane == ActivePane::Disassembly {
                handle_menu_action(
                    app_state,
                    ui_state,
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
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::ToggleSplitter,
                );
            }
        }
        KeyCode::Char('|')
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
        {
            if ui_state.active_pane == ActivePane::Disassembly
                || ui_state.active_pane == ActivePane::Blocks
            {
                handle_menu_action(
                    app_state,
                    ui_state,
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
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::LineComment,
                )
            }
        }
        // Handle Shift+; as :
        KeyCode::Char(':') if key.modifiers == KeyModifiers::SHIFT => {
            if ui_state.active_pane == ActivePane::Disassembly {
                handle_menu_action(
                    app_state,
                    ui_state,
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
                let max_char_index = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

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
                app_state,
                ui_state,
                crate::ui_state::MenuAction::NextImmediateFormat,
            );
        }

        // Previous Immediate Format (Shift+d)
        KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::PreviousImmediateFormat,
            );
        }

        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_menu_action(
                app_state,
                ui_state,
                crate::ui_state::MenuAction::ToggleCollapsedBlock,
            );
        }

        // External File
        KeyCode::Char('e') if key.modifiers.is_empty() => {
            handle_menu_action(
                app_state,
                ui_state,
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
                    if key.modifiers.contains(KeyModifiers::SHIFT) || ui_state.is_visual_mode {
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
                    } else if ui_state.cursor_index < app_state.disassembly.len().saturating_sub(1)
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
        KeyCode::Up | KeyCode::Char('k') if key.code == KeyCode::Up || key.modifiers.is_empty() => {
            ui_state.input_buffer.clear();
            match ui_state.active_pane {
                ActivePane::Disassembly => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) || ui_state.is_visual_mode {
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
                ui_state.cursor_index =
                    (ui_state.cursor_index + 30).min(app_state.disassembly.len().saturating_sub(1));
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
                ui_state.sprites_cursor_index =
                    (ui_state.sprites_cursor_index + 10).min(total_sprites.saturating_sub(1));
            } else if ui_state.active_pane == ActivePane::Charset {
                let origin = app_state.origin as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                let end_addr = origin + app_state.raw_data.len();
                let max_char_index = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                ui_state.charset_cursor_index =
                    (ui_state.charset_cursor_index + 256).min(max_char_index.saturating_sub(1));
            }
        }
        KeyCode::PageUp => {
            ui_state.input_buffer.clear();
            match ui_state.active_pane {
                ActivePane::Disassembly => {
                    ui_state.cursor_index = ui_state.cursor_index.saturating_sub(10);
                }
                ActivePane::HexDump => {
                    ui_state.hex_cursor_index = ui_state.hex_cursor_index.saturating_sub(10);
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
                    ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1)
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
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::ToggleSpriteMulticolor,
                )
            } else if ui_state.active_pane == ActivePane::Charset {
                handle_menu_action(
                    app_state,
                    ui_state,
                    crate::ui_state::MenuAction::ToggleCharsetMulticolor,
                )
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if ui_state.active_pane == ActivePane::Charset && ui_state.charset_cursor_index > 0 {
                ui_state.charset_cursor_index -= 1;
            }
        }
        KeyCode::Right => {
            if ui_state.active_pane == ActivePane::Charset {
                let origin = app_state.origin as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                let end_addr = origin + app_state.raw_data.len();
                let max_char_index = (end_addr.saturating_sub(aligned_start_addr)).div_ceil(8);

                if ui_state.charset_cursor_index < max_char_index.saturating_sub(1) {
                    ui_state.charset_cursor_index += 1;
                }
            }
        }
        _ => {}
    }
}
