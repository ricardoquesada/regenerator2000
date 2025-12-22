use crate::disassembler::{Disassembler, DisassemblyLine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    AppleII,
    Atari8Bit,
    BBCMicro,
    Commodore128,
    Commodore1541,
    Commodore64,
    CommodorePET20,
    CommodorePET40,
    CommodorePlus4,
    CommodoreVIC20,
    NES,
    Oric10,
    Oric11,
}

impl Default for Platform {
    fn default() -> Self {
        Platform::Commodore64
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::AppleII => write!(f, "Apple II"),
            Platform::Atari8Bit => write!(f, "Atari 8 bit"),
            Platform::BBCMicro => write!(f, "BBC Micro"),
            Platform::Commodore128 => write!(f, "Commodore 128"),
            Platform::Commodore1541 => write!(f, "Commodore 1541"),
            Platform::Commodore64 => write!(f, "Commodore 64"),
            Platform::CommodorePET20 => write!(f, "Commodore PET 2.0"),
            Platform::CommodorePET40 => write!(f, "Commodore PET 4.0"),
            Platform::CommodorePlus4 => write!(f, "Commodore Plus/4"),
            Platform::CommodoreVIC20 => write!(f, "Commodore VIC 20"),
            Platform::NES => write!(f, "NES"),
            Platform::Oric10 => write!(f, "Oric 1.0"),
            Platform::Oric11 => write!(f, "Oric 1.1"),
        }
    }
}

