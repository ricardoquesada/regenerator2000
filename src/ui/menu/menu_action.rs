use crate::ui_state::{ActivePane, UIState};
use regenerator_core::Core;
use regenerator_core::event::{CommentKind as DialogCommentKind, CoreEvent, DialogType};
use regenerator_core::state::AppState;
pub use regenerator_core::state::actions::AppAction;

/// Returns the file stem (no extension, no path) of the currently open file or project,
/// to be used as a default autocomplete value in save/export dialogs.
fn get_default_filename_stem(app_state: &AppState) -> Option<String> {
    // Prefer project_path if it exists, otherwise fall back to file_path.
    let path = app_state
        .project_path
        .as_ref()
        .or(app_state.file_path.as_ref())?;
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(std::string::ToString::to_string)
}

fn vice_open_breakpoint_dialog(app_state: &AppState, ui_state: &mut UIState) {
    let prefill = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address.0);
    ui_state.active_dialog = Some(Box::new(
        crate::ui::dialog_breakpoint_address::BreakpointAddressDialog::new(prefill),
    ));
}

fn vice_toggle_breakpoint_at(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    addr: crate::state::Addr,
) {
    if let Some(client) = &app_state.vice_client {
        if let Some(pos) = app_state.vice_state.breakpoints.iter().position(|bp| {
            bp.address == addr.0 && bp.kind == crate::vice::state::BreakpointKind::Exec
        }) {
            let id = app_state.vice_state.breakpoints[pos].id;
            client.send_checkpoint_delete(id);
            app_state.vice_state.breakpoints.remove(pos);
            ui_state.set_status_message(format!("Breakpoint removed at ${addr:04X}"));
        } else {
            client.send_checkpoint_set_exec(addr.0);
            ui_state.set_status_message(format!("Breakpoint set at ${addr:04X}"));
        }
    } else {
        ui_state.set_status_message("Not connected to VICE");
    }
}

fn vice_open_watchpoint_dialog(app_state: &AppState, ui_state: &mut UIState) {
    let prefill = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address.0);
    ui_state.active_dialog = Some(Box::new(
        crate::ui::dialog_watchpoint_address::WatchpointAddressDialog::new(prefill),
    ));
}

fn vice_toggle_watchpoint(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    addr: crate::state::Addr,
    kind: crate::vice::state::BreakpointKind,
) {
    if let Some(client) = &app_state.vice_client {
        if let Some(pos) = app_state
            .vice_state
            .breakpoints
            .iter()
            .position(|bp| bp.address == addr.0 && bp.kind == kind)
        {
            let id = app_state.vice_state.breakpoints[pos].id;
            client.send_checkpoint_delete(id);
            app_state.vice_state.breakpoints.remove(pos);
            ui_state.set_status_message(format!(
                "[{}] watchpoint removed at ${:04X}",
                kind.label(),
                addr
            ));
        } else {
            match kind {
                crate::vice::state::BreakpointKind::Load => client.send_checkpoint_set_load(addr.0),
                crate::vice::state::BreakpointKind::Store => {
                    client.send_checkpoint_set_store(addr.0)
                }
                crate::vice::state::BreakpointKind::LoadStore => {
                    client.send_checkpoint_set_load_store(addr.0);
                }
                crate::vice::state::BreakpointKind::Exec => {}
            }
            ui_state.set_status_message(format!(
                "[{}] watchpoint set at ${:04X}",
                kind.label(),
                addr
            ));
        }
    } else {
        ui_state.set_status_message("Not connected to VICE");
    }
}

