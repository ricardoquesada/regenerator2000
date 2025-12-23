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
pub enum Assembler {
    Tass64,
    Acme,
}

impl Default for Assembler {
    fn default() -> Self {
        Assembler::Tass64
    }
}

impl std::fmt::Display for Assembler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Assembler::Tass64 => write!(f, "64tass"),
            Assembler::Acme => write!(f, "ACME"),
        }
    }
}

impl Assembler {
    pub fn all() -> &'static [Assembler] {
        &[Assembler::Tass64, Assembler::Acme]
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
    #[serde(default)]
    pub assembler: Assembler, // default Tass64
    #[serde(default = "default_max_xref")]
    pub max_xref_count: usize, // default 5
}

fn default_true() -> bool {
    true
}

fn default_max_xref() -> usize {
    5
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            all_labels: false,
            use_w_prefix: true,
            brk_single_byte: false,
            patch_brk: false,
            platform: Platform::default(),
            assembler: Assembler::default(),
            max_xref_count: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    Code,
    DataByte,
    DataWord,
    Address, // Reference to an address
    Text,
    Screencode,
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
    pub label_type: LabelType,
    pub kind: LabelKind,
    pub refs: Vec<u16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub type_: BlockType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectState {
    pub origin: u16,
    #[serde(rename = "raw_data_base64")]
    pub raw_data: String,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub labels: HashMap<u16, Vec<Label>>,
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
    pub block_types: Vec<BlockType>,
    pub labels: HashMap<u16, Vec<Label>>,
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
            block_types: Vec::new(),
            labels: HashMap::new(),
            settings: DocumentSettings::default(),
            undo_stack: crate::commands::UndoStack::new(),
        }
    }

    pub fn get_formatter(&self) -> Box<dyn crate::disassembler::formatter::Formatter> {
        Disassembler::create_formatter(self.settings.assembler)
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
            if ext.eq_ignore_ascii_case("regen2000proj") {
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

        self.block_types = vec![BlockType::Code; self.raw_data.len()];
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
        self.raw_data = decode_raw_data_from_base64(&project.raw_data)?;

        // Expand address types
        self.block_types = expand_blocks(&project.blocks, self.raw_data.len());
        self.labels = project.labels;
        self.settings = project.settings;

        // Perform analysis to regenerate autogenerated labels
        let analyzed_labels = crate::analyzer::analyze(self);
        self.labels = analyzed_labels;

        self.undo_stack = crate::commands::UndoStack::new();

        self.disassemble();
        Ok(())
    }

    pub fn save_project(&mut self) -> anyhow::Result<()> {
        if let Some(path) = &self.project_path {
            let project = ProjectState {
                origin: self.origin,
                raw_data: encode_raw_data_to_base64(&self.raw_data),
                blocks: compress_block_types(&self.block_types),
                labels: self
                    .labels
                    .clone()
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            v.into_iter()
                                .filter(|label| label.kind == LabelKind::User)
                                .collect::<Vec<_>>(),
                        )
                    })
                    .filter(|(_, v)| !v.is_empty())
                    .collect(),
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

        // Capture old labels
        let mut old_labels_map = std::collections::HashMap::new();
        for k in labels.keys() {
            old_labels_map.insert(*k, self.labels.get(k).cloned().unwrap_or_default());
        }
        // Also verify any removed labels? analyze replaces everything?
        // analyze returns ALL labels.
        // Assuming analyze returns the complete set of labels for the program.

        // But wait, user defined labels might not be in analyzer result if the analyzer doesn't preserve them?
        // Analyzer SHOULD preserve user labels.

        let command = crate::commands::Command::SetLabels {
            labels: labels, // SetLabels now expects HashMap<u16, Vec<Label>>
            old_labels: old_labels_map,
        };
        command.apply(self);
        self.undo_stack.push(command);
        self.disassemble();
        "Analysis Complete".to_string()
    }

    pub fn set_block_type_region(
        &mut self,
        new_type: BlockType,
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
            let max_len = self.block_types.len();
            if start < max_len {
                let valid_end = end.min(max_len);
                let range_end = valid_end + 1;
                let range = start..range_end;

                let old_types = self.block_types[range.clone()].to_vec();

                let command = crate::commands::Command::SetBlockType {
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

        for (addr, labels) in &self.labels {
            if self.is_external(*addr) {
                for label in labels {
                    candidates.push((*addr, label.label_type, &label.name));
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

        let formatter = self.get_formatter();

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
                    show_bytes: true,
                });

                for (addr, name) in group {
                    lines.push(DisassemblyLine {
                        address: 0,
                        bytes: vec![],
                        mnemonic: formatter.format_definition(name, addr, is_zp),
                        operand: String::new(),
                        comment: String::new(),
                        label: None,
                        opcode: None,
                        show_bytes: true,
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
                    show_bytes: true,
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
            &self.block_types,
            &self.labels,
            self.origin,
            &self.settings,
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
    pub fn get_line_index_for_address(&self, address: u16) -> Option<usize> {
        // First pass: try to find exact match with content (bytes not empty)
        // This avoids matching external label headers that might be at the same address (e.g. 0)
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.address == address && !line.bytes.is_empty())
        {
            return Some(idx);
        }

        // Second pass: try to find any exact match
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.address == address)
        {
            return Some(idx);
        }

        // Third pass: find first address >= target
        self.disassembly
            .iter()
            .position(|line| line.address >= address)
    }
}

fn compress_block_types(types: &[BlockType]) -> Vec<Block> {
    if types.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current_type = types[0];

    for (i, t) in types.iter().enumerate().skip(1) {
        if *t != current_type {
            ranges.push(Block {
                start,
                end: i - 1,
                type_: current_type,
            });
            start = i;
            current_type = *t;
        }
    }

    // Last range
    ranges.push(Block {
        start,
        end: types.len() - 1,
        type_: current_type,
    });

    ranges
}

fn expand_blocks(ranges: &[Block], len: usize) -> Vec<BlockType> {
    let mut types = vec![BlockType::Code; len];

    for range in ranges {
        let end = range.end.min(len - 1);
        if range.start <= end {
            types[range.start..=end].fill(range.type_);
        }
    }

    types
}

use base64::{engine::general_purpose, Engine as _};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Read;
use std::io::Write;

pub(crate) fn encode_raw_data_to_base64(data: &[u8]) -> String {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    let compressed_data = encoder.finish().unwrap();
    general_purpose::STANDARD.encode(compressed_data)
}

pub(crate) fn decode_raw_data_from_base64(data: &str) -> anyhow::Result<Vec<u8>> {
    let decoded_compressed = general_purpose::STANDARD.decode(data)?;
    let mut decoder = GzDecoder::new(&decoded_compressed[..]);
    let mut raw_data = Vec::new();
    decoder.read_to_end(&mut raw_data)?;
    Ok(raw_data)
}

#[cfg(test)]
mod serialization_tests {
    use super::*;

    #[test]
    fn test_compress_block_types() {
        let types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::DataByte,
            BlockType::DataByte,
            BlockType::Code,
        ];
        let ranges = compress_block_types(&types);
        assert_eq!(ranges.len(), 3);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, 1);
        assert_eq!(ranges[0].type_, BlockType::Code);

        assert_eq!(ranges[1].start, 2);
        assert_eq!(ranges[1].end, 3);
        assert_eq!(ranges[1].type_, BlockType::DataByte);

        assert_eq!(ranges[2].start, 4);
        assert_eq!(ranges[2].end, 4);
        assert_eq!(ranges[2].type_, BlockType::Code);
    }

    #[test]
    fn test_expand_blocks() {
        let ranges = vec![
            Block {
                start: 0,
                end: 1,
                type_: BlockType::Code,
            },
            Block {
                start: 2,
                end: 3,
                type_: BlockType::DataByte,
            },
            Block {
                start: 4,
                end: 4,
                type_: BlockType::Code,
            },
        ];
        let types = expand_blocks(&ranges, 5);
        assert_eq!(types.len(), 5);
        assert_eq!(types[0], BlockType::Code);
        assert_eq!(types[1], BlockType::Code);
        assert_eq!(types[2], BlockType::DataByte);
        assert_eq!(types[3], BlockType::DataByte);
        assert_eq!(types[4], BlockType::Code);
    }

    #[test]
    fn test_encode_decode_raw_data() {
        let data: Vec<u8> = (0..100).collect();
        let encoded = encode_raw_data_to_base64(&data);
        // Base64 string should not contain spaces
        assert!(!encoded.contains(' '));

        let decoded = decode_raw_data_from_base64(&encoded).unwrap();
        assert_eq!(data, decoded);
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
            vec![Label {
                name: "DeleteMe".to_string(),
                label_type: LabelType::UserDefined,
                kind: LabelKind::User,
                refs: vec![],
            }],
        );
        app_state.project_path = Some(PathBuf::from("fake_project.regen2000proj"));

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

#[cfg(test)]
mod save_project_tests {
    use super::*;

    #[test]
    fn test_save_excludes_auto_and_names() {
        let mut app_state = AppState::new();
        app_state.project_path = Some(PathBuf::from("test_project.regen2000proj"));

        // 1. Add USER label
        app_state.labels.insert(
            0x1000,
            vec![Label {
                name: "UserLabel".to_string(),
                label_type: LabelType::AbsoluteAddress,
                kind: LabelKind::User,
                refs: vec![0x2000],
            }],
        );

        // 2. Add AUTO label
        app_state.labels.insert(
            0x1005,
            vec![Label {
                name: "AutoLabel".to_string(),
                label_type: LabelType::Branch,
                kind: LabelKind::Auto,
                refs: vec![0x2002],
            }],
        );

        // 3. Save Project (mocking write separately, but logic is in save_project internal construction)
        // Since `save_project` writes to file, we can verify by checking the filtering logic directly
        // OR we can actually run save_project to a temp file and deserialize. Let's do the latter.

        let mut path = std::env::temp_dir();
        path.push("test_project_serialization.json");
        app_state.project_path = Some(path.clone());

        app_state.save_project().expect("Save failed");

        // 4. Read back JSON manually to inspect
        let data = std::fs::read_to_string(&path).expect("Read failed");
        let project: ProjectState = serde_json::from_str(&data).expect("Deserialize failed");

        // 5. Verify AUTO label is GONE
        assert!(
            !project.labels.contains_key(&0x1005),
            "Autogenerated label should NOT be saved"
        );

        // 6. Verify USER label is PRESENT
        let user_label = project
            .labels
            .get(&0x1000)
            .expect("User label should be saved");
        assert_eq!(user_label.first().unwrap().name, "UserLabel");
        assert_eq!(user_label.first().unwrap().kind, LabelKind::User);

        // 7. Verify `names` map is EMPTY (skipped)
        // When deserialized, because it was skipped, it should get the default value (empty Map)
        // NOTE: We need to make sure `Label` implements `Default` for `names` or serde handles missing field as default.
        // `HashMap` default is empty. `#[serde(skip)]` means it won't be in JSON.
        // When reading back, if the field is missing in JSON, we need `#[serde(default)]` on the struct field
        // OR rely on the fact that we are deserializing into a struct where we removed `skip`?
        // NO. `Label` definition HAS `#[serde(skip)]`. So `serde` will NOT write it.
        // But when reading `ProjectState`, it uses the SAME `Label` definition.
        // Serde `skip` on a field means it is NOT serialized AND NOT deserialized (it takes default).
        // So `user_label.names` should be empty.

        assert_eq!(
            user_label.first().unwrap().label_type,
            LabelType::AbsoluteAddress,
            "Label type should be preserved"
        );

        // Cleanup
        let _ = std::fs::remove_file(path);
    }
}
#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn test_get_line_index_skips_headers() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;

        // Simulate external label definition at 0
        app_state.disassembly.push(DisassemblyLine {
            address: 0,
            bytes: vec![],
            mnemonic: "; EXTERNAL".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            label: None,
            opcode: None,
            show_bytes: true,
        });

        // Simulate code at origin
        app_state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            label: None,
            opcode: None,
            show_bytes: true,
        });

        // Should return index 1 (the code), not index 0 (the header)
        // Note: address 0 is NOT the origin here, but if origin WAS 0, we'd want to skip index 0 if it's empty.
        // Let's test the case where origin is 0 and we have external labels for 0.

        let mut app_state_zero = AppState::new();
        app_state_zero.origin = 0;

        // External label for $0000
        app_state_zero.disassembly.push(DisassemblyLine {
            address: 0,
            bytes: vec![],
            mnemonic: "ExtLabel".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            label: None,
            opcode: None,
            show_bytes: true,
        });

        // Actual code at $0000
        app_state_zero.disassembly.push(DisassemblyLine {
            address: 0,
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            label: None,
            opcode: None,
            show_bytes: true,
        });

        let idx = app_state_zero.get_line_index_for_address(0);
        assert_eq!(
            idx,
            Some(1),
            "Should skip empty line at address 0 and find code line"
        );
    }
}
