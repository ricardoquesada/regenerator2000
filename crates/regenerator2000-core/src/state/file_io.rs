use super::app_state::AppState;
use super::project::{
    LoadedProjectData, PROJECT_FORMAT_VERSION, ProjectSaveContext, ProjectState,
    compress_block_types, decode_raw_data_from_base64, encode_raw_data_to_base64, expand_blocks,
};
use super::settings::DocumentSettings;
use super::types::{Addr, BlockType, HexdumpViewMode, LabelKind, LabelType, Platform};
use std::collections::BTreeMap;
use std::path::PathBuf;

impl AppState {
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
        let mut suggested_platform = None;

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
                    self.system_config.last_project_path = Some(abs_path.clone());
                    self.system_config.add_recent_project(abs_path);
                    let _ = self.system_config.save();
                }
                return res;
            }

            if ext.eq_ignore_ascii_case("dis65") {
                let res = self.load_dis65_project(path.clone());
                if res.is_ok() {
                    let abs_path = std::fs::canonicalize(&path).unwrap_or(path.clone());
                    self.system_config.last_project_path = Some(abs_path);
                    let _ = self.system_config.save();
                }
                return res;
            }

            if ext.eq_ignore_ascii_case("prg") || ext.eq_ignore_ascii_case("t64") {
                let prg_bytes_holder;
                let prg_ref = if ext.eq_ignore_ascii_case("prg") {
                    &data
                } else {
                    prg_bytes_holder = crate::parser::t64::parse_t64(&data)
                        .map_err(|e| anyhow::anyhow!("Failed to parse T64: {e}"))?;
                    &prg_bytes_holder
                };
                let prg_data = crate::parser::prg::parse_prg(prg_ref)
                    .map_err(|e| anyhow::anyhow!("Failed to parse PRG: {e}"))?;
                self.origin = Addr(prg_data.origin);
                self.raw_data = prg_data.raw_data;
                let default_platform = if ext.eq_ignore_ascii_case("t64") {
                    Some(Platform::new(Platform::C64))
                } else {
                    None
                };
                suggested_platform = prg_data.suggested_platform.or(default_platform);
                cursor_start = prg_data.suggested_entry_point;
            } else if ext.eq_ignore_ascii_case("crt") {
                let (origin, raw_data) = crate::parser::crt::parse_crt(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse CRT: {e}"))?;
                self.origin = Addr(origin);
                self.raw_data = raw_data;
            } else if ext.eq_ignore_ascii_case("vsf") {
                let vsf_data = crate::parser::vice_vsf::parse_vsf(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to parse VSF: {e}"))?;
                self.origin = Addr::ZERO;
                self.raw_data = vsf_data.memory;
                cursor_start = vsf_data.start_address;
                suggested_platform = match vsf_data.machine_name.as_str() {
                    "C64" => Some(Platform::new(Platform::C64)),
                    "C128" => Some(Platform::new(Platform::C128)),
                    "VIC20" => Some(Platform::new(Platform::VIC20)),
                    "PET" => Some(Platform::new(Platform::PET)),
                    "PLUS4" => Some(Platform::new(Platform::PLUS4)),
                    _ => None,
                };
            } else if ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("raw") {
                self.origin = Addr::ZERO; // Default for .bin
                self.raw_data = data;
            } else {
                return Err(anyhow::anyhow!(
                    "Unsupported file extension: .{ext}\nSupported extensions: .prg, .crt, .vsf, .t64, .d64, .d71, .d81, .bin, .raw, .dis65, .regen2000proj"
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "File has no extension.\nSupported extensions: .prg, .crt, .vsf, .t64, .d64, .d71, .d81, .bin, .raw, .regen2000proj"
            ));
        }

        let initial_block_type = if self.system_config.default_is_unexplored {
            BlockType::Undefined
        } else {
            BlockType::Code
        };
        self.block_types = vec![initial_block_type; self.raw_data.len()];
        self.undo_stack = crate::commands::UndoStack::new();
        self.last_saved_pointer = 0;

        self.load_system_assets();
        self.disassemble();
        self.load_system_assets();
        self.disassemble();

        if self.settings.auto_analyze {
            self.perform_analysis();
        }

        Ok(LoadedProjectData {
            cursor_address: cursor_start.map(Addr),
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
            suggested_entry_point: cursor_start.map(Addr),
            suggested_platform,
        })
    }

    pub fn load_binary(
        &mut self,
        origin: Addr,
        data: Vec<u8>,
    ) -> anyhow::Result<LoadedProjectData> {
        self.origin = origin;
        self.raw_data = data;
        let initial_block_type = if self.system_config.default_is_unexplored {
            BlockType::Undefined
        } else {
            BlockType::Code
        };
        self.block_types = vec![initial_block_type; self.raw_data.len()];
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

        if self.settings.auto_analyze {
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
            suggested_entry_point: None,
            suggested_platform: None,
        })
    }

    pub(super) fn check_entropy(&self) -> Option<f32> {
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

        // Reject project files from newer versions we don't understand
        if project.version > PROJECT_FORMAT_VERSION {
            return Err(anyhow::anyhow!(
                "Project was saved by a newer version (format v{}). \
                 This build only supports up to format v{}. \
                 Please update Regenerator 2000.",
                project.version,
                PROJECT_FORMAT_VERSION
            ));
        }

        self.project_path = Some(path);
        self.origin = project.origin;

        // Decode raw data
        self.raw_data = decode_raw_data_from_base64(&project.raw_data)?;

        // Expand address types and collapsed blocks
        let (block_types, collapsed_ranges) = expand_blocks(&project.blocks, self.raw_data.len());

        self.scopes = project.scopes;

        self.block_types = block_types;
        self.labels = project.labels;
        self.user_side_comments = project.user_side_comments;
        self.user_line_comments = project.user_line_comments;
        self.immediate_value_formats = project.immediate_value_formats;
        self.bookmarks = project.bookmarks;
        self.settings = project.settings;

        // Migration for legacy platform names
        match self.settings.platform.as_str() {
            "Commodore64" => self.settings.platform = Platform::new(Platform::C64),
            "Commodore128" => self.settings.platform = Platform::new(Platform::C128),
            "Commodore1541" => self.settings.platform = Platform::new(Platform::C1541),
            "CommodorePET20" => self.settings.platform = Platform::new(Platform::PET20),
            "CommodorePET40" => self.settings.platform = Platform::new(Platform::PET),
            "CommodorePlus4" => self.settings.platform = Platform::new(Platform::PLUS4),
            "CommodoreVIC20" => self.settings.platform = Platform::new(Platform::VIC20),
            _ => {}
        }

        self.splitters = project.splitters;
        self.last_import_labels_path = None;
        self.last_export_labels_filename = None;
        self.last_save_as_filename = None;
        self.last_export_asm_filename = None;

        self.load_system_assets();

        // Perform analysis to regenerate autogenerated labels
        if self.settings.auto_analyze {
            let result = crate::analyzer::analyze(self);
            self.labels = result.labels;
            self.cross_refs = result.cross_refs;
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
            suggested_entry_point: None,
            suggested_platform: None,
        })
    }

    pub fn load_dis65_project(&mut self, path: PathBuf) -> anyhow::Result<LoadedProjectData> {
        let content = std::fs::read_to_string(&path)?;
        let project = crate::parser::dis65::parse_dis65(&content)?;

        let mut binary_path = path.clone();
        binary_path.set_extension(""); // Strip .dis65
        if !binary_path.exists() {
            return Err(anyhow::anyhow!("Binary file not found next to .dis65"));
        }

        let binary_data = std::fs::read(&binary_path)?;
        let calculated_crc = crate::utils::calculate_crc32(&binary_data);
        if calculated_crc != project.file_data_crc32 {
            return Err(anyhow::anyhow!(
                "CRC32 mismatch: expected {}, got {}",
                project.file_data_crc32,
                calculated_crc
            ));
        }

        self.raw_data = binary_data;

        if let Some(first_map) = project.address_map.first() {
            self.origin = Addr(first_map.addr);
        } else {
            self.origin = Addr::ZERO;
        }

        let (block_types, seeds) =
            project.to_block_types_and_seeds(self.raw_data.len(), self.origin);
        self.block_types = block_types;

        for seed_addr in seeds {
            let ranges = crate::analyzer::flow_analyze(self, seed_addr);
            for range in ranges {
                for i in range.start..range.end {
                    if i < self.block_types.len() {
                        self.block_types[i] = BlockType::Code;
                    }
                }
            }
        }

        let mut candidates = std::collections::BTreeSet::new();
        let mut trusted_locals = std::collections::BTreeSet::new();

        for (offset_str, label_entry) in project.user_labels {
            if let Ok(offset) = offset_str.parse::<usize>()
                && offset < self.raw_data.len()
            {
                let addr = Addr(label_entry.value);
                if label_entry.label_type == "LocalOrGlobalAddr" {
                    candidates.insert(addr);
                } else if label_entry.label_type == "NonUniqueLocalAddr" {
                    trusted_locals.insert(addr);
                }

                self.labels
                    .entry(addr)
                    .or_default()
                    .push(super::project::Label {
                        name: label_entry.label,
                        kind: LabelKind::User,
                        label_type: if label_entry.label_type == "Subroutine" {
                            LabelType::Subroutine
                        } else if label_entry.label_type == "NonUniqueLocalAddr"
                            || label_entry.label_type == "LocalOrGlobalAddr"
                        {
                            LabelType::LocalUserDefined
                        } else {
                            LabelType::UserDefined
                        },
                    });
            }
        }

        for (offset_str, comment) in project.comments {
            if let Ok(offset) = offset_str.parse::<usize>()
                && offset < self.raw_data.len()
            {
                let addr = self.origin + offset as u16;
                self.user_side_comments.insert(addr, comment);
            }
        }

        for (offset_str, long_comment) in project.long_comments {
            if let Ok(offset) = offset_str.parse::<usize>()
                && offset < self.raw_data.len()
            {
                let addr = self.origin + offset as u16;
                self.user_line_comments.insert(addr, long_comment.text);
            }
        }

        self.undo_stack = crate::commands::UndoStack::new();
        self.last_saved_pointer = 0;

        self.load_system_assets();
        self.disassemble();
        self.load_system_assets();
        self.disassemble();

        self.perform_analysis(); // We need xrefs first!

        // === LocalOrGlobalAddr Heuristic (Iterative Fixed-Point) ===
        // Always run the Candidate Heuristic for `.dis65` loads to resolve LocalOrGlobalAddr candidates.
        let mut must_be_local = trusted_locals.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let mut newbies = Vec::new();
            for &l in &must_be_local {
                if let Some(refs) = self.cross_refs.get(&l) {
                    for &r in refs {
                        for &c in &candidates {
                            if !must_be_local.contains(&c)
                                && !newbies.contains(&c)
                                && ((r < c && c < l) || (l < c && c < r))
                            {
                                newbies.push(c);
                                changed = true;
                            }
                        }
                    }
                }
            }
            for c in newbies {
                must_be_local.insert(c);
            }
        }

        // Demote candidates that are not in must_be_local
        for c in candidates {
            if !must_be_local.contains(&c)
                && let Some(labels) = self.labels.get_mut(&c)
            {
                for label in labels {
                    if label.label_type == LabelType::LocalUserDefined {
                        label.label_type = LabelType::UserDefined; // Demote to Global
                    }
                }
            }
        }

        self.disassemble();

        Ok(LoadedProjectData {
            cursor_address: Some(self.origin),
            hex_dump_cursor_address: None,
            sprites_cursor_address: None,
            right_pane_visible: None,
            charset_cursor_address: None,
            bitmap_cursor_address: None,
            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            bitmap_multicolor_mode: Some(false),
            hexdump_view_mode: HexdumpViewMode::default(),
            blocks_view_cursor: None,
            entropy_warning: self.check_entropy(),
            suggested_entry_point: None,
            suggested_platform: None,
        })
    }

    pub fn save_project(
        &mut self,
        ctx: ProjectSaveContext,
        update_global_config: bool,
    ) -> anyhow::Result<()> {
        if let Some(path) = &self.project_path {
            let project = ProjectState {
                version: PROJECT_FORMAT_VERSION,
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
                scopes: self.scopes.clone(),
            };
            let data = serde_json::to_string_pretty(&project)?;
            std::fs::write(path, data)?;
            self.last_saved_pointer = self.undo_stack.get_pointer();

            if update_global_config {
                let abs_path = std::fs::canonicalize(path).unwrap_or(path.clone());
                self.system_config.last_project_path = Some(abs_path.clone());
                self.system_config.add_recent_project(abs_path);
                let _ = self.system_config.save();
            }

            Ok(())
        } else {
            Err(anyhow::anyhow!("No project path set"))
        }
    }

    pub fn import_vice_labels(&mut self, path: PathBuf) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(path)?;
        let parsed = crate::parser::vice_lbl::parse_vice_labels(&content)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        let mut new_labels_vec = Vec::new();
        let mut old_labels_map = BTreeMap::new();

        for (raw_addr, name) in parsed {
            let addr = Addr(raw_addr);
            let label = super::project::Label {
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
            new_labels: new_labels_vec.clone(),
            old_labels: old_labels_map,
        };
        command.apply(self);
        self.push_command(command);
        self.disassemble();

        if self.settings.auto_analyze {
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
                    export_list.push((addr.0, label.name.clone()));
                }
            }
        }
        let content = crate::parser::vice_lbl::generate_vice_labels(&export_list);
        std::fs::write(&path, content)?;
        Ok(format!("Labels exported to {path:?}"))
    }
}

