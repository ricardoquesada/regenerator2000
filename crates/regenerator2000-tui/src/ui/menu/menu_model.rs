//! Top-level menu bar state, hierarchy definition, and context-sensitive item availability.
//!
//! [`MenuState::new`] defines Layer 2 of the 4-layer keyboard shortcut architecture.
//! [`MenuState::update_availability`] enables or disables menu items for the active pane,
//! document state, and cursor mode.

use crate::state::actions::AppAction;
use crate::ui_state::ActivePane;

/// State of the top-level application menu bar (Layer 2 of the keyboard shortcut architecture).
#[derive(Default)]
pub struct MenuState {
    pub active: bool,
    pub categories: Vec<MenuCategory>,
    pub selected_category: usize,
    pub selected_item: Option<usize>,
}

impl MenuState {
    /// Constructs the canonical menu bar hierarchy with shortcut labels and [`AppAction`] bindings.
    ///
    /// The Pillar 3 contract test checks every shortcut against `ShortcutsDialog::get_shortcuts`
    /// and `docs/keyboard_shortcuts.md`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            categories: vec![
                MenuCategory {
                    name: "File".to_string(),
                    accelerator: 'f',
                    items: vec![
                        MenuItem::new("Open...", Some("Ctrl+O"), Some(AppAction::Open)),
                        MenuItem::new(
                            "Open Recents...",
                            Some("Alt+O"),
                            Some(AppAction::OpenRecent),
                        ),
                        MenuItem::new("Save", Some("Ctrl+S"), Some(AppAction::Save)),
                        MenuItem::new("Save As...", Some("Alt+S"), Some(AppAction::SaveAs)),
                        MenuItem::separator(),
                        MenuItem::new("Export to .asm", Some("Ctrl+E"), Some(AppAction::ExportAsm)),
                        MenuItem::new(
                            "Export to .asm as...",
                            Some("Alt+E"),
                            Some(AppAction::ExportAsmAs),
                        ),
                        MenuItem::new("Export to .html", None, Some(AppAction::ExportHtml)),
                        MenuItem::new("Export to .html as...", None, Some(AppAction::ExportHtmlAs)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Import VICE Labels...",
                            None,
                            Some(AppAction::ImportViceLabels),
                        ),
                        MenuItem::new(
                            "Export VICE Labels...",
                            None,
                            Some(AppAction::ExportViceLabels),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Unpack Binary...", None, Some(AppAction::UnpackDialog)),
                        MenuItem::new("File Info...", None, Some(AppAction::FileInfo)),
                        MenuItem::separator(),
                        MenuItem::new("Start MCP Server", None, Some(AppAction::StartMcpServer)),
                        MenuItem::new("Stop MCP Server", None, Some(AppAction::StopMcpServer)),
                        MenuItem::separator(),
                        MenuItem::new("Settings", Some("Alt+P"), Some(AppAction::SystemSettings)),
                        MenuItem::separator(),
                        MenuItem::new("Exit", Some("Ctrl+Q"), Some(AppAction::Exit)),
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
                    accelerator: 't',
                    items: vec![
                        MenuItem::new("Undo", Some("U"), Some(AppAction::Undo)),
                        MenuItem::new("Redo", Some("Ctrl+R"), Some(AppAction::Redo)),
                        MenuItem::separator(),
                        MenuItem::new("Scope", Some("R"), Some(AppAction::Scope)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Disassemble Address",
                            Some("D"),
                            Some(AppAction::DisassembleAddress),
                        ),
                        MenuItem::new("Code", Some("C"), Some(AppAction::Code)),
                        MenuItem::new("Byte", Some("B"), Some(AppAction::Byte)),
                        MenuItem::new("Word", Some("W"), Some(AppAction::Word)),
                        MenuItem::new("Address", Some("A"), Some(AppAction::Address)),
                        MenuItem::new("Lo/Hi Word Table", Some(","), Some(AppAction::SetLoHiWord)),
                        MenuItem::new("Hi/Lo Word Table", Some("."), Some(AppAction::SetHiLoWord)),
                        MenuItem::new(
                            "Lo/Hi Address Table",
                            Some("<"),
                            Some(AppAction::SetLoHiAddress),
                        ),
                        MenuItem::new(
                            "Hi/Lo Address Table",
                            Some(">"),
                            Some(AppAction::SetHiLoAddress),
                        ),
                        MenuItem::new(
                            "Pack Lo/Hi Address",
                            Some("["),
                            Some(AppAction::PackLoHiAddress),
                        ),
                        MenuItem::new(
                            "Pack Hi/Lo Address",
                            Some("]"),
                            Some(AppAction::PackHiLoAddress),
                        ),
                        MenuItem::new("PETSCII Text", Some("P"), Some(AppAction::PetsciiText)),
                        MenuItem::new(
                            "Screencode Text",
                            Some("S"),
                            Some(AppAction::ScreencodeText),
                        ),
                        MenuItem::new(
                            "External File",
                            Some("Shift+E"),
                            Some(AppAction::SetExternalFile),
                        ),
                        MenuItem::new("Undefined", Some("?"), Some(AppAction::Undefined)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Next Imm. Mode Format",
                            Some("i"),
                            Some(AppAction::NextImmediateFormat),
                        ),
                        MenuItem::new(
                            "Prev Imm. Mode Format",
                            Some("Shift+I"),
                            Some(AppAction::PreviousImmediateFormat),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Side Comment", Some(";"), Some(AppAction::SideComment)),
                        MenuItem::new("Line Comment", Some(":"), Some(AppAction::LineComment)),
                        MenuItem::new("Set Label", Some("L"), Some(AppAction::SetLabel)),
                        MenuItem::new(
                            "Symbolic Offset...",
                            Some("O"),
                            Some(AppAction::SymbolicOffsetDialog),
                        ),
                        MenuItem::new(
                            "Increment Symbolic Offset",
                            Some("+"),
                            Some(AppAction::NudgeSymbolicOffset { delta: 1 }),
                        ),
                        MenuItem::new(
                            "Decrement Symbolic Offset",
                            Some("-"),
                            Some(AppAction::NudgeSymbolicOffset { delta: -1 }),
                        ),
                        MenuItem::new(
                            "Toggle Bookmark",
                            Some("Ctrl+B"),
                            Some(AppAction::ToggleBookmark),
                        ),
                        MenuItem::new(
                            "List Bookmarks...",
                            Some("Ctrl+Shift+B"),
                            Some(AppAction::ListBookmarks),
                        ),
                        MenuItem::new("Apply Enum...", Some("E"), Some(AppAction::ApplyEnumDialog)),
                        MenuItem::new("Manage Enums...", None, Some(AppAction::ManageEnums)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Splitter",
                            Some("|"),
                            Some(AppAction::ToggleSplitter),
                        ),
                        MenuItem::new(
                            "Toggle Collapsed Block",
                            Some("Ctrl+K"),
                            Some(AppAction::ToggleCollapsedBlock),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Change Origin", None, Some(AppAction::ChangeOrigin)),
                        MenuItem::separator(),
                        MenuItem::new("Analyze", Some("Ctrl+A"), Some(AppAction::Analyze)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Document Settings",
                            Some("Alt+D"),
                            Some(AppAction::DocumentSettings),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Jump".to_string(),
                    accelerator: 'j',
                    items: vec![
                        MenuItem::new(
                            "Jump to address...",
                            Some("Alt+G"),
                            Some(AppAction::JumpToAddress),
                        ),
                        MenuItem::new(
                            "Jump to line...",
                            Some("Alt+Shift+G"),
                            Some(AppAction::JumpToLine),
                        ),
                        MenuItem::new(
                            "Jump to operand",
                            Some("Enter"),
                            Some(AppAction::JumpToOperand),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Next Unexplored Block",
                            Some("Ctrl+N"),
                            Some(AppAction::JumpNextUnexplored),
                        ),
                        MenuItem::new(
                            "Prev Unexplored Block",
                            Some("Ctrl+Shift+N"),
                            Some(AppAction::JumpPrevUnexplored),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Search".to_string(),
                    accelerator: 'r',
                    items: vec![
                        MenuItem::new("Search...", Some("Ctrl+F"), Some(AppAction::Search)),
                        MenuItem::new("Find Next", Some("F3"), Some(AppAction::FindNext)),
                        MenuItem::new(
                            "Find Previous",
                            Some("Shift+F3"),
                            Some(AppAction::FindPrevious),
                        ),
                        MenuItem::new(
                            "Go to symbol...",
                            Some("Ctrl+P"),
                            Some(AppAction::GoToSymbol),
                        ),
                        MenuItem::new(
                            "Find References...",
                            Some("Ctrl+x"),
                            Some(AppAction::FindReferences),
                        ),
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),
                    accelerator: 'v',
                    items: vec![
                        MenuItem::new(
                            "Next Hex Dump Mode",
                            Some("M"),
                            Some(AppAction::HexdumpViewModeNext),
                        ),
                        MenuItem::new(
                            "Prev Hex Dump Mode",
                            Some("Shift+M"),
                            Some(AppAction::HexdumpViewModePrev),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Sprites",
                            Some("M"),
                            Some(AppAction::ToggleSpriteMulticolor),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Bitmap",
                            Some("M"),
                            Some(AppAction::ToggleBitmapMulticolor),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Charset",
                            Some("M"),
                            Some(AppAction::ToggleCharsetMulticolor),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Blocks View",
                            Some("Alt+1"),
                            Some(AppAction::ToggleBlocksView),
                        ),
                        MenuItem::new(
                            "Cycle Hex Dump (16/8/Off)",
                            Some("Alt+2"),
                            Some(AppAction::ToggleHexDump),
                        ),
                        MenuItem::new(
                            "Cycle Sprites (2-Col/1-Col/Off)",
                            Some("Alt+3"),
                            Some(AppAction::ToggleSpritesView),
                        ),
                        MenuItem::new(
                            "Cycle Charset (8-Col/4-Col/Off)",
                            Some("Alt+4"),
                            Some(AppAction::ToggleCharsetView),
                        ),
                        MenuItem::new(
                            "Toggle Bitmap View",
                            Some("Alt+5"),
                            Some(AppAction::ToggleBitmapView),
                        ),
                        MenuItem::new(
                            "Toggle Debugger View",
                            Some("Alt+6"),
                            Some(AppAction::ToggleDebuggerView),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Debugger".to_string(),
                    accelerator: 'u',
                    items: vec![
                        MenuItem::new("Connect to VICE...", None, Some(AppAction::ViceConnect)),
                        MenuItem::new(
                            "Disconnect from VICE",
                            None,
                            Some(AppAction::ViceDisconnect),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Step Instruction", Some("F7"), Some(AppAction::ViceStep)),
                        MenuItem::new("Step Over", Some("F8"), Some(AppAction::ViceStepOver)),
                        MenuItem::new("Step Out", Some("Shift+F8"), Some(AppAction::ViceStepOut)),
                        MenuItem::new("Continue", Some("F9"), Some(AppAction::ViceContinue)),
                        MenuItem::new(
                            "Run to Cursor",
                            Some("F4"),
                            Some(AppAction::ViceRunToCursor),
                        ),
                        MenuItem::new(
                            "Toggle Breakpoint",
                            Some("F2"),
                            Some(AppAction::ViceToggleBreakpoint),
                        ),
                        MenuItem::new(
                            "Toggle Breakpoint...",
                            Some("Shift+F2"),
                            Some(AppAction::ViceBreakpointDialog),
                        ),
                        MenuItem::new(
                            "Watchpoint...",
                            Some("F6"),
                            Some(AppAction::ViceToggleWatchpoint),
                        ),
                        MenuItem::new(
                            "Memory Dump...",
                            Some("M"),
                            Some(AppAction::ViceMemoryDumpDialog),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Help".to_string(),
                    accelerator: 'h',
                    items: vec![
                        MenuItem::new(
                            "Keyboard Shortcuts",
                            None,
                            Some(AppAction::KeyboardShortcuts),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Documentation", None, Some(AppAction::OpenDocumentation)),
                        MenuItem::new("Examples", None, Some(AppAction::OpenExamples)),
                        MenuItem::separator(),
                        MenuItem::new("About", None, Some(AppAction::About)),
                    ],
                },
            ],
            selected_category: 0,
            selected_item: None,
        }
    }

    /// Selects the category matching `accelerator` (case-insensitive) and highlights its first
    /// enabled item. Returns `true` when a category matches.
    pub fn select_category_by_accelerator(&mut self, accelerator: char) -> bool {
        if let Some(pos) = self
            .categories
            .iter()
            .position(|c| c.accelerator.eq_ignore_ascii_case(&accelerator))
        {
            self.selected_category = pos;
            self.select_first_enabled_item();
            true
        } else {
            false
        }
    }

    pub fn next_category(&mut self) {
        self.selected_category = (self.selected_category + 1) % self.categories.len();
        // If we are active, select the first non-separator item
        if self.active {
            self.select_first_enabled_item();
        }
    }

    pub fn previous_category(&mut self) {
        if self.selected_category == 0 {
            self.selected_category = self.categories.len() - 1;
        } else {
            self.selected_category -= 1;
        }
        if self.active {
            self.select_first_enabled_item();
        }
    }

    pub fn next_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if count == 0 {
            return;
        }
        let mut next = match self.selected_item {
            Some(current) => (current + 1) % count,
            None => 0,
        };

        // Skip separators and disabled items.
        // Iterate at most `count` times to avoid an infinite loop.
        for _ in 0..count {
            let item = &self.categories[self.selected_category].items[next];
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(next);
                return;
            }
            next = (next + 1) % count;
        }
    }

    pub fn previous_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if count == 0 {
            return;
        }
        let current = self.selected_item.unwrap_or(0);

        let mut prev = if current == 0 { count - 1 } else { current - 1 };

        // We iterate at most `count` times to avoid infinite loop
        for _ in 0..count {
            let item = &self.categories[self.selected_category].items[prev];
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(prev);
                return;
            }
            prev = if prev == 0 { count - 1 } else { prev - 1 };
        }
    }

    pub fn select_first_enabled_item(&mut self) {
        let items = &self.categories[self.selected_category].items;
        for (i, item) in items.iter().enumerate() {
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(i);
                return;
            }
        }
        self.selected_item = None;
    }

    /// Computes the screen rectangle for the active category popup menu, clamped to `term_area`.
    #[must_use]
    pub fn popup_rect(
        &self,
        bar_area: ratatui::layout::Rect,
        term_area: ratatui::layout::Rect,
    ) -> ratatui::layout::Rect {
        if self.categories.is_empty() || self.selected_category >= self.categories.len() {
            return ratatui::layout::Rect::default();
        }
        let mut x_offset = 0u16;
        for category in &self.categories[..self.selected_category] {
            x_offset = x_offset.saturating_add(category.bar_width());
        }

        let category = &self.categories[self.selected_category];
        let width = category.popup_width();
        let height = category.items.len() as u16 + 2;

        let area = ratatui::layout::Rect::new(
            bar_area.x.saturating_add(x_offset),
            bar_area.y.saturating_add(1),
            width,
            height,
        );
        area.intersection(term_area)
    }

    /// Computes the vertical list scroll offset for a popup with `inner_height` visible item rows.
    #[must_use]
    pub fn popup_scroll_offset(&self, inner_height: usize) -> usize {
        if inner_height == 0 || self.selected_category >= self.categories.len() {
            return 0;
        }
        let total_items = self.categories[self.selected_category].items.len();
        if total_items <= inner_height {
            return 0;
        }
        let Some(selected) = self.selected_item else {
            return 0;
        };
        if selected < inner_height {
            0
        } else {
            (selected + 1 - inner_height).min(total_items.saturating_sub(inner_height))
        }
    }
    pub fn update_availability(
        &mut self,
        app_state: &crate::state::AppState,
        cursor_index: usize,
        last_search_empty: bool,
        active_pane: ActivePane,
    ) {
        let has_document = !app_state.raw_data.is_empty();
        for category in &mut self.categories {
            for item in &mut category.items {
                if let Some(action) = &item.action {
                    if action.requires_document() && !has_document {
                        item.disabled = true;
                    } else {
                        // Context-specific checks
                        match action {
                            AppAction::FindNext | AppAction::FindPrevious => {
                                item.disabled = last_search_empty;
                            }
                            AppAction::PackLoHiAddress | AppAction::PackHiLoAddress => {
                                let mut is_valid = false;
                                if has_document
                                    && let Some(line) = app_state.disassembly.get(cursor_index)
                                    && let Some(opcode) = &line.opcode
                                    && opcode.mode == crate::cpu::AddressingMode::Immediate
                                    && (opcode.mnemonic == "LDA"
                                        || opcode.mnemonic == "LDX"
                                        || opcode.mnemonic == "LDY")
                                {
                                    is_valid = true;
                                }
                                item.disabled = !is_valid;
                            }
                            AppAction::NextImmediateFormat | AppAction::PreviousImmediateFormat => {
                                let mut is_immediate = false;
                                if has_document
                                    && let Some(line) = app_state.disassembly.get(cursor_index)
                                    && let Some(opcode) = &line.opcode
                                    && opcode.mode == crate::cpu::AddressingMode::Immediate
                                {
                                    is_immediate = true;
                                }
                                item.disabled = !is_immediate;
                            }
                            AppAction::HexdumpViewModeNext | AppAction::HexdumpViewModePrev => {
                                item.disabled = active_pane != ActivePane::HexDump;
                            }
                            AppAction::ToggleSpriteMulticolor => {
                                item.disabled = active_pane != ActivePane::Sprites;
                            }
                            AppAction::ToggleCharsetMulticolor => {
                                item.disabled = active_pane != ActivePane::Charset;
                            }
                            AppAction::ToggleBitmapMulticolor => {
                                item.disabled = active_pane != ActivePane::Bitmap;
                            }
                            // Enable label, symbolic-offset, reference, bookmark, and enum actions
                            // only when Disassembly holds focus.
                            AppAction::SetLabel
                            | AppAction::SymbolicOffsetDialog
                            | AppAction::NudgeSymbolicOffset { .. }
                            | AppAction::FindReferences
                            | AppAction::ToggleBookmark
                            | AppAction::ApplyEnumDialog => {
                                item.disabled = active_pane != ActivePane::Disassembly;
                            }
                            AppAction::ViceConnect => {
                                item.disabled = app_state.vice_client.is_some();
                            }
                            AppAction::StartMcpServer => {
                                item.disabled = app_state.mcp_server_running;
                            }
                            AppAction::StopMcpServer => {
                                item.disabled = !app_state.mcp_server_running;
                            }
                            AppAction::ViceDisconnect
                            | AppAction::ViceToggleBreakpoint
                            | AppAction::ViceBreakpointDialog
                            | AppAction::ViceToggleWatchpoint
                            | AppAction::ViceMemoryDumpDialog => {
                                item.disabled = app_state.vice_client.is_none();
                            }
                            AppAction::ViceStep
                            | AppAction::ViceStepOver
                            | AppAction::ViceStepOut
                            | AppAction::ViceContinue
                            | AppAction::ViceRunToCursor => {
                                item.disabled =
                                    app_state.vice_client.is_none() || app_state.vice_state.running;
                            }
                            _ => item.disabled = false,
                        }
                    }
                }
            }
        }
    }
}

pub struct MenuCategory {
    pub name: String,
    pub accelerator: char,
    pub items: Vec<MenuItem>,
}

impl MenuCategory {
    /// Returns the rendered cell width of this category on the top menu bar (`" <name> "`).
    #[must_use]
    pub fn bar_width(&self) -> u16 {
        self.name.len() as u16 + 2
    }

    /// Returns `(max_name_len, max_shortcut_len)` across all items in this category.
    #[must_use]
    pub fn column_widths(&self) -> (usize, usize) {
        let mut max_name_len = 0usize;
        let mut max_shortcut_len = 0usize;
        for item in &self.items {
            max_name_len = max_name_len.max(item.name.len());
            max_shortcut_len =
                max_shortcut_len.max(item.shortcut.as_ref().map_or(0, std::string::String::len));
        }
        (max_name_len, max_shortcut_len)
    }

    /// Returns the outer popup width (including 1-cell left/right borders), with a minimum of 20.
    #[must_use]
    pub fn popup_width(&self) -> u16 {
        let (max_name_len, max_shortcut_len) = self.column_widths();
        let content_width = max_name_len + 2 + max_shortcut_len;
        (content_width as u16 + 2).max(20)
    }
}

#[derive(Clone)]
pub struct MenuItem {
    pub name: String,
    pub shortcut: Option<String>,
    pub is_separator: bool,
    pub action: Option<AppAction>,
    pub disabled: bool,
}

impl MenuItem {
    #[must_use]
    pub fn new(name: &str, shortcut: Option<&str>, action: Option<AppAction>) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.map(std::string::ToString::to_string),
            is_separator: false,
            action,
            disabled: false,
        }
    }

    #[must_use]
    pub fn separator() -> Self {
        Self {
            name: String::new(),
            shortcut: None,
            is_separator: true,
            action: None,
            disabled: false,
        }
    }
}
