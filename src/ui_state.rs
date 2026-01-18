use crate::theme::Theme;
use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Disassembly,
    HexDump,
    Sprites,

    Charset,
    Blocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPane {
    None,
    #[default]
    HexDump,
    Sprites,

    Charset,
    Blocks,
}

use crate::state::PetsciiMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Exit,
    Open,
    Save,
    SaveAs,
    ExportProject,
    ExportProjectAs,
    Undo,
    Redo,
    Code,
    Byte,
    Word,
    Address,
    Text,
    Screencode,
    Analyze,
    DocumentSettings,
    JumpToAddress,
    JumpToLine,
    JumpToOperand,

    SetLoHi,
    SetHiLo,
    SetExternalFile,
    SideComment,
    LineComment,
    ToggleHexDump,
    ToggleSpritesView,
    About,
    ChangeOrigin,
    KeyboardShortcuts,
    Undefined,
    SystemSettings,
    NextImmediateFormat,
    PreviousImmediateFormat,
    Search,
    FindNext,
    FindPrevious,
    TogglePetsciiMode,
    ToggleSpriteMulticolor,
    ToggleCharsetView,
    ToggleCharsetMulticolor,

    ToggleBlocksView,
    ToggleCollapsedBlock,
    ToggleSplitter,
}

impl MenuAction {
    pub fn requires_document(&self) -> bool {
        !matches!(
            self,
            MenuAction::Exit
                | MenuAction::Open
                | MenuAction::About
                | MenuAction::KeyboardShortcuts
                | MenuAction::SystemSettings
                | MenuAction::Search
        )
    }
}

#[derive(Default)]