#[cfg(test)]
mod load_file_tests {
    use super::super::app_state::AppState;
    use super::super::types::*;
    use std::io::Write;

    #[test]
    fn test_load_file_clears_state() {
        let mut app_state = AppState::new();

        // Set some state that should be cleared on load
        app_state
            .user_side_comments
            .insert(Addr(0x1000), "test".to_string());
        app_state.bookmarks.insert(Addr(0x1000), "bm".to_string());
        app_state.splitters.insert(Addr(0x2000));
        app_state.collapsed_blocks.push((0, 10));
        app_state.last_import_labels_path = Some(std::path::PathBuf::from("/tmp/test.lbl"));
        app_state.last_export_labels_filename = Some("test".to_string());
        app_state.last_save_as_filename = Some("test".to_string());
        app_state.last_export_asm_filename = Some("test".to_string());

        // Create a temp file with .prg extension
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_clear_state.prg");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            // Write load address $0801 + some data
            f.write_all(&[0x01, 0x08, 0xA9, 0x00]).unwrap();
        }

        let result = app_state.load_file(file_path.clone());
        assert!(result.is_ok());

        // All should be cleared
        assert!(app_state.user_side_comments.is_empty());
        assert!(app_state.bookmarks.is_empty());
        assert!(app_state.splitters.is_empty());
        assert!(app_state.collapsed_blocks.is_empty());
        assert!(app_state.last_import_labels_path.is_none());
        assert!(app_state.last_export_labels_filename.is_none());
        assert!(app_state.last_save_as_filename.is_none());
        assert!(app_state.last_export_asm_filename.is_none());

