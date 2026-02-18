use super::project::{
    Block, Label, LoadedProjectData, ProjectSaveContext, ProjectState, compress_block_types,
    decode_raw_data_from_base64, encode_raw_data_to_base64, expand_blocks,
};
use super::settings::DocumentSettings;
use super::types::{
    BlockType, CachedArrow, HexdumpViewMode, ImmediateFormat, LabelKind, LabelType,
};
use crate::config::SystemConfig;
use crate::disassembler::{Disassembler, DisassemblyLine};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum BlockItem {
    Block {
        start: u16,
        end: u16,
        type_: BlockType,
        collapsed: bool,
    },
    Splitter(u16),
}

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub export_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub cached_arrows: Vec<CachedArrow>,
    pub disassembler: Disassembler,
    pub origin: u16,

    // Data Conversion State
    pub block_types: Vec<BlockType>,
    pub labels: BTreeMap<u16, Vec<Label>>,
    pub settings: DocumentSettings,
    pub system_comments: BTreeMap<u16, String>,
    pub user_side_comments: BTreeMap<u16, String>,
    pub user_line_comments: BTreeMap<u16, String>,
    pub immediate_value_formats: BTreeMap<u16, ImmediateFormat>,
    pub cross_refs: BTreeMap<u16, Vec<u16>>,
    pub bookmarks: BTreeMap<u16, String>,

    pub system_config: SystemConfig,

    pub undo_stack: crate::commands::UndoStack,
    pub last_saved_pointer: usize,
    pub excluded_addresses: std::collections::HashSet<u16>,
    pub collapsed_blocks: Vec<(usize, usize)>,
    pub splitters: BTreeSet<u16>,
    pub last_import_labels_path: Option<PathBuf>,
    pub last_export_labels_filename: Option<String>,
    pub last_save_as_filename: Option<String>,
    pub last_export_asm_filename: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            project_path: None,
            export_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            cached_arrows: Vec::new(),
            disassembler: Disassembler::new(),
            origin: 0,
            block_types: Vec::new(),
            labels: BTreeMap::new(),
            settings: DocumentSettings::default(),
            system_comments: BTreeMap::new(),
            user_side_comments: BTreeMap::new(),
            user_line_comments: BTreeMap::new(),
            immediate_value_formats: BTreeMap::new(),
            cross_refs: BTreeMap::new(),
            bookmarks: BTreeMap::new(),
            system_config: SystemConfig::load(),
            undo_stack: crate::commands::UndoStack::new(),
            last_saved_pointer: 0,
            excluded_addresses: std::collections::HashSet::new(),
            collapsed_blocks: Vec::new(),
            splitters: BTreeSet::new(),
            last_import_labels_path: None,
            last_export_labels_filename: None,
            last_save_as_filename: None,
            last_export_asm_filename: None,
        }
    }

    pub fn get_compressed_blocks(&self) -> Vec<Block> {
        compress_block_types(&self.block_types, &self.collapsed_blocks)
    }

    pub fn load_system_assets(&mut self) {
        // Clear existing system labels
        for labels in self.labels.values_mut() {
            labels.retain(|l| l.kind != LabelKind::System);
        }
        // Remove empty entries
        self.labels.retain(|_, v| !v.is_empty());

        // Load comments
        self.system_comments = crate::assets::load_comments(&self.settings.platform);

        // Load labels
        let system_labels = crate::assets::load_labels(
            &self.settings.platform,
            Some(&self.settings.enabled_features),
        );
        for (addr, label) in system_labels {
            self.labels.entry(addr).or_default().push(label);
        }

        // Load excludes
        let excludes = crate::assets::load_excludes(&self.settings.platform);
        self.excluded_addresses = excludes.into_iter().collect();
    }

    pub fn get_formatter(&self) -> Box<dyn crate::disassembler::formatter::Formatter> {
        Disassembler::create_formatter(self.settings.assembler)
    }

    pub fn get_block_range(&self, address: u16) -> Option<(u16, u16)> {
        let origin = self.origin;
        if address < origin {
            return None;
        }
        let index = (address - origin) as usize;
        if index >= self.block_types.len() {
            return None;
        }

        let target_type = self.block_types[index];
        let mut start = index;
        let mut end = index;

        // Search backward
        while start > 0
            && self.block_types[start - 1] == target_type
            && !self.splitters.contains(&origin.wrapping_add(start as u16))
        {
            start -= 1;
        }

        // Search forward
        while end < self.block_types.len() - 1
            && self.block_types[end + 1] == target_type
            && !self
                .splitters
                .contains(&origin.wrapping_add((end + 1) as u16))
        {
            end += 1;
        }

        let start_addr = origin.wrapping_add(start as u16);
        let end_addr = origin.wrapping_add(end as u16);

        Some((start_addr, end_addr))
    }

    pub fn load_file(&mut self, path: PathBuf) -> anyhow::Result<LoadedProjectData> {
        let data = std::fs::read(&path)?;
        self.file_path = Some(path.clone());
        self.project_path = None; // clear project path
        self.export_path = None; // clear export path
        self.labels.clear(); // clear existing labels
        self.settings = DocumentSettings::default(); // reset settings
        self.user_side_comments.clear();
        self.user_line_comments.clear();
        self.immediate_value_formats.clear();
        self.bookmarks.clear();
        self.collapsed_blocks.clear(); // clear collapsed blocks
        self.splitters.clear(); // clear splitters
        self.last_import_labels_path = None;
        self.last_export_labels_filename = None;
        self.last_save_as_filename = None;
        self.last_export_asm_filename = None;

        let mut cursor_start = None;
        let hex_cursor_start = None;

        if let Some(ext) = self
            .file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
        {
            if ext.eq_ignore_ascii_case("regen2000proj") {
                // If we loaded a project successfully, update system config
                let res = self.load_project(path.clone());
                if res.is_ok() {
                    let abs_path = std::fs::canonicalize(&path).unwrap_or(path.clone());
                    self.system_config.last_project_path = Some(abs_path);
                    let _ = self.system_config.save();
                }
                return res;
            }

            if ext.eq_ignore_ascii_case("prg") && data.len() >= 2 {
                self.origin = (data[1] as u16) << 8 | (data[0] as u16);
                self.raw_data = data[2..].to_vec();
            } else if ext.eq_ignore_ascii_case("crt") {
                let (origin, raw_data) = crate::parser::crt::parse_crt(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse CRT: {}", e))?;
                self.origin = origin;
                self.raw_data = raw_data;
            } else if ext.eq_ignore_ascii_case("vsf") {
                let vsf_data = crate::parser::vice_vsf::parse_vsf(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse VSF: {}", e))?;
                self.origin = 0;
                self.raw_data = vsf_data.memory;
                cursor_start = vsf_data.start_address;
            } else if ext.eq_ignore_ascii_case("t64") {
                let (load_address, raw_data) = crate::parser::t64::parse_t64(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse T64: {}", e))?;
                self.origin = load_address;
                self.raw_data = raw_data;
            } else if ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("raw") {
                self.origin = 0; // Default for .bin
                self.raw_data = data;
            } else {
                return Err(anyhow::anyhow!(
                    "Unsupported file extension: .{}\nSupported extensions: .prg, .crt, .vsf, .t64, .d64, .d71, .d81, .bin, .raw, .regen2000proj",
                    ext
                ));
            }
        } else {
            // specific handling for files without extension?
            // "if a user tries to load a file ... and the extension is not one of the suported ones"
            // This implies files without extensions might also be rejected or accepted as binary?
            // usually "extension is not one of supported" includes "no extension".
            // But traditionally, no extension might be treated as binary.
            // Given the strictness, I'll reject it or maybe better to assume binary if user forces it?
            // But the request says "if ... extension is not one of the supported ones".
            // I will reject files without extension to be safe and consistent with the requirement.
            return Err(anyhow::anyhow!(
                "File has no extension.\nSupported extensions: .prg, .crt, .vsf, .t64, .d64, .d71, .d81, .bin, .raw, .regen2000proj"
            ));
        }

        self.block_types = vec![BlockType::Code; self.raw_data.len()];
        self.undo_stack = crate::commands::UndoStack::new();
        self.last_saved_pointer = 0;

        self.load_system_assets();
        self.disassemble();
        self.load_system_assets();
        self.disassemble();

        if self.system_config.auto_analyze {
            self.perform_analysis();
        }

        Ok(LoadedProjectData {
            cursor_address: cursor_start,
            hex_dump_cursor_address: hex_cursor_start,
            sprites_cursor_address: None,
            right_pane_visible: None,
            charset_cursor_address: None,
            bitmap_cursor_address: None,
            charset_multicolor_mode: false,
            sprite_multicolor_mode: false,
            bitmap_multicolor_mode: None,
            hexdump_view_mode: HexdumpViewMode::default(),
            blocks_view_cursor: None,
            entropy_warning: self.check_entropy(),
        })
    }

    pub fn load_binary(&mut self, origin: u16, data: Vec<u8>) -> anyhow::Result<LoadedProjectData> {
        self.origin = origin;
        self.raw_data = data;
        self.block_types = vec![BlockType::Code; self.raw_data.len()];
        self.undo_stack = crate::commands::UndoStack::new();
        self.last_saved_pointer = 0;
        self.project_path = None;
        self.file_path = None;
        self.export_path = None;
        self.labels.clear();
        self.settings = DocumentSettings::default();
        self.user_side_comments.clear();
        self.user_line_comments.clear();
        self.immediate_value_formats.clear();
        self.bookmarks.clear();
        self.collapsed_blocks.clear();
        self.splitters.clear();
        self.last_import_labels_path = None;
        self.last_export_labels_filename = None;
        self.last_save_as_filename = None;
        self.last_export_asm_filename = None;

        self.load_system_assets();
        self.disassemble();
        self.load_system_assets();
        self.disassemble();

        if self.system_config.auto_analyze {
            self.perform_analysis();
        }

        Ok(LoadedProjectData {
            cursor_address: Some(origin),
            hex_dump_cursor_address: None,
            sprites_cursor_address: None,
            right_pane_visible: None,
            charset_cursor_address: None,
            bitmap_cursor_address: None,
            charset_multicolor_mode: false,
            sprite_multicolor_mode: false,
            bitmap_multicolor_mode: None,
            hexdump_view_mode: HexdumpViewMode::default(),
            blocks_view_cursor: None,
            entropy_warning: self.check_entropy(),
        })
    }

    fn check_entropy(&self) -> Option<f32> {
        let entropy = crate::utils::calculate_entropy(&self.raw_data);
        if entropy > self.system_config.entropy_threshold {
            Some(entropy)
        } else {
            None
        }
    }

    pub fn resolve_initial_load(
        &mut self,
        file_to_load: Option<&str>,
    ) -> Option<anyhow::Result<(LoadedProjectData, PathBuf)>> {
        if let Some(path_str) = file_to_load {
            let path = PathBuf::from(path_str);
            Some(self.load_file(path.clone()).map(|d| (d, path)))
        } else if self.system_config.open_last_project
            && let Some(last_path) = self.system_config.last_project_path.clone()
            && last_path.exists()
        {
            Some(self.load_file(last_path.clone()).map(|d| (d, last_path)))
        } else {
            None
        }
    }

    pub fn load_project(&mut self, path: PathBuf) -> anyhow::Result<LoadedProjectData> {
        let data = std::fs::read_to_string(&path)?;
        let project: ProjectState = serde_json::from_str(&data)?;

        self.project_path = Some(path);
        self.origin = project.origin;

        // Decode raw data
        self.raw_data = decode_raw_data_from_base64(&project.raw_data)?;

        // Expand address types and collapsed blocks
        let (block_types, collapsed_ranges) = expand_blocks(&project.blocks, self.raw_data.len());
        self.block_types = block_types;
        self.labels = project.labels;
        self.user_side_comments = project.user_side_comments;
        self.user_line_comments = project.user_line_comments;
        self.immediate_value_formats = project.immediate_value_formats;
        self.bookmarks = project.bookmarks;
        self.settings = project.settings;

        // Migration for legacy platform names
        match self.settings.platform.as_str() {
            "Commodore64" => self.settings.platform = "Commodore 64".to_string(),
            "Commodore128" => self.settings.platform = "Commodore 128".to_string(),
            "Commodore1541" => self.settings.platform = "Commodore 1541".to_string(),
            "CommodorePET20" => self.settings.platform = "Commodore PET 2.0".to_string(),
            "CommodorePET40" => self.settings.platform = "Commodore PET 4.0".to_string(),
            "CommodorePlus4" => self.settings.platform = "Commodore Plus4".to_string(),
            "CommodoreVIC20" => self.settings.platform = "Commodore VIC-20".to_string(),
            _ => {}
        }

        self.splitters = project.splitters;
        self.last_import_labels_path = None;
        self.last_export_labels_filename = None;
        self.last_save_as_filename = None;
        self.last_export_asm_filename = None;

        self.load_system_assets();

        // Perform analysis to regenerate autogenerated labels
        if self.system_config.auto_analyze {
            let (analyzed_labels, cross_refs) = crate::analyzer::analyze(self);
            self.labels = analyzed_labels;
            self.cross_refs = cross_refs;
        }

        self.collapsed_blocks = collapsed_ranges;
        self.undo_stack = crate::commands::UndoStack::new();
        self.last_saved_pointer = 0;

        self.disassemble();
        Ok(LoadedProjectData {
            cursor_address: project.cursor_address,
            hex_dump_cursor_address: project.hex_dump_cursor_address,
            sprites_cursor_address: project.sprites_cursor_address,
            right_pane_visible: project.right_pane_visible,
            charset_cursor_address: project.charset_cursor_address,
            bitmap_cursor_address: project.bitmap_cursor_address,
            sprite_multicolor_mode: project.sprite_multicolor_mode,
            charset_multicolor_mode: project.charset_multicolor_mode,
            bitmap_multicolor_mode: Some(project.bitmap_multicolor_mode),
            hexdump_view_mode: project.hexdump_view_mode,
            blocks_view_cursor: project.blocks_view_cursor,
            entropy_warning: None,
        })
    }

    pub fn save_project(
        &mut self,
        ctx: ProjectSaveContext,
        update_global_config: bool,
    ) -> anyhow::Result<()> {
        if let Some(path) = &self.project_path {
            let project = ProjectState {
                origin: self.origin,
                raw_data: encode_raw_data_to_base64(&self.raw_data)?,
                blocks: compress_block_types(&self.block_types, &self.collapsed_blocks),
                labels: self
                    .labels
                    .iter()
                    .map(|(k, v)| {
                        let mut user_labels: Vec<_> = v
                            .iter()
                            .filter(|label| label.kind == LabelKind::User)
                            .cloned()
                            .collect();
                        user_labels.sort_by(|a, b| a.name.cmp(&b.name));
                        (*k, user_labels)
                    })
                    .filter(|(_, v)| !v.is_empty())
                    .collect(),
                user_side_comments: self.user_side_comments.clone(),
                user_line_comments: self.user_line_comments.clone(),
                immediate_value_formats: self.immediate_value_formats.clone(),
                bookmarks: self.bookmarks.clone(),
                settings: self.settings.clone(),
                cursor_address: ctx.cursor_address,
                hex_dump_cursor_address: ctx.hex_dump_cursor_address,
                sprites_cursor_address: ctx.sprites_cursor_address,
                right_pane_visible: ctx.right_pane_visible,
                charset_cursor_address: ctx.charset_cursor_address,
                bitmap_cursor_address: ctx.bitmap_cursor_address,
                sprite_multicolor_mode: ctx.sprite_multicolor_mode,
                charset_multicolor_mode: ctx.charset_multicolor_mode,
                bitmap_multicolor_mode: ctx.bitmap_multicolor_mode,
                hexdump_view_mode: ctx.hexdump_view_mode,

                splitters: ctx.splitters,
                blocks_view_cursor: ctx.blocks_view_cursor,
            };
            let data = serde_json::to_string_pretty(&project)?;
            std::fs::write(path, data)?;
            self.last_saved_pointer = self.undo_stack.get_pointer();

            if update_global_config {
                let abs_path = std::fs::canonicalize(path).unwrap_or(path.clone());
                self.system_config.last_project_path = Some(abs_path);
                let _ = self.system_config.save();
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!("No project path set"))
        }
    }

    pub fn perform_analysis(&mut self) -> String {
        let (labels, cross_refs) = crate::analyzer::analyze(self);

        // Capture old labels (more idiomatic with iterator)
        let old_labels_map = labels
            .keys()
            .map(|k| (*k, self.labels.get(k).cloned().unwrap_or_default()))
            .collect();

        // Also capture old cross_refs
        let old_cross_refs = self.cross_refs.clone();

        let command = crate::commands::Command::SetAnalysisData {
            labels,
            cross_refs,
            old_labels: old_labels_map,
            old_cross_refs,
        };
        command.apply(self);
        self.push_command(command);
        self.disassemble();
        "Analysis Complete".to_string()
    }

    pub fn import_vice_labels(&mut self, path: PathBuf) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(path)?;
        let parsed = crate::parser::vice_lbl::parse_vice_labels(&content)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let mut new_labels_vec = Vec::new();
        let mut old_labels_map = BTreeMap::new();

        for (addr, name) in parsed {
            let label = Label {
                name,
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            };
            new_labels_vec.push((addr, label));

            if let std::collections::btree_map::Entry::Vacant(e) = old_labels_map.entry(addr) {
                e.insert(self.labels.get(&addr).cloned().unwrap_or_default());
            }
        }

        let command = crate::commands::Command::ImportLabels {
            new_labels: new_labels_vec,
            old_labels: old_labels_map,
        };
        command.apply(self);
        self.push_command(command);
        self.disassemble();

        if self.system_config.auto_analyze {
            self.perform_analysis();
        }

        Ok("Labels Imported".to_string())
    }

    pub fn export_vice_labels(&self, path: PathBuf) -> anyhow::Result<String> {
        let mut export_list = Vec::new();
        // Sort by address is automatic due to BTreeMap
        for (addr, labels) in &self.labels {
            for label in labels {
                if label.kind == LabelKind::User {
                    export_list.push((*addr, label.name.clone()));
                }
            }
        }
        let content = crate::parser::vice_lbl::generate_vice_labels(&export_list);
        std::fs::write(&path, content)?;
        Ok(format!("Labels exported to {:?}", path))
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

            // Find first and last lines with bytes in the selected range to determine the byte region
            let first_with_bytes =
                (s..=e).find(|&i| self.disassembly.get(i).is_some_and(|l| !l.bytes.is_empty()));
            let last_with_bytes = (s..=e)
                .rev()
                .find(|&i| self.disassembly.get(i).is_some_and(|l| !l.bytes.is_empty()));

            if let (Some(fs), Some(fe)) = (first_with_bytes, last_with_bytes) {
                let start_line = &self.disassembly[fs];
                let end_line = &self.disassembly[fe];

                let start_addr = start_line.address;
                let end_addr_inclusive = end_line
                    .address
                    .wrapping_add(end_line.bytes.len() as u16)
                    .wrapping_sub(1);

                let start_idx = (start_addr.wrapping_sub(self.origin)) as usize;
                let end_idx = (end_addr_inclusive.wrapping_sub(self.origin)) as usize;

                Some((start_idx, end_idx))
            } else {
                None
            }
        } else {
            // Single line action
            if let Some(line) = self.disassembly.get(cursor_index) {
                if line.bytes.is_empty() {
                    None
                } else {
                    let start_addr = line.address;
                    let end_addr_inclusive = line
                        .address
                        .wrapping_add(line.bytes.len() as u16)
                        .wrapping_sub(1);

                    let start_idx = (start_addr.wrapping_sub(self.origin)) as usize;
                    let end_idx = (end_addr_inclusive.wrapping_sub(self.origin)) as usize;
                    Some((start_idx, end_idx))
                }
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
                self.push_command(command);

                self.disassemble();
            }
        }
    }

    pub fn undo_last_command(&mut self) -> String {
        let mut stack = std::mem::take(&mut self.undo_stack);
        let msg = if let Some(msg) = stack.undo(self) {
            msg
        } else {
            "Nothing to undo".to_string()
        };
        self.undo_stack = stack;
        msg
    }

    pub fn redo_last_command(&mut self) -> String {
        let mut stack = std::mem::take(&mut self.undo_stack);
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
                // Only include if setting enabled

                if let Some(label) =
                    crate::disassembler::resolve_label(labels, *addr, &self.settings)
                {
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
                    mnemonic: format!("{} {}", formatter.comment_prefix(), title),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
                });

                for (addr, name) in group {
                    // Logic for side comment
                    let mut comment = String::new();
                    if let Some(user_comment) = self.user_side_comments.get(&addr) {
                        comment = user_comment.clone();
                    } else if let Some(sys_comment) = self.system_comments.get(&addr) {
                        comment = sys_comment.clone();
                    }

                    lines.push(DisassemblyLine {
                        address: 0,
                        bytes: vec![],
                        mnemonic: formatter.format_definition(name, addr, is_zp),
                        operand: String::new(),
                        comment,
                        line_comment: None,
                        label: None,
                        opcode: None,
                        show_bytes: true,
                        target_address: None,
                        external_label_address: Some(addr),
                        is_collapsed: false,
                    });
                }

                lines.push(DisassemblyLine {
                    address: 0,
                    bytes: vec![],
                    mnemonic: String::new(),
                    operand: String::new(),
                    comment: String::new(),
                    line_comment: None,
                    label: None,
                    opcode: None,
                    show_bytes: true,
                    target_address: None,
                    external_label_address: None,
                    is_collapsed: false,
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
        let ctx = crate::disassembler::DisassemblyContext {
            data: &self.raw_data,
            block_types: &self.block_types,
            labels: &self.labels,
            origin: self.origin,
            settings: &self.settings,
            system_comments: &self.system_comments,
            user_side_comments: &self.user_side_comments,
            user_line_comments: &self.user_line_comments,
            immediate_value_formats: &self.immediate_value_formats,
            cross_refs: &self.cross_refs,
            collapsed_blocks: &self.collapsed_blocks,
            splitters: &self.splitters,
        };
        let mut lines = self.disassembler.disassemble_ctx(&ctx);

        // Add external label definitions at the top if enabled
        if self.settings.all_labels {
            let external_lines = self.get_external_label_definitions();
            // Prepend external lines
            lines.splice(0..0, external_lines);
        }

        self.disassembly = lines;
        self.compute_cached_arrows();
    }

    pub fn compute_cached_arrows(&mut self) {
        let mut arrows = Vec::new();

        for (src_idx, line) in self.disassembly.iter().enumerate() {
            if let Some(target_addr) = line.target_address {
                // Determine if we should draw an arrow
                let should_draw = if let Some(opcode) = &line.opcode {
                    opcode.is_flow_control_with_target()
                } else {
                    // Fallback for JMP without opcode struct (legacy or special case)
                    line.mnemonic.eq_ignore_ascii_case("JMP") && line.operand.contains('(')
                };

                if should_draw {
                    // Find target index using binary search
                    let dst_idx_opt = self
                        .disassembly
                        .binary_search_by_key(&target_addr, |l| l.address)
                        .ok()
                        .or_else(|| {
                            // If exact match not found (maybe inside a multi-byte instruction?)
                            // Try finding the partition point
                            let idx = self
                                .disassembly
                                .partition_point(|l| l.address < target_addr);
                            if idx > 0 {
                                let prev = &self.disassembly[idx - 1];
                                let len = prev.bytes.len() as u16;
                                if target_addr >= prev.address
                                    && target_addr < prev.address.wrapping_add(len)
                                {
                                    return Some(idx - 1);
                                }
                            }
                            None
                        });

                    if let Some(mut dst_idx) = dst_idx_opt {
                        // Refine destination: if multiple lines have same address, pick the first one?
                        // Or match view_disassembly logic:
                        // "while refined_dst > 0 && app_state.disassembly[refined_dst - 1].address == target_addr"
                        while dst_idx > 0 && self.disassembly[dst_idx - 1].address == target_addr {
                            dst_idx -= 1;
                        }

                        arrows.push(CachedArrow {
                            start: src_idx,
                            end: dst_idx,
                            target_addr: Some(target_addr),
                        });
                    }
                    // Note: view_disassembly also handles "relative target but not in disassembly" for filtering,
                    // but for caching we usually only care if we found a valid end line.
                    // If target is outside known memory, we can't draw a connected arrow anyway.
                    // (The partial arrow logic in render needs to know if end is *visible* or *exists*)
                }
            }
        }
        self.cached_arrows = arrows;
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

        // Check for external label definitions (external_label_address matches target)
        if let Some(idx) = self
            .disassembly
            .iter()
            .position(|line| line.external_label_address == Some(address))
        {
            return Some(idx);
        }
        // Third pass: find first address >= target
        self.disassembly
            .iter()
            .position(|line| line.address >= address)
    }

    pub fn get_line_index_containing_address(&self, address: u16) -> Option<usize> {
        // Check if address is in a collapsed block
        for (start_idx, end_idx) in &self.collapsed_blocks {
            let start_addr = self.origin.wrapping_add(*start_idx as u16);
            let end_addr = self.origin.wrapping_add(*end_idx as u16);

            // Check if address is within this collapsed block [start, end]
            // Handle wrap-around if necessary
            let in_range = if start_addr <= end_addr {
                address >= start_addr && address <= end_addr
            } else {
                address >= start_addr || address <= end_addr
            };

            if in_range {
                // Return the index of the line that represents this collapsed block
                // This line starts at start_addr and has is_collapsed=true
                return self.get_line_index_for_address(start_addr);
            }
        }

        self.disassembly.iter().position(|line| {
            let start = line.address;
            let len = line.bytes.len() as u16;

            // For collapsed blocks or special lines with no bytes, we match if address is exact
            // Note: Collapsed blocks are now handled above, but we keep this for other 0-byte lines (e.g. headers)
            if len == 0 {
                return start == address;
            }

            let end = start.wrapping_add(len);

            if start < end {
                address >= start && address < end
            } else {
                // Wrap around case
                address >= start || address < end
            }
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.undo_stack.get_pointer() != self.last_saved_pointer
    }

    pub fn toggle_splitter(&mut self, address: u16) {
        // Toggle splitter for the generic address
        if self.splitters.contains(&address) {
            self.splitters.remove(&address);
        } else {
            self.splitters.insert(address);
        }
        self.disassemble();
    }

    pub fn push_command(&mut self, command: crate::commands::Command) {
        if self.undo_stack.get_pointer() < self.last_saved_pointer {
            self.last_saved_pointer = usize::MAX;
        }
        self.undo_stack.push(command);
    }

    pub fn get_blocks_view_items(&self) -> Vec<BlockItem> {
        let compressed_blocks = self.get_compressed_blocks();
        let mut items = Vec::new();

        // Convert compressed blocks to our list, splicing in splitters
        for block in compressed_blocks {
            let block_start = self.origin.wrapping_add(block.start as u16);
            let block_end = self.origin.wrapping_add(block.end as u16);

            let current_end_idx = block.end;

            // Filter splitters relevant to this block range
            let relevant_splitters: Vec<u16> = self
                .splitters
                .range(block_start..=block_end)
                .copied()
                .collect();

            let origin = self.origin;
            let mut sub_block_start = block.start;

            for splitter_addr in relevant_splitters {
                // Convert splitter address to index
                let splitter_idx = (splitter_addr.wrapping_sub(origin)) as usize;

                // If splitter is outside current bounds (shouldn't happen due to range filter), skip.
                if splitter_idx < sub_block_start || splitter_idx > current_end_idx {
                    continue;
                }

                // If splitter is > sub_block_start, we have a chunk before the splitter.
                if splitter_idx > sub_block_start {
                    items.push(BlockItem::Block {
                        start: sub_block_start as u16,
                        end: (splitter_idx - 1) as u16,
                        type_: block.type_,
                        collapsed: block.collapsed,
                    });
                }

                // Emit Splitter
                items.push(BlockItem::Splitter(splitter_addr));

                sub_block_start = splitter_idx;
            }

            // Emit remainder
            if sub_block_start <= current_end_idx {
                items.push(BlockItem::Block {
                    start: sub_block_start as u16,
                    end: current_end_idx as u16,
                    type_: block.type_,
                    collapsed: block.collapsed,
                });
            }
        }

        items
    }

    pub fn get_block_index_for_address(&self, address: u16) -> Option<usize> {
        let items = self.get_blocks_view_items();
        items.iter().position(|item| match item {
            BlockItem::Block { start, end, .. } => {
                let s = self.origin.wrapping_add(*start);
                let e = self.origin.wrapping_add(*end);
                // Check if address is within [s, e]
                if s <= e {
                    address >= s && address <= e
                } else {
                    // Wrap around
                    address >= s || address <= e
                }
            }
            BlockItem::Splitter(addr) => *addr == address,
        })
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
            }],
        );
        app_state.project_path = Some(PathBuf::from("fake_project.regen2000proj"));
        app_state.export_path = Some(PathBuf::from("fake_export.asm"));
        app_state.collapsed_blocks.push((0, 10));
        app_state.collapsed_blocks.push((20, 30));
        app_state.splitters.insert(0x1000);
        app_state.splitters.insert(0x2000);

        // 2. Create a dummy binary file
        let mut path = std::env::temp_dir();
        path.push("dummy_test.bin");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&[0xEA, 0xEA, 0xEA]).unwrap();

        // 3. Load the new file
        app_state.load_file(path.clone()).unwrap();

        // 4. Verify state is cleared
        // This is expected to FAIL before the fix
        // Verify that ALL remaining labels are System labels.
        // "all labels should be cleared, except the system ones"
        for labels in app_state.labels.values() {
            for label in labels {
                assert_eq!(
                    label.kind,
                    LabelKind::System,
                    "Only System labels should remain after loading a new file. Found a {:?} label: {}",
                    label.kind,
                    label.name
                );
            }
        }

        assert!(
            !app_state.labels.contains_key(&0x1234),
            "Specific user label address should not exist (assuming it doesn't collide with system)"
        );
        assert!(
            app_state.project_path.is_none(),
            "Project path should be None"
        );
        assert!(
            app_state.collapsed_blocks.is_empty(),
            "Collapsed blocks should be cleared"
        );
        assert!(
            app_state.splitters.is_empty(),
            "Splitters should be cleared"
        );
        assert!(
            app_state.export_path.is_none(),
            "Export path should be None"
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
            }],
        );

        // 2. Add AUTO label
        app_state.labels.insert(
            0x1005,
            vec![Label {
                name: "AutoLabel".to_string(),
                label_type: LabelType::Branch,
                kind: LabelKind::Auto,
            }],
        );

        // 3. Save Project (mocking write separately, but logic is in save_project internal construction)
        // Since `save_project` writes to file, we can verify by checking the filtering logic directly
        // OR we can actually run save_project to a temp file and deserialize. Let's do the latter.

        let mut path = std::env::temp_dir();
        path.push("test_project_serialization.json");
        app_state.project_path = Some(path.clone());

        app_state
            .save_project(
                ProjectSaveContext {
                    cursor_address: None,
                    hex_dump_cursor_address: None,
                    sprites_cursor_address: None,
                    right_pane_visible: None,
                    charset_cursor_address: None,
                    bitmap_cursor_address: None,
                    sprite_multicolor_mode: false,
                    charset_multicolor_mode: false,
                    bitmap_multicolor_mode: false,
                    hexdump_view_mode: HexdumpViewMode::default(),
                    splitters: BTreeSet::new(),
                    blocks_view_cursor: None,
                    bookmarks: BTreeMap::new(),
                },
                false,
            )
            .expect("Save failed");

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
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        // Simulate code at origin
        app_state.disassembly.push(DisassemblyLine {
            address: 0x1000,
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
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
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        // Actual code at $0000
        app_state_zero.disassembly.push(DisassemblyLine {
            address: 0,
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: "".to_string(),
            comment: "".to_string(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        let idx = app_state_zero.get_line_index_for_address(0);
        assert_eq!(
            idx,
            Some(1),
            "Should skip empty line at address 0 and find code line"
        );
    }
}

#[cfg(test)]
mod analysis_tests {
    use super::*;

    #[test]
    fn test_perform_analysis_preserves_user_labels() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        // JMP $1005 (4C 05 10)
        app_state.raw_data = vec![0x4C, 0x05, 0x10, 0xEA, 0xEA, 0xEA];
        app_state.block_types = vec![BlockType::Code; 6];

        // 1. Manually add a User Label
        let user_label = Label {
            name: "MyCustomLabel".to_string(),
            kind: LabelKind::User,
            label_type: LabelType::UserDefined,
        };
        app_state.labels.insert(0x1005, vec![user_label]);

        // 2. Perform Analysis
        app_state.perform_analysis();

        // 3. Verify User Label is PRESERVED
        let labels = app_state.labels.get(&0x1005).expect("Should have labels");
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].kind, LabelKind::User);
        assert_eq!(labels[0].name, "MyCustomLabel");
    }

    #[test]
    fn test_perform_analysis_preserves_system_labels() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        // LDA
        app_state.raw_data = vec![0xAD, 0x20, 0xD0];
        app_state.block_types = vec![BlockType::Code; 3];

        // 1. Manually add a System Label (simulating system assets)
        let sys_label = Label {
            name: "VIC_BORDER_COLOR".to_string(),
            kind: LabelKind::System,
            label_type: LabelType::AbsoluteAddress,
        };
        app_state.labels.insert(0xD020, vec![sys_label]);

        // 2. Perform Analysis
        app_state.perform_analysis();

        // 3. Verify System Label is PRESERVED (if used)
        let labels = app_state.labels.get(&0xD020).expect("Should have labels");
        assert_eq!(labels[0].name, "VIC_BORDER_COLOR");
        assert_eq!(labels[0].kind, LabelKind::System);
    }

    #[test]
    fn test_perform_analysis_regenerates_arrows() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        // JMP  (4C 05 10)
        app_state.raw_data = vec![0x4C, 0x05, 0x10, 0xEA, 0xEA, 0xEA];
        app_state.block_types = vec![BlockType::Code; 6];

        // Initially disassembly is empty or not matching.
        // We call perform_analysis which should disassemble and set arrows.
        app_state.perform_analysis();

        // The first line should be the JMP instruction with target_address 1005
        let line = &app_state.disassembly[0];
        assert_eq!(line.mnemonic, "jmp");
        assert_eq!(line.target_address, Some(0x1005));
    }

    #[test]
    fn test_default_settings() {
        let settings = DocumentSettings::default();
        assert_eq!(settings.max_arrow_columns, 6);
        assert!(settings.brk_single_byte);
        assert!(!settings.patch_brk);

        let app_state = AppState::new();
        assert_eq!(app_state.settings.max_arrow_columns, 6);
        assert!(app_state.settings.brk_single_byte);
        assert!(!app_state.settings.patch_brk);
    }
    #[test]
    fn test_set_block_type_lohi_creates_labels() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        // 4 bytes: 00 01 (Lo), 00 20 (Hi).
        // Pair 1: Lo=00, Hi=00 -> $0000 (Internal/ZP)
        // Pair 2: Lo=01, Hi=20 -> $2001 (External, assuming len=4)
        app_state.raw_data = vec![0x00, 0x01, 0x00, 0x20];

        // Initialize as DataByte so we have 1-to-1 mapping in disassembly lines
        app_state.block_types = vec![BlockType::DataByte; 4];
        app_state.disassemble();

        // Apply LoHi
        // Selection is indices of DISASSEMBLY LINES.
        // DataByte grouping put all 4 bytes on ONE line (line 0).
        // So we select line 0 to 0.
        app_state.set_block_type_region(BlockType::LoHiAddress, Some(0), 0);

        // Verify Label $0000 (Internal)
        let l1 = app_state.labels.get(&0x0000);
        assert!(l1.is_some(), "Should generate label for internal address");
        assert_eq!(l1.unwrap()[0].name, "a0000"); // Analyzer generates 'a' for AbsoluteAddress usage

        // Verify Label $2001 (External)
        // Analyzer generates 'a' for AbsoluteAddress usage even if external, unless it's a Jump.
        let l2 = app_state.labels.get(&0x2001);
        assert!(l2.is_some(), "Should generate label for external address");
        assert_eq!(l2.unwrap()[0].name, "a2001");
    }
    #[test]
    fn test_get_line_index_with_collapsed_block() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.raw_data = vec![0xEA, 0xEA, 0xEA];
        state.block_types = vec![BlockType::Code; 3];

        // Collapse middle byte (offset 1, length 1)
        state.collapsed_blocks.push((1, 1));
        state.disassemble();

        // Line 0: NOP ($1000)
        // Line 1: Collapsed ($1001)
        // Line 2: NOP ($1002)

        // Test finding start of collapsed block
        let idx = state.get_line_index_containing_address(0x1001);
        assert_eq!(
            idx,
            Some(1),
            "Should find index of collapsed block summary line"
        );

        // Test finding normal lines
        assert_eq!(state.get_line_index_containing_address(0x1000), Some(0));
        assert_eq!(state.get_line_index_containing_address(0x1002), Some(2));
    }

    #[test]
    fn test_get_block_range_respects_splitters() {
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.raw_data = vec![0xEA; 10]; // 10 bytes of NOP
        state.block_types = vec![BlockType::Code; 10];

        // Without splitters, range should be everything
        let range = state.get_block_range(0x1005).unwrap();
        assert_eq!(range, (0x1000, 0x1009));

        // Add a splitter at 0x1005
        state.splitters.insert(0x1005);

        // Range for 0x1004 should stop at 0x1004
        let range1 = state.get_block_range(0x1004).unwrap();
        assert_eq!(range1, (0x1000, 0x1004));

        // Range for 0x1005 should start at 0x1005
        let range2 = state.get_block_range(0x1005).unwrap();
        assert_eq!(range2, (0x1005, 0x1009));

        // Add another splitter at 0x1008
        state.splitters.insert(0x1008);

        // Range for 0x1006 should be 0x1005 to 0x1007
        let range3 = state.get_block_range(0x1006).unwrap();
        assert_eq!(range3, (0x1005, 0x1007));
    }
    #[test]
    fn test_export_vice_labels() {
        use std::path::PathBuf;
        let mut state = AppState::new();
        state.origin = 0x1000;
        state.labels.insert(
            0x1000,
            vec![Label {
                name: "start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            0x2000,
            vec![Label {
                name: "loop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        // System label should be ignored
        state.labels.insert(
            0xFFD2,
            vec![Label {
                name: "CHROUT".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );

        let path = PathBuf::from("test_export_vice.lbl");
        // Ensure cleanup if exists
        #[allow(unused_must_use)]
        {
            std::fs::remove_file(&path);
        }

        let res = state.export_vice_labels(path.clone());
        assert!(res.is_ok());

        let content = std::fs::read_to_string(&path).unwrap();
        // Check content
        assert!(content.contains("al C:1000 .start"));
        assert!(content.contains("al C:2000 .loop"));
        assert!(!content.contains("CHROUT"));

        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn test_last_project_path_is_full_path() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("regen_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let filename = "test_project.regen2000proj";
        let project_path = dir.join(filename);

        // Create a minimal valid project file
        let mut file = File::create(&project_path).unwrap();
        let valid_base64 = crate::state::project::encode_raw_data_to_base64(&[]).unwrap();

        write!(
            file,
            r#"{{
            "origin": 2048,
            "raw_data_base64": "{}",
            "blocks": [],
            "labels": {{}},
            "user_side_comments": {{}},
            "user_line_comments": {{}},
            "immediate_value_formats": {{}},
            "settings": {{
                "platform": "Commodore 64",
                "assembler": "Ca65",
                "all_labels": false,
                "enabled_features": {{}}
            }},
            "cursor_address": 0,
            "hex_dump_cursor_address": 0,
            "sprites_cursor_address": 0,
            "right_pane_visible": null,
            "charset_cursor_address": 0,
            "bitmap_cursor_address": 0,
            "sprite_multicolor_mode": false,
            "charset_multicolor_mode": false,
            "bitmap_multicolor_mode": false,
            "hexdump_view_mode": "ScreencodeShifted",
            "splitters": [],
            "blocks_view_cursor": 0
         }}"#,
            valid_base64
        )
        .unwrap();
        drop(file);

        let mut app_state = AppState::new();

        // Override config path to avoid writing to user's real config
        let config_path = dir.join("config.json");
        app_state.system_config.config_path_override = Some(config_path);

        // Load using the path (absolute, but might contain symlinks)
        let res = app_state.load_file(project_path.clone());
        assert!(res.is_ok(), "Failed to load project: {:?}", res.err());

        // Check if last_project_path is absolute and canonical
        let stored_path = app_state
            .system_config
            .last_project_path
            .as_ref()
            .expect("last_project_path should be set");

        assert!(
            stored_path.is_absolute(),
            "last_project_path should be absolute, got: {:?}",
            stored_path
        );

        // Verify it is canonicalized
        let canonical_expected = std::fs::canonicalize(&project_path).unwrap();
        assert_eq!(
            *stored_path, canonical_expected,
            "last_project_path should be canonicalized"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
