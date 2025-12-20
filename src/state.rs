use crate::disassembler::{Disassembler, DisassemblyLine};
use std::path::PathBuf;

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
            filter_extensions: vec!["bin".to_string(), "prg".to_string(), "raw".to_string()],
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

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub disassembler: Disassembler,
    pub origin: u16,

    pub file_picker: FilePickerState,

    pub menu: MenuState,

    // UI State
    pub cursor_index: usize,
    pub scroll_index: usize,
    pub should_quit: bool,
    pub status_message: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            disassembler: Disassembler::new(),
            origin: 0,
            file_picker: FilePickerState::new(),
            menu: MenuState::new(),
            cursor_index: 0,
            scroll_index: 0,
            should_quit: false,
            status_message: "Ready".to_string(),
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let data = std::fs::read(&path)?;
        self.file_path = Some(path);

        // Simple heuristic for .prg: first 2 bytes are load address
        if let Some(ext) = self
            .file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
        {
            if ext.eq_ignore_ascii_case("prg") && data.len() >= 2 {
                self.origin = (data[1] as u16) << 8 | (data[0] as u16);
                self.raw_data = data[2..].to_vec();
            } else {
                self.origin = 0; // Default for .bin, or user can change later
                self.raw_data = data;
            }
        } else {
            self.origin = 0;
            self.raw_data = data;
        }

        self.disassemble();
        Ok(())
    }

    pub fn disassemble(&mut self) {
        self.disassembly = self.disassembler.disassemble(&self.raw_data, self.origin);
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
                        MenuItem {
                            name: "New".to_string(),
                            shortcut: Some("Ctrl+N".to_string()),
                        },
                        MenuItem {
                            name: "Open".to_string(),
                            shortcut: Some("Ctrl+O".to_string()),
                        },
                        MenuItem {
                            name: "Save".to_string(),
                            shortcut: Some("Ctrl+S".to_string()),
                        },
                        MenuItem {
                            name: "Save As".to_string(),
                            shortcut: Some("Ctrl+Shift+S".to_string()),
                        },
                        MenuItem {
                            name: "Exit".to_string(),
                            shortcut: Some("Ctrl+Q".to_string()),
                        },
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
                    items: vec![
                        MenuItem {
                            name: "Undo".to_string(),
                            shortcut: Some("Ctrl+Z".to_string()),
                        },
                        MenuItem {
                            name: "Redo".to_string(),
                            shortcut: Some("Ctrl+Shift+Z".to_string()),
                        },
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),
                    items: vec![
                        MenuItem {
                            name: "Zoom In".to_string(),
                            shortcut: Some("Ctrl++".to_string()),
                        },
                        MenuItem {
                            name: "Zoom Out".to_string(),
                            shortcut: Some("Ctrl+-".to_string()),
                        },
                        MenuItem {
                            name: "Reset Zoom".to_string(),
                            shortcut: Some("Ctrl+0".to_string()),
                        },
                    ],
                },
            ],
            selected_category: 0,
            selected_item: None,
        }
    }

    pub fn next_category(&mut self) {
        self.selected_category = (self.selected_category + 1) % self.categories.len();
        self.selected_item = None;
    }

    pub fn previous_category(&mut self) {
        if self.selected_category == 0 {
            self.selected_category = self.categories.len() - 1;
        } else {
            self.selected_category -= 1;
        }
        self.selected_item = None;
    }

    pub fn next_item(&mut self) {
        if let Some(index) = self.selected_item {
            let count = self.categories[self.selected_category].items.len();
            self.selected_item = Some((index + 1) % count);
        } else {
            self.selected_item = Some(0);
        }
    }

    pub fn previous_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if let Some(index) = self.selected_item {
            if index == 0 {
                self.selected_item = Some(count - 1);
            } else {
                self.selected_item = Some(index - 1);
            }
        } else {
            self.selected_item = Some(count - 1);
        }
    }
}

pub struct MenuCategory {
    pub name: String,
    pub items: Vec<MenuItem>,
}

pub struct MenuItem {
    pub name: String,
    pub shortcut: Option<String>,
}