        // Cleanup
        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn test_load_file_suggests_platform() {
        let mut app_state = AppState::new();
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_platform.prg");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            // Write load address $0801 + BASIC line: 10 SYS 2061
            f.write_all(&[
                0x01, 0x08, // Load address $0801
                0x0B, 0x08, // Next line pointer
                0x0A, 0x00, // Line number 10
                0x9E, // SYS token
                0x32, 0x30, 0x36, 0x31, // "2061"
                0x00, // Terminator
            ])
            .unwrap();
        }

        let result = app_state.load_file(file_path.clone());
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.suggested_platform, Some("Commodore 64".to_string()));
        assert_eq!(data.suggested_entry_point, Some(Addr(2061)));

        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn test_load_file_vsf_suggests_platform() {
        let mut app_state = AppState::new();
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_platform.vsf");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();

            let mut data = Vec::new();
            // Magic
            data.extend_from_slice(b"VICE Snapshot File\x1a");
            // Major/Minor
            data.push(0);
            data.push(0);
            // Machine Name "C64" + padding
            data.extend_from_slice(b"C64");
            data.extend_from_slice(&[0u8; 13]);

            // Module "C64MEM"
            let mod_name = b"C64MEM";
            data.extend_from_slice(mod_name);
            data.extend_from_slice(&[0u8; 10]); // padding to 16
            data.push(0); // Major
            data.push(0); // Minor
            let data_size = 4 + 65536;
            let total_size = 22 + data_size;
            data.extend_from_slice(&(total_size as u32).to_le_bytes());
            data.push(0x37); // CPUDATA
            data.push(0x2F); // CPUDIR
            data.push(0); // EXROM
            data.push(0); // GAME
            let ram = vec![0xEA; 65536];
            data.extend_from_slice(&ram);

            f.write_all(&data).unwrap();
        }

        let result = app_state.load_file(file_path.clone());
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.suggested_platform, Some("Commodore 64".to_string()));

        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn test_load_file_unsupported_extension() {
        let mut app_state = AppState::new();

        let dir = std::env::temp_dir();
        let file_path = dir.join("test_file.xyz");
        std::fs::write(&file_path, b"dummy").unwrap();

        let result = app_state.load_file(file_path.clone());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unsupported file extension"));

        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn test_load_file_no_extension() {
        let mut app_state = AppState::new();

        let dir = std::env::temp_dir();
        let file_path = dir.join("test_file_no_ext");
        std::fs::write(&file_path, b"dummy").unwrap();

        let result = app_state.load_file(file_path.clone());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("has no extension"));

        let _ = std::fs::remove_file(file_path);
    }
}

