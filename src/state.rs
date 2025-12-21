use crate::disassembler::{Disassembler, DisassemblyLine};
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    Code,
    DataByte,
    DataWord,
    DataPtr, // Simple word pointer for now
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
                "json".to_string(),
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

pub struct SaveDialogState {
    pub active: bool,
    pub input: String,
}

impl SaveDialogState {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressRange {
    pub start: usize,
    pub end: usize,
    pub type_: AddressType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectState {
    pub origin: u16,
    pub raw_data: Vec<String>, // Chunked Hex
    pub address_ranges: Vec<AddressRange>,
    #[serde(default)]
    pub labels: HashMap<u16, String>,
}

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub disassembler: Disassembler,
    pub origin: u16,

    pub file_picker: FilePickerState,
    pub jump_dialog: JumpDialogState,
    pub save_dialog: SaveDialogState,
    pub label_dialog: LabelDialogState,

    pub menu: MenuState,

    pub navigation_history: Vec<usize>,
    pub disassembly_state: ListState,

    // Data Conversion State
    pub address_types: Vec<AddressType>,
    pub labels: HashMap<u16, String>,
    pub selection_start: Option<usize>,

    // UI State
    pub cursor_index: usize,
    #[allow(dead_code)]
    pub scroll_index: usize,
    pub should_quit: bool,
    pub status_message: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            project_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            disassembler: Disassembler::new(),
            origin: 0,
            file_picker: FilePickerState::new(),
            jump_dialog: JumpDialogState::new(),
            save_dialog: SaveDialogState::new(),
            label_dialog: LabelDialogState::new(),
            menu: MenuState::new(),
            navigation_history: Vec::new(),
            disassembly_state: ListState::default(),
            address_types: Vec::new(),
            labels: HashMap::new(),
            selection_start: None,
            cursor_index: 0,
            scroll_index: 0,
            should_quit: false,
            status_message: "Ready".to_string(),
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let data = std::fs::read(&path)?;
        self.file_path = Some(path.clone());

        if let Some(ext) = self
            .file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
        {
            if ext.eq_ignore_ascii_case("json") {
                return self.load_project(path);
            }

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

        self.address_types = vec![AddressType::Code; self.raw_data.len()];

        self.disassemble();
        self.disassemble();
        Ok(())
    }

    pub fn load_project(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let data = std::fs::read_to_string(&path)?;
        let project: ProjectState = serde_json::from_str(&data)?;

        self.project_path = Some(path);
        self.origin = project.origin;

        // Decode raw data
        self.raw_data = decode_raw_data(&project.raw_data)?;

        // Expand address types
        self.address_types = expand_address_ranges(&project.address_ranges, self.raw_data.len());
        self.labels = project.labels;

        self.disassemble();
        Ok(())
    }

    pub fn save_project(&mut self) -> anyhow::Result<()> {
        if let Some(path) = &self.project_path {
            let project = ProjectState {
                origin: self.origin,
                raw_data: encode_raw_data(&self.raw_data),
                address_ranges: compress_address_types(&self.address_types),
                labels: self.labels.clone(),
            };
            let data = serde_json::to_string_pretty(&project)?;
            std::fs::write(path, data)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No project path set"))
        }
    }

