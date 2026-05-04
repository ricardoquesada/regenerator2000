use super::project::Label;
use super::settings::DocumentSettings;
use super::types::{Addr, BlockType, CachedArrow, ImmediateFormat, LabelKind};
use crate::config::SystemConfig;
use crate::disassembler::{Disassembler, DisassemblyLine};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum BlockItem {
    Block {
        start: Addr,
        end: Addr,
        type_: BlockType,
        collapsed: bool,
    },
    Splitter(Addr),
    Scope {
        start: Addr,
        end: Addr,
        name: Option<String>,
    },
}

pub struct AppState {
    pub file_path: Option<PathBuf>,
    pub project_path: Option<PathBuf>,
    pub export_asm_path: Option<PathBuf>,
    pub export_html_path: Option<PathBuf>,
    pub raw_data: Vec<u8>,
    pub disassembly: Vec<DisassemblyLine>,
    pub cached_arrows: Vec<CachedArrow>,
    pub disassembler: Disassembler,
    pub origin: Addr,

    // Data Conversion State
    pub block_types: Vec<BlockType>,
    pub labels: BTreeMap<Addr, Vec<Label>>,
    pub settings: DocumentSettings,
    pub platform_comments: BTreeMap<Addr, String>,
    pub user_side_comments: BTreeMap<Addr, String>,
    pub user_line_comments: BTreeMap<Addr, String>,
    pub immediate_value_formats: BTreeMap<Addr, ImmediateFormat>,
    pub cross_refs: BTreeMap<Addr, Vec<Addr>>,
    pub bookmarks: BTreeMap<Addr, String>,
    pub scopes: BTreeMap<Addr, Addr>, // start -> end

    pub system_config: SystemConfig,

    pub undo_stack: crate::commands::UndoStack,
    pub last_saved_pointer: usize,
    pub excluded_addresses: std::collections::HashSet<Addr>,
    /// Per-project user-defined excluded addresses, always applied during analysis
    /// regardless of the `exclude_well_known_labels` document setting.
    pub user_excluded_addresses: BTreeSet<Addr>,
    pub collapsed_blocks: Vec<(usize, usize)>,
    pub splitters: BTreeSet<Addr>,
    pub last_import_labels_path: Option<PathBuf>,
    pub last_export_labels_filename: Option<String>,
    pub last_save_as_filename: Option<String>,
    pub last_export_asm_filename: Option<String>,
    pub last_export_html_filename: Option<String>,
    pub vice_state: crate::vice::ViceState,
    pub vice_client: Option<crate::vice::ViceClient>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        // Use default config with a throwaway save path so that tests never
        // read or overwrite the real user config file.  Production code in
        // main.rs replaces system_config with SystemConfig::load() immediately
        // after construction, which resets config_path_override to None and
        // therefore uses the real config directory.
        let default_config = SystemConfig {
            config_path_override: Some(
                std::env::temp_dir().join("regenerator2000_test_config.json"),
            ),
            ..Default::default()
        };