#[cfg(test)]
mod save_project_tests {
    use super::super::app_state::AppState;
    use super::super::project::Label;
    use super::super::types::{Addr, HexdumpViewMode, LabelKind, LabelType};
    use std::io::Write;

    #[test]
    fn test_save_excludes_auto_and_names() {
        let mut app_state = AppState::new();

        // Create a temp .prg file
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_save_filter.prg");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(&[0x01, 0x08, 0xA9, 0x00, 0x60]).unwrap();
        }

        let _ = app_state.load_file(file_path.clone());

        // Add user label
        app_state
            .labels
            .entry(Addr(0x0801))
            .or_default()
            .push(Label {
                name: "my_label".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            });

        // Add auto label
        app_state
            .labels
            .entry(Addr(0x0802))
            .or_default()
            .push(Label {
                name: "a0802".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Subroutine,
            });

        // Add system label
        app_state
            .labels
            .entry(Addr(0x0001))
            .or_default()
            .push(Label {
                name: "SYS_LABEL".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Field,
            });

        // Save project
        let project_path = dir.join("test_save_filter.regen2000proj");
        app_state.project_path = Some(project_path.clone());
        let ctx = crate::state::project::ProjectSaveContext {
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
            splitters: std::collections::BTreeSet::new(),
            blocks_view_cursor: None,
            bookmarks: std::collections::BTreeMap::new(),
        };
        let result = app_state.save_project(ctx, false);
        assert!(result.is_ok());

