use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Disassembly,
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Exit,
    New,
    Open,
    Save,
    SaveAs,
    ExportAsm,
    ExportAsmAs,
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
    ZoomIn,
    ZoomOut,
    ResetZoom,
    JumpToAddress,
    JumpToOperand,
    About,
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

pub struct JumpDialogState {
    pub active: bool,
    pub input: String,
}

impl JumpDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
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
    ExportAsm,
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

pub struct SettingsDialogState {
    pub active: bool,
    pub selected_index: usize,
    pub is_selecting_platform: bool,
    pub is_selecting_assembler: bool,
    pub is_editing_xref_count: bool,
    pub xref_count_input: String,
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
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.selected_index = 0;
        self.is_selecting_platform = false;
        self.is_selecting_assembler = false;
        self.is_editing_xref_count = false;
        self.xref_count_input.clear();
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
        let max_items = 7;
        self.selected_index = (self.selected_index + 1) % max_items;
    }

    pub fn previous(&mut self) {
        let max_items = 7;
        if self.selected_index == 0 {
            self.selected_index = max_items - 1;
        } else {
            self.selected_index -= 1;
        }
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
                        MenuItem::new("New", Some("Ctrl+N"), Some(MenuAction::New)),
                        MenuItem::new("Open", Some("Ctrl+O"), Some(MenuAction::Open)),
                        MenuItem::new("Save", Some("Ctrl+S"), Some(MenuAction::Save)),
                        MenuItem::new("Save As...", Some("Ctrl+Shift+S"), Some(MenuAction::SaveAs)),
                        MenuItem::separator(),
                        MenuItem::new("Export ASM", Some("Ctrl+E"), Some(MenuAction::ExportAsm)),
                        MenuItem::new(
                            "Export ASM As...",
                            Some("Ctrl+Shift+E"),
                            Some(MenuAction::ExportAsmAs),
                        ),
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
                        MenuItem::new("Text", Some("T"), Some(MenuAction::Text)),
                        MenuItem::new("Screencode", Some("S"), Some(MenuAction::Screencode)),
                        MenuItem::separator(),
                        MenuItem::new("Analyze", None, Some(MenuAction::Analyze)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Document Settings",
                            Some("Ctrl+P"),
                            Some(MenuAction::DocumentSettings),
                        ),
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),
                    items: vec![
                        MenuItem::new("Zoom In", Some("Ctrl++"), Some(MenuAction::ZoomIn)),
                        MenuItem::new("Zoom Out", Some("Ctrl+-"), Some(MenuAction::ZoomOut)),
                        MenuItem::new("Reset Zoom", Some("Ctrl+0"), Some(MenuAction::ResetZoom)),
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
                            "Jump to operand",
                            Some("Enter"),
                            Some(MenuAction::JumpToOperand),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Help".to_string(),
                    items: vec![MenuItem::new("About", None, Some(MenuAction::About))],
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
            self.selected_item = Some(0);
        }
    }

    pub fn previous_category(&mut self) {
        if self.selected_category == 0 {
            self.selected_category = self.categories.len() - 1;
        } else {
            self.selected_category -= 1;
        }
        if self.active {
            self.selected_item = Some(0);
        }
    }

    pub fn next_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        let current = self.selected_item.unwrap_or(0);
        let mut next = (current + 1) % count;

        // Skip separators
        while self.categories[self.selected_category].items[next].is_separator {
            next = (next + 1) % count;
            if next == current {
                break;
            } // Avoid infinite loop if all separators (unlikely)
        }

        self.selected_item = Some(next);
    }

    pub fn previous_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        let current = self.selected_item.unwrap_or(0);

        let mut prev = if current == 0 { count - 1 } else { current - 1 };

        // Skip separators
        while self.categories[self.selected_category].items[prev].is_separator {
            prev = if prev == 0 { count - 1 } else { prev - 1 };
            if prev == current {
                break;
            }
        }

        self.selected_item = Some(prev);
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
}

impl MenuItem {
    pub fn new(name: &str, shortcut: Option<&str>, action: Option<MenuAction>) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            is_separator: false,
            action,
        }
    }

    pub fn separator() -> Self {
        Self {
            name: String::new(),
            shortcut: None,
            is_separator: true,
            action: None,
        }
    }
}

pub struct UIState {
    pub file_picker: FilePickerState,
    pub jump_dialog: JumpDialogState,
    pub save_dialog: SaveDialogState,
    pub label_dialog: LabelDialogState,
    pub settings_dialog: SettingsDialogState,
    pub about_dialog: AboutDialogState,
    pub menu: MenuState,

    pub navigation_history: Vec<usize>,
    #[allow(dead_code)]
    pub disassembly_state: ListState,

    // UI Selection/Cursor
    pub selection_start: Option<usize>,
    pub cursor_index: usize,
    #[allow(dead_code)]
    pub scroll_index: usize,

    // Hex View State
    pub hex_cursor_index: usize,
    #[allow(dead_code)]
    pub hex_scroll_index: usize,

    pub active_pane: ActivePane,
    pub should_quit: bool,
    pub status_message: String,

    pub logo: Option<DynamicImage>,
    pub picker: Option<Picker>,
    pub dismiss_logo: bool,
}

impl UIState {
    pub fn new() -> Self {
        Self {
            file_picker: FilePickerState::new(),
            jump_dialog: JumpDialogState::new(),
            save_dialog: SaveDialogState::new(),
            label_dialog: LabelDialogState::new(),
            settings_dialog: SettingsDialogState::new(),
            about_dialog: AboutDialogState::new(),
            menu: MenuState::new(),
            navigation_history: Vec::new(),
            disassembly_state: ListState::default(),
            selection_start: None,
            cursor_index: 0,
            scroll_index: 0,
            hex_cursor_index: 0,
            hex_scroll_index: 0,
            active_pane: ActivePane::Disassembly,
            should_quit: false,
            status_message: "Ready".to_string(),
            logo: crate::utils::load_logo(),
            picker: crate::utils::create_picker(),
            dismiss_logo: false,
        }
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }
}
