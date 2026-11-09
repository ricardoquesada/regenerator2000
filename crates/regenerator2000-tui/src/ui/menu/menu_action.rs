use crate::events::AppEvent;
use crate::ui_state::UIState;
use regenerator2000_core::Core;
use regenerator2000_core::event::{CommentKind as DialogCommentKind, CoreEvent, DialogType};
pub use regenerator2000_core::state::actions::AppAction;

/// Dispatches an [`AppAction`] to [`Core::apply_action`] and translates the resulting
/// [`CoreEvent`]s into TUI state mutations, modal dialogs, or background tasks.
///
/// # State Synchronization Contract
/// Before invoking [`Core::apply_action`], this function synchronizes TUI-managed view fields
/// (`UIState::sync_tui_to_core`) and clones `ui_state.core` into `core.view`. Without this pre-sync,
/// view-local mutations performed immediately prior to returning `WidgetResult::Action(...)`
/// (such as clearing `is_visual_mode` and selection anchors on `'b'` in `HexDumpView`,
/// `SpritesView`, or `CharsetView`, or advancing the cursor between render frames) would be
/// overwritten when `CoreEvent::ViewChanged` copies `core.view` back into `ui_state.core`.
pub fn handle_menu_action(
    core: &mut Core,
    ui_state: &mut UIState,
    action: AppAction,
    event_tx: &std::sync::mpsc::Sender<AppEvent>,
) {
    // Synchronize TUI view state into Core before executing the action so Core operates on the
    // latest cursor/selection state and does not clobber view-local changes on `CoreEvent::ViewChanged`.
    ui_state.sync_tui_to_core();
    core.view = ui_state.core.clone();

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
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_file_browser::FileBrowserDialog::new_open(
                            ui_state.file_dialog_current_dir.clone(),
                        ),
                    ));
                }
                DialogType::OpenRecent => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_open_recent::OpenRecentDialog));
                    ui_state.recent_list_state.select(Some(0));
                }
                DialogType::ImportViceLabels => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_file_browser::FileBrowserDialog::new_import_vice_labels(
                            ui_state.file_dialog_current_dir.clone(),
                            core.state.last_import_labels_path.clone(),
                        ),
                    ));
                }
                DialogType::ExportLabels { initial_filename } => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_file_browser::FileBrowserDialog::new_export_labels(
                            initial_filename,
                            ui_state.file_dialog_current_dir.clone(),
                        ),
                    ));
                }
                DialogType::SaveAs { initial_filename } => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_file_browser::FileBrowserDialog::new_save_project(
                            initial_filename,
                            ui_state.file_dialog_current_dir.clone(),
                        ),
                    ));
                }
                DialogType::ExportAs {
                    initial_filename,
                    format,
                } => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_file_browser::FileBrowserDialog::new_export(
                            initial_filename,
                            format,
                            ui_state.file_dialog_current_dir.clone(),
                        ),
                    ));
                }
                DialogType::DocumentSettings => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_document_settings::DocumentSettingsDialog::new(),
                    ));
                }
                DialogType::FileInfo => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_file_info::FileInfoDialog::new()));
                }
                DialogType::JumpToAddress => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_prompt::PromptDialog::new_jump_to_address(),
                    ));
                }
                DialogType::JumpToLine => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_prompt::PromptDialog::new_jump_to_line(),
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
                        crate::ui::dialog_about::AboutDialog::new(ui_state, event_tx.clone()),
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
                    is_local,
                    ..
                } => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_label::LabelDialog::new(
                            Some(&initial_name),
                            address,
                            is_local,
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
                        crate::ui::dialog_prompt::PromptDialog::new_breakpoint_address(addr),
                    ));
                }
                DialogType::WatchpointAddress(addr) => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_watchpoint_address::WatchpointAddressDialog::new(addr),
                    ));
                }
                DialogType::MemoryDumpAddress(addr) => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_prompt::PromptDialog::new_memory_dump_address(addr),
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
                DialogType::ImportContext {
                    origin,
                    entry_point,
                } => {
                    let packer = core.state.file_info().packer_name;
                    ui_state.push_dialog(Box::new(
                        crate::ui::dialog_import_context::ImportContextDialog::new(
                            core.state.settings.system.as_str(),
                            origin,
                            Some(entry_point),
                            None, // inherit system from current state
                            None, // no entropy warning
                            packer,
                        ),
                    ));
                }
                DialogType::Origin => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_prompt::PromptDialog::new_origin(core.state.origin),
                    ));
                }
                DialogType::ApplyEnum { address, value } => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_apply_enum::ApplyEnumDialog::new(
                            address,
                            value,
                            &core.state,
                        ),
                    ));
                }
                DialogType::ManageEnums => {
                    ui_state.active_dialog = Some(Box::new(
                        crate::ui::dialog_manage_enums::ManageEnumsDialog::new(&core.state),
                    ));
                }
                DialogType::Unpack => {
                    ui_state.active_dialog =
                        Some(Box::new(crate::ui::dialog_unpack::UnpackDialog::new()));
                }
            },
            CoreEvent::DialogDismissalRequested => {
                ui_state.active_dialog = None;
            }
            CoreEvent::ViewChanged | CoreEvent::StateChanged => {
                ui_state.core = core.view.clone();
                ui_state.sync_core_to_tui();
                ui_state.sync_status_message();
                if let Some(dialog) = ui_state.active_dialog.as_mut() {
                    dialog.refresh(&core.state);
                }
            }
            CoreEvent::UnpackStarted {
                raw_data,
                load_addr,
                config,
            } => {
                ui_state.set_status_message("Unpacking... $00000000");
                let tx = event_tx.clone();
                std::thread::spawn(move || {
                    let progress_tx = tx.clone();
                    let last_update = std::cell::Cell::new(std::time::Instant::now());
                    let progress_cb = move |count: u64| {
                        let now = std::time::Instant::now();
                        if now.duration_since(last_update.get())
                            >= std::time::Duration::from_millis(50)
                        {
                            last_update.set(now);
                            let _ =
                                progress_tx.send(crate::events::AppEvent::UnpackProgress(count));
                        }
                    };
                    let result = regenerator2000_core::unpacker::unpack(
                        &raw_data,
                        load_addr,
                        &config,
                        Some(&progress_cb),
                    );
                    let _ = tx.send(crate::events::AppEvent::UnpackComplete(result));
                });
            }
            CoreEvent::StartMcpServerRequested => {
                let _ = event_tx.send(crate::events::AppEvent::McpStartRequested);
            }
            CoreEvent::StopMcpServerRequested => {
                let _ = event_tx.send(crate::events::AppEvent::McpStopRequested);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::WidgetResult;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use regenerator2000_core::state::{Addr, Label, LabelKind, LabelType};

    #[test]
    fn test_set_label_preserves_local_user_label_after_auto_label() {
        let mut core = Core::new();
        core.state.load_binary(Addr(0x1000), vec![0xEA]).unwrap();
        core.state.labels.insert(
            Addr(0x1000),
            vec![
                Label {
                    name: "j1000".to_string(),
                    kind: LabelKind::Auto,
                    label_type: LabelType::Jump,
                },
                Label {
                    name: "local_loop".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::LocalUserDefined,
                },
            ],
        );
        core.state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;
        let (tx, _rx) = std::sync::mpsc::channel();

        handle_menu_action(&mut core, &mut ui_state, AppAction::SetLabel, &tx);

        let mut dialog = ui_state
            .active_dialog
            .take()
            .expect("Expected LabelDialog to be opened");
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let result = dialog.handle_input(enter_key, &mut core.state, &mut ui_state);

        assert_eq!(
            result,
            WidgetResult::Action(AppAction::ApplyLabel {
                address: Addr(0x1000),
                name: "local_loop".to_string(),
                is_local: true,
            })
        );
    }
}