impl Platform {
    pub fn all() -> &'static [Platform] {
        &[
            Platform::AppleII,
            Platform::Atari8Bit,
            Platform::BBCMicro,
            Platform::Commodore128,
            Platform::Commodore1541,
            Platform::Commodore64,
            Platform::CommodorePET20,
            Platform::CommodorePET40,
            Platform::CommodorePlus4,
            Platform::CommodoreVIC20,
            Platform::NES,
            Platform::Oric10,
            Platform::Oric11,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSettings {
    #[serde(default)]
    pub all_labels: bool, // default false
    #[serde(default = "default_true")]
    pub use_w_prefix: bool, // default true
    #[serde(default)]
    pub brk_single_byte: bool, // default false
    #[serde(default)]
    pub patch_brk: bool, // default false
    #[serde(default)]
    pub platform: Platform, // default C64
}

fn default_true() -> bool {
    true
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            all_labels: false,
            use_w_prefix: true,
            brk_single_byte: false,
            patch_brk: false,
            platform: Platform::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    Code,
    DataByte,
    DataWord,
    Address, // Reference to an address
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LabelKind {
    User,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LabelType {
    ZeroPageField = 0,
    Field = 1,
    ZeroPageAbsoluteAddress = 2,
    AbsoluteAddress = 3,
    Pointer = 4,
    ZeroPagePointer = 5,
    Branch = 6,
    Jump = 7,
    Subroutine = 8,
    ExternalJump = 9,
    Predefined = 10,
    UserDefined = 11,
}

impl LabelType {
    pub fn prefix(&self) -> char {
        match self {
            LabelType::ZeroPageField => 'f',
            LabelType::Field => 'f',
            LabelType::ZeroPageAbsoluteAddress => 'a',
            LabelType::AbsoluteAddress => 'a',
            LabelType::Pointer => 'p',
            LabelType::ZeroPagePointer => 'p',
            LabelType::ExternalJump => 'e',
            LabelType::Jump => 'j',
            LabelType::Subroutine => 's',
            LabelType::Branch => 'b',
            LabelType::Predefined => 'L',
            LabelType::UserDefined => 'L',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub names: HashMap<LabelType, String>,
    pub kind: LabelKind,
    pub refs: Vec<u16>,
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
    pub labels: HashMap<u16, Label>,
    #[serde(default)]
    pub settings: DocumentSettings,
}

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub disassembler: Disassembler,
    pub origin: u16,

    // Data Conversion State
    pub address_types: Vec<AddressType>,
    pub labels: HashMap<u16, Label>,
    pub settings: DocumentSettings,

    pub undo_stack: crate::commands::UndoStack,
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
            address_types: Vec::new(),
            labels: HashMap::new(),
            settings: DocumentSettings::default(),
            undo_stack: crate::commands::UndoStack::new(),
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let data = std::fs::read(&path)?;
        self.file_path = Some(path.clone());
        self.project_path = None; // clear project path
        self.labels.clear(); // clear existing labels
        self.settings = DocumentSettings::default(); // reset settings

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
        self.undo_stack = crate::commands::UndoStack::new();

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
        self.settings = project.settings;
        self.undo_stack = crate::commands::UndoStack::new();

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
                settings: self.settings,
            };
            let data = serde_json::to_string_pretty(&project)?;
            std::fs::write(path, data)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No project path set"))
        }
    }

    pub fn perform_analysis(&mut self) -> String {
        let labels = crate::analyzer::analyze(self);
        let mut new_labels_map = std::collections::HashMap::new();
        for (addr, label) in labels {
            new_labels_map.insert(addr, Some(label));
        }
        // Capture old labels
        let mut old_labels_map = std::collections::HashMap::new();
        for k in new_labels_map.keys() {
            old_labels_map.insert(*k, self.labels.get(k).cloned());
        }

        let command = crate::commands::Command::SetLabels {
            labels: new_labels_map,
            old_labels: old_labels_map,
        };
        command.apply(self);
        self.undo_stack.push(command);
        self.disassemble();
        "Analysis Complete".to_string()
    }

    pub fn set_address_type_region(
        &mut self,
        new_type: AddressType,
        selection_start: Option<usize>,
        cursor_index: usize,
    ) {
        let range_opt = if let Some(selection_start) = selection_start {
            let (s, e) = if selection_start < cursor_index {
                (selection_start, cursor_index)
            } else {
                (cursor_index, selection_start)
            };

            if let (Some(start_line), Some(end_line)) =
                (self.disassembly.get(s), self.disassembly.get(e))
            {
                let start_addr = start_line.address;
                let end_addr_inclusive = end_line.address + end_line.bytes.len() as u16 - 1;

                let start_idx = (start_addr.wrapping_sub(self.origin)) as usize;
                let end_idx = (end_addr_inclusive.wrapping_sub(self.origin)) as usize;

                Some((start_idx, end_idx))
            } else {
                None
            }
        } else {
            // Single line action
            if let Some(line) = self.disassembly.get(cursor_index) {
                let start_addr = line.address;
                let end_addr_inclusive = line.address + line.bytes.len() as u16 - 1;

                let start_idx = (start_addr.wrapping_sub(self.origin)) as usize;
                let end_idx = (end_addr_inclusive.wrapping_sub(self.origin)) as usize;
                Some((start_idx, end_idx))
            } else {
                None
            }
        };

        if let Some((start, end)) = range_opt {
            // Boundary check
            let max_len = self.address_types.len();
            if start < max_len {
                let valid_end = end.min(max_len);
                let range_end = valid_end + 1;
                let range = start..range_end;

                let old_types = self.address_types[range.clone()].to_vec();

                let command = crate::commands::Command::SetAddressType {
                    range: range.clone(),
                    new_type,
                    old_types,
                };

                command.apply(self);
                self.undo_stack.push(command);

                self.disassemble();
            }
        }
    }

    pub fn undo_last_command(&mut self) -> String {
        let mut stack = std::mem::replace(&mut self.undo_stack, crate::commands::UndoStack::new());
        let msg = if let Some(msg) = stack.undo(self) {
            msg
        } else {
            "Nothing to undo".to_string()
        };
        self.undo_stack = stack;
        msg
    }

    pub fn redo_last_command(&mut self) -> String {
        let mut stack = std::mem::replace(&mut self.undo_stack, crate::commands::UndoStack::new());
        let msg = if let Some(msg) = stack.redo(self) {
            msg
        } else {
            "Nothing to redo".to_string()
        };
        self.undo_stack = stack;
        msg
    }

    pub fn is_external(&self, addr: u16) -> bool {
        let len = self.raw_data.len();
        let end = self.origin.wrapping_add(len as u16);
        if self.origin < end {
            addr < self.origin || addr >= end
        } else {
            !(addr >= self.origin || addr < end)
        }
    }

    pub fn get_external_label_definitions(&self) -> Vec<DisassemblyLine> {
        let mut candidates: Vec<(u16, LabelType, &String)> = Vec::new();

        for (addr, label) in &self.labels {
            if self.is_external(*addr) {
                for (l_type, name) in &label.names {
                    candidates.push((*addr, *l_type, name));
                }

                let covered = label.names.values().any(|n| n == &label.name);
                if !covered {
                    let l_type = if label.kind == crate::state::LabelKind::User {
                        LabelType::UserDefined
                    } else {
                        LabelType::UserDefined
                    };
                    candidates.push((*addr, l_type, &label.name));
                }
            }
        }

        let mut seen_names = std::collections::HashSet::new();
        let mut all_externals = Vec::new();

        for item in candidates {
            let name = item.2;
            if !seen_names.contains(name) {
                seen_names.insert(name);
                all_externals.push(item);
            }
        }

        let mut zp_fields = Vec::new();
        let mut zp_abs = Vec::new();
        let mut zp_ptrs = Vec::new();
        let mut fields = Vec::new();
        let mut abs = Vec::new();
        let mut ptrs = Vec::new();
        let mut ext_jumps = Vec::new();
        let mut others = Vec::new();

        for (addr, l_type, name) in all_externals {
            match l_type {
                LabelType::ZeroPageField => zp_fields.push((addr, name)),
                LabelType::ZeroPageAbsoluteAddress => zp_abs.push((addr, name)),
                LabelType::ZeroPagePointer => zp_ptrs.push((addr, name)),
                LabelType::Field => fields.push((addr, name)),
                LabelType::AbsoluteAddress => abs.push((addr, name)),
                LabelType::Pointer => ptrs.push((addr, name)),
                LabelType::ExternalJump => ext_jumps.push((addr, name)),
                _ => others.push((addr, name)),
            }
        }

        let sort_group = |group: &mut Vec<(u16, &String)>| {
            group.sort_by_key(|(a, _)| *a);
        };

        sort_group(&mut zp_fields);
        sort_group(&mut zp_abs);
        sort_group(&mut zp_ptrs);
        sort_group(&mut fields);
        sort_group(&mut abs);
        sort_group(&mut ptrs);
        sort_group(&mut ext_jumps);
        sort_group(&mut others);

        let mut lines = Vec::new();

        let mut add_group = |title: &str, group: Vec<(u16, &String)>, is_zp: bool| {
            if !group.is_empty() {
                lines.push(DisassemblyLine {
                    address: 0,
                    bytes: vec![],
                    mnemonic: format!("; {}", title),
                    operand: String::new(),
                    comment: String::new(),
                    label: None,
                    opcode: None,
                });

                for (addr, name) in group {
                    let operand = if is_zp && addr <= 0xFF {
                        format!("${:02X}", addr)
                    } else {
                        format!("${:04X}", addr)
                    };

                    lines.push(DisassemblyLine {
                        address: 0,
                        bytes: vec![],
                        mnemonic: format!("{} = {}", name, operand),
                        operand: String::new(),
                        comment: String::new(),
                        label: None,
                        opcode: None,
                    });
                }

                lines.push(DisassemblyLine {
                    address: 0,
                    bytes: vec![],
                    mnemonic: String::new(),
                    operand: String::new(),
                    comment: String::new(),
                    label: None,
                    opcode: None,
                });
            }
        };

        add_group("ZP FIELDS", zp_fields, true);
        add_group("ZP ABSOLUTE ADDRESSES", zp_abs, true);
        add_group("ZP POINTERS", zp_ptrs, true);
        add_group("FIELDS", fields, false);
        add_group("ABSOLUTE ADDRESSES", abs, false);
        add_group("POINTERS", ptrs, false);
        add_group("EXTERNAL JUMPS", ext_jumps, false);
        add_group("OTHERS", others, false);

        lines
    }

    pub fn disassemble(&mut self) {
        let mut lines = self.disassembler.disassemble(
            &self.raw_data,
            &self.address_types,
            &self.labels,
            self.origin,
        );

        // Add external label definitions at the top
        let external_lines = self.get_external_label_definitions();
        if !external_lines.is_empty() {
            let mut all_lines = external_lines;
            all_lines.extend(lines);
            lines = all_lines;
        }

        self.disassembly = lines;
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
    data.chunks(32)
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
    let mut raw = Vec::with_capacity(data.len() * 32);
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
        let data: Vec<u8> = (0..40).collect();
        let encoded = encode_raw_data(&data);
        assert_eq!(encoded.len(), 2);
        // First chunk 32 bytes: 00 01 ... 1F
        let chunk1: Vec<String> = (0..32).map(|b| format!("{:02X}", b)).collect();
        assert_eq!(encoded[0], chunk1.join(" "));

        // Second chunk 6 bytes: 20 21 22 23 24 25 26 27
        let chunk2: Vec<String> = (32..40).map(|b| format!("{:02X}", b)).collect();
        assert_eq!(encoded[1], chunk2.join(" "));
    }

    #[test]
    fn test_decode_raw_data() {
        let encoded = vec!["00 01 02".to_string(), "FF".to_string()];
        let decoded = decode_raw_data(&encoded).unwrap();
        assert_eq!(decoded, vec![0x00, 0x01, 0x02, 0xFF]);
    }
}

#[cfg(test)]
mod load_file_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_file_clears_state() {
        let mut app_state = AppState::new();

        // 1. Set some initial state
        app_state.labels.insert(
            0x1234,
            Label {
                name: "DeleteMe".to_string(),
                names: HashMap::new(),
                kind: LabelKind::User,
                refs: vec![],
            },
        );
        app_state.project_path = Some(PathBuf::from("fake_project.json"));

        // 2. Create a dummy binary file
        let mut path = std::env::temp_dir();
        path.push("dummy_test.bin");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&[0xEA, 0xEA, 0xEA]).unwrap();

        // 3. Load the new file
        app_state.load_file(path.clone()).unwrap();

        // 4. Verify state is cleared
        // This is expected to FAIL before the fix
        assert!(
            app_state.labels.is_empty(),
            "Labels should be empty after loading new file"
        );
        assert!(
            app_state.project_path.is_none(),
            "Project path should be None"
        );

        // Cleanup
        let _ = std::fs::remove_file(path);
    }
}
