use crate::state::AppState;
use crate::ui_state::ActivePane;
use crate::ui_state::UIState;

pub fn handle_menu_action(
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

pub fn execute_menu_action(
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
                                // Fallback
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

                    // Check if already collapsed
                    if let Some(&range) = app_state
                        .collapsed_blocks
                        .iter()
                        .find(|(s, e)| *s == start_offset && *e == end_offset)
                    {
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