pub fn handle_menu_action(core: &mut Core, ui_state: &mut UIState, action: AppAction) {
    if action.requires_document() && core.state.raw_data.is_empty() {
        ui_state.set_status_message("No open document");
        return;
    }

    // Context-specific checks for actions that didn't fit in update_availability
    // or need enforcement even via shortcuts
    if action == AppAction::FindReferences && ui_state.active_pane != ActivePane::Disassembly {
        ui_state.set_status_message("Action only available in Disassembly View");
        return;
    }

    // Check for changes on destructive actions
    let is_destructive = matches!(
        action,
        AppAction::Exit | AppAction::Open | AppAction::OpenRecent
    );

    if is_destructive && core.state.is_dirty() {
        ui_state.active_dialog = Some(Box::new(
            crate::ui::dialog_confirmation::ConfirmationDialog::new(
                "Unsaved Changes",
                "You have unsaved changes. Proceed?",
                action,
            ),
        ));
        return;
    }

    let events = core.apply_action(action.clone());
    for event in events {
        match event {
            CoreEvent::QuitRequested => ui_state.should_quit = true,
            CoreEvent::StatusMessage(msg) => ui_state.set_status_message(msg),
            CoreEvent::DialogRequested(dialog_type) => {
                match dialog_type {
                    DialogType::Open => {
                        ui_state.active_dialog =
                            Some(Box::new(crate::ui::dialog_open::OpenDialog::new(
                                ui_state.file_dialog_current_dir.clone(),
                            )));
                    }
                    DialogType::OpenRecent => {
                        ui_state.active_dialog =
                            Some(Box::new(crate::ui::dialog_open_recent::OpenRecentDialog));
                        ui_state.recent_list_state.select(Some(0));
                    }
                    DialogType::ImportViceLabels => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_open::OpenDialog::new_import_vice_labels(
                                ui_state.file_dialog_current_dir.clone(),
                                core.state.last_import_labels_path.clone(),
                            ),
                        ));
                    }
                    DialogType::ExportLabels { initial_filename } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_export_labels::ExportLabelsDialog::new(
                                initial_filename,
                            ),
                        ));
                    }
                    DialogType::SaveAs { initial_filename } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_save_as::SaveAsDialog::new(initial_filename),
                        ));
                    }
                    DialogType::ExportAs { initial_filename } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_export_as::ExportAsDialog::new(initial_filename),
                        ));
                    }
                    DialogType::DocumentSettings => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_document_settings::DocumentSettingsDialog::new(),
                        ));
                    }
                    DialogType::JumpToAddress => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_jump_to_address::JumpToAddressDialog::new(),
                        ));
                    }
                    DialogType::JumpToLine => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_jump_to_line::JumpToLineDialog::new(),
                        ));
                    }
                    DialogType::Search { query, filters } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_search::SearchDialog::new(query, filters),
                        ));
                    }
                    DialogType::GoToSymbol => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_go_to_symbol::GoToSymbolDialog::new(&core.state),
                        ));
                    }
                    DialogType::KeyboardShortcuts => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_keyboard_shortcut::ShortcutsDialog::new(),
                        ));
                    }
                    DialogType::About => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_about::AboutDialog::new(ui_state),
                        ));
                    }
                    DialogType::ViceConnect => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_vice_connect::ViceConnectDialog::new(),
                        ));
                    }
                    DialogType::Label {
                        address,
                        initial_name,
                        ..
                    } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_label::LabelDialog::new(Some(&initial_name), address),
                        ));
                    }
                    DialogType::Comment {
                        address,
                        current,
                        kind,
                    } => {
                        let dialog_kind = match kind {
                            DialogCommentKind::Side => crate::ui::dialog_comment::CommentType::Side,
                            DialogCommentKind::Line => crate::ui::dialog_comment::CommentType::Line,
                        };
                        ui_state.active_dialog =
                            Some(Box::new(crate::ui::dialog_comment::CommentDialog::new(
                                current.as_deref(),
                                dialog_kind,
                                address,
                            )));
                    }
                    DialogType::Confirmation {
                        title,
                        message,
                        action,
                    } => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_confirmation::ConfirmationDialog::new(
                                title, message, action,
                            ),
                        ));
                    }
                    DialogType::Bookmarks => {
                        ui_state.active_dialog =
                            Some(Box::new(crate::ui::dialog_bookmarks::BookmarksDialog::new()));
                    }
                    DialogType::FindReferences(addr) => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_find_references::FindReferencesDialog::new(
                                &core.state,
                                addr,
                            ),
                        ));
                    }
                    DialogType::BreakpointAddress(addr) => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_breakpoint_address::BreakpointAddressDialog::new(
                                addr,
                            ),
                        ));
                    }
                    DialogType::WatchpointAddress(addr) => {
                        ui_state.active_dialog = Some(Box::new(
                            crate::ui::dialog_watchpoint_address::WatchpointAddressDialog::new(
                                addr,
                            ),
                        ));
                    }
                    _ => {
                        // All dialogs should be handled here now.
                    }
                }
            }
            CoreEvent::DialogDismissalRequested => {
                ui_state.active_dialog = None;
            }
            _ => {
                // For other events, we might need more handling or they are already applied to core state/view
                // The TUI loop in events.rs already syncs core.view back to ui_state.core
            }
        }
    }
}