    pub fn disassemble(&mut self) {
        self.disassembly = self.disassembler.disassemble(
            &self.raw_data,
            &self.address_types,
            &self.labels,
            self.origin,
        );
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
                        MenuItem::new("New", Some("Ctrl+N")),
                        MenuItem::new("Open", Some("Ctrl+O")),
                        MenuItem::new("Save", Some("Ctrl+S")),
                        MenuItem::new("Save As", Some("Ctrl+Shift+S")),
                        MenuItem::separator(),
                        MenuItem::new("Exit", Some("Ctrl+Q")),
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
                    items: vec![
                        MenuItem::new("Undo", Some("Ctrl+Z")),
                        MenuItem::new("Redo", Some("Ctrl+Shift+Z")),
                        MenuItem::separator(),
                        MenuItem::new("Code", Some("C")),
                        MenuItem::new("Byte", Some("B")),
                        MenuItem::new("Word", Some("W")),
                        MenuItem::new("Pointer", Some("P")),
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),
                    items: vec![
                        MenuItem::new("Zoom In", Some("Ctrl++")),
                        MenuItem::new("Zoom Out", Some("Ctrl+-")),
                        MenuItem::new("Reset Zoom", Some("Ctrl+0")),
                    ],
                },
                MenuCategory {
                    name: "Jump".to_string(),
                    items: vec![
                        MenuItem::new("Jump to address", Some("G")),
                        MenuItem::new("Jump to operand", Some("Enter")),
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
}

impl MenuItem {
    pub fn new(name: &str, shortcut: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            is_separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            name: String::new(),
            shortcut: None,
            is_separator: true,
        }
    }
}

fn compress_address_types(types: &[AddressType]) -> Vec<AddressRange> {
    if types.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current_type = types[0];

    for (i, t) in types.iter().enumerate().skip(1) {
        if *t != current_type {
            ranges.push(AddressRange {
                start,
                end: i - 1,
                type_: current_type,
            });
            start = i;
            current_type = *t;
        }
    }

    // Last range
    ranges.push(AddressRange {
        start,
        end: types.len() - 1,
        type_: current_type,
    });

    ranges
}

fn expand_address_ranges(ranges: &[AddressRange], len: usize) -> Vec<AddressType> {
    let mut types = vec![AddressType::Code; len];

    for range in ranges {
        let end = range.end.min(len - 1);
        if range.start <= end {
            types[range.start..=end].fill(range.type_);
        }
    }

    types
}

fn encode_raw_data(data: &[u8]) -> Vec<String> {
    data.chunks(16)
        .map(|chunk| {
            chunk
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn decode_raw_data(data: &[String]) -> anyhow::Result<Vec<u8>> {
    let mut raw = Vec::with_capacity(data.len() * 16);
    for line in data {
        for byte_str in line.split_whitespace() {
            let byte = u8::from_str_radix(byte_str, 16)?;
            raw.push(byte);
        }
    }
    Ok(raw)
}

#[cfg(test)]
mod serialization_tests {
    use super::*;

    #[test]
    fn test_compress_address_types() {
        let types = vec![
            AddressType::Code,
            AddressType::Code,
            AddressType::DataByte,
            AddressType::DataByte,
            AddressType::Code,
        ];
        let ranges = compress_address_types(&types);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, 1);
        assert_eq!(ranges[0].type_, AddressType::Code);

        assert_eq!(ranges[1].start, 2);
        assert_eq!(ranges[1].end, 3);
        assert_eq!(ranges[1].type_, AddressType::DataByte);

        assert_eq!(ranges[2].start, 4);
        assert_eq!(ranges[2].end, 4);
        assert_eq!(ranges[2].type_, AddressType::Code);
    }

    #[test]
    fn test_expand_address_ranges() {
        let ranges = vec![
            AddressRange {
                start: 0,
                end: 1,
                type_: AddressType::Code,
            },
            AddressRange {
                start: 2,
                end: 3,
                type_: AddressType::DataByte,
            },
            AddressRange {
                start: 4,
                end: 4,
                type_: AddressType::Code,
            },
        ];
        let types = expand_address_ranges(&ranges, 5);
        assert_eq!(types.len(), 5);
        assert_eq!(types[0], AddressType::Code);
        assert_eq!(types[1], AddressType::Code);
        assert_eq!(types[2], AddressType::DataByte);
        assert_eq!(types[3], AddressType::DataByte);
        assert_eq!(types[4], AddressType::Code);
    }

    #[test]
    fn test_encode_raw_data() {
        let data: Vec<u8> = (0..20).collect();
        let encoded = encode_raw_data(&data);
        assert_eq!(encoded.len(), 2);
        // First chunk 16 bytes: 00 01 ... 0F
        assert_eq!(
            encoded[0],
            "00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F"
        );
        // Second chunk 4 bytes: 10 11 12 13
        assert_eq!(encoded[1], "10 11 12 13");
    }

    #[test]
    fn test_decode_raw_data() {
        let encoded = vec!["00 01 02".to_string(), "FF".to_string()];
        let decoded = decode_raw_data(&encoded).unwrap();
        assert_eq!(decoded, vec![0x00, 0x01, 0x02, 0xFF]);
    }
}