pub struct MenuState {
    pub active: bool,
    pub categories: Vec<MenuCategory>,
    pub selected_category: usize,
    pub selected_item: Option<usize>,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            active: false,
            categories: vec![
                MenuCategory {
                    name: "File".to_string(),
                    items: vec![
                        MenuItem::new("Open", Some("Ctrl+O"), Some(MenuAction::Open)),
                        MenuItem::new("Save", Some("Ctrl+S"), Some(MenuAction::Save)),
                        MenuItem::new("Save As...", Some("Ctrl+Shift+S"), Some(MenuAction::SaveAs)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Export Project",
                            Some("Ctrl+E"),
                            Some(MenuAction::ExportProject),
                        ),
                        MenuItem::new(
                            "Export Project As...",
                            Some("Ctrl+Shift+E"),
                            Some(MenuAction::ExportProjectAs),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Settings", Some("Ctrl+,"), Some(MenuAction::SystemSettings)),
                        MenuItem::separator(),
                        MenuItem::new("Exit", Some("Ctrl+Q"), Some(MenuAction::Exit)),
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
                    items: vec![
                        MenuItem::new("Undo", Some("U"), Some(MenuAction::Undo)),
                        MenuItem::new("Redo", Some("Ctrl+R"), Some(MenuAction::Redo)),
                        MenuItem::separator(),
                        MenuItem::new("Code", Some("C"), Some(MenuAction::Code)),
                        MenuItem::new("Byte", Some("B"), Some(MenuAction::Byte)),
                        MenuItem::new("Word", Some("W"), Some(MenuAction::Word)),
                        MenuItem::new("Address", Some("A"), Some(MenuAction::Address)),
                        MenuItem::new("Lo/Hi Address", Some("<"), Some(MenuAction::SetLoHi)),
                        MenuItem::new("Hi/Lo Address", Some(">"), Some(MenuAction::SetHiLo)),
                        MenuItem::new(
                            "External File",
                            Some("e"),
                            Some(MenuAction::SetExternalFile),
                        ),
                        MenuItem::new("Text", Some("T"), Some(MenuAction::Text)),
                        MenuItem::new("Screencode", Some("S"), Some(MenuAction::Screencode)),
                        MenuItem::new("Undefined", Some("?"), Some(MenuAction::Undefined)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Next Imm. Mode Format",
                            Some("d"),
                            Some(MenuAction::NextImmediateFormat),
                        ),
                        MenuItem::new(
                            "Prev Imm. Mode Format",
                            Some("Shift+D"),
                            Some(MenuAction::PreviousImmediateFormat),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Splitter",
                            Some("|"),
                            Some(MenuAction::ToggleSplitter),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Side Comment", Some(";"), Some(MenuAction::SideComment)),
                        MenuItem::new(
                            "Line Comment",
                            Some("Shift+;"),
                            Some(MenuAction::LineComment),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Collapsed Block",
                            Some("Ctrl+K"),
                            Some(MenuAction::ToggleCollapsedBlock),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Change Origin", None, Some(MenuAction::ChangeOrigin)),
                        MenuItem::separator(),
                        MenuItem::new("Analyze", Some("Ctrl+A"), Some(MenuAction::Analyze)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Document Settings",
                            Some("Ctrl+Shift+D"),
                            Some(MenuAction::DocumentSettings),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Jump".to_string(),
                    items: vec![
                        MenuItem::new(
                            "Jump to address",
                            Some("G"),
                            Some(MenuAction::JumpToAddress),
                        ),
                        MenuItem::new(
                            "Jump to line",
                            Some("Ctrl+Shift+G"),
                            Some(MenuAction::JumpToLine),
                        ),
                        MenuItem::new(
                            "Jump to operand",
                            Some("Enter"),
                            Some(MenuAction::JumpToOperand),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Search".to_string(),
                    items: vec![
                        MenuItem::new("Search...", Some("Ctrl+F"), Some(MenuAction::Search)),
                        MenuItem::new("Find Next", Some("F3"), Some(MenuAction::FindNext)),
                        MenuItem::new(
                            "Find Previous",
                            Some("Shift+F3"),
                            Some(MenuAction::FindPrevious),
                        ),
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),

                    items: vec![
                        MenuItem::new(
                            "Toggle PETSCII Shifted/Unshifted",
                            Some("m"),
                            Some(MenuAction::TogglePetsciiMode),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Sprites",
                            Some("m"),
                            Some(MenuAction::ToggleSpriteMulticolor),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Charset",
                            Some("m"),
                            Some(MenuAction::ToggleCharsetMulticolor),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Hex Dump",
                            Some("Ctrl+2"),
                            Some(MenuAction::ToggleHexDump),
                        ),
                        MenuItem::new(
                            "Toggle Sprites View",
                            Some("Ctrl+3"),
                            Some(MenuAction::ToggleSpritesView),
                        ),
                        MenuItem::new(
                            "Toggle Charset View",
                            Some("Ctrl+4"),
                            Some(MenuAction::ToggleCharsetView),
                        ),
                        MenuItem::new(
                            "Toggle Blocks View",
                            Some("Ctrl+5"),
                            Some(MenuAction::ToggleBlocksView),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Help".to_string(),
                    items: vec![
                        MenuItem::new(
                            "Keyboard Shortcuts",
                            None,
                            Some(MenuAction::KeyboardShortcuts),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("About", None, Some(MenuAction::About)),
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
                            MenuAction::FindNext | MenuAction::FindPrevious => {
                                item.disabled = last_search_empty;
                            }
                            MenuAction::NextImmediateFormat
                            | MenuAction::PreviousImmediateFormat => {
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
                            MenuAction::TogglePetsciiMode => {
                                item.disabled = active_pane != ActivePane::HexDump;
                            }
                            MenuAction::ToggleSpriteMulticolor => {
                                item.disabled = active_pane != ActivePane::Sprites;
                            }
                            MenuAction::ToggleCharsetMulticolor => {
                                item.disabled = active_pane != ActivePane::Charset;
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
    pub action: Option<MenuAction>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(name: &str, shortcut: Option<&str>, action: Option<MenuAction>) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            is_separator: false,
            action,
            disabled: false,
        }
    }

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

pub struct UIState {
    pub open_dialog: crate::dialog_open::OpenDialog,
    pub jump_to_address_dialog: crate::dialog_jump_to_address::JumpToAddressDialog,
    pub jump_to_line_dialog: crate::dialog_jump_to_line::JumpToLineDialog,
    pub save_as_dialog: crate::dialog_save_as::SaveAsDialog,
    pub export_as_dialog: crate::dialog_export_as::ExportAsDialog,
    pub label_dialog: crate::dialog_label::LabelDialogState,
    pub comment_dialog: crate::dialog_comment::CommentDialogState,
    pub settings_dialog: crate::dialog_document_settings::DocumentSettingsDialog,
    pub about_dialog: crate::dialog_about::AboutDialog,
    pub shortcuts_dialog: crate::dialog_keyboard_shortcut::ShortcutsDialog,
    pub origin_dialog: crate::dialog_origin::OriginDialogState,
    pub confirmation_dialog: crate::dialog_confirmation::ConfirmationDialogState,
    pub system_settings_dialog: crate::dialog_settings::SettingsDialog,
    pub search_dialog: crate::dialog_search::SearchDialog,
    pub menu: MenuState,

    pub navigation_history: Vec<(ActivePane, usize)>,
    #[allow(dead_code)]
    pub disassembly_state: ListState,

    // UI Selection/Cursor
    pub selection_start: Option<usize>,
    pub cursor_index: usize,
    pub sub_cursor_index: usize,
    #[allow(dead_code)]
    pub scroll_index: usize,

    // Hex View State
    pub hex_cursor_index: usize,
    pub sprites_cursor_index: usize,
    pub charset_cursor_index: usize,

    pub blocks_list_state: ListState,
    #[allow(dead_code)]
    pub hex_scroll_index: usize,
    pub right_pane: RightPane,
    pub sprite_multicolor_mode: bool,
    pub charset_multicolor_mode: bool,
    pub petscii_mode: PetsciiMode,

    pub active_pane: ActivePane,
    pub should_quit: bool,
    pub status_message: String,

    pub logo: Option<DynamicImage>,
    pub picker: Option<Picker>,
    pub dismiss_logo: bool,
    pub is_visual_mode: bool,
    pub input_buffer: String,

    pub theme: Theme,

    // Vim-like search
    pub vim_search_active: bool,
    pub vim_search_input: String,
}

impl UIState {
    pub fn new(theme: Theme) -> Self {
        Self {
            open_dialog: crate::dialog_open::OpenDialog::new(),
            jump_to_address_dialog: crate::dialog_jump_to_address::JumpToAddressDialog::new(),
            jump_to_line_dialog: crate::dialog_jump_to_line::JumpToLineDialog::new(),
            save_as_dialog: crate::dialog_save_as::SaveAsDialog::new(),
            export_as_dialog: crate::dialog_export_as::ExportAsDialog::new(),
            label_dialog: crate::dialog_label::LabelDialogState::new(),
            comment_dialog: crate::dialog_comment::CommentDialogState::new(),
            settings_dialog: crate::dialog_document_settings::DocumentSettingsDialog::new(),
            about_dialog: crate::dialog_about::AboutDialog::new(),
            shortcuts_dialog: crate::dialog_keyboard_shortcut::ShortcutsDialog::new(),
            origin_dialog: crate::dialog_origin::OriginDialogState::new(),
            confirmation_dialog: crate::dialog_confirmation::ConfirmationDialogState::new(),
            system_settings_dialog: crate::dialog_settings::SettingsDialog::new(),
            search_dialog: crate::dialog_search::SearchDialog::new(),
            menu: MenuState::new(),
            navigation_history: Vec::new(),
            disassembly_state: ListState::default(),
            selection_start: None,
            cursor_index: 0,
            sub_cursor_index: 0,
            scroll_index: 0,
            hex_cursor_index: 0,
            sprites_cursor_index: 0,

            charset_cursor_index: 0,
            blocks_list_state: ListState::default(),
            hex_scroll_index: 0,

            right_pane: RightPane::HexDump,
            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            petscii_mode: PetsciiMode::Unshifted,
            active_pane: ActivePane::Disassembly,
            should_quit: false,
            status_message: "Ready".to_string(),
            logo: crate::utils::load_logo(),
            picker: crate::utils::create_picker(),
            dismiss_logo: false,
            is_visual_mode: false,
            input_buffer: String::new(),
            vim_search_active: false,
            vim_search_input: String::new(),
            theme,
        }
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    pub fn restore_session(
        &mut self,
        loaded_data: &crate::state::LoadedProjectData,
        app_state: &crate::state::AppState,
    ) {
        let loaded_cursor = loaded_data.cursor_address;
        let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
        let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
        let loaded_right_pane = &loaded_data.right_pane_visible;
        let loaded_charset_cursor = loaded_data.charset_cursor_address;

        self.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
        self.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
        self.petscii_mode = loaded_data.petscii_mode;
        let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
        if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
            self.cursor_index = idx;
        }

        // Also restore hex cursor if present
        if let Some(hex_addr) = loaded_hex_cursor
            && !app_state.raw_data.is_empty()
        {
            let origin = app_state.origin as usize;
            let alignment_padding = origin % 16;
            let aligned_origin = origin - alignment_padding;
            let target = hex_addr as usize;

            if target >= aligned_origin {
                let offset = target - aligned_origin;
                let row = offset / 16;
                // Ensure row is within bounds
                let total_len = app_state.raw_data.len() + alignment_padding;
                let max_rows = total_len.div_ceil(16);
                if row < max_rows {
                    self.hex_cursor_index = row;
                }
            }
        }

        // Restore Right Pane and Sprites Cursor
        if let Some(pane_str) = loaded_right_pane {
            match pane_str.as_str() {
                "HexDump" => self.right_pane = RightPane::HexDump,
                "Sprites" => self.right_pane = RightPane::Sprites,
                "Charset" => self.right_pane = RightPane::Charset,
                "Blocks" => self.right_pane = RightPane::Blocks,
                _ => {}
            }
        }
        if let Some(idx) = loaded_data.blocks_view_cursor {
            self.blocks_list_state.select(Some(idx));
        }
        if let Some(sprites_addr) = loaded_sprites_cursor {
            let origin = app_state.origin as usize;
            let padding = (64 - (origin % 64)) % 64;
            let addr = sprites_addr as usize;
            if addr >= origin + padding {
                let offset = addr - (origin + padding);
                self.sprites_cursor_index = offset / 64;
            }
        }
        if let Some(charset_addr) = loaded_charset_cursor {
            let origin = app_state.origin as usize;
            let base_alignment = 0x400;
            let aligned_start_addr = (origin / base_alignment) * base_alignment;
            let addr = charset_addr as usize;
            if addr >= aligned_start_addr {
                let offset = addr - aligned_start_addr;
                self.charset_cursor_index = offset / 8;
            }
        }
    }
}
