use crate::state::actions::AppAction;
use crate::ui_state::ActivePane;

#[derive(Default)]
pub struct MenuState {
    pub active: bool,
    pub categories: Vec<MenuCategory>,
    pub selected_category: usize,
    pub selected_item: Option<usize>,
}

impl MenuState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            categories: vec![
                MenuCategory {
                    name: "File".to_string(),
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
                        MenuItem::new("Export to .lst", None, Some(AppAction::ExportLst)),
                        MenuItem::new("Export to .lst as...", None, Some(AppAction::ExportLstAs)),
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
                        MenuItem::new("Settings", Some("Alt+P"), Some(AppAction::SystemSettings)),
                        MenuItem::separator(),
                        MenuItem::new("Exit", Some("Ctrl+Q"), Some(AppAction::Exit)),
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
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
                        MenuItem::new("External File", Some("E"), Some(AppAction::SetExternalFile)),
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
                            "Toggle Bookmark",
                            Some("Ctrl+B"),
                            Some(AppAction::ToggleBookmark),
                        ),
                        MenuItem::new(
                            "List Bookmarks...",
                            Some("Ctrl+Shift+B"),
                            Some(AppAction::ListBookmarks),
                        ),
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
                    ],
                },
                MenuCategory {
                    name: "Search".to_string(),
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
                            "Toggle Hex Dump",
                            Some("Alt+2"),
                            Some(AppAction::ToggleHexDump),
                        ),
                        MenuItem::new(
                            "Toggle Sprites View",
                            Some("Alt+3"),
                            Some(AppAction::ToggleSpritesView),
                        ),
                        MenuItem::new(
                            "Toggle Charset View",
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
                    items: vec![
                        MenuItem::new(
                            "Keyboard Shortcuts",
                            None,
                            Some(AppAction::KeyboardShortcuts),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("About", None, Some(AppAction::About)),
                    ],
                },
            ],
            selected_category: 0,
            selected_item: None,
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
        let current = self.selected_item.unwrap_or(0);
        let mut next = (current + 1) % count;

        // Skip separators and disabled items
        // We iterate at most `count` times to avoid infinite loop
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
                            AppAction::SetLabel
                            | AppAction::FindReferences
                            | AppAction::ToggleBookmark => {
                                item.disabled = active_pane != ActivePane::Disassembly;
                            }
                            AppAction::ViceConnect => {
                                item.disabled = app_state.vice_client.is_some();
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
    pub items: Vec<MenuItem>,
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
