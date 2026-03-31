use crate::cpu::AddressingMode;
use crate::event::CoreEvent;
use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
use crate::view_state::CoreViewState;

/// The central engine of Regenerator 2000.
///
/// Manages persistent state (`AppState`) and transient view state (`CoreViewState`).
/// Frontends interact with this via `apply_action`.
pub struct Core {
    pub state: AppState,
    pub view: CoreViewState,
}

impl Core {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            view: CoreViewState::new(),
        }
    }

    /// Handles a semantic action and returns a list of events for the frontend.
    pub fn apply_action(&mut self, action: AppAction) -> Vec<CoreEvent> {
        let mut events = Vec::new();

        // Check for unsaved changes on destructive actions
        if action.is_destructive() && self.state.is_dirty() {
            events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Confirmation {
                    title: "Unsaved Changes".to_string(),
                    message: "You have unsaved changes. Proceed anyway?".to_string(),
                    action,
                },
            ));
            return events;
        }

        // Unwrap Confirmed for processing
        let action = match action {
            AppAction::Confirmed(inner) => *inner,
            other => other,
        };

        if action.requires_document() && self.state.raw_data.is_empty() {
            events.push(CoreEvent::StatusMessage("No open document".to_string()));
            return events;
        }

        if action == AppAction::FindReferences
            && self.view.active_pane != crate::view_state::ActivePane::Disassembly
        {
            events.push(CoreEvent::StatusMessage(
                "Action only available in Disassembly View".to_string(),
            ));
            return events;
        }

        match action {
            AppAction::Exit => {
                events.push(CoreEvent::QuitRequested);
            }
            AppAction::Open => {
                events.push(CoreEvent::DialogRequested(crate::event::DialogType::Open));
                events.push(CoreEvent::StatusMessage(
                    "Select a file to open".to_string(),
                ));
            }
            AppAction::OpenRecent => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::OpenRecent,
                ));
                events.push(CoreEvent::StatusMessage("Open recent project".to_string()));
            }
            AppAction::About => {
                events.push(CoreEvent::DialogRequested(crate::event::DialogType::About));
                events.push(CoreEvent::StatusMessage(
                    "About Regenerator 2000".to_string(),
                ));
            }
            AppAction::KeyboardShortcuts => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::KeyboardShortcuts,
                ));
                events.push(CoreEvent::StatusMessage("Keyboard Shortcuts".to_string()));
            }
            AppAction::SystemSettings => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Settings,
                ));
                events.push(CoreEvent::StatusMessage("System Settings".to_string()));
            }
            AppAction::DocumentSettings => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::DocumentSettings,
                ));
                events.push(CoreEvent::StatusMessage("Document Settings".to_string()));
            }
            AppAction::Analyze => {
                let current_addr = self
                    .state
                    .disassembly
                    .get(self.view.cursor_index)
                    .map(|l| l.address);

                let (cmd, msg) = self.state.perform_analysis();
                self.state.push_command(cmd);

                if let Some(addr) = current_addr {
                    if let Some(idx) = self.state.get_line_index_containing_address(addr) {
                        self.view.cursor_index = idx;
                    } else if let Some(idx) = self.state.get_line_index_for_address(addr) {
                        self.view.cursor_index = idx;
                    }
                }

                events.push(CoreEvent::StatusMessage(msg));
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::Undo => {
                let msg = self.state.undo_last_command();
                events.push(CoreEvent::StatusMessage(msg));
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::Redo => {
                let msg = self.state.redo_last_command();
                events.push(CoreEvent::StatusMessage(msg));
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleBookmark => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    if line.external_label_address.is_none() {
                        let address = line.address;
                        let is_bookmarked = self.state.bookmarks.contains_key(&address);

                        let command = crate::commands::Command::SetBookmark {
                            address,
                            new_name: if is_bookmarked {
                                None
                            } else {
                                Some(String::new())
                            },
                            old_name: self.state.bookmarks.get(&address).cloned(),
                        };
                        command.apply(&mut self.state);
                        self.state.push_command(command);

                        let msg = if is_bookmarked {
                            format!("Bookmark removed at ${address:04X}")
                        } else {
                            format!("Bookmark set at ${address:04X}")
                        };
                        events.push(CoreEvent::StatusMessage(msg));
                        events.push(CoreEvent::StateChanged);
                    } else {
                        events.push(CoreEvent::StatusMessage(
                            "Cannot bookmark external label definition".to_string(),
                        ));
                    }
                }
            }
            AppAction::JumpToAddress => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::JumpToAddress,
                ));
                events.push(CoreEvent::StatusMessage("Enter address (Hex)".to_string()));
            }
            AppAction::JumpToLine => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::JumpToLine,
                ));
                events.push(CoreEvent::StatusMessage(
                    "Enter Line Number (Dec)".to_string(),
                ));
            }
            AppAction::ChangeOrigin => {
                events.push(CoreEvent::DialogRequested(crate::event::DialogType::Origin));
                events.push(CoreEvent::StatusMessage("Change Origin...".to_string()));
            }
            AppAction::ApplyOrigin(new_origin) => {
                let old_origin = self.state.origin;
                let command = crate::commands::Command::ChangeOrigin {
                    new_origin,
                    old_origin,
                };
                command.apply(&mut self.state);
                self.state.push_command(command);
                self.state.disassemble();
                events.push(CoreEvent::StatusMessage(format!(
                    "Origin changed to ${:04X}",
                    new_origin.0
                )));
                events.push(CoreEvent::StateChanged);
            }
            AppAction::Search => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Search {
                        query: String::new(), // Frontend can fill with last search if desired, or we could pass it here
                        filters: crate::state::search::SearchFilters::default(),
                    },
                ));
                events.push(CoreEvent::StatusMessage("Search...".to_string()));
            }
            AppAction::GoToSymbol => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::GoToSymbol,
                ));
                events.push(CoreEvent::StatusMessage("Go to Symbol...".to_string()));
            }
            AppAction::ToggleHexDump => {
                if self.view.right_pane == crate::view_state::RightPane::HexDump {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Hex Dump View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::HexDump {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::HexDump;
                    self.view.active_pane = crate::view_state::ActivePane::HexDump;
                    events.push(CoreEvent::StatusMessage("Hex Dump View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleSpritesView => {
                if self.view.right_pane == crate::view_state::RightPane::Sprites {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Sprites View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::Sprites {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::Sprites;
                    self.view.active_pane = crate::view_state::ActivePane::Sprites;
                    events.push(CoreEvent::StatusMessage("Sprites View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleCharsetView => {
                if self.view.right_pane == crate::view_state::RightPane::Charset {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Charset View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::Charset {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::Charset;
                    self.view.active_pane = crate::view_state::ActivePane::Charset;
                    events.push(CoreEvent::StatusMessage("Charset View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleBitmapView => {
                if self.view.right_pane == crate::view_state::RightPane::Bitmap {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Bitmap View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::Bitmap {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::Bitmap;
                    self.view.active_pane = crate::view_state::ActivePane::Bitmap;
                    events.push(CoreEvent::StatusMessage("Bitmap View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::NavigateToAddress(target_addr) => {
                self.handle_navigate_to_address(target_addr, &mut events);
            }
            AppAction::Scope => {
                self.handle_add_scope(&mut events);
            }
            AppAction::NudgeScopeBoundary { expand } => {
                self.handle_nudge_scope_boundary(expand, &mut events);
            }
            AppAction::RemoveScope => {
                self.handle_remove_scope(&mut events);
            }
            AppAction::Code => self.apply_block_type(crate::state::BlockType::Code, &mut events),
            AppAction::DisassembleAddress => {
                let addr = if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    line.address
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Invalid cursor position".to_string(),
                    ));
                    return events;
                };
                let ranges = crate::analyzer::flow_analyze(&self.state, addr);
                let mut commands = Vec::new();
                for range in ranges {
                    let old_types = self.state.block_types[range.start..range.end].to_vec();
                    commands.push(crate::commands::Command::SetBlockType {
                        range: range.clone(),
                        new_type: crate::state::BlockType::Code,
                        old_types,
                    });
                }
                if !commands.is_empty() {
                    let batch = crate::commands::Command::Batch(commands);
                    batch.apply(&mut self.state);
                    let (analysis_cmd, _) = self.state.perform_analysis();
                    let final_cmd = crate::commands::Command::Batch(vec![batch, analysis_cmd]);
                    self.state.push_command(final_cmd);
                    events.push(CoreEvent::StatusMessage(format!(
                        "Flow analyzed from ${:04X}",
                        addr.0
                    )));
                    events.push(CoreEvent::StateChanged);
                    events.push(CoreEvent::ViewChanged);
                } else {
                    events.push(CoreEvent::StatusMessage(format!(
                        "No new code found from ${:04X}",
                        addr.0
                    )));
                }
            }
            AppAction::Byte => {
                self.apply_block_type(crate::state::BlockType::DataByte, &mut events);
            }
            AppAction::Word => {
                self.apply_block_type(crate::state::BlockType::DataWord, &mut events);
            }
            AppAction::Address => {
                self.apply_block_type(crate::state::BlockType::Address, &mut events);
            }
            AppAction::PetsciiText => {
                self.apply_block_type(crate::state::BlockType::PetsciiText, &mut events);
            }
            AppAction::ScreencodeText => {
                self.apply_block_type(crate::state::BlockType::ScreencodeText, &mut events);
            }
            AppAction::Undefined => {
                self.apply_block_type(crate::state::BlockType::Undefined, &mut events);
            }
            AppAction::SetLabel => self.handle_set_label(&mut events),
            AppAction::PackLoHiAddress => self.handle_lo_hi_packing(true, &mut events),
            AppAction::PackHiLoAddress => self.handle_lo_hi_packing(false, &mut events),
            AppAction::SetLoHiAddress => {
                self.apply_block_type(crate::state::BlockType::LoHiAddress, &mut events);
            }
            AppAction::SetHiLoAddress => {
                self.apply_block_type(crate::state::BlockType::HiLoAddress, &mut events);
            }
            AppAction::SetLoHiWord => {
                self.apply_block_type(crate::state::BlockType::LoHiWord, &mut events);
            }
            AppAction::SetHiLoWord => {
                self.apply_block_type(crate::state::BlockType::HiLoWord, &mut events);
            }
            AppAction::ToggleSplitter => {
                use crate::view_state::ActivePane;
                if self.view.active_pane == ActivePane::Blocks {
                    let blocks = self.state.get_blocks_view_items();
                    if let Some(idx) = self.view.blocks_selected_index
                        && idx < blocks.len()
                        && let crate::state::BlockItem::Splitter(addr) = blocks[idx]
                    {
                        let command = crate::commands::Command::ToggleSplitter { address: addr };
                        command.apply(&mut self.state);
                        self.state.push_command(command);
                        events.push(CoreEvent::StatusMessage(format!(
                            "Removed splitter at ${addr:04X}"
                        )));
                        events.push(CoreEvent::StateChanged);
                    }
                } else if self.view.active_pane == ActivePane::Disassembly
                    && let Some(line) = self.state.disassembly.get(self.view.cursor_index)
                {
                    let addr = line.address;
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(&mut self.state);
                    self.state.push_command(command);
                    events.push(CoreEvent::StatusMessage(format!(
                        "Toggled splitter at ${addr:04X}"
                    )));
                    events.push(CoreEvent::StateChanged);
                }
            }
            AppAction::Save => {
                if let Some(path) = self.state.project_path.clone() {
                    let context = self.create_save_context();
                    match self.state.save_project(context, true) {
                        Ok(_) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            events.push(CoreEvent::StatusMessage(format!("Saved: {filename}")));
                        }
                        Err(e) => {
                            events.push(CoreEvent::StatusMessage(format!("Error saving: {e}")));
                        }
                    }
                } else {
                    let initial = self
                        .state
                        .last_save_as_filename
                        .clone()
                        .or_else(|| self.get_default_filename_stem());
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::SaveAs {
                            initial_filename: initial,
                        },
                    ));
                    events.push(CoreEvent::StatusMessage(
                        "Enter Project filename".to_string(),
                    ));
                }
            }
            AppAction::ImportViceLabels => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ImportViceLabels,
                ));
                events.push(CoreEvent::StatusMessage(
                    "Select a VICE label file to import".to_string(),
                ));
            }
            AppAction::ExportViceLabels => {
                let initial = self
                    .state
                    .last_export_labels_filename
                    .clone()
                    .or_else(|| self.get_default_filename_stem());
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ExportLabels {
                        initial_filename: initial,
                    },
                ));
                events.push(CoreEvent::StatusMessage(
                    "Enter VICE label filename".to_string(),
                ));
            }
            AppAction::SaveAs => {
                let initial = self
                    .state
                    .last_save_as_filename
                    .clone()
                    .or_else(|| self.get_default_filename_stem());
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::SaveAs {
                        initial_filename: initial,
                    },
                ));
                events.push(CoreEvent::StatusMessage(
                    "Enter Project filename".to_string(),
                ));
            }
            AppAction::ExportProject => {
                if let Some(path) = self.state.export_path.clone() {
                    match crate::exporter::export_asm(&self.state, &path) {
                        Ok(_) => {
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            events.push(CoreEvent::StatusMessage(format!("Exported: {filename}")));
                        }
                        Err(e) => {
                            events.push(CoreEvent::StatusMessage(format!("Error exporting: {e}")));
                        }
                    }
                } else {
                    let initial = self
                        .state
                        .last_export_asm_filename
                        .clone()
                        .or_else(|| self.get_default_filename_stem());
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::ExportAs {
                            initial_filename: initial,
                        },
                    ));
                    events.push(CoreEvent::StatusMessage("Enter .asm filename".to_string()));
                }
            }
            AppAction::ExportProjectAs => {
                let initial = self
                    .state
                    .last_export_asm_filename
                    .clone()
                    .or_else(|| self.get_default_filename_stem());
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ExportAs {
                        initial_filename: initial,
                    },
                ));
                events.push(CoreEvent::StatusMessage("Enter .asm filename".to_string()));
            }
            AppAction::ListBookmarks => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::Bookmarks,
                ));
            }
            AppAction::FindNext => {
                self.perform_search(true, &mut events);
            }
            AppAction::FindPrevious => {
                self.perform_search(false, &mut events);
            }
            AppAction::FindReferences => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let addr = if line.bytes.is_empty() {
                        line.external_label_address.unwrap_or(line.address)
                    } else if line.bytes.len() > 1 {
                        let mut resolved = line.address;
                        let mut current_sub_index = 0;
                        let mut found = false;
                        for offset in 1..line.bytes.len() {
                            let mid_addr = line.address.wrapping_add(offset as u16);
                            if let Some(labels) = self.state.labels.get(&mid_addr) {
                                for _ in labels {
                                    if current_sub_index == self.view.sub_cursor_index {
                                        resolved = mid_addr;
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
                        resolved
                    } else {
                        line.address
                    };
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::FindReferences(addr),
                    ));
                }
            }
            AppAction::ToggleSpriteMulticolor => {
                self.view.sprite_multicolor_mode = !self.view.sprite_multicolor_mode;
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleCharsetMulticolor => {
                self.view.charset_multicolor_mode = !self.view.charset_multicolor_mode;
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleBitmapMulticolor => {
                self.view.bitmap_multicolor_mode = !self.view.bitmap_multicolor_mode;
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleBlocksView => {
                if self.view.right_pane == crate::view_state::RightPane::Blocks {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Blocks View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::Blocks {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::Blocks;
                    self.view.active_pane = crate::view_state::ActivePane::Blocks;
                    events.push(CoreEvent::StatusMessage("Blocks View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleDebuggerView => {
                if self.view.right_pane == crate::view_state::RightPane::Debugger {
                    self.view.right_pane = crate::view_state::RightPane::None;
                    events.push(CoreEvent::StatusMessage("Debugger View Hidden".to_string()));
                    if self.view.active_pane == crate::view_state::ActivePane::Debugger {
                        self.view.active_pane = crate::view_state::ActivePane::Disassembly;
                    }
                } else {
                    self.view.right_pane = crate::view_state::RightPane::Debugger;
                    self.view.active_pane = crate::view_state::ActivePane::Debugger;
                    events.push(CoreEvent::StatusMessage("Debugger View Shown".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::NavigateBack => {
                if let Some((pane, target)) = self.view.navigation_history.pop() {
                    self.view.active_pane = pane;
                    match target {
                        crate::view_state::NavigationTarget::Address(addr) => {
                            crate::navigation::perform_jump_to_address_no_history(
                                &self.state,
                                &mut self.view,
                                crate::state::Addr(addr),
                            );
                        }
                        crate::view_state::NavigationTarget::Index(idx) => {
                            self.view.cursor_index = idx;
                            self.view.scroll_index = idx;
                            self.view.scroll_sub_index = 0;
                            self.view.sub_cursor_index = 0;
                        }
                    }
                    self.view.status_message = Some("Navigated back".to_string());
                    events.push(CoreEvent::ViewChanged);
                } else {
                    self.view.status_message = Some("No history".to_string());
                    events.push(CoreEvent::ViewChanged);
                }
            }
            AppAction::SideComment => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let address = line.address;
                    let current = self.state.user_side_comments.get(&address).cloned();
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::Comment {
                            address,
                            current,
                            kind: crate::state::types::CommentKind::Side,
                        },
                    ));
                    events.push(CoreEvent::StatusMessage(format!(
                        "Edit Side Comment at ${address:04X}"
                    )));
                }
            }
            AppAction::LineComment => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let address = line.address;
                    let current = self.state.user_line_comments.get(&address).cloned();
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::Comment {
                            address,
                            current,
                            kind: crate::state::types::CommentKind::Line,
                        },
                    ));
                    events.push(CoreEvent::StatusMessage(format!(
                        "Edit Line Comment at ${address:04X}"
                    )));
                }
            }
            AppAction::SetExternalFile => {
                self.apply_block_type(crate::state::BlockType::ExternalFile, &mut events);
            }
            AppAction::ViceConnect => {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::ViceConnect,
                ));
                events.push(CoreEvent::StatusMessage(
                    "Enter VICE hostname and port".to_string(),
                ));
            }
            AppAction::ViceDisconnect => {
                self.state.vice_client = None;
                self.state.vice_state.connected = false;
                events.push(CoreEvent::StatusMessage(
                    "Disconnected from VICE".to_string(),
                ));
                events.push(CoreEvent::StateChanged);
            }
            AppAction::ViceStep => {
                if let Some(client) = &self.state.vice_client {
                    self.state.vice_state.previous = Some(self.state.vice_state.snapshot());
                    client.send_advance_instruction();
                    self.state.vice_state.running = true;
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
            }
            AppAction::ViceContinue => {
                if let Some(client) = &self.state.vice_client {
                    self.state.vice_state.previous = Some(self.state.vice_state.snapshot());
                    client.send_continue();
                    self.state.vice_state.running = true;
                    events.push(CoreEvent::StatusMessage("VICE: Running...".to_string()));
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
            }
            AppAction::ViceStepOver => {
                if let Some(client) = &self.state.vice_client {
                    self.state.vice_state.previous = Some(self.state.vice_state.snapshot());
                    client.send_step_over();
                    self.state.vice_state.running = true;
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
            }
            AppAction::ViceStepOut => {
                if let Some(client) = &self.state.vice_client {
                    self.state.vice_state.previous = Some(self.state.vice_state.snapshot());
                    client.send_execute_until_return();
                    self.state.vice_state.running = true;
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
            }
            AppAction::ViceRunToCursor => {
                if let Some(client) = &self.state.vice_client {
                    if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                        self.state.vice_state.previous = Some(self.state.vice_state.snapshot());
                        client.send_checkpoint_set_exec_temp(line.address.0);
                        client.send_continue();
                        self.state.vice_state.running = true;
                    }
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "Not connected to VICE".to_string(),
                    ));
                }
            }
            AppAction::ViceToggleBreakpoint => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let checkpoint_id = self
                        .state
                        .vice_state
                        .breakpoints
                        .iter()
                        .find(|bp| {
                            bp.address == line.address.0
                                && bp.kind == crate::vice::state::BreakpointKind::Exec
                        })
                        .map(|bp| bp.id);

                    if let Some(id) = checkpoint_id {
                        if let Some(client) = &self.state.vice_client {
                            client.send_checkpoint_delete(id);
                            events.push(CoreEvent::StatusMessage(format!(
                                "Deleting breakpoint #{} at ${:04X}",
                                id, line.address.0
                            )));
                        }
                    } else if let Some(client) = &self.state.vice_client {
                        client.send_checkpoint_set_exec(line.address.0);
                        events.push(CoreEvent::StatusMessage(format!(
                            "Creating breakpoint at ${:04X}",
                            line.address.0
                        )));
                    }
                    events.push(CoreEvent::StateChanged);
                }
            }
            AppAction::ViceBreakpointDialog => {
                let prefill = self
                    .state
                    .disassembly
                    .get(self.view.cursor_index)
                    .map(|l| l.address.0);
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::BreakpointAddress(prefill),
                ));
            }
            AppAction::ViceSetBreakpointAt { address } => {
                if let Some(client) = &self.state.vice_client {
                    let existing_id = self
                        .state
                        .vice_state
                        .breakpoints
                        .iter()
                        .find(|bp| {
                            bp.address == address.0
                                && bp.kind == crate::vice::state::BreakpointKind::Exec
                        })
                        .map(|bp| bp.id);

                    if let Some(id) = existing_id {
                        client.send_checkpoint_delete(id);
                        events.push(CoreEvent::StatusMessage(format!(
                            "Deleting breakpoint #{} at ${:04X}",
                            id, address.0
                        )));
                    } else {
                        client.send_checkpoint_set_exec(address.0);
                        events.push(CoreEvent::StatusMessage(format!(
                            "Creating breakpoint at ${:04X}",
                            address.0
                        )));
                    }
                    events.push(CoreEvent::StateChanged);
                }
            }
            AppAction::ViceToggleWatchpoint => {
                let prefill = self
                    .state
                    .disassembly
                    .get(self.view.cursor_index)
                    .map(|l| l.address.0);
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::WatchpointAddress(prefill),
                ));
            }
            AppAction::ViceMemoryDumpDialog => {
                let prefill = self.state.vice_state.dump_address;
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::MemoryDumpAddress(prefill),
                ));
            }
            AppAction::ViceSetMemoryDumpAddress { address } => {
                self.state.vice_state.dump_address = Some(address.0);
                if let Some(client) = &self.state.vice_client
                    && !self.state.vice_state.running
                {
                    client.send_memory_get(address.0, address.0.saturating_add(63), 6);
                }
                events.push(CoreEvent::StatusMessage(format!(
                    "Memory dump set to ${:04X}",
                    address.0
                )));
                events.push(CoreEvent::StateChanged);
            }
            AppAction::ViceSetWatchpoint { address, kind } => {
                if let Some(client) = &self.state.vice_client {
                    let existing_id = self
                        .state
                        .vice_state
                        .breakpoints
                        .iter()
                        .find(|bp| {
                            bp.address == address.0
                                && bp.kind != crate::vice::state::BreakpointKind::Exec
                        })
                        .map(|bp| bp.id);

                    if let Some(id) = existing_id {
                        client.send_checkpoint_delete(id);
                        events.push(CoreEvent::StatusMessage(format!(
                            "Deleting watchpoint #{} at ${:04X}",
                            id, address.0
                        )));
                    } else {
                        match kind {
                            crate::vice::state::BreakpointKind::Load => {
                                client.send_checkpoint_set_load(address.0);
                            }
                            crate::vice::state::BreakpointKind::Store => {
                                client.send_checkpoint_set_store(address.0);
                            }
                            crate::vice::state::BreakpointKind::LoadStore => {
                                client.send_checkpoint_set_load_store(address.0);
                            }
                            _ => {
                                client.send_checkpoint_set_load_store(address.0);
                            }
                        }
                        events.push(CoreEvent::StatusMessage(format!(
                            "Creating watchpoint at ${:04X}",
                            address.0
                        )));
                    }
                    events.push(CoreEvent::StateChanged);
                }
            }
            AppAction::JumpToOperand => {
                let target_addr = match self.view.active_pane {
                    crate::view_state::ActivePane::Disassembly => {
                        if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                            if let Some(opcode) = &line.opcode {
                                match opcode.mode {
                                    AddressingMode::Immediate => {
                                        if let Some(fmt) =
                                            self.state.immediate_value_formats.get(&line.address)
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
                    crate::view_state::ActivePane::HexDump => {
                        let origin = self.state.origin.0 as usize;
                        let alignment_padding = origin % 16;
                        let aligned_origin = origin - alignment_padding;
                        Some(crate::state::Addr(
                            (aligned_origin + self.view.hex_cursor_index * 16) as u16,
                        ))
                    }
                    crate::view_state::ActivePane::Sprites => {
                        let origin = self.state.origin.0 as usize;
                        let padding = (64 - (origin % 64)) % 64;
                        Some(crate::state::Addr(
                            (origin + padding + self.view.sprites_cursor_index * 64) as u16,
                        ))
                    }
                    crate::view_state::ActivePane::Charset => {
                        let origin = self.state.origin.0 as usize;
                        let base_alignment = 0x400;
                        let aligned_start_addr = (origin / base_alignment) * base_alignment;
                        Some(crate::state::Addr(
                            (aligned_start_addr + self.view.charset_cursor_index * 8) as u16,
                        ))
                    }
                    crate::view_state::ActivePane::Blocks => {
                        let blocks = self.state.get_blocks_view_items();
                        let idx = self.view.blocks_selected_index.unwrap_or(0);
                        if idx < blocks.len() {
                            match blocks[idx] {
                                crate::state::BlockItem::Block { start, .. } => Some(start),
                                crate::state::BlockItem::Splitter(addr) => Some(addr),
                                crate::state::BlockItem::Scope { start, .. } => Some(start),
                            }
                        } else {
                            None
                        }
                    }
                    crate::view_state::ActivePane::Bitmap => {
                        let origin = self.state.origin.0 as usize;
                        let first_aligned_addr = ((origin / 8192) * 8192)
                            + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                        let bitmap_addr =
                            first_aligned_addr + (self.view.bitmap_cursor_index * 8192);
                        Some(crate::state::Addr(bitmap_addr as u16))
                    }
                    _ => None,
                };

                if let Some(addr) = target_addr {
                    self.handle_navigate_to_address(addr, &mut events);
                } else {
                    events.push(CoreEvent::StatusMessage(
                        "No valid operand to jump to".to_string(),
                    ));
                }
            }
            AppAction::ApplyLabel { address, name } => {
                self.handle_apply_label(address, name, &mut events);
            }
            AppAction::ApplyComment {
                address,
                text,
                kind,
            } => {
                self.handle_apply_comment(address, text, kind, &mut events);
            }
            AppAction::CyclePane => {
                use crate::view_state::ActivePane;
                self.view.active_pane = match self.view.active_pane {
                    ActivePane::Disassembly => match self.view.right_pane {
                        crate::view_state::RightPane::None => ActivePane::Disassembly,
                        crate::view_state::RightPane::HexDump => ActivePane::HexDump,
                        crate::view_state::RightPane::Sprites => ActivePane::Sprites,
                        crate::view_state::RightPane::Charset => ActivePane::Charset,
                        crate::view_state::RightPane::Bitmap => ActivePane::Bitmap,
                        crate::view_state::RightPane::Blocks => ActivePane::Blocks,
                        crate::view_state::RightPane::Debugger => ActivePane::Debugger,
                    },
                    ActivePane::HexDump
                    | ActivePane::Sprites
                    | ActivePane::Charset
                    | ActivePane::Bitmap
                    | ActivePane::Blocks
                    | ActivePane::Debugger => ActivePane::Disassembly,
                };
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::Cancel => {
                if self.view.is_visual_mode {
                    self.view.is_visual_mode = false;
                    self.view.selection_start = None;
                    events.push(CoreEvent::StatusMessage("Visual Mode Exited".to_string()));
                } else if self.view.selection_start.is_some() {
                    self.view.selection_start = None;
                    events.push(CoreEvent::StatusMessage("Selection cleared".to_string()));
                }
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::NextImmediateFormat => {
                self.cycle_immediate_format(true, &mut events);
            }
            AppAction::PreviousImmediateFormat => {
                self.cycle_immediate_format(false, &mut events);
            }
            AppAction::HexdumpViewModeNext => {
                use crate::state::types::HexdumpViewMode;
                self.view.hexdump_view_mode = match self.view.hexdump_view_mode {
                    HexdumpViewMode::ScreencodeShifted => HexdumpViewMode::ScreencodeUnshifted,
                    HexdumpViewMode::ScreencodeUnshifted => HexdumpViewMode::PETSCIIShifted,
                    HexdumpViewMode::PETSCIIShifted => HexdumpViewMode::PETSCIIUnshifted,
                    HexdumpViewMode::PETSCIIUnshifted => HexdumpViewMode::ScreencodeShifted,
                };
                events.push(CoreEvent::StatusMessage(format!(
                    "Hexdump Mode: {:?}",
                    self.view.hexdump_view_mode
                )));
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::HexdumpViewModePrev => {
                use crate::state::types::HexdumpViewMode;
                self.view.hexdump_view_mode = match self.view.hexdump_view_mode {
                    HexdumpViewMode::ScreencodeShifted => HexdumpViewMode::PETSCIIUnshifted,
                    HexdumpViewMode::ScreencodeUnshifted => HexdumpViewMode::ScreencodeShifted,
                    HexdumpViewMode::PETSCIIShifted => HexdumpViewMode::ScreencodeUnshifted,
                    HexdumpViewMode::PETSCIIUnshifted => HexdumpViewMode::PETSCIIShifted,
                };
                events.push(CoreEvent::StatusMessage(format!(
                    "Hexdump Mode: {:?}",
                    self.view.hexdump_view_mode
                )));
                events.push(CoreEvent::ViewChanged);
            }
            AppAction::ToggleCollapsedBlock => {
                self.toggle_collapsed_block(&mut events);
            }
            _ => {}
        }

        events
    }

    fn cycle_immediate_format(&mut self, forward: bool, events: &mut Vec<CoreEvent>) {
        use crate::state::types::ImmediateFormat;
        if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
            let address = line.address;
            let current = self.state.immediate_value_formats.get(&address);

            let next = match (current, forward) {
                (None, true) => Some(ImmediateFormat::Hex),
                (None, false) => Some(ImmediateFormat::HighByte(Addr::ZERO)),
                (Some(ImmediateFormat::Hex), true) => Some(ImmediateFormat::InvertedHex),
                (Some(ImmediateFormat::Hex), false) => None,
                (Some(ImmediateFormat::InvertedHex), true) => Some(ImmediateFormat::Decimal),
                (Some(ImmediateFormat::InvertedHex), false) => Some(ImmediateFormat::Hex),
                (Some(ImmediateFormat::Decimal), true) => Some(ImmediateFormat::NegativeDecimal),
                (Some(ImmediateFormat::Decimal), false) => Some(ImmediateFormat::InvertedHex),
                (Some(ImmediateFormat::NegativeDecimal), true) => Some(ImmediateFormat::Binary),
                (Some(ImmediateFormat::NegativeDecimal), false) => Some(ImmediateFormat::Decimal),
                (Some(ImmediateFormat::Binary), true) => Some(ImmediateFormat::InvertedBinary),
                (Some(ImmediateFormat::Binary), false) => Some(ImmediateFormat::NegativeDecimal),
                (Some(ImmediateFormat::InvertedBinary), true) => {
                    Some(ImmediateFormat::LowByte(Addr::ZERO))
                }
                (Some(ImmediateFormat::InvertedBinary), false) => Some(ImmediateFormat::Binary),
                (Some(ImmediateFormat::LowByte(_)), true) => {
                    Some(ImmediateFormat::HighByte(Addr::ZERO))
                }
                (Some(ImmediateFormat::LowByte(_)), false) => Some(ImmediateFormat::InvertedBinary),
                (Some(ImmediateFormat::HighByte(_)), true) => None,
                (Some(ImmediateFormat::HighByte(_)), false) => {
                    Some(ImmediateFormat::LowByte(Addr::ZERO))
                }
            };

            let command = crate::commands::Command::SetImmediateFormat {
                address,
                new_format: next,
                old_format: current.cloned(),
            };
            command.apply(&mut self.state);
            self.state.push_command(command);
            self.state.disassemble();
            events.push(CoreEvent::StateChanged);

            if let Some(fmt) = next {
                events.push(CoreEvent::StatusMessage(format!("Set format to {fmt:?}")));
            } else {
                events.push(CoreEvent::StatusMessage(
                    "Reset format to default".to_string(),
                ));
            }
        }
    }

    fn toggle_collapsed_block(&mut self, events: &mut Vec<CoreEvent>) {
        if self.view.active_pane == crate::view_state::ActivePane::Blocks {
            let blocks = self.state.get_blocks_view_items();
            let idx = self.view.blocks_selected_index.unwrap_or(0);
            if idx < blocks.len()
                && let crate::state::BlockItem::Block {
                    start,
                    end,
                    collapsed,
                    ..
                } = blocks[idx]
            {
                let start_offset = start.offset_from(self.state.origin);
                let end_offset = end.offset_from(self.state.origin);
                let range = (start_offset, end_offset);

                let command = if collapsed {
                    crate::commands::Command::UncollapseBlock { range }
                } else {
                    crate::commands::Command::CollapseBlock { range }
                };
                command.apply(&mut self.state);
                self.state.push_command(command);
                self.state.disassemble();
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::StatusMessage(if collapsed {
                    "Uncollapsed block".to_string()
                } else {
                    "Collapsed block".to_string()
                }));
            }
        } else if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
            if line.is_collapsed {
                let start_offset = line.address.offset_from(self.state.origin);
                if let Some(range) = self
                    .state
                    .collapsed_blocks
                    .iter()
                    .find(|(s, _)| *s == start_offset)
                    .cloned()
                {
                    let command = crate::commands::Command::UncollapseBlock { range };
                    command.apply(&mut self.state);
                    self.state.push_command(command);
                    self.state.disassemble();
                    events.push(CoreEvent::StateChanged);
                    events.push(CoreEvent::StatusMessage("Uncollapsed block".to_string()));
                }
            } else {
                // To collapse from disassembly, we need a block at cursor.
                // In TUI this was possible if cursor was on a block header.
                // The analyzer already has blocks.
                let addr = line.address;
                let offset = addr.offset_from(self.state.origin);
                let block = self
                    .state
                    .get_compressed_blocks()
                    .into_iter()
                    .find(|b| offset >= b.start && offset <= b.end);

                if let Some(b) = block {
                    let range = (b.start, b.end);
                    let command = crate::commands::Command::CollapseBlock { range };
                    command.apply(&mut self.state);
                    self.state.push_command(command);
                    self.state.disassemble();
                    events.push(CoreEvent::StateChanged);
                    events.push(CoreEvent::StatusMessage("Collapsed block".to_string()));
                }
            }
        }
    }

    fn apply_block_type(
        &mut self,
        block_type: crate::state::BlockType,
        events: &mut Vec<CoreEvent>,
    ) {
        let needs_even = matches!(
            block_type,
            crate::state::BlockType::LoHiAddress
                | crate::state::BlockType::HiLoAddress
                | crate::state::BlockType::LoHiWord
                | crate::state::BlockType::HiLoWord
        );

        if self.view.active_pane == crate::view_state::ActivePane::Blocks {
            let blocks = self.state.get_blocks_view_items();
            if let Some(idx) = self.view.blocks_selected_index
                && idx < blocks.len()
                && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
            {
                let len = end.offset_from(start) + 1;
                if needs_even && len % 2 != 0 {
                    events.push(CoreEvent::StatusMessage(format!(
                        "Error: {block_type} requires even number of bytes"
                    )));
                    return;
                }
                let cmd1 = self.state.set_block_type_region(
                    block_type,
                    Some(start.offset_from(self.state.origin)),
                    end.offset_from(self.state.origin),
                );
                let (cmd2, _) = self.state.perform_analysis();
                if let Some(cmd) = cmd1 {
                    self.state
                        .push_command(crate::commands::Command::Batch(vec![cmd, cmd2]));
                } else {
                    self.state.push_command(cmd2);
                }
                events.push(CoreEvent::StatusMessage(format!(
                    "Set block type to {block_type}"
                )));
                events.push(CoreEvent::StateChanged);
            }
        } else if let Some(start_index) = self.view.selection_start {
            let start = start_index.min(self.view.cursor_index);
            let end = start_index.max(self.view.cursor_index);
            let len = end - start + 1;

            if needs_even && len % 2 != 0 {
                events.push(CoreEvent::StatusMessage(format!(
                    "Error: {block_type} requires even number of bytes"
                )));
                return;
            }

            let target_address = if let Some(line) = self.state.disassembly.get(end) {
                line.address
                    .wrapping_add(line.bytes.len() as u16)
                    .wrapping_sub(1)
            } else {
                crate::state::Addr::ZERO
            };

            let cmd1 = self
                .state
                .set_block_type_region(block_type, Some(start), end);
            self.view.selection_start = None;
            self.view.is_visual_mode = false;

            if let Some(idx) = self.state.get_line_index_containing_address(target_address) {
                self.view.cursor_index = idx;
            }

            let (cmd2, _) = self.state.perform_analysis();
            if let Some(cmd) = cmd1 {
                self.state
                    .push_command(crate::commands::Command::Batch(vec![cmd, cmd2]));
            } else {
                self.state.push_command(cmd2);
            }

            events.push(CoreEvent::StatusMessage(format!(
                "Set block type to {block_type}"
            )));
            events.push(CoreEvent::StateChanged);
            events.push(CoreEvent::ViewChanged);
        } else {
            // Single line
            if needs_even {
                events.push(CoreEvent::StatusMessage(format!(
                    "Error: {block_type} requires even number of bytes"
                )));
                return;
            }

            let current_addr = self
                .state
                .disassembly
                .get(self.view.cursor_index)
                .map(|l| l.address);

            let cmd1 = self.state.set_block_type_region(
                block_type,
                None, // selection_start is None
                self.view.cursor_index,
            );

            let (cmd2, _) = self.state.perform_analysis();
            if let Some(cmd) = cmd1 {
                self.state
                    .push_command(crate::commands::Command::Batch(vec![cmd, cmd2]));
            } else {
                self.state.push_command(cmd2);
            }

            events.push(CoreEvent::StatusMessage(format!(
                "Set block type to {block_type}"
            )));

            if let Some(addr) = current_addr
                && let Some(idx) = self.state.get_line_index_containing_address(addr)
            {
                self.view.cursor_index = idx;
            }
            events.push(CoreEvent::StateChanged);
            events.push(CoreEvent::ViewChanged);
        }
    }

    fn handle_add_scope(&mut self, events: &mut Vec<CoreEvent>) {
        if self.view.active_pane == crate::view_state::ActivePane::Blocks {
            events.push(CoreEvent::StatusMessage(
                "Adding Scope only supported in Disassembly view".to_string(),
            ));
            return;
        }

        let process_scope = |state: &mut crate::state::AppState,
                             start_addr: crate::state::Addr,
                             end_addr: crate::state::Addr|
         -> crate::commands::Command {
            let mut commands = Vec::new();

            // Generate a default label for the scope if one does not exist
            let has_label = state.labels.get(&start_addr).is_some_and(|l| !l.is_empty());
            if !has_label {
                let label = crate::state::Label {
                    name: format!("scope_{:04X}", start_addr.0),
                    kind: crate::state::LabelKind::User,
                    label_type: crate::state::LabelType::UserDefined,
                };
                commands.push(crate::commands::Command::SetLabel {
                    address: start_addr,
                    new_label: Some(vec![label]),
                    old_label: None,
                });
            }

            let old_end = state.scopes.get(&start_addr).copied();
            commands.push(crate::commands::Command::AddScope {
                start: start_addr,
                end: end_addr,
                old_end,
            });

            if commands.len() == 1 {
                commands.remove(0)
            } else {
                crate::commands::Command::Batch(commands)
            }
        };

        if let Some(start_index) = self.view.selection_start {
            let start = start_index.min(self.view.cursor_index);
            let end = start_index.max(self.view.cursor_index);

            let start_line = self.state.disassembly.get(start);
            let end_line = self.state.disassembly.get(end);

            if let (Some(sl), Some(el)) = (start_line, end_line) {
                let start_addr = sl.address;
                let end_addr = el
                    .address
                    .wrapping_add(el.bytes.len() as u16)
                    .wrapping_sub(1);

                let mut overlaps = false;
                for (&s, &e) in &self.state.scopes {
                    if start_addr <= e && end_addr >= s {
                        overlaps = true;
                        break;
                    }
                }

                if overlaps {
                    events.push(CoreEvent::StatusMessage(
                        "Cannot create scope: overlaps with an existing scope".to_string(),
                    ));
                    self.view.selection_start = None;
                    self.view.is_visual_mode = false;
                    events.push(CoreEvent::ViewChanged);
                    return;
                }

                let command = process_scope(&mut self.state, start_addr, end_addr);
                command.apply(&mut self.state);

                self.view.selection_start = None;
                self.view.is_visual_mode = false;

                let (analysis_cmd, msg) = self.state.perform_analysis();
                self.state
                    .push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));

                events.push(CoreEvent::StatusMessage(format!(
                    "Added Scope from ${:04X} to ${:04X}. {}",
                    start_addr.0, end_addr.0, msg
                )));
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
        } else if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
            let start_addr = line.address;
            let end_addr = crate::analyzer::guess_scope_end(&self.state, start_addr);

            let mut overlaps = false;
            for (&s, &e) in &self.state.scopes {
                if start_addr <= e && end_addr >= s {
                    overlaps = true;
                    break;
                }
            }

            if overlaps {
                events.push(CoreEvent::StatusMessage(
                    "Cannot create scope: overlaps with an existing scope".to_string(),
                ));
                return;
            }

            let command = process_scope(&mut self.state, start_addr, end_addr);
            command.apply(&mut self.state);

            let (analysis_cmd, msg) = self.state.perform_analysis();
            self.state
                .push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));

            events.push(CoreEvent::StatusMessage(format!(
                "Added Scope from ${:04X} to ${:04X}. {}",
                start_addr.0, end_addr.0, msg
            )));
            events.push(CoreEvent::StateChanged);
            events.push(CoreEvent::ViewChanged);
        } else {
            events.push(CoreEvent::StatusMessage(
                "Could not determine starting address for Scope".to_string(),
            ));
        }
    }

    fn handle_nudge_scope_boundary(&mut self, expand: bool, events: &mut Vec<CoreEvent>) {
        if self.view.active_pane != crate::view_state::ActivePane::Disassembly {
            return;
        }

        let current_addr = if let Some(l) = self.state.disassembly.get(self.view.cursor_index) {
            l.address
        } else {
            return;
        };

        let mut target_scope = None;
        for (&start, &end) in &self.state.scopes {
            if current_addr >= start && current_addr <= end {
                target_scope = Some((start, end));
                break;
            }
        }

        if let Some((start, end)) = target_scope {
            let mut new_end = end;
            let end_idx_opt = self.state.disassembly.iter().position(|l| l.address == end);

            if let Some(end_idx) = end_idx_opt {
                if expand {
                    if end_idx + 1 < self.state.disassembly.len() {
                        let next_line = &self.state.disassembly[end_idx + 1];
                        let bytes = next_line.bytes.len() as u16;
                        if bytes > 0 {
                            new_end = next_line.address.wrapping_add(bytes).wrapping_sub(1);
                        } else {
                            new_end = next_line.address;
                        }
                    }
                } else if end_idx > 0 {
                    let prev_line = &self.state.disassembly[end_idx - 1];
                    if prev_line.address >= start {
                        let bytes = prev_line.bytes.len() as u16;
                        if bytes > 0 {
                            new_end = prev_line.address.wrapping_add(bytes).wrapping_sub(1);
                        } else {
                            new_end = prev_line.address;
                        }
                    }
                }
            }

            let mut overlaps = false;
            for (&s, &e) in &self.state.scopes {
                if s != start && start <= e && new_end >= s {
                    overlaps = true;
                    break;
                }
            }

            if overlaps {
                events.push(CoreEvent::StatusMessage(
                    "Cannot nudge scope: overlaps with another scope".to_string(),
                ));
            } else if new_end != end {
                let command = crate::commands::Command::AddScope {
                    start,
                    end: new_end,
                    old_end: Some(end),
                };
                command.apply(&mut self.state);

                let (analysis_cmd, msg) = self.state.perform_analysis();
                self.state
                    .push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));

                events.push(CoreEvent::StatusMessage(format!(
                    "Resized scope bounds to ${:04X}. {}",
                    new_end.0, msg
                )));
                self.state.disassemble();
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
        } else {
            events.push(CoreEvent::StatusMessage("Not inside a scope".to_string()));
        }
    }

    fn handle_remove_scope(&mut self, events: &mut Vec<CoreEvent>) {
        let current_addr = if let Some(l) = self.state.disassembly.get(self.view.cursor_index) {
            l.address
        } else {
            return;
        };

        let mut scope_to_remove = None;
        for (&start, &end) in &self.state.scopes {
            if current_addr >= start && current_addr <= end {
                scope_to_remove = Some(start);
                break;
            }
        }

        // If the cursor is on the `.bend` or `.pend` line, its address might be immediately after the scope ends.
        // We can find the scope by checking the previous disassembly line, which must be inside the scope.
        if scope_to_remove.is_none()
            && self.view.cursor_index > 0
            && let Some(prev_line) = self.state.disassembly.get(self.view.cursor_index - 1)
        {
            let prev_addr = prev_line.address;
            for (&start, &end) in &self.state.scopes {
                if prev_addr >= start && prev_addr <= end {
                    scope_to_remove = Some(start);
                    break;
                }
            }
        }

        if let Some(start) = scope_to_remove {
            if let Some(old_end) = self.state.scopes.get(&start).copied() {
                let command = crate::commands::Command::RemoveScope {
                    address: start,
                    old_end,
                };
                command.apply(&mut self.state);

                let (analysis_cmd, msg) = self.state.perform_analysis();
                self.state
                    .push_command(crate::commands::Command::Batch(vec![command, analysis_cmd]));

                events.push(CoreEvent::StatusMessage(format!("Removed scope. {}", msg)));
                self.state.disassemble();
                events.push(CoreEvent::StateChanged);
                events.push(CoreEvent::ViewChanged);
            }
        } else {
            events.push(CoreEvent::StatusMessage("Not inside a scope".to_string()));
        }
    }

    fn handle_navigate_to_address(
        &mut self,
        target_addr: crate::state::Addr,
        events: &mut Vec<CoreEvent>,
    ) {
        use crate::view_state::ActivePane;

        match self.view.active_pane {
            ActivePane::Disassembly => {
                crate::navigation::perform_jump_to_address(
                    &self.state,
                    &mut self.view,
                    target_addr,
                );
            }
            ActivePane::HexDump => {
                let origin = self.state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let end_addr = origin + self.state.raw_data.len();

                if target >= origin && target < end_addr {
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let offset = target - aligned_origin;
                    let row = offset / 16;
                    self.view.hex_cursor_index = row;
                    events.push(CoreEvent::StatusMessage(format!(
                        "Jumped to ${target_addr:04X}"
                    )));
                } else {
                    events.push(CoreEvent::StatusMessage(format!(
                        "Address ${target_addr:04X} out of range"
                    )));
                }
            }
            ActivePane::Sprites => {
                let origin = self.state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let padding = (64 - (origin % 64)) % 64;
                let aligned_start = origin + padding;
                let end_addr = origin + self.state.raw_data.len();

                if target >= aligned_start && target < end_addr {
                    let offset = target - aligned_start;
                    let sprite_idx = offset / 64;
                    self.view.sprites_cursor_index = sprite_idx;
                    events.push(CoreEvent::StatusMessage(format!(
                        "Jumped to sprite at ${target_addr:04X}"
                    )));
                } else {
                    events.push(CoreEvent::StatusMessage(format!(
                        "Address ${target_addr:04X} out of range or unaligned"
                    )));
                }
            }
            ActivePane::Charset => {
                let origin = self.state.origin.0 as usize;
                let target = target_addr.0 as usize;
                let base_alignment = 0x400;
                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                let end_addr = origin + self.state.raw_data.len();

                if target >= aligned_start_addr && target < end_addr {
                    let offset = target - aligned_start_addr;
                    let char_idx = offset / 8;
                    self.view.charset_cursor_index = char_idx;
                    events.push(CoreEvent::StatusMessage(format!(
                        "Jumped to char at ${target_addr:04X}"
                    )));
                } else {
                    events.push(CoreEvent::StatusMessage(format!(
                        "Address ${target_addr:04X} out of range"
                    )));
                }
            }
            ActivePane::Blocks => {
                events.push(CoreEvent::StatusMessage(
                    "Jump to address not supported in Blocks view".to_string(),
                ));
            }
            ActivePane::Bitmap => {
                events.push(CoreEvent::StatusMessage(
                    "Jump to address not supported in Bitmap view".to_string(),
                ));
            }
            ActivePane::Debugger => {
                events.push(CoreEvent::StatusMessage(
                    "Jump to address not supported in Debugger view".to_string(),
                ));
            }
        }
        events.push(CoreEvent::ViewChanged);
    }
    pub fn create_save_context(&self) -> crate::state::ProjectSaveContext {
        crate::navigation::create_save_context(&self.state, &self.view)
    }

    pub fn perform_search(&mut self, forward: bool, events: &mut Vec<CoreEvent>) {
        let query = self.view.last_search_query.clone();
        if query.is_empty() {
            events.push(CoreEvent::StatusMessage("No search query".to_string()));
            return;
        }

        let query_lower = query.to_lowercase();
        let disassembly_len = self.state.disassembly.len();
        if disassembly_len == 0 {
            return;
        }

        let start_idx = self.view.cursor_index;
        let mut found_idx = None;
        let mut found_sub_idx = 0;

        use crate::state::search;

        let hex_pattern = if self.view.search_filters.hex_bytes {
            search::parse_hex_pattern(&query)
        } else {
            None
        };
        let filters = &self.view.search_filters;

        // Check current line first for subsequent matches
        if let Some(line) = self.state.disassembly.get(start_idx) {
            let matches = search::get_line_matches(
                line,
                &self.state,
                &query_lower,
                hex_pattern.as_deref(),
                filters,
            );

            let candidate = if forward {
                matches
                    .into_iter()
                    .find(|&sub| sub > self.view.sub_cursor_index)
            } else {
                matches
                    .into_iter()
                    .rev()
                    .find(|&sub| sub < self.view.sub_cursor_index)
            };

            if let Some(sub) = candidate {
                self.view.navigation_history.push((
                    crate::view_state::ActivePane::Disassembly,
                    crate::view_state::NavigationTarget::Index(self.view.cursor_index),
                ));
                self.view.sub_cursor_index = sub;
                events.push(CoreEvent::StatusMessage(format!("Found '{query}'")));
                events.push(CoreEvent::ViewChanged);
                return;
            }
        }

        // Iterate other lines
        for i in 1..disassembly_len {
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

            if let Some(line) = self.state.disassembly.get(idx) {
                let matches = search::get_line_matches(
                    line,
                    &self.state,
                    &query_lower,
                    hex_pattern.as_deref(),
                    filters,
                );
                if !matches.is_empty() {
                    found_idx = Some(idx);
                    found_sub_idx = if forward {
                        matches[0]
                    } else {
                        matches[matches.len() - 1]
                    };
                    break;
                }

                // Check collapsed content
                let pc = line.address.offset_from(self.state.origin);
                if self
                    .state
                    .collapsed_blocks
                    .iter()
                    .find(|(s, _)| *s == pc)
                    .copied()
                    .is_some_and(|(start, end)| {
                        search::search_collapsed_content(
                            &self.state,
                            start,
                            end,
                            &query_lower,
                            hex_pattern.as_deref(),
                            filters,
                        )
                    })
                {
                    found_idx = Some(idx);
                    found_sub_idx = 0;
                    break;
                }
            }
        }

        if let Some(idx) = found_idx {
            self.view.navigation_history.push((
                crate::view_state::ActivePane::Disassembly,
                crate::view_state::NavigationTarget::Index(self.view.cursor_index),
            ));
            self.view.cursor_index = idx;
            self.view.sub_cursor_index = found_sub_idx;
            events.push(CoreEvent::StatusMessage(format!("Found '{query}'")));
            events.push(CoreEvent::ViewChanged);
        } else {
            events.push(CoreEvent::StatusMessage(format!("'{query}' not found")));
        }
    }

    fn handle_apply_label(&mut self, address: Addr, name: String, events: &mut Vec<CoreEvent>) {
        let label_name = name.trim().to_string();

        let old_label_vec = self.state.labels.get(&address).cloned();

        if label_name.is_empty() {
            let command = crate::commands::Command::SetLabel {
                address,
                new_label: None,
                old_label: old_label_vec,
            };
            command.apply(&mut self.state);
            self.state.push_command(command);
            events.push(CoreEvent::StatusMessage("Label removed".to_string()));
        } else {
            let exists = self.state.labels.iter().any(|(addr, label_vec)| {
                *addr != address && label_vec.iter().any(|l| l.name == label_name)
            });

            if exists {
                events.push(CoreEvent::StatusMessage(format!(
                    "Error: Label '{label_name}' already exists"
                )));
                return;
            }

            let mut new_label_vec = old_label_vec.clone().unwrap_or_default();
            let new_label_entry = crate::state::Label {
                name: label_name,
                kind: crate::state::LabelKind::User,
                label_type: crate::state::LabelType::UserDefined,
            };

            if new_label_vec.is_empty() {
                new_label_vec.push(new_label_entry);
            } else {
                new_label_vec[0] = new_label_entry;
            }

            let command = crate::commands::Command::SetLabel {
                address,
                new_label: Some(new_label_vec),
                old_label: old_label_vec,
            };
            command.apply(&mut self.state);
            self.state.push_command(command);
            events.push(CoreEvent::StatusMessage("Label set".to_string()));
        }

        // Trigger re-disassembly as it might have changed labels in the view
        self.state.disassemble();
        events.push(CoreEvent::StateChanged);
        events.push(CoreEvent::ViewChanged);
        events.push(CoreEvent::DialogDismissalRequested);
    }

    fn handle_apply_comment(
        &mut self,
        address: Addr,
        text: String,
        kind: crate::state::types::CommentKind,
        events: &mut Vec<CoreEvent>,
    ) {
        let new_text = text.trim().to_string();
        let new_comment_opt = if new_text.is_empty() {
            None
        } else {
            Some(new_text)
        };

        let command = match kind {
            crate::state::types::CommentKind::Side => {
                let old_comment = self.state.user_side_comments.get(&address).cloned();
                crate::commands::Command::SetUserSideComment {
                    address,
                    new_comment: new_comment_opt,
                    old_comment,
                }
            }
            crate::state::types::CommentKind::Line => {
                let old_comment = self.state.user_line_comments.get(&address).cloned();
                crate::commands::Command::SetUserLineComment {
                    address,
                    new_comment: new_comment_opt,
                    old_comment,
                }
            }
        };

        command.apply(&mut self.state);
        self.state.push_command(command);

        events.push(CoreEvent::StatusMessage("Comment set".to_string()));
        self.state.disassemble();
        events.push(CoreEvent::StateChanged);
        events.push(CoreEvent::ViewChanged);
        events.push(CoreEvent::DialogDismissalRequested);
    }
    pub fn get_default_filename_stem(&self) -> Option<String> {
        let path = self
            .state
            .project_path
            .as_ref()
            .or(self.state.file_path.as_ref())?;
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(std::string::ToString::to_string)
    }

    fn handle_set_label(&mut self, events: &mut Vec<CoreEvent>) {
        if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
            let mut target_addr = line.address;
            let mut current_sub_index = 0;
            let mut found = false;

            if line.bytes.is_empty() {
                if let Some(addr) = line.external_label_address {
                    target_addr = addr;
                } else {
                    return;
                }
            } else if line.bytes.len() > 1 {
                for offset in 1..line.bytes.len() {
                    let mid_addr = line.address.wrapping_add(offset as u16);
                    if let Some(labels) = self.state.labels.get(&mid_addr) {
                        for _ in labels {
                            if current_sub_index == self.view.sub_cursor_index {
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

            let initial_name = self
                .state
                .labels
                .get(&target_addr)
                .and_then(|v| v.first())
                .map(|l| l.name.clone())
                .unwrap_or_default();

            events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Label {
                    address: target_addr,
                    initial_name,
                    is_external: self.state.is_external(target_addr),
                },
            ));
            events.push(CoreEvent::StatusMessage("Enter Label".to_string()));
        }
    }

    fn handle_lo_hi_packing(&mut self, lo_first: bool, events: &mut Vec<CoreEvent>) {
        let mut indices = Vec::new();
        if let Some(start) = self.view.selection_start {
            let end = self.view.cursor_index;
            let (low, high) = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            for i in low..=high {
                indices.push(i);
            }
        } else {
            indices.push(self.view.cursor_index);
        }

        if indices.len() == 1 {
            let idx = indices[0];
            if idx + 1 < self.state.disassembly.len() {
                indices.push(idx + 1);
            }
        }

        let mut batch_commands = Vec::new();
        let mut i = 0;
        let mut last_target = 0;

        while i < indices.len() {
            let idx1 = indices[i];
            let val1 = self.state.disassembly.get(idx1).and_then(|l| {
                if let Some(op) = &l.opcode
                    && op.mode == crate::cpu::AddressingMode::Immediate
                    && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
                {
                    l.bytes.get(1).copied()
                } else {
                    None
                }
            });

            if let Some(v1) = val1 {
                let mut j = i + 1;
                let mut found_match = None;
                while j < indices.len() {
                    let idx2 = indices[j];
                    let val2 = self.state.disassembly.get(idx2).and_then(|l| {
                        if let Some(op) = &l.opcode
                            && op.mode == crate::cpu::AddressingMode::Immediate
                            && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
                        {
                            l.bytes.get(1).copied()
                        } else {
                            None
                        }
                    });
                    if let Some(v2) = val2 {
                        found_match = Some((j, idx2, v2));
                        break;
                    }
                    j += 1;
                }

                if let Some((match_idx, idx2, v2)) = found_match {
                    let (lo, hi) = if lo_first { (v1, v2) } else { (v2, v1) };
                    let target = (u16::from(hi) << 8) | u16::from(lo);
                    last_target = target;

                    let addr1 = self.state.disassembly[idx1].address;
                    let addr2 = self.state.disassembly[idx2].address;

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

                    batch_commands.push(crate::commands::Command::SetImmediateFormat {
                        address: addr1,
                        new_format: Some(fmt1),
                        old_format: self.state.immediate_value_formats.get(&addr1).copied(),
                    });
                    batch_commands.push(crate::commands::Command::SetImmediateFormat {
                        address: addr2,
                        new_format: Some(fmt2),
                        old_format: self.state.immediate_value_formats.get(&addr2).copied(),
                    });

                    i = match_idx + 1;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        if !batch_commands.is_empty() {
            let batch = crate::commands::Command::Batch(batch_commands);
            batch.apply(&mut self.state);
            self.state.push_command(batch);

            let result = crate::analyzer::analyze(&self.state);
            self.state.labels = result.labels;
            self.state.cross_refs = result.cross_refs;
            self.state.disassemble();

            self.view.selection_start = None;
            self.view.is_visual_mode = false;

            events.push(CoreEvent::StatusMessage(format!(
                "Packed Lo/Hi address for ${last_target:04X}"
            )));
            events.push(CoreEvent::StateChanged);
            events.push(CoreEvent::ViewChanged);
        } else if self.view.selection_start.is_none() {
            let idx = self.view.cursor_index;
            // No pairs found, but if this is a single selection with an immediate instruction,
            // show dialog to complete the address
            if let Some(line) = self.state.disassembly.get(idx)
                && let Some(op) = &line.opcode
                && op.mode == crate::cpu::AddressingMode::Immediate
                && matches!(op.mnemonic, "LDA" | "LDX" | "LDY")
                && let Some(known_byte) = line.bytes.get(1).copied()
            {
                events.push(CoreEvent::DialogRequested(
                    crate::event::DialogType::CompleteAddress {
                        known_byte,
                        lo_first,
                        address: line.address,
                    },
                ));
            } else {
                events.push(CoreEvent::StatusMessage("No Lo/Hi pairs found".to_string()));
            }
        } else {
            events.push(CoreEvent::StatusMessage("No Lo/Hi pairs found".to_string()));
        }
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Addr;
    use crate::view_state::ActivePane;

    #[test]
    fn test_handle_add_scope_default_label() {
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let code_data = vec![0xEA, 0xEA, 0xEA, 0xEA]; // A few NOPs

        // Use state.load_binary which handles setup
        core.state.load_binary(origin, code_data).unwrap();

        core.view.active_pane = ActivePane::Disassembly;
        core.view.cursor_index = 0; // Pointing to first instruction

        core.apply_action(AppAction::Scope);

        // Origin doesn't have a label by default, so it should generate "scope_1000"
        let label = core
            .state
            .labels
            .get(&origin)
            .expect("Expected label at start of scope");
        let first_label = label.first().expect("Expected at least one label");

        assert_eq!(first_label.name, format!("scope_{:04X}", origin.0));
        assert_eq!(first_label.kind, crate::state::LabelKind::User);
        assert_eq!(first_label.label_type, crate::state::LabelType::UserDefined);
    }

    #[test]
    fn test_handle_add_scope_overlapping() {
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let code_data = vec![0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA];

        core.state.load_binary(origin, code_data).unwrap();

        core.state.scopes.insert(Addr(0x1001), Addr(0x1002));

        core.view.active_pane = ActivePane::Disassembly;
        core.view.is_visual_mode = true;

        core.view.selection_start = Some(0);
        core.view.cursor_index = 2;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 1);
        assert_eq!(core.state.scopes.get(&Addr(0x1001)), Some(&Addr(0x1002)));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );

        core.view.is_visual_mode = true;
        core.view.selection_start = Some(2);
        core.view.cursor_index = 4;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 1);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );

        core.view.is_visual_mode = true;
        core.view.selection_start = Some(3);
        core.view.cursor_index = 4;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 2);
        assert_eq!(core.state.scopes.get(&Addr(0x1003)), Some(&Addr(0x1004)));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );
    }
}
