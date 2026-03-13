use crate::event::CoreEvent;
use crate::state::AppState;
use crate::state::actions::AppAction;
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

        match action {
            AppAction::Exit => {
                events.push(CoreEvent::QuitRequested);
            }
            AppAction::Analyze => {
                let msg = self.state.perform_analysis();
                events.push(CoreEvent::StatusMessage(msg));
                events.push(CoreEvent::StateChanged);
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
            AppAction::Code => self.apply_block_type(crate::state::BlockType::Code, &mut events),
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
            AppAction::SetLabel => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let mut target_addr = line.address;
                    let mut current_sub_index = 0;
                    let mut found = false;

                    if line.bytes.is_empty() {
                        if let Some(addr) = line.external_label_address {
                            target_addr = addr;
                        } else {
                            // Header or empty line -> ignore
                            return events;
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
            AppAction::SideComment => {
                if let Some(line) = self.state.disassembly.get(self.view.cursor_index) {
                    let address = line.address;
                    let current = self.state.user_side_comments.get(&address).cloned();
                    events.push(CoreEvent::DialogRequested(
                        crate::event::DialogType::Comment {
                            address,
                            current,
                            kind: crate::event::CommentKind::Side,
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
                            kind: crate::event::CommentKind::Line,
                        },
                    ));
                    events.push(CoreEvent::StatusMessage(format!(
                        "Edit Line Comment at ${address:04X}"
                    )));
                }
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
                if let Some(_path) = &self.state.export_path {
                    // Note: Exporter might need to be in core too
                    // For now, if we can't do it here, we'll emit an event for the frontend to do it.
                    events.push(CoreEvent::StatusMessage("Exporting...".to_string()));
                    // Let's assume for now the frontend handles actual export if we emit a certain event
                    // or we move exporter to core.
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
            // Add more as needed...
            _ => {}
        }

        events
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
                self.state.set_block_type_region(
                    block_type,
                    Some(start.offset_from(self.state.origin)),
                    end.offset_from(self.state.origin),
                );
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

            self.state
                .set_block_type_region(block_type, Some(start), end);
            self.view.selection_start = None;
            self.view.is_visual_mode = false;

            if let Some(idx) = self.state.get_line_index_containing_address(target_address) {
                self.view.cursor_index = idx;
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

            self.state.set_block_type_region(
                block_type,
                None, // selection_start is None
                self.view.cursor_index,
            );
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
                    events.push(CoreEvent::StatusMessage("Address out of range".to_string()));
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
                    events.push(CoreEvent::StatusMessage(
                        "Address out of range or unaligned".to_string(),
                    ));
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
                    events.push(CoreEvent::StatusMessage("Address out of range".to_string()));
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
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}