        Self {
            file_path: None,
            project_path: None,
            export_asm_path: None,
            export_html_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            cached_arrows: Vec::new(),
            disassembler: Disassembler::new(),
            origin: Addr::ZERO,
            block_types: Vec::new(),
            labels: BTreeMap::new(),
            settings: DocumentSettings::default(),
            platform_comments: BTreeMap::new(),
            user_side_comments: BTreeMap::new(),
            user_line_comments: BTreeMap::new(),
            immediate_value_formats: BTreeMap::new(),
            cross_refs: BTreeMap::new(),
            bookmarks: BTreeMap::new(),
            scopes: BTreeMap::new(),
            system_config: default_config,
            undo_stack: crate::commands::UndoStack::new(),
            last_saved_pointer: 0,
            excluded_addresses: std::collections::HashSet::new(),
            user_excluded_addresses: BTreeSet::new(),
            collapsed_blocks: Vec::new(),
            splitters: BTreeSet::new(),
            last_import_labels_path: None,
            last_export_labels_filename: None,
            last_save_as_filename: None,
            last_export_asm_filename: None,
            last_export_html_filename: None,
            vice_state: crate::vice::ViceState::new(),
            vice_client: None,
        }
    }

    pub fn load_system_assets(&mut self) {
        // Clear existing system labels
        for labels in self.labels.values_mut() {
            labels.retain(|l| l.kind != LabelKind::System);
        }
        // Remove empty entries
        self.labels.retain(|_, v| !v.is_empty());

        // Load excludes
        let excludes = crate::assets::load_excludes(&self.settings.platform);

        if self.settings.exclude_well_known_labels {
            self.excluded_addresses = excludes.iter().map(|&a| Addr(a)).collect();
        } else {
            self.excluded_addresses.clear();
        }

        // Always apply user-defined per-project excluded addresses,
        // independent of the `exclude_well_known_labels` setting.
        self.excluded_addresses
            .extend(self.user_excluded_addresses.iter().copied());

        // Load comments (conditionally)
        if self.settings.show_platform_comments {
            let comments = crate::assets::load_comments(&self.settings.platform);
            self.platform_comments = comments.into_iter().map(|(k, v)| (Addr(k), v)).collect();
        } else {
            self.platform_comments.clear();
        }

        // Load labels
        let mut platform_labels = crate::assets::load_labels(
            &self.settings.platform,
            Some(&self.settings.enabled_features),
        );

        if self.settings.exclude_well_known_labels {
            platform_labels.retain(|(k, _)| !excludes.contains(k));
        }

        for (addr, label) in platform_labels {
            self.labels.entry(Addr(addr)).or_default().push(label);
        }
    }

    #[must_use]
    pub fn get_formatter(&self) -> Box<dyn crate::disassembler::formatter::Formatter> {
        Disassembler::create_formatter(self.settings.assembler)
    }

    pub fn perform_analysis(&mut self) -> (crate::commands::Command, String) {
        let result = crate::analyzer::analyze(self);

        // Capture old labels (more idiomatic with iterator)
        let old_labels_map = result
            .labels
            .keys()
            .map(|k| (*k, self.labels.get(k).cloned().unwrap_or_default()))
            .collect();

        // Also capture old cross_refs
        let old_cross_refs = self.cross_refs.clone();

        let command = crate::commands::Command::SetAnalysisData {
            labels: result.labels,
            cross_refs: result.cross_refs,
            old_labels: old_labels_map,
            old_cross_refs,
        };
        command.apply(self);
        self.disassemble();
        (command, "Analysis Complete".to_string())
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

    #[must_use]
    pub fn is_external(&self, addr: Addr) -> bool {
        let len = self.raw_data.len();
        let end = self.origin.wrapping_add(len as u16);
        if self.origin < end {
            addr < self.origin || addr >= end
        } else {
            !(addr >= self.origin || addr < end)
        }
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.undo_stack.get_pointer() != self.last_saved_pointer
    }

    #[must_use]
    pub fn is_virtual_splitter(&self, addr: Addr) -> bool {
        if self.splitters.contains(&addr) {
            return true;
        }
        if self.scopes.contains_key(&addr) {
            return true;
        }
        for &end in self.scopes.values() {
            if end.wrapping_add(1) == addr {
                return true;
            }
        }
        false
    }

    pub fn push_command(&mut self, command: crate::commands::Command) {
        if self.undo_stack.get_pointer() < self.last_saved_pointer {
            self.last_saved_pointer = usize::MAX;
        }
        self.undo_stack.push(command);
    }

    /// Creates a [`Command::SetLabel`] for a user-defined label at the given address,
    /// validating that no other address already uses the same name.
    ///
    /// If `name` is empty the returned command will **remove** any existing label.
    /// When `is_local` is true the label receives [`LabelType::LocalUserDefined`];
    /// otherwise the existing label type is preserved (useful when renaming an
    /// external/system label) or defaults to [`LabelType::UserDefined`].
    ///
    /// # Errors
    ///
    /// Returns `Err` with a human-readable message when a label with the same
    /// name already exists at a different address.
    pub fn create_set_user_label_command(
        &self,
        address: Addr,
        name: &str,
        is_local: bool,
    ) -> Result<crate::commands::Command, String> {
        let label_name = name.trim();
        let old_label = self.labels.get(&address).cloned();

        if label_name.is_empty() {
            return Ok(crate::commands::Command::SetLabel {
                address,
                new_label: None,
                old_label,
            });
        }

        // Reject duplicate label names at other addresses
        let exists = self.labels.iter().any(|(addr, label_vec)| {
            *addr != address && label_vec.iter().any(|l| l.name == label_name)
        });
        if exists {
            return Err(format!("Label '{label_name}' already exists"));
        }

        let mut new_label_vec = old_label.clone().unwrap_or_default();

        // Preserve the existing label_type when renaming so that external labels
        // (e.g. ZeroPageAbsoluteAddress) remain in their display category.
        // Only fall back to UserDefined when there is no prior label to inherit from.
        let inherited_type = new_label_vec
            .first()
            .map(|l| l.label_type)
            .unwrap_or(super::LabelType::UserDefined);

        let new_label_entry = Label {
            name: label_name.to_string(),
            kind: LabelKind::User,
            label_type: if is_local {
                super::LabelType::LocalUserDefined
            } else {
                inherited_type
            },
        };

        if new_label_vec.is_empty() {
            new_label_vec.push(new_label_entry);
        } else {
            new_label_vec[0] = new_label_entry;
        }

        Ok(crate::commands::Command::SetLabel {
            address,
            new_label: Some(new_label_vec),
            old_label,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::LabelType;
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = super::super::settings::DocumentSettings::default();
        assert_eq!(settings.max_arrow_columns, 6);
        assert!(settings.brk_single_byte);
        assert!(!settings.patch_brk);
    }

    #[test]
    fn test_perform_analysis_preserves_user_labels() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        app_state.raw_data = vec![0x20, 0x10, 0xC0, 0xEA, 0xEA, 0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 7];
        app_state.disassemble();

        // Add a user label
        app_state
            .labels
            .entry(Addr(0xC000))
            .or_default()
            .push(Label {
                name: "my_routine".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            });

        app_state.perform_analysis();

        // User label should still be there
        let labels_at_c000 = app_state.labels.get(&Addr(0xC000));
        assert!(labels_at_c000.is_some());
        let has_user = labels_at_c000
            .unwrap()
            .iter()
            .any(|l| l.name == "my_routine" && l.kind == LabelKind::User);
        assert!(has_user, "User label 'my_routine' should be preserved");
    }

    #[test]
    fn test_perform_analysis_preserves_platform_labels() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        app_state.raw_data = vec![0xA9, 0x00, 0x85, 0xFB, 0x60];
        app_state.block_types = vec![BlockType::Code; 5];
        app_state.disassemble();

        // Add a system label (simulating loaded from assets)
        app_state
            .labels
            .entry(Addr(0x00FB))
            .or_default()
            .push(Label {
                name: "SYS_LABEL".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Field,
            });

        app_state.perform_analysis();

        // System label should still be preserved
        let labels_at_fb = app_state.labels.get(&Addr(0x00FB));
        assert!(labels_at_fb.is_some());
        let has_system = labels_at_fb
            .unwrap()
            .iter()
            .any(|l| l.name == "SYS_LABEL" && l.kind == LabelKind::System);
        assert!(has_system, "System label should be preserved");
    }

    #[test]
    fn test_load_system_assets_respects_exclude_labels() {
        let mut app_state = AppState::new();
        app_state.settings.platform = crate::state::Platform::from("Commodore 64".to_string());
        app_state.settings.exclude_well_known_labels = true;

        app_state.load_system_assets();

        let d020 = Addr(0xD020);
        assert!(app_state.excluded_addresses.contains(&d020));
        assert!(
            !app_state.labels.contains_key(&d020),
            "Label for D020 should be excluded"
        );
    }
}
