//! Bridge between TUI [`AppAction`] triggers, [`Core::apply_action`], and modal [`DialogType`]
//! instantiation.

use crate::events::AppEvent;
use crate::ui::dialog_about::AboutDialog;
use crate::ui::dialog_apply_enum::ApplyEnumDialog;
use crate::ui::dialog_bookmarks::BookmarksDialog;
use crate::ui::dialog_comment::{CommentDialog, CommentType};
use crate::ui::dialog_complete_address::{CompleteAddressDialog, KnownBytePosition};
use crate::ui::dialog_confirmation::ConfirmationDialog;
use crate::ui::dialog_document_settings::DocumentSettingsDialog;
use crate::ui::dialog_file_browser::FileBrowserDialog;
use crate::ui::dialog_file_info::FileInfoDialog;
use crate::ui::dialog_find_references::FindReferencesDialog;
use crate::ui::dialog_go_to_symbol::GoToSymbolDialog;
use crate::ui::dialog_import_context::ImportContextDialog;
use crate::ui::dialog_jump_to_operand::JumpToOperandDialog;
use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
use crate::ui::dialog_label::LabelDialog;
use crate::ui::dialog_manage_enums::ManageEnumsDialog;
use crate::ui::dialog_open_recent::OpenRecentDialog;
use crate::ui::dialog_prompt::PromptDialog;
use crate::ui::dialog_search::SearchDialog;
use crate::ui::dialog_settings::SettingsDialog;
use crate::ui::dialog_symbolic_offset::{SymbolicOffsetDialog, SymbolicOffsetDialogParams};
use crate::ui::dialog_unpack::UnpackDialog;
use crate::ui::dialog_vice_connect::ViceConnectDialog;
use crate::ui::dialog_watchpoint_address::WatchpointAddressDialog;
use crate::ui_state::UIState;
use ratatui::layout::Rect;
use regenerator2000_core::Core;
use regenerator2000_core::event::{CommentKind as DialogCommentKind, CoreEvent, DialogType};
use regenerator2000_core::state::actions::AppAction;
use regenerator2000_core::state::{AddrSpan, LabelLocality};
use regenerator2000_core::unpacker;
use std::cell::Cell;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

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
    event_tx: &Sender<AppEvent>,
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
                ui_state.set_status_message(format!("Opened URL: {url}"));
            }
            CoreEvent::StatusMessage(msg) => ui_state.set_status_message(msg),
            CoreEvent::DialogRequested(dialog_type) => {
                if !matches!(dialog_type, DialogType::JumpToOperand { .. }) {
                    // Reset stale hit-test bounds when replacing `active_dialog` directly before its first render.
                    ui_state.active_dialog_area = Rect::default();
                }
                match dialog_type {
                    DialogType::Open => {
                        ui_state.active_dialog = Some(Box::new(FileBrowserDialog::new_open(
                            ui_state.file_dialog_current_dir.clone(),
                        )));
                    }
                    DialogType::OpenRecent => {
                        ui_state.active_dialog = Some(Box::new(OpenRecentDialog));
                        ui_state.recent_list_state.select(Some(0));
                    }
                    DialogType::ImportViceLabels => {
                        ui_state.active_dialog =
                            Some(Box::new(FileBrowserDialog::new_import_vice_labels(
                                ui_state.file_dialog_current_dir.clone(),
                                core.state.last_import_labels_path.clone(),
                            )));
                    }
                    DialogType::ExportLabels { initial_filename } => {
                        ui_state.active_dialog =
                            Some(Box::new(FileBrowserDialog::new_export_labels(
                                initial_filename,
                                ui_state.file_dialog_current_dir.clone(),
                            )));
                    }
                    DialogType::SaveAs { initial_filename } => {
                        ui_state.active_dialog =
                            Some(Box::new(FileBrowserDialog::new_save_project(
                                initial_filename,
                                ui_state.file_dialog_current_dir.clone(),
                            )));
                    }
                    DialogType::ExportAs {
                        initial_filename,
                        format,
                    } => {
                        ui_state.active_dialog = Some(Box::new(FileBrowserDialog::new_export(
                            initial_filename,
                            format,
                            ui_state.file_dialog_current_dir.clone(),
                        )));
                    }
                    DialogType::DocumentSettings => {
                        ui_state.active_dialog = Some(Box::new(DocumentSettingsDialog::new()));
                    }
                    DialogType::FileInfo => {
                        ui_state.active_dialog = Some(Box::new(FileInfoDialog::new()));
                    }
                    DialogType::JumpToAddress => {
                        ui_state.active_dialog =
                            Some(Box::new(PromptDialog::new_jump_to_address()));
                    }
                    DialogType::JumpToOperand {
                        source_address,
                        targets,
                    } => {
                        // Use `push_dialog` rather than overwriting `active_dialog` so any queued
                        // dialog behind the picker promotes cleanly upon dismissal.
                        ui_state.push_dialog(Box::new(JumpToOperandDialog::new(
                            source_address,
                            targets,
                        )));
                    }
                    DialogType::JumpToLine => {
                        ui_state.active_dialog = Some(Box::new(PromptDialog::new_jump_to_line()));
                    }
                    DialogType::Search { query, filters } => {
                        ui_state.active_dialog = Some(Box::new(SearchDialog::new(query, filters)));
                    }
                    DialogType::GoToSymbol => {
                        ui_state.active_dialog = Some(Box::new(GoToSymbolDialog::new(&core.state)));
                    }
                    DialogType::KeyboardShortcuts => {
                        ui_state.active_dialog = Some(Box::new(ShortcutsDialog::new()));
                    }
                    DialogType::About => {
                        ui_state.active_dialog =
                            Some(Box::new(AboutDialog::new(ui_state, event_tx.clone())));
                    }
                    DialogType::ViceConnect => {
                        ui_state.active_dialog = Some(Box::new(ViceConnectDialog::new()));
                    }
                    DialogType::Label {
                        address,
                        initial_name,
                        is_local,
                        ..
                    } => {
                        let locality = if is_local {
                            LabelLocality::Local
                        } else {
                            LabelLocality::Global
                        };
                        ui_state.active_dialog = Some(Box::new(LabelDialog::new(
                            Some(&initial_name),
                            address,
                            locality,
                        )));
                    }
                    DialogType::Comment {
                        address,
                        current,
                        kind,
                    } => {
                        let dialog_kind = match kind {
                            DialogCommentKind::Side => CommentType::Side,
                            DialogCommentKind::Line => CommentType::Line,
                        };
                        ui_state.active_dialog = Some(Box::new(CommentDialog::new(
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
                        ui_state.active_dialog =
                            Some(Box::new(ConfirmationDialog::new(title, message, action)));
                    }
                    DialogType::Bookmarks => {
                        ui_state.active_dialog = Some(Box::new(BookmarksDialog::new()));
                    }
                    DialogType::FindReferences(addr) => {
                        ui_state.active_dialog =
                            Some(Box::new(FindReferencesDialog::new(&core.state, addr)));
                    }
                    DialogType::BreakpointAddress(addr) => {
                        ui_state.active_dialog =
                            Some(Box::new(PromptDialog::new_breakpoint_address(addr)));
                    }
                    DialogType::WatchpointAddress(addr) => {
                        ui_state.active_dialog = Some(Box::new(WatchpointAddressDialog::new(addr)));
                    }
                    DialogType::MemoryDumpAddress(addr) => {
                        ui_state.active_dialog =
                            Some(Box::new(PromptDialog::new_memory_dump_address(addr)));
                    }
                    DialogType::Settings => {
                        ui_state.active_dialog = Some(Box::new(SettingsDialog::new()));
                    }
                    DialogType::CompleteAddress {
                        known_byte,
                        lo_first,
                        address,
                    } => {
                        let position = if lo_first {
                            KnownBytePosition::Low
                        } else {
                            KnownBytePosition::High
                        };
                        ui_state.active_dialog = Some(Box::new(CompleteAddressDialog::new(
                            known_byte, position, address,
                        )));
                    }
                    DialogType::ImportContext {
                        origin,
                        entry_point,
                    } => {
                        let packer = core.state.file_info().packer_name;
                        ui_state.push_dialog(Box::new(ImportContextDialog::new(
                            core.state.settings.system.as_str(),
                            origin,
                            Some(entry_point),
                            None, // inherit system from current state
                            None, // no entropy warning
                            packer,
                        )));
                    }
                    DialogType::Origin => {
                        ui_state.active_dialog =
                            Some(Box::new(PromptDialog::new_origin(core.state.origin)));
                    }
                    DialogType::ApplyEnum { address, value } => {
                        ui_state.active_dialog =
                            Some(Box::new(ApplyEnumDialog::new(address, value, &core.state)));
                    }
                    DialogType::ManageEnums => {
                        ui_state.active_dialog =
                            Some(Box::new(ManageEnumsDialog::new(&core.state)));
                    }
                    DialogType::SymbolicOffset {
                        site_address,
                        target_address,
                        address_range,
                        initial_input,
                        is_global,
                        lock_global,
                        is_zero_page_mode,
                    } => {
                        ui_state.active_dialog = Some(Box::new(SymbolicOffsetDialog::new(
                            SymbolicOffsetDialogParams {
                                site_address,
                                target_address,
                                address_range: address_range
                                    .map(|(start, end)| AddrSpan { start, end }),
                                initial_input: &initial_input,
                                is_global,
                                lock_global,
                                is_zero_page_mode,
                            },
                        )));
                    }
                    DialogType::Unpack => {
                        ui_state.active_dialog = Some(Box::new(UnpackDialog::new()));
                    }
                }
            }
            CoreEvent::DialogDismissalRequested => {
                ui_state.active_dialog = None;
                ui_state.active_dialog_area = Rect::default();
            }
            CoreEvent::ViewChanged | CoreEvent::StateChanged => {
                ui_state.core = core.view.clone();
                ui_state.sync_core_to_tui();
                ui_state.sync_status_message();
                if !core.state.vice_state.connected {
                    ui_state.clear_debugger_state();
                }
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
                thread::spawn(move || {
                    let progress_tx = tx.clone();
                    let last_update = Cell::new(Instant::now());
                    let progress_cb = move |count: u64| {
                        let now = Instant::now();
                        if now.duration_since(last_update.get()) >= Duration::from_millis(50) {
                            last_update.set(now);
                            let _ = progress_tx.send(AppEvent::UnpackProgress(count));
                        }
                    };
                    let result =
                        unpacker::unpack(&raw_data, load_addr, &config, Some(&progress_cb));
                    let _ = tx.send(AppEvent::UnpackComplete(result));
                });
            }
            CoreEvent::StartMcpServerRequested => {
                let _ = event_tx.send(AppEvent::McpStartRequested);
            }
            CoreEvent::StopMcpServerRequested => {
                let _ = event_tx.send(AppEvent::McpStopRequested);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui::ui;
    use crate::ui::widget::WidgetResult;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell as BufferCell;
    use regenerator2000_core::state::{Addr, BlockType, Label, LabelKind, LabelType};
    use std::iter;
    use std::sync::mpsc;

    #[test]
    fn test_set_label_preserves_local_user_label_after_auto_label() {
        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x1000), vec![0xEA])
            .expect("load 1-byte NOP fixture");
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

        let mut ui_state = UIState::new(Theme::default());
        ui_state.cursor_index = 0;
        let (tx, _rx) = mpsc::channel();

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

    #[test]
    fn handle_menu_action_jump_to_operand_pushes_dialog_and_renders_targets() {
        let mut core = Core::new();
        core.state.origin = Addr(0x0830);
        // 2-entry LoHiAddress table at $0830 targeting $0836 and $0839.
        // $0830..$0834: <$0836, <$0839, >$0836, >$0839; $0834..$083C: NOPs + RTS.
        let mut raw = vec![0x36, 0x39, 0x08, 0x08];
        raw.extend(iter::repeat_n(0xEA, 8));
        core.state.raw_data = raw;
        core.state.block_types = vec![BlockType::Code; core.state.raw_data.len()];
        for bt in &mut core.state.block_types[0..4] {
            *bt = BlockType::LoHiAddress;
        }
        core.state.labels.insert(
            Addr(0x0836),
            vec![Label {
                name: "state_0_border_red".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        core.state.labels.insert(
            Addr(0x0839),
            vec![Label {
                name: "state_1_border_green".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        core.state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.dismiss_logo = true;
        ui_state.cursor_index = core
            .state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x0830) && !l.bytes.is_empty())
            .expect("LoHiAddress line at $0830");
        let (tx, _rx) = mpsc::channel();

        handle_menu_action(&mut core, &mut ui_state, AppAction::JumpToOperand, &tx);

        assert!(ui_state.active_dialog.is_some());
        assert!(ui_state.core.navigation_history.is_empty());
        assert!(AppAction::NavigateToAddress(Addr(0x0836)).closes_dialog());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        terminal
            .draw(|f| ui(f, &core.state, &mut ui_state))
            .expect("draw jump-to-operand dialog");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(BufferCell::symbol)
            .collect();
        assert!(rendered.contains("Jump to Operand from $0830"));
        assert!(rendered.contains("[1]"));
        assert!(rendered.contains("[2]"));
        assert!(rendered.contains("$0836"));
        assert!(rendered.contains("$0839"));
    }
}