        // Load saved JSON and verify
        let saved_data = std::fs::read_to_string(&project_path).unwrap();
        let saved: crate::state::project::ProjectState = serde_json::from_str(&saved_data).unwrap();

        // Should only have user label at 0x0801
        assert!(saved.labels.contains_key(&Addr(0x0801)));
        assert!(!saved.labels.contains_key(&Addr(0x0802))); // auto excluded
        assert!(!saved.labels.contains_key(&Addr(0x0001))); // system excluded

        let labels_0801 = &saved.labels[&Addr(0x0801)];
        assert_eq!(labels_0801.len(), 1);
        assert_eq!(labels_0801[0].name, "my_label");

        // Cleanup
        let _ = std::fs::remove_file(file_path);
        let _ = std::fs::remove_file(project_path);
    }
}

#[cfg(test)]
mod config_tests {
    use super::super::app_state::AppState;
    use std::io::Write;

    #[test]
    fn test_last_project_path_is_full_path() {
        let mut app_state = AppState::new();

        // Create a temporary .prg file
        let dir = std::env::temp_dir();
        let file_path = dir.join("test_config_path.prg");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(&[0x01, 0x08, 0xA9, 0x00, 0x60]).unwrap();
        }

        // Save as a project
        let _ = app_state.load_file(file_path.clone());
        let project_path = dir.join("test_config_path.regen2000proj");
        app_state.project_path = Some(project_path.clone());
        let ctx = crate::state::project::ProjectSaveContext {
            cursor_address: None,
            hex_dump_cursor_address: None,
            sprites_cursor_address: None,
            right_pane_visible: None,
            charset_cursor_address: None,
            bitmap_cursor_address: None,
            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            bitmap_multicolor_mode: false,
            hexdump_view_mode: crate::state::types::HexdumpViewMode::default(),
            splitters: std::collections::BTreeSet::new(),
            blocks_view_cursor: None,
            bookmarks: std::collections::BTreeMap::new(),
        };
        let _result = app_state.save_project(ctx, true);

        // Verify that last_project_path is an absolute canonical path
        if let Some(lpp) = &app_state.system_config.last_project_path {
            assert!(lpp.is_absolute());
        }

        // Cleanup
        let _ = std::fs::remove_file(file_path);
        let _ = std::fs::remove_file(project_path);
    }
}

#[cfg(test)]
mod vice_label_tests {
    use super::super::app_state::AppState;
    use super::super::project::Label;
    use super::super::types::*;

    #[test]
    fn test_export_vice_labels() {
        use std::path::PathBuf;
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "start".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "loop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        // System label should be ignored
        state.labels.insert(
            Addr(0xFFD2),
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
