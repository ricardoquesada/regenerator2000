use crate::ui_state::UIState;
use regenerator2000_core::Core;
use regenerator2000_core::event::{CommentKind as DialogCommentKind, CoreEvent, DialogType};
pub use regenerator2000_core::state::actions::AppAction;

pub fn handle_menu_action(core: &mut Core, ui_state: &mut UIState, action: AppAction) {
    // Dispatch to Core and handle results reactively
    let events = core.apply_action(action);

    for event in events {
        match event {
            CoreEvent::QuitRequested => ui_state.should_quit = true,
            CoreEvent::OpenUrl(url) => {
                let _ = opener::open(&url);
                ui_state.set_status_message(format!("Opened URL: {}", url));
            }
            CoreEvent::StatusMessage(msg) => ui_state.set_status_message(msg),
            CoreEvent::DialogRequested(dialog_type) => match dialog_type {
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
                            ui_state.file_dialog_current_dir.clone(),
                        ),
                    ));
                }
                DialogType::SaveAs { initial_filename } => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_save_as::SaveAsDialog::new(
                            initial_filename,
                            ui_state.file_dialog_current_dir.clone(),
                        )));
                }
                DialogType::ExportAs {
                    initial_filename,
                    format,
                } => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_export_as::ExportAsDialog::new(
                            initial_filename,
                            format,
                            ui_state.file_dialog_current_dir.clone(),
                        )));
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
                DialogType::Search { .. } => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_search::SearchDialog::new(
                            ui_state.last_search_query.clone(),
                            ui_state.search_filters.clone(),
                        )));
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
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_label::LabelDialog::new(
                            Some(&initial_name),
                            address,
                            false,
                        )));
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
                        crate::ui::dialog_breakpoint_address::BreakpointAddressDialog::new(addr),
                    ));
                }
                DialogType::WatchpointAddress(addr) => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_watchpoint_address::WatchpointAddressDialog::new(addr),
                    ));
                }
                DialogType::MemoryDumpAddress(addr) => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_memory_dump_address::MemoryDumpAddressDialog::new(addr),
                    ));
                }
                DialogType::Settings => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_settings::SettingsDialog::new()));
                }
                DialogType::CompleteAddress {
                    known_byte,
                    lo_first,
                    address,
                } => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_complete_address::CompleteAddressDialog::new(
                            known_byte, lo_first, address,
                        ),
                    ));
                }
                DialogType::Origin => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_origin::OriginDialog::new(core.state.origin),
                    ));
                }
            },
            CoreEvent::DialogDismissalRequested => {
                ui_state.active_dialog = None;
            }
            CoreEvent::ViewChanged | CoreEvent::StateChanged => {
                ui_state.core = core.view.clone();
                ui_state.sync_core_to_tui();
                ui_state.sync_status_message();
            }
        }
    }
}
