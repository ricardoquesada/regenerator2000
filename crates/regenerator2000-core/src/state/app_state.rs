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
    pub system_comments: BTreeMap<Addr, String>,
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
            system_comments: BTreeMap::new(),
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

        // Load comments (conditionally)
        if self.settings.show_system_comments {
            self.system_comments = crate::assets::load_comments(&self.settings.platform)
                .into_iter()
                .map(|(k, v)| (Addr(k), v))
                .collect();
        } else {
            self.system_comments.clear();
        }

        // Load labels
        let system_labels = crate::assets::load_labels(
            &self.settings.platform,
            Some(&self.settings.enabled_features),
        );
        for (addr, label) in system_labels {
            self.labels.entry(Addr(addr)).or_default().push(label);
        }

        // Load excludes
        let excludes = crate::assets::load_excludes(&self.settings.platform);
        self.excluded_addresses = excludes.into_iter().map(Addr).collect();
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
    fn test_perform_analysis_preserves_system_labels() {
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
}
