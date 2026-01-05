use crate::theme::Theme;
use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Disassembly,
    HexDump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetsciiMode {
    Unshifted,
    Shifted,
}

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
    SetPetsciiUnshifted,
    SetPetsciiShifted,
    SetLoHi,
    SetHiLo,
    SideComment,
    LineComment,
    ToggleHexDump,
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

pub struct SearchDialogState {
    pub active: bool,
    pub input: String,
    pub last_search: String,
}

impl SearchDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            last_search: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        // Pre-fill with last search is a nice touch, but optional.
        self.input = self.last_search.clone();
    }

    pub fn close(&mut self) {
        self.active = false;
        // Keep input for next time or save to last_search when executing
    }
}

pub struct ConfirmationDialogState {
    pub active: bool,
    pub title: String,
    pub message: String,
    pub action_on_confirm: Option<MenuAction>,
}

impl ConfirmationDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            title: String::new(),
            message: String::new(),
            action_on_confirm: None,
        }
    }

    pub fn open(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        action: MenuAction,
    ) {
        self.active = true;
        self.title = title.into();
        self.message = message.into();
        self.action_on_confirm = Some(action);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.action_on_confirm = None;
    }
}

pub struct AboutDialogState {
    pub active: bool,
}

impl AboutDialogState {
    pub fn new() -> Self {
        Self { active: false }
    }

    pub fn open(&mut self) {
        self.active = true;
    }

    pub fn close(&mut self) {
        self.active = false;
    }
}

pub struct ShortcutsDialogState {
    pub active: bool,
    pub scroll_offset: usize,
}

impl ShortcutsDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            scroll_offset: 0,
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.scroll_offset = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

pub struct FilePickerState {
    pub active: bool,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub filter_extensions: Vec<String>,
}

impl FilePickerState {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            active: false,
            current_dir,
            files: Vec::new(),
            selected_index: 0,
            filter_extensions: vec![
                "bin".to_string(),
                "prg".to_string(),
                "crt".to_string(),
                "vsf".to_string(),
                "t64".to_string(),
                "raw".to_string(),
                "regen2000proj".to_string(),
            ],
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.refresh_files();
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn refresh_files(&mut self) {
        self.files = crate::utils::list_files(&self.current_dir, &self.filter_extensions);
    }

    pub fn next(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.files.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.files.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.files.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum JumpDialogMode {
    Address,
    Line,
}

pub struct JumpDialogState {
    pub active: bool,
    pub input: String,
    pub mode: JumpDialogMode,
}

impl JumpDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            mode: JumpDialogMode::Address,
        }
    }

    pub fn open(&mut self, mode: JumpDialogMode) {
        self.active = true;
        self.mode = mode;
        self.input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SaveDialogMode {
    Project,
    ExportProject,
}

pub struct SaveDialogState {
    pub active: bool,
    pub input: String,
    pub mode: SaveDialogMode,
}

impl SaveDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            mode: SaveDialogMode::Project,
        }
    }

    pub fn open(&mut self, mode: SaveDialogMode) {
        self.active = true;
        self.input.clear();
        self.mode = mode;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub struct LabelDialogState {
    pub active: bool,
    pub input: String,
}

impl LabelDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self, current_label: Option<&str>) {
        self.active = true;
        self.input = current_label.unwrap_or("").to_string();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentType {
    Side,
    Line,
}

pub struct CommentDialogState {
    pub active: bool,
    pub input: String,
    pub comment_type: CommentType,
}

impl CommentDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            comment_type: CommentType::Side,
        }
    }

    pub fn open(&mut self, current_comment: Option<&str>, comment_type: CommentType) {
        self.active = true;
        self.input = current_comment.unwrap_or("").to_string();
        self.comment_type = comment_type;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub struct OriginDialogState {
    pub active: bool,
    pub input: String,
}

impl OriginDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self, current_origin: u16) {
        self.active = true;
        self.input = format!("{:04X}", current_origin);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub struct SettingsDialogState {
    pub active: bool,
    pub selected_index: usize,
    pub is_selecting_platform: bool,
    pub is_selecting_assembler: bool,
    pub is_editing_xref_count: bool,
    pub xref_count_input: String,
    pub is_editing_arrow_columns: bool,
    pub arrow_columns_input: String,
    pub is_editing_text_char_limit: bool,
    pub text_char_limit_input: String,
}

impl SettingsDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            selected_index: 0,
            is_selecting_platform: false,
            is_selecting_assembler: false,
            is_editing_xref_count: false,
            xref_count_input: String::new(),
            is_editing_arrow_columns: false,
            arrow_columns_input: String::new(),
            is_editing_text_char_limit: false,
            text_char_limit_input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected_index = 0;
        self.is_selecting_platform = false;
        self.is_selecting_assembler = false;
        self.is_editing_xref_count = false;
        self.xref_count_input.clear();
        self.is_editing_arrow_columns = false;
        self.arrow_columns_input.clear();
        self.is_editing_text_char_limit = false;
        self.text_char_limit_input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn next(&mut self) {
        // Items:
        // 0: All Labels
        // 1: Use @w
        // 2: BRK single byte
        // 3: Patch BRK
        // 4: Platform
        // 5: Assembler
        // 6: Max X-Refs
        // 7: Arrow Columns
        // 8: Use Illegal Opcodes
        // 9: Text Line Limit
        let max_items = 10;
        self.selected_index = (self.selected_index + 1) % max_items;
    }