pub fn execute_menu_action(app_state: &mut AppState, ui_state: &mut UIState, action: AppAction) {
    ui_state.set_status_message(format!("Action: {action:?}"));

    match action {
        AppAction::NavigateBack => {
            // This is primarily handled in Core::apply_action, but included here for completeness
            // if called via legacy paths.
            if let Some((pane, target)) = ui_state.navigation_history.pop() {
                ui_state.active_pane = pane;
                match target {
                    crate::ui_state::NavigationTarget::Address(addr) => {
                        crate::navigation::perform_jump_to_address_no_history(
                            app_state,
                            &mut ui_state.core,
                            crate::state::Addr(addr),
                        );
                    }
                    crate::ui_state::NavigationTarget::Index(idx) => {
                        ui_state.cursor_index = idx;
                    }
                }
                ui_state.set_status_message("Navigated back");
            } else {
                ui_state.set_status_message("No history");
            }
        }
        AppAction::Exit => ui_state.should_quit = true,

        AppAction::Open => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_open::OpenDialog::new(
                ui_state.file_dialog_current_dir.clone(),
            )));
            ui_state.set_status_message("Select a file to open");
        }
        AppAction::OpenRecent => {
            ui_state.active_dialog =
                Some(Box::new(crate::ui::dialog_open_recent::OpenRecentDialog));
            ui_state.recent_list_state.select(Some(0));
            ui_state.set_status_message("Open recent project");
        }
        AppAction::ImportViceLabels => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_open::OpenDialog::new_import_vice_labels(
                    ui_state.file_dialog_current_dir.clone(),
                    app_state.last_import_labels_path.clone(),
                ),
            ));
            ui_state.set_status_message("Select a VICE label file to import");
        }
        AppAction::ExportViceLabels => {
            let initial = app_state
                .last_export_labels_filename
                .clone()
                .or_else(|| get_default_filename_stem(app_state));
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_export_labels::ExportLabelsDialog::new(initial),
            ));
            ui_state.set_status_message("Enter VICE label filename");
        }
        AppAction::Save => {
            if app_state.project_path.is_some() {
                let context = create_save_context(app_state, ui_state);
                if let Err(e) = app_state.save_project(context, true) {
                    ui_state.set_status_message(format!("Error saving: {e}"));
                } else {
                    let filename = app_state
                        .project_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy();
                    ui_state.set_status_message(format!("Saved: {filename}"));
                }
            } else {
                let initial = app_state
                    .last_save_as_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(app_state));
                ui_state.active_dialog = Some(Box::new(
                    crate::ui::dialog_save_as::SaveAsDialog::new(initial),
                ));
                ui_state.set_status_message("Enter Project filename");
            }
        }
        AppAction::SaveAs => {
            let initial = app_state
                .last_save_as_filename
                .clone()
                .or_else(|| get_default_filename_stem(app_state));
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_save_as::SaveAsDialog::new(
                initial,
            )));
            ui_state.set_status_message("Enter Project filename");
        }
        AppAction::ExportProject => {
            if let Some(path) = &app_state.export_path {
                if let Err(e) = crate::exporter::export_asm(app_state, path) {
                    ui_state.set_status_message(format!("Error exporting: {e}"));
                } else {
                    let filename = path.file_name().unwrap_or_default().to_string_lossy();
                    ui_state.set_status_message(format!("Exported: {filename}"));
                }
            } else {
                let initial = app_state
                    .last_export_asm_filename
                    .clone()
                    .or_else(|| get_default_filename_stem(app_state));
                ui_state.active_dialog = Some(Box::new(
                    crate::ui::dialog_export_as::ExportAsDialog::new(initial),
                ));
                ui_state.set_status_message("Enter .asm filename");
            }
        }
        AppAction::ExportProjectAs => {
            let initial = app_state
                .last_export_asm_filename
                .clone()
                .or_else(|| get_default_filename_stem(app_state));
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_export_as::ExportAsDialog::new(initial),
            ));
            ui_state.set_status_message("Enter .asm filename");
        }
        AppAction::DocumentSettings => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_document_settings::DocumentSettingsDialog::new(),
            ));
            ui_state.set_status_message("Document Settings");
        }
        AppAction::Analyze => {
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
        AppAction::SetLabel => {
            crate::ui::view_disassembly::action_set_label(app_state, ui_state);
        }
        AppAction::Undo => {
            ui_state.set_status_message(app_state.undo_last_command());
        }
        AppAction::Redo => {
            ui_state.set_status_message(app_state.redo_last_command());
        }
        AppAction::ToggleBookmark => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                if line.external_label_address.is_some() {
                    ui_state.set_status_message("Cannot bookmark external label definition");
                } else {
                    let address = line.address;
                    let is_bookmarked = app_state.bookmarks.contains_key(&address);

                    let command = crate::commands::Command::SetBookmark {
                        address,
                        new_name: if is_bookmarked {
                            None
                        } else {
                            Some(String::new())
                        },
                        old_name: app_state.bookmarks.get(&address).cloned(),
                    };
                    command.apply(app_state);
                    app_state.push_command(command);

                    if is_bookmarked {
                        ui_state.set_status_message(format!("Bookmark removed at ${address:04X}"));
                    } else {
                        ui_state.set_status_message(format!("Bookmark set at ${address:04X}"));
                    }
                }
            }
        }
        AppAction::ListBookmarks => {
            let dialog = crate::ui::dialog_bookmarks::BookmarksDialog;
            ui_state.active_dialog = Some(Box::new(dialog));
            ui_state.bookmarks_list_state.select(Some(0));
        }
        AppAction::ViceConnect => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_vice_connect::ViceConnectDialog::new(),
            ));
            ui_state.set_status_message("Enter VICE hostname and port (e.g. localhost:6502)");
        }
        AppAction::ViceConnectAddress(_) => {
            // Handled directly in run_app
        }

        AppAction::Code => apply_block_type(app_state, ui_state, crate::state::BlockType::Code),
        AppAction::Byte => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::DataByte);
        }
        AppAction::Word => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::DataWord);
        }
        AppAction::SetExternalFile => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::ExternalFile);
        }
        AppAction::Address => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::Address);
        }
        AppAction::PetsciiText => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::PetsciiText);
        }
        AppAction::ScreencodeText => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::ScreencodeText);
        }
        AppAction::Undefined => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::Undefined);
        }
        AppAction::JumpToAddress => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_jump_to_address::JumpToAddressDialog::new(),
            ));
            ui_state.set_status_message("Enter address (Hex)");
        }
        AppAction::JumpToLine => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_jump_to_line::JumpToLineDialog::new(),
            ));
            ui_state.set_status_message("Enter Line Number (Dec)");
        }
        AppAction::Search => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_search::SearchDialog::new(
                ui_state.last_search_query.clone(),
                ui_state.search_filters.clone(),
            )));
            ui_state.set_status_message("Search...");
        }
        AppAction::GoToSymbol => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_go_to_symbol::GoToSymbolDialog::new(app_state),
            ));
            ui_state.set_status_message("Go to Symbol...");
        }
        AppAction::FindNext => {
            crate::ui::dialog_search::perform_search(app_state, ui_state, true);
        }
        AppAction::FindPrevious => {
            crate::ui::dialog_search::perform_search(app_state, ui_state, false);
        }
        AppAction::FindReferences => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                // Resolve the effective address under the cursor, mirroring
                // the logic used in action_set_label:
                //
                //  1. External label definition (bytes empty, external_label_address set)
                //     e.g.  "s11CA =*=$01  ; x-ref $1031"  at the top of the listing.
                //  2. Relative / mid-instruction label (bytes.len() > 1, sub_cursor
                //     is on one of the inline labels).
                //  3. Normal instruction / data line → use line.address.
                let addr = if line.bytes.is_empty() {
                    // External label definition row (or blank separator row)
                    line.external_label_address.unwrap_or(line.address)
                } else if line.bytes.len() > 1 {
                    // A multi-byte line may expose relative-address labels
                    // (rendered as sub-rows before the instruction itself).
                    // Walk through them the same way action_set_label does.
                    let mut resolved = line.address;
                    let mut current_sub_index = 0;
                    'outer: for offset in 1..line.bytes.len() {
                        let mid_addr = line.address.wrapping_add(offset as u16);
                        if let Some(labels) = app_state.labels.get(&mid_addr) {
                            for _ in labels {
                                if current_sub_index == ui_state.sub_cursor_index {
                                    resolved = mid_addr;
                                    break 'outer;
                                }
                                current_sub_index += 1;
                            }
                        }
                    }
                    resolved
                } else {
                    line.address
                };
                ui_state.active_dialog = Some(Box::new(
                    crate::ui::dialog_find_references::FindReferencesDialog::new(app_state, addr),
                ));
                ui_state.set_status_message(format!("References to ${addr:04X}"));
            } else {
                ui_state.set_status_message("No address selected");
            }
        }
        AppAction::NavigateToAddress(target_addr) => match ui_state.active_pane {
            ActivePane::Disassembly => {
                perform_jump_to_address(app_state, ui_state, target_addr);
            }
            ActivePane::HexDump => {
                let origin = app_state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let end_addr = origin + app_state.raw_data.len();

                if target >= origin && target < end_addr {
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let offset = target - aligned_origin;
                    let row = offset / 16;
                    ui_state.hex_cursor_index = row;
                    ui_state.set_status_message(format!("Jumped to ${target_addr:04X}"));
                } else {
                    ui_state.set_status_message("Address out of range");
                }
            }
            ActivePane::Sprites => {
                let origin = app_state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let padding = (64 - (origin % 64)) % 64;
                let aligned_start = origin + padding;
                let end_addr = origin + app_state.raw_data.len();

                if target >= aligned_start && target < end_addr {
                    let offset = target - aligned_start;
                    let sprite_idx = offset / 64;
                    ui_state.sprites_cursor_index = sprite_idx;
                    ui_state.set_status_message(format!("Jumped to sprite at ${target_addr:04X}"));
                } else {
                    ui_state.set_status_message("Address out of range or unaligned");
                }
            }
            ActivePane::Charset => {
                let origin = app_state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                let end_addr = origin + app_state.raw_data.len();

                if target >= aligned_start_addr && target < end_addr {
                    let offset = target - aligned_start_addr;
                    let char_idx = offset / 8;
                    ui_state.charset_cursor_index = char_idx;
                    ui_state.set_status_message(format!("Jumped to char at ${target_addr:04X}"));
                } else {
                    ui_state.set_status_message("Address out of range");
                }
            }
            ActivePane::Blocks => {
                ui_state.set_status_message("Jump to address not supported in Blocks view");
            }
            ActivePane::Bitmap => {
                ui_state.set_status_message("Jump to address not supported in Bitmap view");
            }
            ActivePane::Debugger => {
                ui_state.set_status_message("Jump to address not supported in Debugger view");
            }
        },

        AppAction::JumpToOperand => {
            let target_addr = match ui_state.active_pane {
                ActivePane::Disassembly => {
                    if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                        // Try to extract address from operand.
                        // We utilize the opcode mode if available.
                        if let Some(opcode) = &line.opcode {
                            use crate::cpu::AddressingMode;
                            match opcode.mode {
                                AddressingMode::Immediate => {
                                    if let Some(fmt) =
                                        app_state.immediate_value_formats.get(&line.address)
                                    {
                                        match fmt {
                                            crate::state::ImmediateFormat::LowByte(target) => {
                                                Some(*target)
                                            }
                                            crate::state::ImmediateFormat::HighByte(target) => {
                                                Some(*target)
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Absolute
                                | AddressingMode::AbsoluteX
                                | AddressingMode::AbsoluteY => {
                                    if line.bytes.len() >= 3 {
                                        Some(crate::state::Addr(
                                            u16::from(line.bytes[2]) << 8
                                                | u16::from(line.bytes[1]),
                                        ))
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Indirect => {
                                    // JMP ($1234) -> target is $1234
                                    if line.bytes.len() >= 3 {
                                        Some(crate::state::Addr(
                                            u16::from(line.bytes[2]) << 8
                                                | u16::from(line.bytes[1]),
                                        ))
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
                                        Some(crate::state::Addr(u16::from(line.bytes[1])))
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            line.external_label_address
                        }
                    } else {
                        None
                    }
                }
                ActivePane::HexDump => {
                    let origin = app_state.origin.0 as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    Some(crate::state::Addr(
                        (aligned_origin + ui_state.hex_cursor_index * 16) as u16,
                    ))
                }
                ActivePane::Sprites => {
                    let origin = app_state.origin.0 as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    Some(crate::state::Addr(
                        (origin + padding + ui_state.sprites_cursor_index * 64) as u16,
                    ))
                }
                ActivePane::Charset => {
                    let origin = app_state.origin.0 as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    Some(crate::state::Addr(
                        (aligned_start_addr + ui_state.charset_cursor_index * 8) as u16,
                    ))
                }
                ActivePane::Blocks => {
                    // Jump to start of selected block
                    let blocks = app_state.get_blocks_view_items();
                    let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                    if idx < blocks.len() {
                        match blocks[idx] {
                            crate::state::BlockItem::Block { start, .. } => Some(start),
                            crate::state::BlockItem::Splitter(addr) => Some(addr),
                        }
                    } else {
                        None
                    }
                }
                ActivePane::Bitmap => {
                    let origin = app_state.origin.0 as usize;
                    // Bitmaps must be aligned to 8192-byte boundaries
                    let first_aligned_addr = ((origin / 8192) * 8192)
                        + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                    let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
                    Some(crate::state::Addr(bitmap_addr as u16))
                }
                ActivePane::Debugger => None,
            };

            if let Some(addr) = target_addr {
                execute_menu_action(app_state, ui_state, AppAction::NavigateToAddress(addr));
            } else {
                ui_state.set_status_message("No valid operand to jump to");
            }
        }
        AppAction::About => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_about::AboutDialog::new(
                ui_state,
            )));
            ui_state.set_status_message("About Regenerator 2000");
        }
        AppAction::HexdumpViewModeNext => {
            let new_mode = match ui_state.hexdump_view_mode {
                crate::state::HexdumpViewMode::ScreencodeShifted => {
                    crate::state::HexdumpViewMode::ScreencodeUnshifted
                }
                crate::state::HexdumpViewMode::ScreencodeUnshifted => {
                    crate::state::HexdumpViewMode::PETSCIIShifted
                }
                crate::state::HexdumpViewMode::PETSCIIShifted => {
                    crate::state::HexdumpViewMode::PETSCIIUnshifted
                }
                crate::state::HexdumpViewMode::PETSCIIUnshifted => {
                    crate::state::HexdumpViewMode::ScreencodeShifted
                }
            };
            ui_state.hexdump_view_mode = new_mode;
            update_hexdump_status(ui_state, new_mode);
        }
        AppAction::HexdumpViewModePrev => {
            let new_mode = match ui_state.hexdump_view_mode {
                crate::state::HexdumpViewMode::ScreencodeShifted => {
                    crate::state::HexdumpViewMode::PETSCIIUnshifted
                }
                crate::state::HexdumpViewMode::ScreencodeUnshifted => {
                    crate::state::HexdumpViewMode::ScreencodeShifted
                }
                crate::state::HexdumpViewMode::PETSCIIShifted => {
                    crate::state::HexdumpViewMode::ScreencodeUnshifted
                }
                crate::state::HexdumpViewMode::PETSCIIUnshifted => {
                    crate::state::HexdumpViewMode::PETSCIIShifted
                }
            };
            ui_state.hexdump_view_mode = new_mode;
            update_hexdump_status(ui_state, new_mode);
        }
        AppAction::ToggleSplitter => {
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
                    ui_state.set_status_message(format!("Removed splitter at ${addr:04X}"));
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
                    ui_state.set_status_message(format!("Toggled splitter at ${addr:04X}"));
                }
            }
        }
        AppAction::ToggleSpriteMulticolor => {
            ui_state.sprite_multicolor_mode = !ui_state.sprite_multicolor_mode;
            if ui_state.sprite_multicolor_mode {
                ui_state.set_status_message("Sprites: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Sprites: Single Color Mode");
            }
        }
        AppAction::ToggleCharsetMulticolor => {
            ui_state.charset_multicolor_mode = !ui_state.charset_multicolor_mode;
            if ui_state.charset_multicolor_mode {
                ui_state.set_status_message("Charset: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Charset: Single Color Mode");
            }
        }
        AppAction::PackLoHiAddress => {
            apply_lo_hi_packing(app_state, ui_state, true);
        }
        AppAction::PackHiLoAddress => {
            apply_lo_hi_packing(app_state, ui_state, false);
        }
        AppAction::SetLoHiAddress => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::LoHiAddress);
        }
        AppAction::SetHiLoAddress => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::HiLoAddress);
        }
        AppAction::SetLoHiWord => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::LoHiWord);
        }
        AppAction::SetHiLoWord => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::HiLoWord);
        }
        AppAction::SideComment | AppAction::LineComment => {
            // Handled in handle_menu_action via core.apply_action
        }
        AppAction::ToggleHexDump => {
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
        AppAction::ToggleSpritesView => {
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
        AppAction::ToggleCharsetView => {
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
        AppAction::ToggleBitmapView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Bitmap {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Bitmap View Hidden");
                if ui_state.active_pane == ActivePane::Bitmap {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Bitmap;
                ui_state.active_pane = ActivePane::Bitmap;
                ui_state.set_status_message("Bitmap View Shown");
            }
        }
        AppAction::ToggleBitmapMulticolor => {
            ui_state.bitmap_multicolor_mode = !ui_state.bitmap_multicolor_mode;
            ui_state.set_status_message(if ui_state.bitmap_multicolor_mode {
                "Multicolor mode enabled"
            } else {
                "Single color mode enabled"
            });
        }
        AppAction::ToggleBlocksView => {
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
        AppAction::ViceDisconnect => {
            app_state.vice_client = None;
            app_state.vice_state.connected = false;
            ui_state.set_status_message("Disconnected from VICE");
        }
        AppAction::ViceStep => {
            if let Some(client) = &app_state.vice_client {
                client.send_advance_instruction();
                app_state.vice_state.running = true;
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceContinue => {
            if let Some(client) = &app_state.vice_client {
                client.send_continue();
                app_state.vice_state.running = true;
                ui_state.set_status_message("VICE: Running...");
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceStepOver => {
            if let Some(client) = &app_state.vice_client {
                client.send_step_over();
                app_state.vice_state.running = true;
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceStepOut => {
            if let Some(client) = &app_state.vice_client {
                client.send_execute_until_return();
                // We must tell the editor that the vice client is running so that
                // step controls are disabled until the command hits the return point.
                app_state.vice_state.running = true;
                ui_state.set_status_message("VICE: Stepping out...");
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceRunToCursor => {
            if let Some(client) = &app_state.vice_client {
                let target_addr = match ui_state.active_pane {
                    ActivePane::Disassembly => app_state
                        .disassembly
                        .get(ui_state.cursor_index)
                        .map(|l| l.address),
                    ActivePane::HexDump => {
                        let origin = app_state.origin.0 as usize;
                        let alignment_padding = origin % 16;
                        let aligned_origin = origin - alignment_padding;
                        Some(crate::state::Addr(
                            (aligned_origin + ui_state.hex_cursor_index * 16) as u16,
                        ))
                    }
                    ActivePane::Sprites => {
                        let origin = app_state.origin.0 as usize;
                        let padding = (64 - (origin % 64)) % 64;
                        Some(crate::state::Addr(
                            (origin + padding + ui_state.sprites_cursor_index * 64) as u16,
                        ))
                    }
                    ActivePane::Charset => {
                        let origin = app_state.origin.0 as usize;
                        let base_alignment = 0x400;
                        let aligned_start_addr = (origin / base_alignment) * base_alignment;
                        Some(crate::state::Addr(
                            (aligned_start_addr + ui_state.charset_cursor_index * 8) as u16,
                        ))
                    }
                    ActivePane::Bitmap => {
                        let origin = app_state.origin.0 as usize;
                        let first_aligned_addr = ((origin / 8192) * 8192)
                            + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                        Some(crate::state::Addr(
                            (first_aligned_addr + ui_state.bitmap_cursor_index * 8192) as u16,
                        ))
                    }
                    ActivePane::Blocks => {
                        let blocks = app_state.get_blocks_view_items();
                        let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                        if idx < blocks.len() {
                            match blocks[idx] {
                                crate::state::BlockItem::Block { start, .. } => Some(start),
                                crate::state::BlockItem::Splitter(addr) => Some(addr),
                            }
                        } else {
                            None
                        }
                    }
                    ActivePane::Debugger => app_state
                        .disassembly
                        .get(ui_state.cursor_index)
                        .map(|l| l.address),
                };

                if let Some(addr) = target_addr {
                    client.send_checkpoint_set_exec_temp(addr.0);
                    client.send_continue();
                    app_state.vice_state.running = true;
                    ui_state.set_status_message(format!("VICE: Running to ${addr:04X}..."));
                }
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceToggleBreakpoint => {
            if let Some(client) = &app_state.vice_client {
                let cursor_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|l| l.address);
                if let Some(addr) = cursor_addr {
                    if let Some(pos) = app_state.vice_state.breakpoints.iter().position(|bp| {
                        bp.address == addr.0 && bp.kind == crate::vice::state::BreakpointKind::Exec
                    }) {
                        let id = app_state.vice_state.breakpoints[pos].id;
                        client.send_checkpoint_delete(id);
                        app_state.vice_state.breakpoints.remove(pos);
                        ui_state.set_status_message(format!("Breakpoint removed at ${addr:04X}"));
                    } else {
                        client.send_checkpoint_set_exec(addr.0);
                        ui_state.set_status_message(format!("Breakpoint set at ${addr:04X}"));
                    }
                }
            } else {
                ui_state.set_status_message("Not connected to VICE");
            }
        }
        AppAction::ViceBreakpointDialog => {
            vice_open_breakpoint_dialog(app_state, ui_state);
        }
        AppAction::ViceSetBreakpointAt { address } => {
            vice_toggle_breakpoint_at(app_state, ui_state, address);
        }
        AppAction::ViceToggleWatchpoint => {
            vice_open_watchpoint_dialog(app_state, ui_state);
        }
        AppAction::ViceSetWatchpoint { address, kind } => {
            vice_toggle_watchpoint(app_state, ui_state, address, kind);
        }
        AppAction::ToggleDebuggerView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Debugger {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                if ui_state.active_pane == ActivePane::Debugger {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
                ui_state.set_status_message("Debugger Panel Hidden");
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Debugger;
                ui_state.active_pane = ActivePane::Debugger;
                ui_state.set_status_message("Debugger Panel Shown");
            }
        }
        AppAction::KeyboardShortcuts => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_keyboard_shortcut::ShortcutsDialog::new(),
            ));
            ui_state.set_status_message("Keyboard Shortcuts");
        }
        AppAction::ChangeOrigin => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_origin::OriginDialog::new(
                app_state.origin,
            )));
            ui_state.set_status_message("Enter new origin (Hex)");
        }
        AppAction::SystemSettings => {
            ui_state.active_dialog =
                Some(Box::new(crate::ui::dialog_settings::SettingsDialog::new()));
            ui_state.set_status_message("Settings");
        }
        AppAction::NextImmediateFormat => {
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
                        crate::state::ImmediateFormat::LowByte(_)
                        | crate::state::ImmediateFormat::HighByte(_) => {
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
        AppAction::PreviousImmediateFormat => {
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
                        crate::state::ImmediateFormat::LowByte(_)
                        | crate::state::ImmediateFormat::HighByte(_) => {
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
        AppAction::SetBytesBlockByOffset { start, end } => {
            // Set block type to DataByte for a specific byte offset range
            let block_type = crate::state::BlockType::DataByte;
            let max_len = app_state.block_types.len();
            if start < max_len {
                let valid_end = end.min(max_len.saturating_sub(1));
                let range = start..(valid_end + 1);

                let old_types = app_state.block_types[range.clone()].to_vec();

                let command = crate::commands::Command::SetBlockType {
                    range: range.clone(),
                    new_type: block_type,
                    old_types,
                };

                command.apply(app_state);
                app_state.push_command(command);
                app_state.disassemble();

                let start_addr = app_state.origin.wrapping_add(start as u16);
                let end_addr = app_state.origin.wrapping_add(valid_end as u16);
                // Re-anchor cursor to the end of the newly-created bytes block.
                // The old cursor_index is stale because disassemble() may have
                // merged several instruction rows into one bytes-block row.
                // Use get_line_index_containing_address so we land on the block
                // itself (not the instruction after it).
                let anchor_addr = end_addr;
                if let Some(idx) = app_state.get_line_index_containing_address(anchor_addr) {
                    let sub = crate::ui::view_disassembly::DisassemblyView::get_visual_line_counts(
                        &app_state.disassembly[idx],
                        app_state,
                    );
                    let opcode_sub = sub.labels + sub.comments;
                    ui_state.cursor_index = idx;
                    ui_state.sub_cursor_index = opcode_sub;
                    ui_state.scroll_index = idx;
                    ui_state.scroll_sub_index = opcode_sub;
                } else if let Some(idx) = app_state.get_line_index_for_address(start_addr) {
                    let sub = crate::ui::view_disassembly::DisassemblyView::get_visual_line_counts(
                        &app_state.disassembly[idx],
                        app_state,
                    );
                    let opcode_sub = sub.labels + sub.comments;
                    ui_state.cursor_index = idx;
                    ui_state.sub_cursor_index = opcode_sub;
                    ui_state.scroll_index = idx;
                    ui_state.scroll_sub_index = opcode_sub;
                }
                ui_state.set_status_message(format!(
                    "Set bytes block ${:04X}-${:04X} ({} bytes)",
                    start_addr,
                    end_addr,
                    valid_end - start + 1
                ));
            } else {
                ui_state.set_status_message("Error: offset out of range");
            }
        }
        AppAction::ToggleCollapsedBlock => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected() {
                    if let Some(crate::state::BlockItem::Block { start, end, .. }) = blocks.get(idx)
                    {
                        let start_offset = start.offset_from(app_state.origin);
                        let end_offset = end.offset_from(app_state.origin);

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
                    .map_or(crate::state::Addr::ZERO, |line| line.address);

                // First check if we are ON a collapsed block placeholder (Uncollapse case)
                if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                    let offset =
                        (line.address.0 as usize).wrapping_sub(app_state.origin.0 as usize);
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
                    let start_offset = start_addr.offset_from(app_state.origin);
                    let end_offset = end_addr.offset_from(app_state.origin);

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
        _ => {
            // Unhandled actions
        }
    }
}

fn apply_lo_hi_packing(app_state: &mut AppState, ui_state: &mut UIState, lo_first: bool) {
    let is_single_selection = ui_state.selection_start.is_none();
    let single_cursor_index = if is_single_selection {
        Some(ui_state.cursor_index)
    } else {
        None
    };

    let mut indices = Vec::new();
    if let Some(start) = ui_state.selection_start {
        let end = ui_state.cursor_index;
        let (low, high) = if start < end {
            (start, end)
        } else {
            (end, start)
        };
        for i in low..=high {
            indices.push(i);
        }
    } else {
        indices.push(ui_state.cursor_index);
    }

    // If single line selected, try to include next line to allow single-line cursor packing
    if indices.len() == 1 {
        let idx = indices[0];
        if idx + 1 < app_state.disassembly.len() {
            indices.push(idx + 1);
        }
    }

    let mut i = 0;
    let mut batch_commands = Vec::new();

    let get_imm = |app_state: &AppState, idx: usize| -> Option<u8> {
        if let Some(line) = app_state.disassembly.get(idx)
            && let Some(opcode) = &line.opcode
            && opcode.mode == crate::cpu::AddressingMode::Immediate
            && matches!(opcode.mnemonic, "LDA" | "LDX" | "LDY")
        {
            line.bytes.get(1).copied()
        } else {
            None
        }
    };

    while i < indices.len() {
        let idx1 = indices[i];
        let val1_opt = get_imm(app_state, idx1);

        if let Some(val1) = val1_opt {
            // Found start of pair, look for next match
            let mut j = i + 1;
            let mut found_match = None;
            while j < indices.len() {
                let idx2 = indices[j];
                let val2_opt = get_imm(app_state, idx2);
                if let Some(val2) = val2_opt {
                    found_match = Some((j, idx2, val2));
                    break;
                }
                j += 1;
            }

            if let Some((match_idx, idx2, val2)) = found_match {
                let (lo, hi) = if lo_first { (val1, val2) } else { (val2, val1) };
                let target = (u16::from(hi) << 8) | u16::from(lo);

                // Create label if needed (removed explicit label creation, relying on analyzer)
                // BUT user earlier asked for Analyzer to do it.
                // However, analyzer runs on load/analysis.
                // If we want immediate feedback, we might need to TRIGGER analysis?
                // Or just wait for refresh?
                // For now, removing SetLabel command as agreed.

                let fmt1 = if lo_first {
                    crate::state::ImmediateFormat::LowByte(crate::state::Addr(target))
                } else {
                    crate::state::ImmediateFormat::HighByte(crate::state::Addr(target))
                };
                let fmt2 = if lo_first {
                    crate::state::ImmediateFormat::HighByte(crate::state::Addr(target))
                } else {
                    crate::state::ImmediateFormat::LowByte(crate::state::Addr(target))
                };

                let addr1 = app_state.disassembly[idx1].address;
                let addr2 = app_state.disassembly[idx2].address;

                let old_fmt1 = app_state.immediate_value_formats.get(&addr1).copied();
                let old_fmt2 = app_state.immediate_value_formats.get(&addr2).copied();

                batch_commands.push(crate::commands::Command::SetImmediateFormat {
                    address: addr1,
                    new_format: Some(fmt1),
                    old_format: old_fmt1,
                });

                batch_commands.push(crate::commands::Command::SetImmediateFormat {
                    address: addr2,
                    new_format: Some(fmt2),
                    old_format: old_fmt2,
                });

                ui_state.set_status_message(format!("Packed Lo/Hi address for ${target:04X}"));

                // Advance past the pair
                i = match_idx + 1;
            } else {
                // No pair found for this instruction
                i += 1;
            }
        } else {
            // Not a candidate start
            i += 1;
        }
    }

    if !batch_commands.is_empty() {
        let batch_cmd = crate::commands::Command::Batch(batch_commands);
        batch_cmd.apply(app_state);
        app_state.push_command(batch_cmd);

        // Re-analyze to generate new auto-labels for Lo/Hi addresses
        let result = crate::analyzer::analyze(app_state);
        app_state.labels = result.labels;
        app_state.cross_refs = result.cross_refs;

        app_state.disassemble();
    } else if let Some(cursor_idx) = single_cursor_index {
        // No pairs found, but if this is a single selection with an immediate instruction,
        // show dialog to complete the address
        let get_imm = |app_state: &AppState, idx: usize| -> Option<u8> {
            if let Some(line) = app_state.disassembly.get(idx)
                && let Some(opcode) = &line.opcode
                && opcode.mode == crate::cpu::AddressingMode::Immediate
                && matches!(opcode.mnemonic, "LDA" | "LDX" | "LDY")
            {
                line.bytes.get(1).copied()
            } else {
                None
            }
        };

        if let Some(known_byte) = get_imm(app_state, cursor_idx) {
            let address = app_state.disassembly[cursor_idx].address;
            let dialog = crate::ui::dialog_complete_address::CompleteAddressDialog::new(
                known_byte, lo_first, address,
            );
            ui_state.active_dialog = Some(Box::new(dialog));
        }
    }
}

pub(super) fn apply_block_type(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    block_type: crate::state::BlockType,
) {
    let needs_even = matches!(
        block_type,
        crate::state::BlockType::LoHiAddress
            | crate::state::BlockType::HiLoAddress
            | crate::state::BlockType::LoHiWord
            | crate::state::BlockType::HiLoWord
    );

    if ui_state.active_pane == ActivePane::Blocks {
        let blocks = app_state.get_blocks_view_items();
        if let Some(idx) = ui_state.blocks_list_state.selected()
            && idx < blocks.len()
            && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
        {
            let len = end.offset_from(start) + 1;
            if needs_even && !len.is_multiple_of(2) {
                ui_state.set_status_message(format!(
                    "Error: {block_type} requires even number of bytes"
                ));
                return;
            }
            app_state.set_block_type_region(
                block_type,
                Some(start.offset_from(app_state.origin)),
                end.offset_from(app_state.origin),
            );
            ui_state.set_status_message(format!("Set block type to {block_type}"));
        }
    } else if let Some(start_index) = ui_state.selection_start {
        let start = start_index.min(ui_state.cursor_index);
        let end = start_index.max(ui_state.cursor_index);
        let len = end - start + 1;

        if needs_even && len % 2 != 0 {
            ui_state
                .set_status_message(format!("Error: {block_type} requires even number of bytes"));
            return;
        }

        let target_address = if let Some(line) = app_state.disassembly.get(end) {
            line.address
                .wrapping_add(line.bytes.len() as u16)
                .wrapping_sub(1)
        } else {
            crate::state::Addr::ZERO
        };

        app_state.set_block_type_region(block_type, Some(start), end);
        ui_state.selection_start = None;
        ui_state.is_visual_mode = false;

        if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
            ui_state.cursor_index = idx;
        }

        ui_state.set_status_message(format!("Set block type to {block_type}"));
    } else {
        // Single line
        if needs_even {
            ui_state
                .set_status_message(format!("Error: {block_type} requires even number of bytes"));
            return;
        }

        let current_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map(|l| l.address);

        app_state.set_block_type_region(
            block_type,
            ui_state.selection_start,
            ui_state.cursor_index,
        );
        ui_state.set_status_message(format!("Set block type to {block_type}"));

        if let Some(addr) = current_addr {
            if let Some(idx) = app_state.get_line_index_containing_address(addr) {
                ui_state.cursor_index = idx;
            } else if let Some(idx) = app_state.get_line_index_for_address(addr) {
                ui_state.cursor_index = idx;
            }
        }
    }
}

fn create_save_context(
    app_state: &AppState,
    ui_state: &UIState,
) -> crate::state::ProjectSaveContext {
    crate::navigation::create_save_context(app_state, &ui_state.core)
}

fn update_hexdump_status(ui_state: &mut UIState, mode: crate::state::HexdumpViewMode) {
    let status = match mode {
        crate::state::HexdumpViewMode::PETSCIIUnshifted => "Unshifted (PETSCII)",
        crate::state::HexdumpViewMode::PETSCIIShifted => "Shifted (PETSCII)",
        crate::state::HexdumpViewMode::ScreencodeShifted => "Shifted (Screencode)",
        crate::state::HexdumpViewMode::ScreencodeUnshifted => "Unshifted (Screencode)",
    };
    ui_state.set_status_message(format!("Hex Dump: {status}"));
}

/// Thin wrapper – delegates to [`crate::navigation::perform_jump_to_address`].
pub fn perform_jump_to_address(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) {
    crate::navigation::perform_jump_to_address(app_state, &mut ui_state.core, target_addr);
    ui_state.sync_status_message();
}

/// Thin wrapper – delegates to [`crate::navigation::perform_jump_to_address_no_history`].
pub fn perform_jump_to_address_no_history(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) {
    crate::navigation::perform_jump_to_address_no_history(
        app_state,
        &mut ui_state.core,
        target_addr,
    );
    ui_state.sync_status_message();
}