    pub fn previous(&mut self) {
        let max_items = 10;
        if self.selected_index == 0 {
            self.selected_index = max_items - 1;
        } else {
            self.selected_index -= 1;
        }
    }
}

#[derive(Default)]
pub struct SystemSettingsDialogState {
    pub active: bool,
    pub selected_index: usize,
    pub is_selecting_theme: bool,
}

impl SystemSettingsDialogState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected_index = 0;
        self.is_selecting_theme = false; // Reset on open
    }

    pub fn close(&mut self) {
        self.active = false;
    }
}

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
                        MenuItem::new("Settings", None, Some(MenuAction::SystemSettings)),
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
                        MenuItem::new("Text", Some("T"), Some(MenuAction::Text)),
                        MenuItem::new("Screencode", Some("S"), Some(MenuAction::Screencode)),
                        MenuItem::new("Undefined", Some("Shift+U"), Some(MenuAction::Undefined)),
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
                        MenuItem::new("Side Comment", Some(";"), Some(MenuAction::SideComment)),
                        MenuItem::new(
                            "Line Comment",
                            Some("Shift+;"),
                            Some(MenuAction::LineComment),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Change Origin", None, Some(MenuAction::ChangeOrigin)),
                        MenuItem::separator(),
                        MenuItem::new("Analyze", Some("Ctrl+A"), Some(MenuAction::Analyze)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Document Settings",
                            Some("Ctrl+P"),
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
                            "Unshifted PETSCII",
                            Some("Ctrl+Shift+L"),
                            Some(MenuAction::SetPetsciiUnshifted),
                        ),
                        MenuItem::new(
                            "Shifted PETSCII",
                            Some("Ctrl+L"),
                            Some(MenuAction::SetPetsciiShifted),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Hex Dump",
                            Some("Ctrl+2"),
                            Some(MenuAction::ToggleHexDump),
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
        // If nothing found (all disabled/separators), keep as is or set to None?
        // Let's keep as is if we can't find anything better, or maybe current is valid?
        // If current is invalid (e.g. became disabled), we might want to change it.
    }

    pub fn previous_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if count == 0 {
            return;
        }
        let current = self.selected_item.unwrap_or(0);

        let mut prev = if current == 0 { count - 1 } else { current - 1 };

        // Skip separators and disabled items
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
    pub file_picker: FilePickerState,
    pub jump_dialog: JumpDialogState,
    pub save_dialog: SaveDialogState,
    pub label_dialog: LabelDialogState,
    pub comment_dialog: CommentDialogState,
    pub settings_dialog: SettingsDialogState,
    pub about_dialog: AboutDialogState,
    pub shortcuts_dialog: ShortcutsDialogState,
    pub origin_dialog: OriginDialogState,
    pub confirmation_dialog: ConfirmationDialogState,
    pub system_settings_dialog: SystemSettingsDialogState,
    pub search_dialog: SearchDialogState,
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
    #[allow(dead_code)]
    pub hex_scroll_index: usize,
    pub show_hex_dump: bool,
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
            file_picker: FilePickerState::new(),
            jump_dialog: JumpDialogState::new(),
            save_dialog: SaveDialogState::new(),
            label_dialog: LabelDialogState::new(),
            comment_dialog: CommentDialogState::new(),
            settings_dialog: SettingsDialogState::new(),
            about_dialog: AboutDialogState::new(),
            shortcuts_dialog: ShortcutsDialogState::new(),
            origin_dialog: OriginDialogState::new(),
            confirmation_dialog: ConfirmationDialogState::new(),
            system_settings_dialog: SystemSettingsDialogState::new(),
            search_dialog: SearchDialogState::new(),
            menu: MenuState::new(),
            navigation_history: Vec::new(),
            disassembly_state: ListState::default(),
            selection_start: None,
            cursor_index: 0,
            sub_cursor_index: 0,
            scroll_index: 0,
            hex_cursor_index: 0,
            hex_scroll_index: 0,
            show_hex_dump: true,
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
}
