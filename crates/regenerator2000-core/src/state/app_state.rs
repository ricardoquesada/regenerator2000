//! Central, UI-agnostic application state and core reverse-engineering domain model.
//!
//! This module defines [`AppState`], [`BlockItem`], and [`FileInfo`]. Both the TUI and MCP
//! server share [`AppState`] methods for operand extraction, [`SymbolicOffset`] parsing and
//! suggestion, label checking, and [`Command::Batch`](crate::commands::Command::Batch) undo history.

use super::annotations::AnnotationManager;
use super::project::Label;
use super::settings::DocumentSettings;
use super::types::{
    Addr, BlockType, CachedArrow, CommentKind, EnumDefinition, ImmediateFormat, LabelKind,
    LabelType, OffsetParentheses, OffsetRadix, OperandTargetSite, SymbolicOffset,
};
use crate::config::SystemConfig;
use crate::cpu::AddressingMode;
use crate::disassembler::{Disassembler, DisassemblyLine};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[cfg(test)]
thread_local! {
    static ANALYSIS_PASS_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// An item displayed in the Blocks View side pane, produced by [`AppState::get_blocks_view_items`].
#[derive(Debug, Clone, PartialEq)]
pub enum BlockItem {
    /// A contiguous memory region sharing a single [`BlockType`] and collapsed state.
    Block {
        /// Inclusive start [`Addr`] of the block.
        start: Addr,
        /// Inclusive end [`Addr`] of the block.
        end: Addr,
        /// Interpretation [`BlockType`] applied to the bytes in `start..=end`.
        type_: BlockType,
        /// Whether the block is currently collapsed into a single summary line in the disassembly view.
        collapsed: bool,
    },
    /// A user-defined manual splitter at the specified [`Addr`].
    Splitter(Addr),
    /// A lexical symbol scope spanning the inclusive range `start..=end`.
    Scope {
        /// Inclusive start [`Addr`] of the lexical scope.
        start: Addr,
        /// Inclusive end [`Addr`] of the lexical scope.
        end: Addr,
        /// Optional primary label name at `start`, if defined.
        name: Option<String>,
    },
}

/// Information about the currently loaded binary file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileInfo {
    /// Start address (origin) in memory.
    pub start_addr: Addr,
    /// End address in memory (inclusive).
    pub end_addr: Addr,
    /// Size of raw data in bytes.
    pub size: usize,
    /// Entry point address, if detected.
    pub entry_point: Option<Addr>,
    /// Calculated Shannon entropy (0.0 to 8.0).
    pub entropy: f32,
    /// Whether the file is detected as packed.
    pub is_packed: bool,
    /// Name of the detected packer, if any.
    pub packer_name: Option<&'static str>,
}

/// Central, UI-agnostic state for a loaded binary and reverse-engineering project.
///
/// Both the TUI and MCP server consume and mutate `AppState` through shared methods
/// and undoable [`Command`](crate::commands::Command)s.
pub struct AppState {
    /// Project schema version (see [`PROJECT_FORMAT_VERSION`](super::project::PROJECT_FORMAT_VERSION)).
    pub schema_version: u32,
    /// Path to the source binary or disk/tape/cartridge image file, if loaded from disk.
    pub file_path: Option<PathBuf>,
    /// Path to the `.regen2000proj` project file, if saved or loaded.
    pub project_path: Option<PathBuf>,
    /// Last used destination path when exporting assembly source (`.asm`).
    pub export_asm_path: Option<PathBuf>,
    /// Last used destination path when exporting HTML disassembly (`.html`).
    pub export_html_path: Option<PathBuf>,
    /// Raw binary payload bytes loaded at [`AppState::origin`].
    pub raw_data: Vec<u8>,
    /// Cached decoded [`DisassemblyLine`]s produced by the most recent [`AppState::disassemble`] pass.
    pub disassembly: Vec<DisassemblyLine>,
    /// Cached control-flow branch/jump arrows aligned with [`AppState::disassembly`].
    pub cached_arrows: Vec<CachedArrow>,
    /// Stateless 6502 disassembler engine instance.
    pub disassembler: Disassembler,
    /// Load address (origin) of [`AppState::raw_data`] in 16-bit 6502 memory.
    pub origin: Addr,
    /// Detected or user-configured entry point [`Addr`], if any.
    pub entry_point: Option<Addr>,
    /// Cached Shannon entropy (`0.0..=8.0`) of [`AppState::raw_data`].
    pub entropy: Option<f32>,

    // Data Conversion State
    /// Per-byte [`BlockType`] classification parallel to [`AppState::raw_data`].
    pub block_types: Vec<BlockType>,
    /// Address-keyed map of user, auto-analyzed, and system [`Label`]s.
    pub labels: BTreeMap<Addr, Vec<Label>>,
    /// Address-keyed map of feature-enabled platform system [`Label`]s from assets.
    pub system_labels: BTreeMap<Addr, Vec<Label>>,
    /// Per-document configuration settings (target [`System`](super::types::System), [`Assembler`](super::types::Assembler), etc.).
    pub settings: DocumentSettings,
    /// Sparse address-keyed annotations (comments, immediate formats, bookmarks, enums, Tier 1/Tier 2
    /// symbolic offsets, and indexed lexical scopes).
    pub annotations: AnnotationManager,
    /// Cross-reference map from target [`Addr`] to typed [`XRef`](super::types::XRef)s.
    pub cross_refs: BTreeMap<Addr, Vec<super::types::XRef>>,

    // Enums State
    /// Project-local enum definitions keyed by enum name.
    pub enums: BTreeMap<String, EnumDefinition>,
    /// User global enum definitions loaded from user configuration.
    pub user_global_enums: BTreeMap<String, EnumDefinition>,
    /// Built-in platform enum definitions loaded from system assets.
    pub builtin_enums: BTreeMap<String, EnumDefinition>,

    /// Global system/application configuration.
    pub system_config: SystemConfig,

    /// Undo/redo command history stack.
    pub undo_stack: crate::commands::UndoStack,
    /// Undo stack pointer recorded at the last project save, used by [`AppState::is_dirty`].
    pub last_saved_pointer: usize,
    /// Effective set of addresses excluded from automatic label generation during analysis.
    pub excluded_addresses: std::collections::HashSet<Addr>,
    /// Per-project user-defined excluded addresses, always applied during analysis
    /// regardless of the `exclude_well_known_labels` document setting.
    pub user_excluded_addresses: BTreeSet<Addr>,
    /// Inclusive `(start_offset, end_offset)` byte-offset ranges of collapsed blocks relative to [`AppState::origin`].
    pub collapsed_blocks: Vec<(usize, usize)>,
    /// Addresses where manual block splitters (`{splitter}`) have been placed.
    pub splitters: BTreeSet<Addr>,
    /// Last directory or file path used when importing VICE labels.
    pub last_import_labels_path: Option<PathBuf>,
    /// Last filename used when exporting VICE labels.
    pub last_export_labels_filename: Option<String>,
    /// Last filename used in the Save Project As dialog.
    pub last_save_as_filename: Option<String>,
    /// Last filename used in the Export ASM dialog.
    pub last_export_asm_filename: Option<String>,
    /// Last filename used in the Export HTML dialog.
    pub last_export_html_filename: Option<String>,
    /// Live VICE binary monitor debugger state (registers, breakpoints, memory snapshot).
    pub vice_state: crate::vice::ViceState,
    /// Active VICE binary monitor network client, if connected.
    pub vice_client: Option<crate::vice::ViceClient>,
    /// Whether an HTTP or stdio MCP server is currently attached to this session.
    pub mcp_server_running: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Creates a new [`AppState`] with default document settings and a temporary configuration path.
    ///
    /// The temporary path ensures that tests never overwrite the user's configuration file.
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
            schema_version: super::project::PROJECT_FORMAT_VERSION,
            file_path: None,
            project_path: None,
            export_asm_path: None,
            export_html_path: None,
            raw_data: Vec::new(),
            disassembly: Vec::new(),
            cached_arrows: Vec::new(),
            disassembler: Disassembler::new(),
            origin: Addr::ZERO,
            entry_point: None,
            entropy: None,
            block_types: Vec::new(),
            labels: BTreeMap::new(),
            system_labels: BTreeMap::new(),
            settings: DocumentSettings::default(),
            annotations: AnnotationManager::default(),
            cross_refs: BTreeMap::new(),
            enums: BTreeMap::new(),
            user_global_enums: BTreeMap::new(),
            builtin_enums: BTreeMap::new(),
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
            mcp_server_running: false,
        }
    }

    /// Reloads platform system labels, comments, and excluded addresses for [`DocumentSettings::system`].
    ///
    /// Preserves all user-defined labels and annotations.
    pub fn load_system_assets(&mut self) {
        // Clear existing system labels
        for labels in self.labels.values_mut() {
            labels.retain(|l| l.kind != LabelKind::System);
        }
        // Remove empty entries
        self.labels.retain(|_, v| !v.is_empty());

        // Load excludes
        let excludes = crate::assets::load_excludes(self.settings.system.as_str());

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
        if self.settings.show_system_comments {
            let comments = crate::assets::load_comments(self.settings.system.as_str());
            for (k, v) in comments {
                self.annotations
                    .update(Addr(k), |e| e.system_comment = Some(v));
            }
        } else {
            let addrs: Vec<Addr> = self
                .annotations
                .iter()
                .filter(|(_, e)| e.system_comment.is_some())
                .map(|(a, _)| a)
                .collect();
            for a in addrs {
                self.annotations.update(a, |e| e.system_comment = None);
            }
        }

        // Keep all feature-enabled platform labels in `system_labels` for analysis and offset lookup.
        let mut platform_labels = crate::assets::load_labels(
            self.settings.system.as_str(),
            Some(&self.settings.enabled_features),
        );
        self.system_labels.clear();
        for (addr, label) in &platform_labels {
            self.system_labels
                .entry(Addr(*addr))
                .or_default()
                .push(label.clone());
        }

        if self.settings.exclude_well_known_labels {
            platform_labels.retain(|(k, _)| !excludes.contains(k));
        }

        // Also exclude user-excluded addresses from system labels.
        if !self.user_excluded_addresses.is_empty() {
            platform_labels.retain(|(k, _)| !self.user_excluded_addresses.contains(&Addr(*k)));
        }

        for (addr, label) in platform_labels {
            self.labels.entry(Addr(addr)).or_default().push(label);
        }
    }

    /// Creates a syntax [`Formatter`](crate::disassembler::formatter::Formatter) for [`DocumentSettings::assembler`].
    #[must_use]
    pub fn get_formatter(&self) -> Box<dyn crate::disassembler::formatter::Formatter> {
        Disassembler::create_formatter(self.settings.assembler)
    }

    /// Runs auto-analysis over the loaded binary and refreshes the disassembly.
    ///
    /// Applies [`Command::SetAnalysisData`](crate::commands::Command::SetAnalysisData) to update
    /// auto-generated labels and cross-references. Returns the applied command and a status message.
    pub fn perform_analysis(&mut self) -> (crate::commands::Command, String) {
        #[cfg(test)]
        ANALYSIS_PASS_COUNT.with(|c| c.set(c.get().saturating_add(1)));

        let result = crate::analyzer::analyze(self);
        let old_labels_map = self.labels.clone();
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

    /// Removes all auto-generated labels and cross-references, records an undoable
    /// [`Command::SetAnalysisData`](crate::commands::Command::SetAnalysisData), and refreshes the disassembly.
    pub fn clear_auto_analysis(&mut self) {
        let mut new_labels = self.labels.clone();
        for labels_vec in new_labels.values_mut() {
            labels_vec.retain(|l| l.kind != crate::state::LabelKind::Auto);
        }
        new_labels.retain(|_, v| !v.is_empty());

        let command = crate::commands::Command::SetAnalysisData {
            labels: new_labels,
            cross_refs: std::collections::BTreeMap::new(),
            old_labels: self.labels.clone(),
            old_cross_refs: self.cross_refs.clone(),
        };
        command.apply(self);
        self.push_command(command);
        self.disassemble();
    }

    /// Refreshes system assets, auto-analysis, and disassembly after a [`DocumentSettings`] change.
    pub fn apply_settings_change(&mut self, reload_assets: bool, reanalyze: bool) {
        if reload_assets {
            self.load_system_assets();
        }
        if reanalyze && self.settings.auto_analyze {
            let (cmd, _) = self.perform_analysis();
            self.push_command(cmd);
        } else {
            self.disassemble();
        }
    }

    /// Builds the chronological mutation commands to set [`ImmediateFormat`] at `address`
    /// while preserving or clearing Tier 1 `operand_offset` coherently.
    ///
    /// - When `explicit_offset` is `Some(desired)`, normalizes `desired` (`offset != 0`) and keeps
    ///   it only when `new_format` is [`ImmediateFormat::LowByte`] or [`ImmediateFormat::HighByte`]
    ///   whose target satisfies [`SymbolicOffset::matches_target`].
    /// - When `explicit_offset` is `None`, retains any existing `operand_offset` at `address` whose
    ///   target matches `new_format` (`LowByte` / `HighByte`) and clears non-matching stale entries.
    #[must_use]
    pub fn build_set_immediate_format_commands(
        &self,
        address: Addr,
        new_format: Option<ImmediateFormat>,
        explicit_offset: Option<Option<SymbolicOffset>>,
    ) -> Vec<crate::commands::Command> {
        let old_format = self
            .annotations
            .get(address)
            .and_then(|e| e.immediate_format);
        let old_offset = self.annotations.get(address).and_then(|e| e.operand_offset);

        let candidate_offset = explicit_offset.unwrap_or(old_offset);
        let normalized_offset = match new_format {
            Some(ImmediateFormat::LowByte(target) | ImmediateFormat::HighByte(target)) => {
                candidate_offset.filter(|sym| sym.matches_target(target))
            }
            _ => None,
        };

        let mut cmds = Vec::with_capacity(2);
        cmds.push(crate::commands::Command::SetImmediateFormat {
            address,
            new_format,
            old_format,
        });
        if old_offset != normalized_offset {
            cmds.push(crate::commands::Command::SetOperandOffset {
                address,
                new_offset: normalized_offset,
                old_offset,
            });
        }
        cmds
    }

    /// Sets the [`ImmediateFormat`] at `address` and clears any non-matching Tier 1 `operand_offset`.
    ///
    /// Runs auto-analysis when enabled and pushes an undoable [`Command::Batch`](crate::commands::Command::Batch).
    pub fn apply_immediate_format(&mut self, address: Addr, new_format: Option<ImmediateFormat>) {
        let batch = self.build_set_immediate_format_commands(address, new_format, None);
        for cmd in &batch {
            cmd.apply(self);
        }
        self.finalize_symbolic_mutation_batch(batch);
    }

    /// Sets both the [`ImmediateFormat`] and an optional Tier 1 [`SymbolicOffset`] at `address`
    /// in a single chronological undoable [`Command::Batch`](crate::commands::Command::Batch).
    pub fn apply_immediate_format_with_offset(
        &mut self,
        address: Addr,
        new_format: Option<ImmediateFormat>,
        new_offset: Option<SymbolicOffset>,
    ) {
        let batch = self.build_set_immediate_format_commands(address, new_format, Some(new_offset));
        for cmd in &batch {
            cmd.apply(self);
        }
        self.finalize_symbolic_mutation_batch(batch);
    }

    /// Sets or removes a bookmark at `address` and records [`Command::SetBookmark`](crate::commands::Command::SetBookmark) on the undo stack.
    pub fn set_bookmark(&mut self, address: Addr, new_name: Option<String>) {
        let old_name = self
            .annotations
            .get(address)
            .and_then(|e| e.bookmark.clone());
        let command = crate::commands::Command::SetBookmark {
            address,
            new_name,
            old_name,
        };
        command.apply(self);
        self.push_command(command);
    }

    #[cfg(test)]
    pub(crate) fn reset_analysis_pass_count() {
        ANALYSIS_PASS_COUNT.with(|c| c.set(0));
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn analysis_pass_count() -> usize {
        ANALYSIS_PASS_COUNT.with(std::cell::Cell::get)
    }

    #[cfg(test)]
    pub(crate) fn insert_user_labels(&mut self, entries: &[(u16, &str)]) {
        for &(addr, name) in entries {
            self.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::UserDefined,
                }],
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn insert_typed_user_labels(&mut self, entries: &[(u16, &str, LabelType)]) {
        for &(addr, name, label_type) in entries {
            self.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type,
                }],
            );
        }
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn build_64k_symbolic_offset_fixture() -> Self {
        let mut state = Self::new();
        state.origin = Addr::ZERO;
        state.raw_data = vec![0xEA; 65536];
        state.block_types = vec![BlockType::Code; 65536];
        state.block_types[0x4000..0x4200].fill(BlockType::LoHiAddress);
        state.block_types[0x6000..0x6200].fill(BlockType::DataByte);
        for pair_idx in 0..256u16 {
            let rts_target = 0x8000u16 + pair_idx * 16 - 1;
            state.raw_data[0x4000 + usize::from(pair_idx)] = (rts_target & 0xFF) as u8;
            state.raw_data[0x4100 + usize::from(pair_idx)] = (rts_target >> 8) as u8;
        }
        for i in 0..4000u16 {
            let addr = Addr(0x0800u16.wrapping_add(i.wrapping_mul(15)));
            state.labels.entry(addr).or_default().push(Label {
                name: format!("sym_{i:04}"),
                kind: if i % 3 == 0 {
                    LabelKind::User
                } else {
                    LabelKind::Auto
                },
                label_type: LabelType::UserDefined,
            });
        }
        state.insert_user_labels(&[(0x6000, "big_struct")]);
        for s_idx in 0..150u16 {
            let start = Addr(0x1000 + s_idx * 0x0030);
            let end = Addr(0x1000 + s_idx * 0x0030 + 0x0020);
            state.annotations.update(start, |e| e.scope = Some(end));
        }
        state
    }

    /// Undoes the most recent command on [`AppState::undo_stack`] and returns a status message.
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

    /// Redoes the most recently undone command on [`AppState::undo_stack`] and returns a status message.
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

    /// Returns `true` if `addr` lies outside the loaded binary range (`origin..origin + raw_data.len()`).
    #[must_use]
    pub fn is_external(&self, addr: Addr) -> bool {
        addr.offset_from(self.origin) >= self.raw_data.len()
    }

    /// Returns `true` if the project has unsaved modifications since the last save or load.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.undo_stack.get_pointer() != self.last_saved_pointer
    }

    /// Returns the cached entropy value of the binary, or calculates it from raw data if not cached.
    #[must_use]
    pub fn entropy(&self) -> f32 {
        self.entropy
            .unwrap_or_else(|| crate::utils::calculate_entropy(&self.raw_data))
    }

    /// Returns summary information and metadata about the loaded binary.
    #[must_use]
    pub fn file_info(&self) -> FileInfo {
        let size = self.raw_data.len();
        if size == 0 {
            let start_addr = if self.origin == Addr::ZERO {
                Addr(self.settings.system.default_basic_start())
            } else {
                self.origin
            };
            return FileInfo {
                start_addr,
                end_addr: start_addr,
                size: 0,
                entry_point: self.entry_point,
                entropy: self.entropy(),
                is_packed: false,
                packer_name: None,
            };
        }

        let start_addr = self.origin;
        let end_addr = start_addr.wrapping_add(size.saturating_sub(1) as u16);

        let mut mem = vec![0u8; 0x10000];
        let load_start = start_addr.0 as usize;
        let load_end = load_start.saturating_add(size).min(0x10000);
        if load_start < 0x10000 {
            let copy_len = load_end - load_start;
            mem[load_start..load_end].copy_from_slice(&self.raw_data[..copy_len]);
        }

        let load_end_inclusive = (load_start
            .saturating_add(size)
            .min(0x10000)
            .saturating_sub(1)) as u16;
        let image = crate::packers::LoadedImage::new(
            &mem,
            start_addr.0,
            load_end_inclusive,
            &self.settings.system,
        );
        let sig_match = crate::packers::registry::detect(&image);

        let (is_packed, packer_name, entry_point) = if let Some(ref m) = sig_match {
            let info = crate::packers::PackerInfo::resolve(&image, m);
            (
                true,
                Some(info.name),
                info.entry_point.map(Addr).or(self.entry_point),
            )
        } else {
            let sys_ep = self
                .entry_point
                .or_else(|| crate::unpacker::find_sys_address(&mem, start_addr.0).map(Addr));
            (false, None, sys_ep)
        };

        let entropy = self.entropy();

        FileInfo {
            start_addr,
            end_addr,
            size,
            entry_point,
            entropy,
            is_packed,
            packer_name,
        }
    }

    /// Returns `true` if `addr` is a manual splitter or a lexical scope boundary.
    ///
    /// Scope start and exclusive end (`end.wrapping_add(1)`) addresses partition [`BlockType`]
    /// regions without inserting a `{splitter}` pseudo-instruction into the disassembly.
    #[must_use]
    pub fn is_virtual_splitter(&self, addr: Addr) -> bool {
        if self.splitters.is_empty() && self.annotations.scopes().is_empty() {
            return false;
        }
        self.splitters.contains(&addr)
            || self.annotations.is_scope_start(addr)
            || self.annotations.is_scope_end(addr)
    }

    /// Pushes an applied [`Command`](crate::commands::Command) onto [`AppState::undo_stack`].
    ///
    /// Invalidates [`AppState::last_saved_pointer`] if the undo history was rewound past the saved state.
    pub fn push_command(&mut self, command: crate::commands::Command) {
        if self.undo_stack.get_pointer() < self.last_saved_pointer {
            self.last_saved_pointer = usize::MAX;
        }
        self.undo_stack.push(command);
    }

    /// Normalizes and checks a plain [`LabelKind::User`] identifier at `address`.
    ///
    /// Returns `Ok(None)` when `name` is empty or whitespace-only. When `is_local` is `true`,
    /// strips at most one leading local prefix (`'_'`, `'.'`, or `'@'`). Requires the identifier
    /// to match `[A-Za-z_][A-Za-z0-9_]*` and not collide with a label at another address.
    fn normalize_and_validate_plain_user_label<'a>(
        &self,
        address: Addr,
        name: &'a str,
        is_local: bool,
    ) -> Result<Option<&'a str>, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let normalized = if is_local {
            trimmed.strip_prefix(['_', '.', '@']).unwrap_or(trimmed)
        } else {
            trimmed
        };

        let mut chars = normalized.chars();
        let is_valid_ident = chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !is_valid_ident {
            return Err(format!(
                "Invalid label name '{trimmed}' (must start with a letter or '_' and contain only ASCII letters, digits, or '_')"
            ));
        }

        let exists = self.labels.iter().any(|(addr, label_vec)| {
            *addr != address
                && label_vec
                    .iter()
                    .any(|l| l.name == normalized || l.name == trimmed)
        });
        if exists {
            return Err(format!("Label '{normalized}' already exists"));
        }

        Ok(Some(normalized))
    }

    /// Creates a [`SetLabel`](crate::commands::Command::SetLabel) command for a user label at `address`.
    ///
    /// If `name` is empty, the command removes any existing label at `address`.
    /// When `is_local` is `true`, strips an optional single leading local prefix (`'_'`, `'.'`, or
    /// `'@'`) and assigns [`LocalUserDefined`](crate::state::LabelType::LocalUserDefined).
    /// When `is_local` is `false`, keeps the existing [`LabelType`] or defaults to
    /// [`UserDefined`](crate::state::LabelType::UserDefined).
    /// Non-empty plain identifiers must match `[A-Za-z_][A-Za-z0-9_]*`.
    ///
    /// # Errors
    ///
    /// Returns an error message when `name` has invalid assembler identifier syntax or when
    /// another address already uses `name`.
    pub fn create_set_user_label_command(
        &self,
        address: Addr,
        name: &str,
        is_local: bool,
    ) -> Result<crate::commands::Command, String> {
        let old_label = self.labels.get(&address).cloned();
        let Some(label_name) =
            self.normalize_and_validate_plain_user_label(address, name, is_local)?
        else {
            return Ok(crate::commands::Command::SetLabel {
                address,
                new_label: None,
                old_label,
            });
        };

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

    /// Creates a [`SetUserSideComment`](crate::commands::Command::SetUserSideComment) or
    /// [`SetUserLineComment`](crate::commands::Command::SetUserLineComment) command at `address`.
    ///
    /// If `comment` is `None`, empty (`""`), or whitespace-only, `new_comment` is `None` so
    /// [`AnnotationManager`] removes the existing comment.
    #[must_use]
    pub fn create_set_comment_command(
        &self,
        address: Addr,
        comment: Option<&str>,
        kind: CommentKind,
    ) -> crate::commands::Command {
        let new_comment = comment.filter(|s| !s.trim().is_empty()).map(String::from);

        match kind {
            CommentKind::Side => crate::commands::Command::SetUserSideComment {
                address,
                new_comment,
                old_comment: self
                    .annotations
                    .get(address)
                    .and_then(|e| e.user_side_comment.clone()),
            },
            CommentKind::Line => crate::commands::Command::SetUserLineComment {
                address,
                new_comment,
                old_comment: self
                    .annotations
                    .get(address)
                    .and_then(|e| e.user_line_comment.clone()),
            },
        }
    }

    /// Checks whether `name` is valid for a new project enum.
    ///
    /// Ensures `name` is non-empty, uses only ASCII letters, digits, or `'_'`, and does not
    /// collide with local, global, or built-in enums.
    ///
    /// # Errors
    ///
    /// Returns an error message if `name` is empty, has invalid characters, or already exists.
    pub fn validate_new_enum_name(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Enum name cannot be empty".to_string());
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err("Enum name must be alphanumeric or underscores only".to_string());
        }
        if self.enums.contains_key(name)
            || self.user_global_enums.contains_key(name)
            || self.builtin_enums.contains_key(name)
        {
            return Err(format!("Enum '{name}' already exists"));
        }
        Ok(())
    }

    /// Resolves `value` against the enum annotation at `address`, if present.
    ///
    /// Searches project-local, user-global, and built-in enums in precedence order.
    #[must_use]
    pub fn resolve_enum_value(&self, address: Addr, value: u16) -> Option<String> {
        let enum_name = self.annotations.get(address)?.enum_usage.as_ref()?;
        self.enums
            .get(enum_name)
            .or_else(|| self.user_global_enums.get(enum_name))
            .or_else(|| self.builtin_enums.get(enum_name))
            .and_then(|enum_def| enum_def.variants.get(&value).cloned())
    }

    /// Gets an [`EnumDefinition`] by name from project-local, user-global, or built-in enums.
    #[must_use]
    pub fn get_enum(&self, name: &str) -> Option<&EnumDefinition> {
        self.enums
            .get(name)
            .or_else(|| self.user_global_enums.get(name))
            .or_else(|| self.builtin_enums.get(name))
    }

    /// Returns `true` when `addr` starts a new sub-block inside a split byte table.
    ///
    /// Matches the sub-block split rules in `disassembler::handlers` and `analyzer::contiguous_block_len`.
    #[must_use]
    pub(crate) fn is_split_table_subblock_boundary(&self, addr: Addr) -> bool {
        if self.is_virtual_splitter(addr) {
            return true;
        }
        self.annotations
            .get(addr)
            .is_some_and(|e| e.user_line_comment.is_some() || e.user_side_comment.is_some())
    }

    /// Computes `(run_start_pc, byte_count)` for the contiguous `target_type` split-table
    /// sub-block containing byte offset `pc`.
    #[must_use]
    fn split_table_subblock_bounds(&self, pc: usize, target_type: BlockType) -> (usize, usize) {
        let data_len = self.raw_data.len();
        let mut run_start = pc;
        while run_start > 0 {
            let cur_addr = self.origin.wrapping_add(run_start as u16);
            if self.is_split_table_subblock_boundary(cur_addr) {
                break;
            }
            if self.block_types.get(run_start - 1) != Some(&target_type) {
                break;
            }
            run_start -= 1;
        }

        let mut count = 0;
        while run_start + count < data_len
            && self.block_types.get(run_start + count) == Some(&target_type)
        {
            if count > 0 {
                let cur_addr = self.origin.wrapping_add((run_start + count) as u16);
                if self.is_split_table_subblock_boundary(cur_addr) {
                    break;
                }
            }
            count += 1;
        }

        (run_start, count)
    }

    /// Resolves `(first_addr, second_addr, raw_target)` when `addr` lies inside a valid 2-byte pair
    /// of a [`BlockType::LoHiAddress`] or [`BlockType::HiLoAddress`] split address table.
    #[must_use]
    pub(crate) fn split_table_pair_at_address(&self, addr: Addr) -> Option<(Addr, Addr, Addr)> {
        if self.is_external(addr) {
            return None;
        }
        let pc = addr.offset_from(self.origin);
        let target_type = *self.block_types.get(pc)?;
        if !matches!(target_type, BlockType::LoHiAddress | BlockType::HiLoAddress) {
            return None;
        }

        let (run_start, count) = self.split_table_subblock_bounds(pc, target_type);
        let pair_count = count / 2;
        let offset_in_run = pc - run_start;
        if pair_count == 0 || offset_in_run >= pair_count * 2 {
            return None;
        }

        let pair_idx = offset_in_run % pair_count;
        let first_byte = *self.raw_data.get(run_start + pair_idx)?;
        let second_byte = *self.raw_data.get(run_start + pair_count + pair_idx)?;
        let (lo, hi) = if target_type == BlockType::LoHiAddress {
            (first_byte, second_byte)
        } else {
            (second_byte, first_byte)
        };

        let first_addr = self.origin.wrapping_add((run_start + pair_idx) as u16);
        let second_addr = self
            .origin
            .wrapping_add((run_start + pair_count + pair_idx) as u16);
        let raw_target = Addr(u16::from_le_bytes([lo, hi]));
        Some((first_addr, second_addr, raw_target))
    }

    /// Extracts [`OperandTargetSite`] records across `start_addr..=end_addr`.
    ///
    /// Scans in `O(N)` time and deduplicates results by canonical `site_addr` in ascending order.
    /// For [`BlockType::LoHiAddress`] and [`BlockType::HiLoAddress`] pairs, `site_addr` is the first-half
    /// byte address and `second_pair_addr` is `Some(second_half_addr)`. For [`BlockType::Code`] and
    /// [`BlockType::Address`] entries, `second_pair_addr` is `None`.
    #[must_use]
    pub fn effective_targets_with_pair_in_range(
        &self,
        start_addr: Addr,
        end_addr: Addr,
    ) -> Vec<OperandTargetSite> {
        if self.raw_data.is_empty() || start_addr > end_addr {
            return Vec::new();
        }

        let data_len = self.raw_data.len();
        let bin_start = usize::from(self.origin.0);
        let bin_end = bin_start
            .saturating_add(data_len.saturating_sub(1))
            .min(0xFFFF);
        let clamped_start = usize::from(start_addr.0).max(bin_start);
        let clamped_end = usize::from(end_addr.0).min(bin_end);
        if clamped_start > clamped_end {
            return Vec::new();
        }

        let start_pc = clamped_start - bin_start;
        let end_pc = (clamped_end - bin_start).min(data_len.saturating_sub(1));

        let mut seen = BTreeSet::new();
        let mut ordered = BTreeMap::new();
        let mut pc = start_pc;

        while pc <= end_pc {
            let current_type = self.block_types.get(pc).copied().unwrap_or(BlockType::Code);
            match current_type {
                BlockType::Code => {
                    // Align decoding from the start of the contiguous `Code` run so an interior
                    // operand byte at `start_addr` never misdecodes as a standalone opcode.
                    let mut cur_pc = pc;
                    while cur_pc > 0 && self.block_types.get(cur_pc - 1) == Some(&BlockType::Code) {
                        cur_pc -= 1;
                    }

                    while cur_pc <= end_pc && self.block_types.get(cur_pc) == Some(&BlockType::Code)
                    {
                        let opcode_byte = self.raw_data[cur_pc];
                        let valid_opcode = self
                            .disassembler
                            .opcodes
                            .get(usize::from(opcode_byte))
                            .and_then(|op| op.as_ref())
                            .filter(|op| !op.illegal || self.settings.use_illegal_opcodes);

                        let Some(opcode) = valid_opcode else {
                            cur_pc += 1;
                            continue;
                        };

                        if opcode.mnemonic == "BRK"
                            && !self.settings.brk_single_byte
                            && cur_pc + 1 < data_len
                            && self.block_types.get(cur_pc + 1) == Some(&BlockType::Code)
                        {
                            cur_pc += 2;
                            continue;
                        }

                        let size = usize::from(opcode.size);
                        let all_code = cur_pc + size <= data_len
                            && (1..size).all(|i| {
                                self.block_types.get(cur_pc + i) == Some(&BlockType::Code)
                            });
                        if !all_code {
                            cur_pc += 1;
                            continue;
                        }

                        if cur_pc >= start_pc {
                            let site_addr = self.origin.wrapping_add(cur_pc as u16);
                            let extracted = match opcode.mode {
                                AddressingMode::Immediate => self
                                    .annotations
                                    .get(site_addr)
                                    .and_then(|e| e.immediate_format)
                                    .and_then(|fmt| match fmt {
                                        ImmediateFormat::LowByte(target)
                                        | ImmediateFormat::HighByte(target) => {
                                            Some((target, false))
                                        }
                                        _ => None,
                                    }),
                                AddressingMode::ZeroPage
                                | AddressingMode::ZeroPageX
                                | AddressingMode::ZeroPageY
                                | AddressingMode::IndirectX
                                | AddressingMode::IndirectY => {
                                    Some((Addr(u16::from(self.raw_data[cur_pc + 1])), true))
                                }
                                AddressingMode::Absolute
                                | AddressingMode::AbsoluteX
                                | AddressingMode::AbsoluteY
                                | AddressingMode::Indirect => Some((
                                    Addr(u16::from_le_bytes([
                                        self.raw_data[cur_pc + 1],
                                        self.raw_data[cur_pc + 2],
                                    ])),
                                    false,
                                )),
                                // Exclude `Relative` branches (`BNE`, `BEQ`, ...) so nudging an
                                // unrolled loop in Visual Mode leaves branch targets untouched.
                                AddressingMode::Relative
                                | AddressingMode::Implied
                                | AddressingMode::Accumulator
                                | AddressingMode::Unknown => None,
                            };

                            if let Some((raw_target, is_zp)) = extracted
                                && seen.insert(site_addr)
                            {
                                ordered.insert(
                                    site_addr,
                                    OperandTargetSite {
                                        site_addr,
                                        raw_target,
                                        is_zero_page_mode: is_zp,
                                        second_pair_addr: None,
                                    },
                                );
                            }
                        }

                        cur_pc += size;
                    }

                    pc = cur_pc.max(pc + 1);
                }
                BlockType::Address => {
                    let mut cur_pc = pc;
                    while cur_pc > 0
                        && self.block_types.get(cur_pc - 1) == Some(&BlockType::Address)
                    {
                        cur_pc -= 1;
                    }

                    while cur_pc <= end_pc
                        && self.block_types.get(cur_pc) == Some(&BlockType::Address)
                    {
                        let next_addr = self.origin.wrapping_add((cur_pc + 1) as u16);
                        let has_pair = cur_pc + 1 < data_len
                            && self.block_types.get(cur_pc + 1) == Some(&BlockType::Address)
                            && !self.is_virtual_splitter(next_addr);

                        if has_pair {
                            if cur_pc + 1 >= start_pc && cur_pc <= end_pc {
                                let word_addr = self.origin.wrapping_add(cur_pc as u16);
                                let raw_target = Addr(u16::from_le_bytes([
                                    self.raw_data[cur_pc],
                                    self.raw_data[cur_pc + 1],
                                ]));
                                if seen.insert(word_addr) {
                                    ordered.insert(
                                        word_addr,
                                        OperandTargetSite {
                                            site_addr: word_addr,
                                            raw_target,
                                            is_zero_page_mode: false,
                                            second_pair_addr: None,
                                        },
                                    );
                                }
                            }
                            cur_pc += 2;
                        } else {
                            cur_pc += 1;
                        }
                    }

                    pc = cur_pc.max(pc + 1);
                }
                BlockType::LoHiAddress | BlockType::HiLoAddress => {
                    // Walk backward to `run_start` so selecting only second-half `.byte >...` rows
                    // still resolves the true `pair_count` and maps back to canonical `site_addr`.
                    let (run_start, count) = self.split_table_subblock_bounds(pc, current_type);
                    let pair_count = count / 2;
                    let sub_end_exclusive = (run_start + count).min(end_pc + 1);

                    if pair_count > 0 {
                        for b_pc in pc..sub_end_exclusive {
                            let offset_in_run = b_pc - run_start;
                            if offset_in_run >= pair_count * 2 {
                                break;
                            }
                            let pair_idx = offset_in_run % pair_count;
                            let first_byte = self.raw_data[run_start + pair_idx];
                            let second_byte = self.raw_data[run_start + pair_count + pair_idx];
                            let (lo, hi) = if current_type == BlockType::LoHiAddress {
                                (first_byte, second_byte)
                            } else {
                                (second_byte, first_byte)
                            };
                            let site_addr = self.origin.wrapping_add((run_start + pair_idx) as u16);
                            if seen.insert(site_addr) {
                                let second_addr = self
                                    .origin
                                    .wrapping_add((run_start + pair_count + pair_idx) as u16);
                                let raw_target = Addr(u16::from_le_bytes([lo, hi]));
                                ordered.insert(
                                    site_addr,
                                    OperandTargetSite {
                                        site_addr,
                                        raw_target,
                                        is_zero_page_mode: false,
                                        second_pair_addr: Some(second_addr),
                                    },
                                );
                            }
                        }
                    }

                    pc = (run_start + count).max(pc + 1);
                }
                _ => {
                    pc += 1;
                }
            }
        }

        ordered.into_values().collect()
    }

    /// Extracts `(site_addr, raw_target, is_zero_page_mode)` tuples across `start_addr..=end_addr`.
    ///
    /// Deduplicates results by canonical `site_addr` in ascending order:
    ///
    /// - **`BlockType::Code`**: Decodes aligned instructions. Includes zero-page, indirect,
    ///   absolute, `LowByte`, and `HighByte` operands. Excludes relative branches and literals.
    /// - **`BlockType::Address`**: Yields each 2-byte `.word` pointer intersecting the range.
    /// - **`BlockType::LoHiAddress` / `BlockType::HiLoAddress`**: Scans backward to `run_start`
    ///   to map both halves to the canonical first-half `site_addr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use regenerator2000_core::state::{Addr, AppState, BlockType};
    ///
    /// let mut state = AppState::new();
    /// state.origin = Addr(0x1000);
    /// // $1000: STA $FC (ZeroPage), $1002: LDA $1FFF,X (AbsoluteX), $1005: BNE $1000 (Relative)
    /// state.raw_data = vec![0x85, 0xFC, 0xBD, 0xFF, 0x1F, 0xD0, 0xF9];
    /// state.block_types = vec![BlockType::Code; state.raw_data.len()];
    ///
    /// let targets = state.effective_targets_in_range(Addr(0x1000), Addr(0x1006));
    /// assert_eq!(
    ///     targets,
    ///     vec![
    ///         (Addr(0x1000), Addr(0x00FC), true),
    ///         (Addr(0x1002), Addr(0x1FFF), false),
    ///     ]
    /// );
    /// ```
    #[must_use]
    pub fn effective_targets_in_range(
        &self,
        start_addr: Addr,
        end_addr: Addr,
    ) -> Vec<(Addr, Addr, bool)> {
        self.effective_targets_with_pair_in_range(start_addr, end_addr)
            .into_iter()
            .map(|t| (t.site_addr, t.raw_target, t.is_zero_page_mode))
            .collect()
    }

    /// Extracts `(site_addr, raw_target, is_zero_page_mode)` at the single address `addr`, if any.
    #[must_use]
    pub fn effective_target_at_address(&self, addr: Addr) -> Option<(Addr, Addr, bool)> {
        self.effective_targets_in_range(addr, addr)
            .into_iter()
            .next()
    }

    /// Returns active labels at `addr`, falling back to non-excluded [`AppState::system_labels`]
    /// when [`AppState::labels`] has no entry for `addr`.
    #[must_use]
    pub(crate) fn effective_labels_at(&self, addr: Addr) -> &[Label] {
        if let Some(v) = self.labels.get(&addr).filter(|v| !v.is_empty()) {
            return v.as_slice();
        }
        if !self.excluded_addresses.contains(&addr)
            && let Some(sys_v) = self.system_labels.get(&addr).filter(|v| !v.is_empty())
        {
            return sys_v.as_slice();
        }
        &[]
    }

    /// Returns the inclusive `(start, end)` bounds of the lexical scope enclosing `addr` in `O(log S)` time.
    #[must_use]
    pub fn enclosing_scope_for_address(&self, addr: Addr) -> Option<(Addr, Addr)> {
        self.annotations
            .scopes()
            .range(..=addr)
            .next_back()
            .filter(|&(_, &e)| addr <= e)
            .map(|(&s, &e)| (s, e))
    }

    /// Resolves `base_str` to its [`Addr`], using `cache` to skip repeated label scans in range loops.
    fn resolve_symbolic_base_cached(
        &self,
        base_str: &str,
        current_scope: Option<(Addr, Addr)>,
        cache: Option<&mut SymbolicBaseCache>,
    ) -> Result<Addr, String> {
        let trimmed = base_str.trim();
        if let Some(c) = cache.as_deref()
            && let Some(entry) = &c.last
            && entry.token == trimmed
            && entry.scope == current_scope
        {
            return entry.result.clone();
        }

        let res = self.resolve_symbolic_base(trimmed, current_scope);
        if let Some(c) = cache {
            c.last = Some(CachedBaseLookup {
                token: trimmed.to_string(),
                scope: current_scope,
                result: res.clone(),
            });
        }
        res
    }

    /// Searches `scopes` for `name` (or its stripped local form when `LocalUserDefined`).
    fn find_label_in_scope(&self, name: &str, scopes: &[(Addr, Addr)]) -> Option<Addr> {
        let stripped = name.strip_prefix(['_', '.', '@']);
        let matches_local = |l: &Label| {
            l.label_type == LabelType::LocalUserDefined
                && (l.name == name || stripped.is_some_and(|st| l.name == st))
        };
        for &(s, e) in scopes {
            for (&addr, label_vec) in self.labels.range(s..=e) {
                if label_vec.iter().any(|l| {
                    (l.name == name || matches_local(l))
                        && (l.kind == LabelKind::User
                            || !crate::disassembler::symbols::has_active_address_offset(
                                addr,
                                &self.labels,
                                &self.annotations,
                            ))
                }) {
                    return Some(addr);
                }
            }
        }
        None
    }

    /// Resolves a base symbol identifier or numeric address literal to its [`Addr`].
    fn resolve_symbolic_base(
        &self,
        base_str: &str,
        current_scope: Option<(Addr, Addr)>,
    ) -> Result<Addr, String> {
        let trimmed = base_str.trim();
        if trimmed.is_empty() {
            return Err("Missing base symbol or address".to_string());
        }

        // 1. Hexadecimal (`$XXXX`, `0xXXXX`) or decimal numeric address literals.
        if let Some(hex) = trimmed.strip_prefix('$') {
            return u16::from_str_radix(hex, 16)
                .map(Addr)
                .map_err(|_| format!("Invalid hexadecimal base address '{trimmed}'"));
        }
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            return u16::from_str_radix(hex, 16)
                .map(Addr)
                .map_err(|_| format!("Invalid hexadecimal base address '{trimmed}'"));
        }
        if trimmed.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
            return trimmed
                .parse::<u16>()
                .map(Addr)
                .map_err(|_| format!("Invalid decimal base address '{trimmed}'"));
        }

        // 2. Explicit scope-qualified reference (`scope.member` or `scope::member`).
        let first_len = trimmed.chars().next().map_or(0, char::len_utf8);
        let scoped_split = trimmed.split_once("::").or_else(|| {
            trimmed[first_len..].find('.').map(|dot_rel| {
                (
                    &trimmed[..first_len + dot_rel],
                    &trimmed[first_len + dot_rel + 1..],
                )
            })
        });
        if let Some((scope_name, member_name)) = scoped_split {
            if scope_name.is_empty() || member_name.is_empty() {
                return Err(format!("Invalid scoped symbol '{trimmed}'"));
            }
            let matching_scopes: Vec<(Addr, Addr)> = self
                .annotations
                .scopes()
                .iter()
                .filter(|&(&s, _)| {
                    self.labels
                        .get(&s)
                        .is_some_and(|v| v.iter().any(|l| l.name == scope_name))
                        && !crate::disassembler::symbols::has_active_address_offset(
                            s,
                            &self.labels,
                            &self.annotations,
                        )
                })
                .map(|(&s, &e)| (s, e))
                .collect();

            if matching_scopes.is_empty() {
                return Err(format!(
                    "Unknown scope '{scope_name}' in expression '{trimmed}'"
                ));
            }

            return self
                .find_label_in_scope(member_name, &matching_scopes)
                .ok_or_else(|| format!("Unknown scoped symbol '{trimmed}'"));
        }

        // 3. Unqualified or local-prefixed (`_name`, `.name`, `@name`) symbol inside `current_scope`.
        if let Some(scope) = current_scope
            && let Some(addr) = self.find_label_in_scope(trimmed, &[scope])
        {
            return Ok(addr);
        }

        // 4. Global symbol lookup ordered by precedence (`User` > `System` > `Auto`).
        let stripped_local = trimmed.strip_prefix(['_', '.', '@']);
        let mut best_match: Option<(u8, Addr)> = None;
        for (&addr, label_vec) in &self.labels {
            for label in label_vec {
                let matches_name = label.name == trimmed
                    || stripped_local.is_some_and(|st| {
                        label.label_type == LabelType::LocalUserDefined && label.name == st
                    });
                if !matches_name {
                    continue;
                }
                let rank = match label.kind {
                    LabelKind::User => 0,
                    LabelKind::System => 1,
                    LabelKind::Auto => 2,
                };
                if best_match.is_some_and(|(best_rank, _)| rank >= best_rank) {
                    continue;
                }
                if label.kind != LabelKind::User
                    && crate::disassembler::symbols::has_active_address_offset(
                        addr,
                        &self.labels,
                        &self.annotations,
                    )
                {
                    continue;
                }
                if label.label_type == LabelType::LocalUserDefined
                    && self.enclosing_scope_for_address(addr).is_some()
                {
                    continue;
                }
                best_match = Some((rank, addr));
                if rank == 0 {
                    return Ok(addr);
                }
            }
        }

        best_match = self.resolve_unreferenced_system_base(trimmed, best_match);

        best_match
            .map(|(_, addr)| addr)
            .ok_or_else(|| format!("Unknown base symbol '{trimmed}'"))
    }

    /// Checks `self.system_labels` for `trimmed` when no higher-rank match exists in `self.labels`.
    fn resolve_unreferenced_system_base(
        &self,
        trimmed: &str,
        mut best_match: Option<(u8, Addr)>,
    ) -> Option<(u8, Addr)> {
        if best_match.is_some_and(|(best_rank, _)| best_rank <= 1) {
            return best_match;
        }
        for (&addr, label_vec) in &self.system_labels {
            if !label_vec.iter().any(|l| l.name == trimmed) {
                continue;
            }
            if self.excluded_addresses.contains(&addr)
                || crate::disassembler::symbols::has_active_address_offset(
                    addr,
                    &self.labels,
                    &self.annotations,
                )
            {
                continue;
            }
            best_match = Some((1, addr));
            break;
        }
        best_match
    }

    /// Returns `true` when `input` is a symbolic-offset clear token (`""`, `"0"`, `"+0"`, `"-0"`,
    /// `"$0"`, `"0x0"`, `"0X0"`) after trimming whitespace and balanced outer parentheses.
    #[must_use]
    pub fn is_symbolic_clear_input(input: &str) -> bool {
        let s = strip_balanced_parens(input);
        if matches!(s, "" | "0" | "+0" | "-0" | "$0" | "0x0" | "0X0") {
            return true;
        }
        if let Some(rest) = s.strip_prefix(['+', '-']) {
            let mag = rest.trim();
            return matches!(mag, "0" | "$0" | "0x0" | "0X0");
        }
        false
    }

    /// Parses a symbolic offset input expression against `effective_addr`.
    ///
    /// Supported forms:
    ///
    /// - Clear tokens (`""`, `"0"`, `"+0"`, `"-0"`, `"$0"`, `"0x0"`) returning `Ok(None)`.
    /// - Full binary expressions (`"zp_ptr + 1"`, `"sub_C100 - $10"`, `"$0400 + 40"`,
    ///   `"(0xC100 - 1)"`, `"irq.loop_var + 1"`, `"_loop_var + 1"`).
    /// - Signed displacement shorthands (`"+1"`, `"-1"`, `"- $10"`), computing
    ///   `base = effective_addr - offset`.
    /// - Bare base symbols or addresses (`"SCREEN_RAM"`, `"$0400"`), computing
    ///   `offset = effective_addr - base` (returning `Ok(None)` when `base == effective_addr`).
    ///
    /// # Errors
    ///
    /// Returns a diagnostic message when the base symbol is unknown or the displacement overflows `i16`.
    /// Also returns an error when 16-bit arithmetic wraps, `base + offset != effective_addr`, or a
    /// zero-page instruction receives `base > $00FF`.
    ///
    /// # Examples
    ///
    /// ```
    /// use regenerator2000_core::state::{
    ///     Addr, AppState, Label, LabelKind, LabelType, OffsetRadix, SymbolicOffset,
    /// };
    ///
    /// let mut state = AppState::new();
    /// state.labels.insert(
    ///     Addr(0x0400),
    ///     vec![Label {
    ///         name: "SCREEN_RAM".to_string(),
    ///         kind: LabelKind::User,
    ///         label_type: LabelType::Field,
    ///     }],
    /// );
    ///
    /// // Bare base symbol auto-computes `offset = $0428 - $0400 = 40`:
    /// assert_eq!(
    ///     state.parse_symbolic_offset_input("SCREEN_RAM", Addr(0x0428), false, None),
    ///     Ok(Some(SymbolicOffset {
    ///         base: Addr(0x0400),
    ///         offset: 40,
    ///         radix: OffsetRadix::Decimal,
    ///     }))
    /// );
    ///
    /// // Hexadecimal displacement preserves `OffsetRadix::Hex`:
    /// assert_eq!(
    ///     state.parse_symbolic_offset_input("SCREEN_RAM + $28", Addr(0x0428), false, None),
    ///     Ok(Some(SymbolicOffset {
    ///         base: Addr(0x0400),
    ///         offset: 40,
    ///         radix: OffsetRadix::Hex,
    ///     }))
    /// );
    /// ```
    pub fn parse_symbolic_offset_input(
        &self,
        input: &str,
        effective_addr: Addr,
        is_zero_page_mode: bool,
        current_scope: Option<(Addr, Addr)>,
    ) -> Result<Option<SymbolicOffset>, String> {
        self.parse_symbolic_offset_input_inner(
            input,
            effective_addr,
            is_zero_page_mode,
            current_scope,
            None,
        )
    }

    /// Range-optimized variant of [`AppState::parse_symbolic_offset_input`] sharing `cache` across
    /// consecutive addresses in `disassembly_handler` and MCP range loops.
    pub(crate) fn parse_symbolic_offset_input_cached(
        &self,
        input: &str,
        effective_addr: Addr,
        is_zero_page_mode: bool,
        current_scope: Option<(Addr, Addr)>,
        cache: &mut SymbolicBaseCache,
    ) -> Result<Option<SymbolicOffset>, String> {
        self.parse_symbolic_offset_input_inner(
            input,
            effective_addr,
            is_zero_page_mode,
            current_scope,
            Some(cache),
        )
    }

    fn parse_symbolic_offset_input_inner(
        &self,
        input: &str,
        effective_addr: Addr,
        is_zero_page_mode: bool,
        current_scope: Option<(Addr, Addr)>,
        cache: Option<&mut SymbolicBaseCache>,
    ) -> Result<Option<SymbolicOffset>, String> {
        if Self::is_symbolic_clear_input(input) {
            return Ok(None);
        }
        let s = strip_balanced_parens(input);

        // 1. Leading `+` or `-`: signed displacement shorthand (`"+1"`, `"-1"`, `"- $10"`).
        if let Some(sign_char) = s.chars().next().filter(|&c| c == '+' || c == '-') {
            let mag_str = s[sign_char.len_utf8()..].trim();
            let (mag, radix) = parse_unsigned_offset_literal(mag_str)?;
            if mag == 0 {
                return Ok(None);
            }
            let signed_i64 = if sign_char == '-' {
                -i64::from(mag)
            } else {
                i64::from(mag)
            };
            let offset = i16::try_from(signed_i64).map_err(|_| {
                format!("Offset '{s}' overflows signed 16-bit displacement range (-32768..=32767)")
            })?;
            let base_i32 = i32::from(effective_addr.0) - i32::from(offset);
            if !(0..=0xFFFF).contains(&base_i32) {
                return Err(format!(
                    "Offset '{s}' for ${effective_addr:04X} produces out-of-bounds base address (must lie within $0000..=$FFFF)"
                ));
            }
            let base = Addr(base_i32 as u16);
            if is_zero_page_mode && base.0 > 0x00FF {
                return Err(format!(
                    "2-byte zero-page instruction targeting ${:02X} requires a zero-page base address (<= $00FF), got ${base:04X}",
                    effective_addr.0
                ));
            }
            return Ok(Some(SymbolicOffset {
                base,
                offset,
                radix,
            }));
        }

        // 2. Binary `base +/- offset` expression.
        if let Some(op_idx) = s.find(['+', '-']) {
            let base_str = s[..op_idx].trim();
            let op_char = s.as_bytes()[op_idx] as char;
            let rhs_str = s[op_idx + 1..].trim();
            if base_str.is_empty() || rhs_str.is_empty() {
                return Err(format!("Malformed symbolic offset expression '{s}'"));
            }

            let base = self.resolve_symbolic_base_cached(base_str, current_scope, cache)?;
            let (mag, radix) = parse_unsigned_offset_literal(rhs_str)?;
            let signed_i64 = if op_char == '-' {
                -i64::from(mag)
            } else {
                i64::from(mag)
            };

            let eval_i64 = i64::from(base.0) + signed_i64;
            if !(0..=0xFFFF).contains(&eval_i64) {
                return Err(format!(
                    "Expression '{s}' evaluates outside non-wrapping 16-bit address bounds ($0000..=$FFFF)"
                ));
            }

            let eval_addr = Addr(eval_i64 as u16);
            if eval_addr != effective_addr {
                if let Some(suggested) =
                    SymbolicOffset::from_base_and_effective(base, effective_addr, radix)
                {
                    let suggestion = suggested.format_expression(base_str, OffsetParentheses::Bare);
                    return Err(format!(
                        "Expression '{s}' evaluates to ${eval_addr:04X}, expected ${effective_addr:04X} (try '{suggestion}')"
                    ));
                }
                if base == effective_addr {
                    return Err(format!(
                        "Expression '{s}' evaluates to ${eval_addr:04X}, expected ${effective_addr:04X}. Offset is 0. Use '' to clear."
                    ));
                }
                return Err(format!(
                    "Expression '{s}' evaluates to ${eval_addr:04X}, expected ${effective_addr:04X}"
                ));
            }

            if signed_i64 == 0 {
                return Ok(None);
            }

            let offset = i16::try_from(signed_i64).map_err(|_| {
                format!(
                    "Offset in '{s}' overflows signed 16-bit displacement range (-32768..=32767)"
                )
            })?;

            if is_zero_page_mode && base.0 > 0x00FF {
                return Err(format!(
                    "2-byte zero-page instruction targeting ${:02X} requires a zero-page base symbol (<= $00FF), got '{base_str}' (${base:04X})",
                    effective_addr.0
                ));
            }

            return Ok(Some(SymbolicOffset {
                base,
                offset,
                radix,
            }));
        }

        // 3. Bare base symbol or base address (`"SCREEN_RAM"`, `"$0400"`).
        let base = self.resolve_symbolic_base_cached(s, current_scope, cache)?;
        if base == effective_addr {
            return Ok(None);
        }
        let diff = i32::from(effective_addr.0) - i32::from(base.0);
        let offset = i16::try_from(diff).map_err(|_| {
            format!(
                "Displacement {diff} from '{s}' (${base:04X}) to ${effective_addr:04X} overflows signed 16-bit range (-32768..=32767)"
            )
        })?;
        if is_zero_page_mode && base.0 > 0x00FF {
            return Err(format!(
                "2-byte zero-page instruction targeting ${:02X} requires a zero-page base symbol (<= $00FF), got '{s}' (${base:04X})",
                effective_addr.0
            ));
        }

        Ok(Some(SymbolicOffset {
            base,
            offset,
            radix: OffsetRadix::Decimal,
        }))
    }

    /// Formats `sym` into an unparenthesized assembler expression string (such as `"zp_ptr + 1"`
    /// or `"l00 + 1"`) using the active [`Formatter`](crate::disassembler::formatter::Formatter).
    ///
    /// Mirrors [`resolve_symbolic_label_name`](crate::disassembler::resolve_symbolic_label_name)
    /// across scope-local labels (`l00`), scope qualifiers, and non-excluded [`AppState::system_labels`].
    /// When `sym.base` lacks a visible label, emits hex fallback literals (`$fb + 1` zero-page, `$00fb + 1` 16-bit).
    #[must_use]
    pub fn format_symbolic_offset(
        &self,
        sym: SymbolicOffset,
        site_addr: Option<Addr>,
        is_zero_page_mode: bool,
    ) -> String {
        let formatter = Disassembler::formatter(self.settings.assembler);
        let supports_scopes = formatter.supports_scopes();

        let scope_header_name = |s: Addr| -> Option<String> {
            (!crate::disassembler::symbols::has_active_address_offset(
                s,
                &self.labels,
                &self.annotations,
            ))
            .then(|| {
                self.labels.get(&s).and_then(|v| {
                    crate::disassembler::resolve_label(v, s.0, &self.settings)
                        .map(|l| l.name.clone())
                })
            })
            .flatten()
        };

        let base_scope = supports_scopes
            .then(|| self.enclosing_scope_for_address(sym.base))
            .flatten();
        let base_scope_name = base_scope.and_then(|(s, _)| scope_header_name(s));
        let current_scope_name = supports_scopes
            .then(|| {
                site_addr
                    .and_then(|sa| self.enclosing_scope_for_address(sa))
                    .and_then(|(s, _)| scope_header_name(s))
            })
            .flatten();

        let local_map = base_scope.and_then(|(s_start, s_end)| {
            let start_pc = usize::from(s_start.0.checked_sub(self.origin.0)?);
            if start_pc >= self.raw_data.len() {
                return None;
            }
            let end_pc = usize::from(s_end.0.checked_sub(self.origin.0)?)
                .min(self.raw_data.len().saturating_sub(1));
            Some(
                crate::disassembler::symbols::compute_local_label_names_in_range(
                    &self.disassembler.opcodes,
                    &self.raw_data,
                    self.origin,
                    &self.labels,
                    &self.annotations,
                    start_pc,
                    end_pc,
                    formatter,
                ),
            )
        });

        let mut scope_map = crate::disassembler::symbols::ScopeNameMap::default();
        if let (Some((s_start, s_end)), Some(sname)) = (base_scope, base_scope_name) {
            scope_map.insert_range(s_start, s_end, sname);
        }

        let sys_fallback = self
            .labels
            .get(&sym.base)
            .is_none_or(Vec::is_empty)
            .then(|| {
                let eff = self.effective_labels_at(sym.base);
                (!eff.is_empty()).then(|| BTreeMap::from([(sym.base, eff.to_vec())]))
            })
            .flatten();
        let labels_for_base = sys_fallback.as_ref().unwrap_or(&self.labels);

        crate::disassembler::symbols::format_symbolic_offset_expr(
            sym,
            labels_for_base,
            &self.settings,
            &self.annotations,
            local_map.as_ref(),
            (!scope_map.is_empty()).then_some(&scope_map),
            current_scope_name.as_deref(),
            formatter.scope_resolution_separator(),
            formatter.local_label_prefix(),
            OffsetParentheses::Bare,
            |base| {
                if is_zero_page_mode && base.0 <= 0x00FF {
                    formatter.format_byte(base.0 as u8)
                } else {
                    formatter.format_address(base)
                }
            },
        )
    }

    /// Suggests `(initial_expression, default_is_global)` for `effective_addr` (optionally
    /// referenced from `site_addr`).
    ///
    /// Precedence:
    ///
    /// 1. Active Tier 1 `operand_offset` at `site_addr`, returning `(expr, false)`.
    /// 2. Active Tier 2 `address_offset` at `effective_addr`, returning `(expr, true)`.
    /// 3. Nearest candidate symbol (`effective_addr + 1` or preceding label within `$0400` bytes).
    ///    Defaults `is_global` to `true` on external equates and positive-displacement memory instructions.
    #[must_use]
    pub fn suggest_symbolic_offset(
        &self,
        site_addr: Option<Addr>,
        effective_addr: Addr,
        is_zero_page_mode: bool,
    ) -> (String, bool) {
        if let Some(site) = site_addr {
            let tier1 = self
                .annotations
                .get(site)
                .and_then(|e| e.operand_offset)
                .filter(|sym| {
                    sym.matches_target(effective_addr)
                        && (!is_zero_page_mode || sym.base.0 <= 0x00FF)
                })
                .or_else(|| {
                    self.split_table_pair_at_address(site).and_then(
                        |(first_addr, second_addr, _)| {
                            self.annotations
                                .get(first_addr)
                                .and_then(|e| e.operand_offset)
                                .or_else(|| {
                                    self.annotations
                                        .get(second_addr)
                                        .and_then(|e| e.operand_offset)
                                })
                                .filter(|sym| {
                                    sym.matches_target(effective_addr)
                                        && (!is_zero_page_mode || sym.base.0 <= 0x00FF)
                                })
                        },
                    )
                });
            if let Some(sym) = tier1 {
                return (
                    self.format_symbolic_offset(sym, site_addr, is_zero_page_mode),
                    false,
                );
            }
        }

        if crate::disassembler::symbols::has_active_address_offset(
            effective_addr,
            &self.labels,
            &self.annotations,
        ) && let Some(sym) = self
            .annotations
            .get(effective_addr)
            .and_then(|e| e.address_offset)
            .filter(|sym| {
                sym.matches_target(effective_addr) && (!is_zero_page_mode || sym.base.0 <= 0x00FF)
            })
        {
            return (
                self.format_symbolic_offset(sym, site_addr, is_zero_page_mode),
                true,
            );
        }

        let is_code_memory_site = site_addr.is_some_and(|site| {
            if self.is_external(site) {
                return false;
            }
            let pc = site.offset_from(self.origin);
            if self.block_types.get(pc) != Some(&BlockType::Code) {
                return false;
            }
            self.raw_data
                .get(pc)
                .and_then(|&b| self.disassembler.opcodes.get(usize::from(b)))
                .and_then(|op| op.as_ref())
                .is_some_and(|op| op.mode != AddressingMode::Immediate)
        });

        let has_unsuppressed_label = |addr: Addr| -> bool {
            !self.effective_labels_at(addr).is_empty()
                && !crate::disassembler::symbols::has_active_address_offset(
                    addr,
                    &self.labels,
                    &self.annotations,
                )
        };
        let has_user_or_system_label = |addr: Addr| -> bool {
            !crate::disassembler::symbols::has_active_address_offset(
                addr,
                &self.labels,
                &self.annotations,
            ) && self
                .effective_labels_at(addr)
                .iter()
                .any(|l| matches!(l.kind, LabelKind::User | LabelKind::System))
        };

        let plus1_candidate = (effective_addr.0 < 0xFFFF)
            .then(|| Addr(effective_addr.0 + 1))
            .filter(|&cand| {
                (!is_zero_page_mode || cand.0 <= 0x00FF) && has_unsuppressed_label(cand)
            })
            .map(|cand| SymbolicOffset {
                base: cand,
                offset: -1,
                radix: OffsetRadix::Decimal,
            });

        let prev_from_labels = self
            .labels
            .range(..effective_addr)
            .rev()
            .map(|(&cand, _)| cand)
            .take_while(|&cand| effective_addr.0 - cand.0 <= 0x0400)
            .find(|&cand| {
                (!is_zero_page_mode || cand.0 <= 0x00FF) && has_user_or_system_label(cand)
            });
        let prev_from_system = self
            .system_labels
            .range(..effective_addr)
            .rev()
            .map(|(&cand, _)| cand)
            .take_while(|&cand| effective_addr.0 - cand.0 <= 0x0400)
            .find(|&cand| {
                (!is_zero_page_mode || cand.0 <= 0x00FF) && has_user_or_system_label(cand)
            });
        let prev_user_sys_candidate = prev_from_labels
            .into_iter()
            .chain(prev_from_system)
            .max()
            .and_then(|cand| {
                SymbolicOffset::from_base_and_effective(cand, effective_addr, OffsetRadix::Decimal)
            });

        let prev_adj_auto_candidate = (effective_addr.0 > 0)
            .then(|| Addr(effective_addr.0 - 1))
            .filter(|&cand| {
                (!is_zero_page_mode || cand.0 <= 0x00FF) && has_unsuppressed_label(cand)
            })
            .map(|cand| SymbolicOffset {
                base: cand,
                offset: 1,
                radix: OffsetRadix::Decimal,
            });

        let chosen = if site_addr.is_some() && !is_code_memory_site {
            plus1_candidate
                .or(prev_user_sys_candidate)
                .or(prev_adj_auto_candidate)
        } else if prev_user_sys_candidate.is_some_and(|sym| sym.offset == 1) {
            prev_user_sys_candidate
        } else if plus1_candidate.is_some_and(|sym| {
            has_user_or_system_label(sym.base) || prev_user_sys_candidate.is_none()
        }) {
            plus1_candidate
        } else {
            prev_user_sys_candidate
                .or(plus1_candidate)
                .or(prev_adj_auto_candidate)
        };

        if let Some(sym) = chosen {
            let default_is_global = site_addr.is_none() || (is_code_memory_site && sym.offset > 0);
            let expr = self.format_symbolic_offset(sym, site_addr, is_zero_page_mode);
            (expr, default_is_global)
        } else {
            (String::new(), site_addr.is_none() || is_code_memory_site)
        }
    }

    /// Runs analysis (when `auto_analyze` is `true`) or disassembly and pushes one undoable batch.
    fn finalize_symbolic_mutation_batch(&mut self, mut commands: Vec<crate::commands::Command>) {
        if commands.is_empty() {
            return;
        }
        if self.settings.auto_analyze {
            let (analysis_cmd, _) = self.perform_analysis();
            commands.push(analysis_cmd);
        } else {
            // Materialize non-excluded `system_labels` for newly bound `sym.base` addresses so
            // `disassemble()` and `.asm` export emit the platform symbol and equate even when
            // `auto_analyze` is disabled, while preserving exact undo/redo symmetry.
            let mut bases = BTreeSet::new();
            for cmd in &commands {
                match cmd {
                    crate::commands::Command::SetOperandOffset {
                        new_offset: Some(sym),
                        ..
                    }
                    | crate::commands::Command::SetAddressOffset {
                        new_offset: Some(sym),
                        ..
                    } => {
                        bases.insert(sym.base);
                    }
                    _ => {}
                }
            }
            for base in bases {
                if self.labels.get(&base).is_none_or(Vec::is_empty)
                    && !self.excluded_addresses.contains(&base)
                    && let Some(sys_vec) = self
                        .system_labels
                        .get(&base)
                        .filter(|v| !v.is_empty())
                        .cloned()
                {
                    let old_label = self.labels.get(&base).cloned();
                    let label_cmd = crate::commands::Command::SetLabel {
                        address: base,
                        new_label: Some(sys_vec),
                        old_label,
                    };
                    label_cmd.apply(self);
                    commands.push(label_cmd);
                }
            }
            self.disassemble();
        }
        if commands.len() == 1
            && let Some(single) = commands.pop()
        {
            self.push_command(single);
        } else {
            self.push_command(crate::commands::Command::Batch(commands));
        }
    }

    /// Applies Tier 1 `operand_offset` updates and appends their undo commands to `commands`.
    fn append_operand_offset_commands(
        &mut self,
        updates: &[(Addr, Option<SymbolicOffset>)],
        commands: &mut Vec<crate::commands::Command>,
    ) {
        for &(address, raw_new_offset) in updates {
            let new_offset = raw_new_offset.filter(|sym| sym.offset != 0);
            let old_offset = self.annotations.get(address).and_then(|e| e.operand_offset);
            let cmd = crate::commands::Command::SetOperandOffset {
                address,
                new_offset,
                old_offset,
            };
            cmd.apply(self);
            commands.push(cmd);
        }
    }

    /// Applies Tier 2 `address_offset` updates and appends their undo commands to `commands`.
    fn append_address_offset_commands(
        &mut self,
        updates: &[(Addr, Option<SymbolicOffset>)],
        site_addrs: &[Addr],
        commands: &mut Vec<crate::commands::Command>,
    ) {
        let mut cleared_sites = BTreeSet::new();
        let mut candidate_set = BTreeSet::new();
        for &site in site_addrs {
            candidate_set.insert(site);
            if let Some((first_addr, second_addr, _)) = self.split_table_pair_at_address(site) {
                candidate_set.insert(first_addr);
                candidate_set.insert(second_addr);
            }
        }
        let candidate_sites: Vec<Addr> = candidate_set.into_iter().collect();

        for &(target_addr, raw_new_offset) in updates {
            let new_offset = raw_new_offset.filter(|sym| sym.offset != 0);

            if new_offset.is_some() {
                if let Some(old_vec) = self.labels.get(&target_addr).cloned()
                    && old_vec.iter().any(|l| l.kind == LabelKind::User)
                {
                    let retained_system: Vec<Label> = old_vec
                        .iter()
                        .filter(|l| l.kind == LabelKind::System)
                        .cloned()
                        .collect();
                    let clear_label_cmd = crate::commands::Command::SetLabel {
                        address: target_addr,
                        new_label: (!retained_system.is_empty()).then_some(retained_system),
                        old_label: Some(old_vec),
                    };
                    clear_label_cmd.apply(self);
                    commands.push(clear_label_cmd);
                }

                for &cand_site in &candidate_sites {
                    if let Some(old_op_offset) = self
                        .annotations
                        .get(cand_site)
                        .and_then(|e| e.operand_offset)
                        .filter(|sym| sym.matches_target(target_addr))
                        && cleared_sites.insert(cand_site)
                    {
                        let clear_tier1_cmd = crate::commands::Command::SetOperandOffset {
                            address: cand_site,
                            new_offset: None,
                            old_offset: Some(old_op_offset),
                        };
                        clear_tier1_cmd.apply(self);
                        commands.push(clear_tier1_cmd);
                    }
                }
            }

            let old_offset = self
                .annotations
                .get(target_addr)
                .and_then(|e| e.address_offset);
            let set_addr_cmd = crate::commands::Command::SetAddressOffset {
                address: target_addr,
                new_offset,
                old_offset,
            };
            set_addr_cmd.apply(self);
            commands.push(set_addr_cmd);
        }
    }

    /// Applies Tier 1 per-site `operand_offset` updates, runs analysis when `auto_analyze` is enabled, and records one undoable batch.
    pub fn apply_operand_offsets(&mut self, updates: &[(Addr, Option<SymbolicOffset>)]) {
        if updates.is_empty() {
            return;
        }
        let mut commands = Vec::with_capacity(updates.len() + 1);
        self.append_operand_offset_commands(updates, &mut commands);
        self.finalize_symbolic_mutation_batch(commands);
    }

    /// Applies Tier 2 global `address_offset` updates and clears shadowing Tier 1 offsets across `site_addrs`.
    pub fn apply_address_offsets_for_sites(
        &mut self,
        updates: &[(Addr, Option<SymbolicOffset>)],
        site_addrs: &[Addr],
    ) {
        if updates.is_empty() {
            return;
        }
        let mut commands = Vec::with_capacity(updates.len() * 2 + site_addrs.len() + 1);
        self.append_address_offset_commands(updates, site_addrs, &mut commands);
        self.finalize_symbolic_mutation_batch(commands);
    }

    /// Applies Tier 2 global `address_offset` updates and clears conflicting user labels and shadowing Tier 1 offsets at `site_addr`.
    ///
    /// Runs analysis when `auto_analyze` is enabled and records one chronological undoable batch.
    pub fn apply_address_offsets(
        &mut self,
        updates: &[(Addr, Option<SymbolicOffset>)],
        site_addr: Option<Addr>,
    ) {
        self.apply_address_offsets_for_sites(updates, site_addr.as_slice());
    }

    /// Applies mixed Tier 1 `operand_offset` and Tier 2 `address_offset` updates in one undoable batch.
    pub fn apply_combined_symbolic_offsets(
        &mut self,
        operand_updates: &[(Addr, Option<SymbolicOffset>)],
        address_updates: &[(Addr, Option<SymbolicOffset>)],
        site_addrs: &[Addr],
    ) {
        if operand_updates.is_empty() && address_updates.is_empty() {
            return;
        }
        let mut commands =
            Vec::with_capacity(operand_updates.len() + address_updates.len() * 2 + 2);
        self.append_operand_offset_commands(operand_updates, &mut commands);
        self.append_address_offset_commands(address_updates, site_addrs, &mut commands);
        self.finalize_symbolic_mutation_batch(commands);
    }

    /// Checks and plans a user label or Tier 2 symbolic offset submission without mutating state.
    fn plan_user_label_or_address_offset(
        &self,
        address: Addr,
        name: &str,
        is_local: bool,
        site_addr: Option<Addr>,
    ) -> Result<LabelOrAddressOffsetPlan, String> {
        let trimmed = name.trim();
        if trimmed.contains(['+', '-']) {
            let scope = site_addr
                .and_then(|sa| self.enclosing_scope_for_address(sa))
                .or_else(|| self.enclosing_scope_for_address(address));
            let parsed =
                self.parse_symbolic_offset_input(trimmed, address, address.0 <= 0x00FF, scope)?;
            return Ok(LabelOrAddressOffsetPlan::AddressOffset(parsed));
        }

        let _ = self.normalize_and_validate_plain_user_label(address, trimmed, is_local)?;
        Ok(LabelOrAddressOffsetPlan::PlainLabel)
    }

    /// Checks a `LabelDialog` (`l`) or `r2000_set_label_name` submission without mutating [`AppState`].
    ///
    /// Parses symbolic offset expressions when `name` contains `'+'` or `'-'`. Enforces assembler
    /// identifier syntax and rejects duplicate names for non-empty plain labels. Empty or
    /// whitespace-only inputs pass (`Ok(())`) as label clears.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic message if `name` is an invalid identifier, collides with an existing
    /// label at another address, or fails symbolic offset expression parsing.
    ///
    /// # Examples
    ///
    /// ```
    /// use regenerator2000_core::state::{Addr, AppState, Label, LabelKind, LabelType};
    ///
    /// let mut state = AppState::new();
    /// state.labels.insert(
    ///     Addr(0x00FB),
    ///     vec![Label {
    ///         name: "zp_ptr".to_string(),
    ///         kind: LabelKind::User,
    ///         label_type: LabelType::ZeroPagePointer,
    ///     }],
    /// );
    ///
    /// // Valid plain identifier and valid Tier 2 symbolic offset expression:
    /// assert!(state.validate_user_label_or_address_offset(Addr(0x00FC), "ptr_hi", false, None).is_ok());
    /// assert!(state.validate_user_label_or_address_offset(Addr(0x00FC), "zp_ptr + 1", false, None).is_ok());
    ///
    /// // Rejects corrupt plain-label syntax and duplicate label names:
    /// assert!(state.validate_user_label_or_address_offset(Addr(0x00FC), "(corrupt)", false, None).is_err());
    /// assert!(state.validate_user_label_or_address_offset(Addr(0x00FC), "zp_ptr", false, None).is_err());
    /// ```
    pub fn validate_user_label_or_address_offset(
        &self,
        address: Addr,
        name: &str,
        is_local: bool,
        site_addr: Option<Addr>,
    ) -> Result<(), String> {
        self.plan_user_label_or_address_offset(address, name, is_local, site_addr)
            .map(|_| ())
    }

    /// Applies or clears a user label or Tier 2 `address_offset` at `address` with mutual exclusion:
    ///
    /// - When `name` contains `'+'` or `'-'`, parses it via [`AppState::parse_symbolic_offset_input`]
    ///   and sets a Tier 2 `address_offset` at `address`. Clears any user label at `address` and any
    ///   shadowing Tier 1 `operand_offset` at `site_addr`.
    /// - When `name` is a plain identifier or `""`, sets or clears the [`LabelKind::User`] label at
    ///   `address` and clears any active Tier 2 `address_offset`. Runs analysis when `auto_analyze` is
    ///   `true` and records one [`Command::Batch`](crate::commands::Command::Batch).
    ///
    /// # Errors
    ///
    /// Returns a diagnostic message without mutating state if `name` fails expression parsing,
    /// contains invalid plain-label characters, or duplicates an existing label name.
    pub fn apply_user_label_or_address_offset(
        &mut self,
        address: Addr,
        name: &str,
        is_local: bool,
        site_addr: Option<Addr>,
    ) -> Result<Option<SymbolicOffset>, String> {
        match self.plan_user_label_or_address_offset(address, name, is_local, site_addr)? {
            LabelOrAddressOffsetPlan::AddressOffset(parsed) => {
                self.apply_address_offsets(&[(address, parsed)], site_addr);
                Ok(parsed)
            }
            LabelOrAddressOffsetPlan::PlainLabel => {
                let label_cmd = self.create_set_user_label_command(address, name, is_local)?;
                let mut commands = Vec::with_capacity(3);
                label_cmd.apply(self);
                commands.push(label_cmd);

                if let Some(old_offset) =
                    self.annotations.get(address).and_then(|e| e.address_offset)
                {
                    let clear_offset_cmd = crate::commands::Command::SetAddressOffset {
                        address,
                        new_offset: None,
                        old_offset: Some(old_offset),
                    };
                    clear_offset_cmd.apply(self);
                    commands.push(clear_offset_cmd);
                }

                self.finalize_symbolic_mutation_batch(commands);
                Ok(None)
            }
        }
    }
}

/// Checked outcome of [`AppState::plan_user_label_or_address_offset`].
enum LabelOrAddressOffsetPlan {
    AddressOffset(Option<SymbolicOffset>),
    PlainLabel,
}

#[derive(Debug)]
struct CachedBaseLookup {
    token: String,
    scope: Option<(Addr, Addr)>,
    result: Result<Addr, String>,
}

/// Memoizes `(base_token, current_scope) -> Addr` across multi-address range evaluations
/// so scanning a `K`-entry table or struct range against `L` project labels runs in
/// `O(L + K log S)` rather than `O(K * L)`.
#[derive(Debug, Default)]
pub(crate) struct SymbolicBaseCache {
    last: Option<CachedBaseLookup>,
}

/// Strips surrounding whitespace and any balanced outer `(...)` wrapper pairs from `input`.
#[must_use]
fn strip_balanced_parens(input: &str) -> &str {
    let mut s = input.trim();
    while let Some(inner) = s.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        let mut depth = 0i32;
        let mut balanced = true;
        for ch in inner.chars() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        balanced = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if balanced && depth == 0 {
            s = inner.trim();
        } else {
            break;
        }
    }
    s
}

/// Parses an unsigned displacement literal (`$hex`, `0xhex`, or decimal) and its [`OffsetRadix`].
fn parse_unsigned_offset_literal(raw: &str) -> Result<(u32, OffsetRadix), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Missing displacement magnitude".to_string());
    }
    if let Some(hex) = trimmed.strip_prefix('$') {
        let val = u32::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid hexadecimal displacement '{trimmed}'"))?;
        return Ok((val, OffsetRadix::Hex));
    }
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        let val = u32::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid hexadecimal displacement '{trimmed}'"))?;
        return Ok((val, OffsetRadix::Hex));
    }
    if !trimmed.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid displacement '{trimmed}'"));
    }
    let val = trimmed
        .parse::<u32>()
        .map_err(|_| format!("Invalid decimal displacement '{trimmed}'"))?;
    Ok((val, OffsetRadix::Decimal))
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

    #[test]
    fn test_load_system_assets_respects_exclude_labels() {
        let mut app_state = AppState::new();
        app_state.settings.system = crate::state::System::from("Commodore 64".to_string());
        app_state.settings.exclude_well_known_labels = true;

        app_state.load_system_assets();

        let d020 = Addr(0xD020);
        assert!(app_state.excluded_addresses.contains(&d020));
        assert!(
            !app_state.labels.contains_key(&d020),
            "Label for D020 should be excluded"
        );
    }

    #[test]
    fn test_file_info_empty() {
        let mut app_state = AppState::new();
        let info = app_state.file_info();
        assert_eq!(info.start_addr, Addr(0x0801));
        assert_eq!(info.end_addr, Addr(0x0801));
        assert_eq!(info.size, 0);
        assert_eq!(info.entry_point, None);
        assert!(!info.is_packed);
        assert_eq!(info.packer_name, None);

        app_state.settings.system = crate::state::System::Vic20;
        let vic_info = app_state.file_info();
        assert_eq!(vic_info.start_addr, Addr(0x1001));
        assert_eq!(vic_info.end_addr, Addr(0x1001));
    }

    #[test]
    fn test_perform_analysis_undo_restores_exact_labels() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0xC000);
        // JSR $C005 ; NOP ; NOP ; RTS
        app_state.raw_data = vec![0x20, 0x05, 0xC0, 0xEA, 0xEA, 0x60];
        app_state.block_types = vec![BlockType::Code; 6];
        // Pre-existing stale auto label at $C003 that analyze() will remove
        app_state.labels.insert(
            Addr(0xC003),
            vec![Label {
                name: "s_c003".to_string(),
                kind: LabelKind::Auto,
                label_type: LabelType::Subroutine,
            }],
        );
        let original_labels = app_state.labels.clone();

        let (cmd, _) = app_state.perform_analysis();
        app_state.push_command(cmd);
        assert!(app_state.labels.contains_key(&Addr(0xC005)));
        assert!(!app_state.labels.contains_key(&Addr(0xC003)));

        app_state.undo_last_command();
        assert_eq!(app_state.labels, original_labels);
        assert!(!app_state.labels.contains_key(&Addr(0xC005)));
    }

    #[test]
    fn test_file_info_unpacked_without_sys_stub() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.entry_point = Some(Addr(0x8100));
        app_state.raw_data = vec![0xEA; 100];
        let info = app_state.file_info();
        assert_eq!(info.start_addr, Addr(0x0801));
        assert_eq!(info.entry_point, Some(Addr(0x8100)));
        assert!(!info.is_packed);
        assert_eq!(info.packer_name, None);
    }

    #[test]
    fn test_file_info_with_sys_stub() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        // 10 SYS 2061 (header: $0801: 0B 08 0A 00 9E 20 32 30 36 31 00 00 00)
        app_state.raw_data = vec![
            0x0E, 0x08, 0x0A, 0x00, 0x9E, 0x20, 0x32, 0x30, 0x36, 0x31, 0x00, 0x00, 0x00,
        ];
        let info = app_state.file_info();
        assert_eq!(info.start_addr, Addr(0x0801));
        assert_eq!(info.end_addr, Addr(0x080D));
        assert_eq!(info.size, 13);
        assert_eq!(info.entry_point, Some(Addr(2061)));
        assert!(!info.is_packed);
    }

    #[test]
    fn test_file_info_full_64k() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0000);
        app_state.raw_data = vec![0xEA; 65536];
        let info = app_state.file_info();
        assert_eq!(info.start_addr, Addr(0x0000));
        assert_eq!(info.end_addr, Addr(0xFFFF));
        assert_eq!(info.size, 65536);
    }

    #[test]
    fn test_is_external_full_64k() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0000);
        app_state.raw_data = vec![0xEA; 65536];
        assert!(!app_state.is_external(Addr(0x0000)));
        assert!(!app_state.is_external(Addr(0x0810)));
        assert!(!app_state.is_external(Addr(0xFFFF)));
    }

    #[test]
    fn file_info_packer_name_unchanged_for_corpus_samples() {
        use std::path::Path;

        let samples: &[(&str, Option<&str>)] = &[
            ("c64_mule.dali.prg", Some("Dali")),
            ("c64_mule.exo3.prg", Some("Exomizer 3.0")),
            ("c64_mule.pucrunch.prg", Some("PUCrunch")),
            ("c64_mule.tscrunch_x.prg", Some("TSCrunch v1.3+")),
            ("c64_CopperBooze.byte_boozer2.prg", Some("ByteBoozer")),
            ("c64_traveller.tiny_crunch.prg", Some("TinyCrunch")),
            ("c64_boo_alz64.prg", Some("ALZ64/Quiss")),
            ("c64_wavehero_alz64.prg", Some("ALZ64/Kabuto")),
            ("c64_8_bit_ball.meanteam_cruncher.prg", None),
            ("c64_goblin.unk.prg", None),
        ];

        let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/6502");
        for &(filename, expected_packer) in samples {
            let prg = std::fs::read(fixture_dir.join(filename))
                .unwrap_or_else(|e| panic!("{filename}: {e}"));
            let load_addr = u16::from_le_bytes([prg[0], prg[1]]);
            let mut app_state = AppState::new();
            app_state.origin = Addr(load_addr);
            app_state.raw_data = prg[2..].to_vec();

            let info = app_state.file_info();
            assert_eq!(
                info.packer_name, expected_packer,
                "{filename}: unexpected packer_name"
            );
            assert_eq!(
                info.is_packed,
                expected_packer.is_some(),
                "{filename}: unexpected is_packed"
            );
        }
    }

    #[test]
    fn file_info_does_not_allocate_per_spec() {
        use std::alloc::{GlobalAlloc, Layout, System as StdAlloc};
        use std::cell::Cell;

        struct CountingAlloc;

        thread_local! {
            static TRACKING: Cell<bool> = const { Cell::new(false) };
            static ALLOCS: Cell<usize> = const { Cell::new(0) };
        }

        // SAFETY: Delegates directly to `std::alloc::System`; only increments a thread-local
        // `Cell<usize>` counter when `TRACKING` is enabled on the current thread.
        unsafe impl GlobalAlloc for CountingAlloc {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                TRACKING.with(|t| {
                    if t.get() {
                        ALLOCS.with(|a| a.set(a.get().saturating_add(1)));
                    }
                });
                unsafe { StdAlloc.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                unsafe { StdAlloc.dealloc(ptr, layout) }
            }
        }

        #[global_allocator]
        static GLOBAL: CountingAlloc = CountingAlloc;

        fn count_allocations<F: FnOnce() -> R, R>(f: F) -> (R, usize) {
            ALLOCS.with(|a| a.set(0));
            TRACKING.with(|t| t.set(true));
            let result = f();
            TRACKING.with(|t| t.set(false));
            let count = ALLOCS.with(Cell::get);
            (result, count)
        }

        let spec_count = crate::packers::registry::PACKERS.len();
        assert!(spec_count >= 21);

        // 1. Direct `registry::detect` across all 21 specs on an unpacked 64 KB image performs
        // zero heap allocations.
        let mem = vec![0xEAu8; 0x10000];
        let image =
            crate::packers::LoadedImage::new(&mem, 0x0801, 0x2000, &crate::state::System::C64);
        let (matched, detect_allocs) =
            count_allocations(|| crate::packers::registry::detect(&image));
        assert!(matched.is_none());
        assert_eq!(
            detect_allocs, 0,
            "registry::detect across {spec_count} specs must perform 0 heap allocations"
        );

        // 2. `AppState::file_info()` allocates only the single 64 KB working buffer (1 allocation),
        // and never allocates per spec.
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 512];
        app_state.entropy = Some(app_state.entropy());

        let (info, file_info_allocs) = count_allocations(|| app_state.file_info());
        assert!(!info.is_packed);
        assert!(
            file_info_allocs <= 1,
            "file_info() allocated {file_info_allocs} times (expected <= 1 across {spec_count} specs)"
        );
    }

    #[test]
    fn test_create_set_comment_command() {
        let mut app_state = AppState::new();
        let addr = Addr(0x1000);

        let set_cmd =
            app_state.create_set_comment_command(addr, Some("side note"), CommentKind::Side);
        assert_eq!(
            set_cmd,
            crate::commands::Command::SetUserSideComment {
                address: addr,
                new_comment: Some("side note".to_string()),
                old_comment: None,
            }
        );
        set_cmd.apply(&mut app_state);

        // Clearing via empty or whitespace-only string normalizes `new_comment` to `None`
        let clear_cmd = app_state.create_set_comment_command(addr, Some("   "), CommentKind::Side);
        assert_eq!(
            clear_cmd,
            crate::commands::Command::SetUserSideComment {
                address: addr,
                new_comment: None,
                old_comment: Some("side note".to_string()),
            }
        );
        clear_cmd.apply(&mut app_state);
        assert!(app_state.annotations.get(addr).is_none());
    }

    #[test]
    fn test_mutual_exclusion_user_label_and_address_offset_multi_step_undo_redo() {
        use super::super::types::{XRef, XRefKind};

        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: STA $FB (85 FB), $1002: STA $FC (85 FC)
        state.raw_data = vec![0x85, 0xFB, 0x85, 0xFC];
        state.block_types = vec![BlockType::Code; 4];
        state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        // Establish baseline Step 0 before recording undoable transitions.
        state.perform_analysis();
        state.undo_stack = crate::commands::UndoStack::new();

        let operand_at_1002 = |s: &AppState| -> String {
            s.disassembly
                .iter()
                .find(|l| l.external_label_address.is_none() && l.address == Addr(0x1002))
                .expect("$1002 instruction must exist")
                .operand
                .clone()
        };
        let has_preamble_fc = |s: &AppState| -> bool {
            s.get_external_label_definitions(false)
                .iter()
                .any(|l| l.external_label_address == Some(Addr(0x00FC)))
        };

        let assert_step_0 = |s: &AppState, label: &str| {
            let fc_labels = s.labels.get(&Addr(0x00FC)).expect(label);
            assert_eq!(fc_labels[0].name, "zpa_FC", "{label}");
            assert_eq!(fc_labels[0].kind, LabelKind::Auto, "{label}");
            assert!(
                s.annotations
                    .get(Addr(0x00FC))
                    .and_then(|e| e.address_offset)
                    .is_none(),
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FB)).unwrap(),
                &vec![XRef::new(Addr(0x1000), XRefKind::Write)],
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FC)).unwrap(),
                &vec![XRef::new(Addr(0x1002), XRefKind::Write)],
                "{label}"
            );
            assert!(has_preamble_fc(s), "{label}");
            assert_eq!(operand_at_1002(s), "zpa_FC", "{label}");
        };

        let assert_step_1 = |s: &AppState, label: &str| {
            let fc_labels = s.labels.get(&Addr(0x00FC)).expect(label);
            assert_eq!(fc_labels[0].name, "temp_fc", "{label}");
            assert_eq!(fc_labels[0].kind, LabelKind::User, "{label}");
            assert!(
                s.annotations
                    .get(Addr(0x00FC))
                    .and_then(|e| e.address_offset)
                    .is_none(),
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FC)).unwrap(),
                &vec![XRef::new(Addr(0x1002), XRefKind::Write)],
                "{label}"
            );
            assert!(has_preamble_fc(s), "{label}");
            assert_eq!(operand_at_1002(s), "temp_fc", "{label}");
        };

        let expected_sym = SymbolicOffset {
            base: Addr(0x00FB),
            offset: 1,
            radix: OffsetRadix::Decimal,
        };
        let assert_step_2 = |s: &AppState, label: &str| {
            assert!(
                !s.labels.contains_key(&Addr(0x00FC)),
                "{label}: user label must be cleared before perform_analysis"
            );
            assert_eq!(
                s.annotations
                    .get(Addr(0x00FC))
                    .and_then(|e| e.address_offset),
                Some(expected_sym),
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FB)).unwrap(),
                &vec![
                    XRef::new(Addr(0x1000), XRefKind::Write),
                    XRef::new(Addr(0x1002), XRefKind::Write),
                ],
                "{label}"
            );
            assert!(!s.cross_refs.contains_key(&Addr(0x00FC)), "{label}");
            assert!(!has_preamble_fc(s), "{label}");
            assert_eq!(operand_at_1002(s), "zp_ptr + 1", "{label}");
        };

        let assert_step_3 = |s: &AppState, label: &str| {
            let fc_labels = s.labels.get(&Addr(0x00FC)).expect(label);
            assert_eq!(fc_labels[0].name, "ptr_hi", "{label}");
            assert_eq!(fc_labels[0].kind, LabelKind::User, "{label}");
            assert!(
                s.annotations
                    .get(Addr(0x00FC))
                    .and_then(|e| e.address_offset)
                    .is_none(),
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FB)).unwrap(),
                &vec![XRef::new(Addr(0x1000), XRefKind::Write)],
                "{label}"
            );
            assert_eq!(
                s.cross_refs.get(&Addr(0x00FC)).unwrap(),
                &vec![XRef::new(Addr(0x1002), XRefKind::Write)],
                "{label}"
            );
            assert!(has_preamble_fc(s), "{label}");
            assert_eq!(operand_at_1002(s), "ptr_hi", "{label}");
        };

        // Checkpoint 0 (Baseline)
        assert_step_0(&state, "Checkpoint 0");

        // Step 1: Set User label "temp_fc" at $00FC
        state
            .apply_user_label_or_address_offset(Addr(0x00FC), "temp_fc", false, None)
            .unwrap();
        assert_step_1(&state, "Step 1");

        // Step 2: Apply Tier 2 address_offset ("zp_ptr + 1") at $00FC
        let applied_sym = state
            .apply_user_label_or_address_offset(Addr(0x00FC), "zp_ptr + 1", false, None)
            .unwrap();
        assert_eq!(applied_sym, Some(expected_sym));
        assert_step_2(&state, "Step 2");

        // Step 3: Apply plain User label "ptr_hi" at $00FC (clears Tier 2 address_offset)
        state
            .apply_user_label_or_address_offset(Addr(0x00FC), "ptr_hi", false, None)
            .unwrap();
        assert_step_3(&state, "Step 3");

        // Step 4: Clear User label "" at $00FC (regenerates Auto zpa_FC, no ghost offset)
        state
            .apply_user_label_or_address_offset(Addr(0x00FC), "", false, None)
            .unwrap();
        assert_step_0(&state, "Step 4");

        // 4 consecutive Undo calls: 4 -> 3 -> 2 -> 1 -> 0
        state.undo_last_command();
        assert_step_3(&state, "Undo 4 -> 3");
        state.undo_last_command();
        assert_step_2(&state, "Undo 3 -> 2");
        state.undo_last_command();
        assert_step_1(&state, "Undo 2 -> 1");
        state.undo_last_command();
        assert_step_0(&state, "Undo 1 -> 0");

        // 4 consecutive Redo calls: 0 -> 1 -> 2 -> 3 -> 4
        state.redo_last_command();
        assert_step_1(&state, "Redo 0 -> 1");
        state.redo_last_command();
        assert_step_2(&state, "Redo 1 -> 2");
        state.redo_last_command();
        assert_step_3(&state, "Redo 2 -> 3");
        state.redo_last_command();
        assert_step_0(&state, "Redo 3 -> 4");
    }

    #[test]
    fn test_apply_global_address_offset_from_site_clears_shadowing_tier1_operand_offset() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: STA $FB (85 FB), $1002: STA $FC (85 FC)
        state.raw_data = vec![0x85, 0xFB, 0x85, 0xFC];
        state.block_types = vec![BlockType::Code; 4];
        state.labels.insert(
            Addr(0x00FA),
            vec![Label {
                name: "alt_base".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        state.perform_analysis();
        state.undo_stack = crate::commands::UndoStack::new();

        let tier1_sym = SymbolicOffset {
            base: Addr(0x00FA),
            offset: 2,
            radix: OffsetRadix::Decimal,
        };
        state.apply_operand_offsets(&[(Addr(0x1002), Some(tier1_sym))]);
        let op_1002 = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1002))
            .unwrap();
        assert_eq!(op_1002.operand, "alt_base + 2");

        // Apply Tier 2 address_offset on $00FC from site_addr = Some($1002)
        let tier2_sym = SymbolicOffset {
            base: Addr(0x00FB),
            offset: 1,
            radix: OffsetRadix::Decimal,
        };
        state.apply_address_offsets(&[(Addr(0x00FC), Some(tier2_sym))], Some(Addr(0x1002)));

        assert!(
            state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none(),
            "Shadowing Tier 1 operand_offset at $1002 must be cleared"
        );
        assert_eq!(
            state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset),
            Some(tier2_sym)
        );
        assert_eq!(
            state
                .disassembly
                .iter()
                .find(|l| l.address == Addr(0x1002))
                .unwrap()
                .operand,
            "zp_ptr + 1"
        );

        // Undo restores Tier 1 alt_base + 2 at $1002 and clears Tier 2 at $00FC
        state.undo_last_command();
        assert_eq!(
            state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset),
            Some(tier1_sym)
        );
        assert!(
            state
                .annotations
                .get(Addr(0x00FC))
                .and_then(|e| e.address_offset)
                .is_none()
        );
        assert_eq!(
            state
                .disassembly
                .iter()
                .find(|l| l.address == Addr(0x1002))
                .unwrap()
                .operand,
            "alt_base + 2"
        );

        // Redo re-applies Tier 2 zp_ptr + 1 and clears shadowing Tier 1
        state.redo_last_command();
        assert!(
            state
                .annotations
                .get(Addr(0x1002))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(
            state
                .disassembly
                .iter()
                .find(|l| l.address == Addr(0x1002))
                .unwrap()
                .operand,
            "zp_ptr + 1"
        );

        // Unrelated Tier 1 `operand_offset` on $1000 (`STA $FB` -> alt_base + 1) must NOT be
        // clobbered when setting a Tier 2 `address_offset` on a different target ($00FD) with
        // `site_addr = Some($1000)`!
        let unrelated_tier1 = SymbolicOffset {
            base: Addr(0x00FA),
            offset: 1,
            radix: OffsetRadix::Decimal,
        };
        state.apply_operand_offsets(&[(Addr(0x1000), Some(unrelated_tier1))]);
        let fd_tier2 = SymbolicOffset {
            base: Addr(0x00FB),
            offset: 2,
            radix: OffsetRadix::Decimal,
        };
        state.apply_address_offsets(&[(Addr(0x00FD), Some(fd_tier2))], Some(Addr(0x1000)));
        assert_eq!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(unrelated_tier1),
            "Unrelated Tier 1 operand_offset targeting $00FB must be preserved when setting Tier 2 on $00FD"
        );

        // Multi-update slice where the SECOND target ($00FB) matches $1000's Tier 1 offset still clears it
        let fe_tier2 = SymbolicOffset {
            base: Addr(0x00FB),
            offset: 3,
            radix: OffsetRadix::Decimal,
        };
        let fb_tier2 = SymbolicOffset {
            base: Addr(0x00FA),
            offset: 1,
            radix: OffsetRadix::Hex,
        };
        state.apply_address_offsets(
            &[
                (Addr(0x00FE), Some(fe_tier2)),
                (Addr(0x00FB), Some(fb_tier2)),
            ],
            Some(Addr(0x1000)),
        );
        assert!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none(),
            "Multi-update batch must still clear Tier 1 on $1000 when a later target_addr ($00FB) matches"
        );
    }

    #[test]
    fn test_apply_immediate_format_clears_stale_operand_offset_and_preserves_matching() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: LDA #$FF (A9 FF)
        state.raw_data = vec![0xA9, 0xFF];
        state.block_types = vec![BlockType::Code; 2];
        state.labels.insert(
            Addr(0xC100),
            vec![Label {
                name: "sub_C100".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        state.perform_analysis();
        state.undo_stack = crate::commands::UndoStack::new();

        let sym = SymbolicOffset {
            base: Addr(0xC100),
            offset: -1,
            radix: OffsetRadix::Decimal,
        };
        state.apply_immediate_format_with_offset(
            Addr(0x1000),
            Some(ImmediateFormat::LowByte(Addr(0xC0FF))),
            Some(sym),
        );
        assert_eq!(state.disassembly[0].operand, "#<(sub_C100 - 1)");

        // Matching target ($C0FF) keeps operand_offset intact
        state.apply_immediate_format(Addr(0x1000), Some(ImmediateFormat::LowByte(Addr(0xC0FF))));
        assert_eq!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(sym)
        );

        // Switching to Hex clears stale operand_offset in the same undoable batch
        state.apply_immediate_format(Addr(0x1000), Some(ImmediateFormat::Hex));
        assert!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
        assert_eq!(state.disassembly[0].operand, "#$ff");

        // Single undo restores both LowByte($C0FF) and operand_offset
        state.undo_last_command();
        assert_eq!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset),
            Some(sym)
        );
        assert_eq!(state.disassembly[0].operand, "#<(sub_C100 - 1)");

        // Switching to LowByte of a DIFFERENT target ($D0FF) also clears stale operand_offset
        state.apply_immediate_format(Addr(0x1000), Some(ImmediateFormat::LowByte(Addr(0xD0FF))));
        assert!(
            state
                .annotations
                .get(Addr(0x1000))
                .and_then(|e| e.operand_offset)
                .is_none()
        );
    }

    #[test]
    fn test_parse_symbolic_offset_input_valid_forms_and_radices() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 0x30];
        state.block_types = vec![BlockType::Code; 0x30];

        for (addr, name, ltype) in [
            (0x00FB, "zp_ptr", LabelType::UserDefined),
            (0x0400, "SCREEN_RAM", LabelType::UserDefined),
            (0xC100, "sub_C100", LabelType::Subroutine),
            (0x0100, "stack_tab", LabelType::UserDefined),
            (0x1000, "irq", LabelType::Subroutine),
            (0x00FD, "loop_var", LabelType::LocalUserDefined),
            (0x1010, "scoped_local", LabelType::LocalUserDefined),
        ] {
            state.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type: ltype,
                }],
            );
        }
        state
            .annotations
            .update(Addr(0x1000), |e| e.scope = Some(Addr(0x1020)));

        // 1. Decimal positive
        assert_eq!(
            state
                .parse_symbolic_offset_input("zp_ptr + 1", Addr(0x00FC), true, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );

        // 2. Hex `$` negative
        assert_eq!(
            state
                .parse_symbolic_offset_input("sub_C100 - $10", Addr(0xC0F0), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -16,
                radix: OffsetRadix::Hex,
            })
        );

        // 3. Hex `0x` positive
        assert_eq!(
            state
                .parse_symbolic_offset_input("SCREEN_RAM + 0x28", Addr(0x0428), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x0400),
                offset: 40,
                radix: OffsetRadix::Hex,
            })
        );

        // 4. Bare symbol auto-displacement & zero-offset clear
        assert_eq!(
            state
                .parse_symbolic_offset_input("SCREEN_RAM", Addr(0x0428), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x0400),
                offset: 40,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("sub_C100", Addr(0xC0FF), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("zp_ptr", Addr(0x00FB), true, None)
                .unwrap(),
            None
        );

        // 5. Hex/numeric base & outer parentheses
        assert_eq!(
            state
                .parse_symbolic_offset_input("$C100 - 1", Addr(0xC0FF), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("  (0xC100 - 1)  ", Addr(0xC0FF), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("$0400", Addr(0x0428), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x0400),
                offset: 40,
                radix: OffsetRadix::Decimal,
            })
        );

        // 6. Signed displacement shorthand
        assert_eq!(
            state
                .parse_symbolic_offset_input("+1", Addr(0x00FC), true, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x00FB),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("- $10", Addr(0xC0F0), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xC100),
                offset: -16,
                radix: OffsetRadix::Hex,
            })
        );

        // 7. Scoped & local labels (`_`, `.`, `@` inside scope, and `irq.` / `irq::` outside)
        let irq_scope = Some((Addr(0x1000), Addr(0x1020)));
        assert_eq!(state.enclosing_scope_for_address(Addr(0x1010)), irq_scope);
        for local_expr in ["_loop_var + 1", ".loop_var + 1", "@loop_var + 1"] {
            assert_eq!(
                state
                    .parse_symbolic_offset_input(local_expr, Addr(0x00FE), true, irq_scope)
                    .unwrap(),
                Some(SymbolicOffset {
                    base: Addr(0x00FD),
                    offset: 1,
                    radix: OffsetRadix::Decimal,
                }),
                "failed on {local_expr}"
            );
        }
        for scoped_expr in ["irq.scoped_local + 1", "irq::scoped_local + 1"] {
            assert_eq!(
                state
                    .parse_symbolic_offset_input(scoped_expr, Addr(0x1011), false, None)
                    .unwrap(),
                Some(SymbolicOffset {
                    base: Addr(0x1010),
                    offset: 1,
                    radix: OffsetRadix::Decimal,
                }),
                "failed on {scoped_expr}"
            );
        }
        // Unscoped local outside `irq` must not leak into explicit `irq.loop_var` lookups.
        assert!(
            state
                .parse_symbolic_offset_input("irq.loop_var + 1", Addr(0x00FE), true, None)
                .is_err()
        );

        // 8. Clear tokens
        for clear_tok in ["", "   ", "0", "+0", "-0", "$0", "0x0"] {
            assert_eq!(
                state
                    .parse_symbolic_offset_input(clear_tok, Addr(0x00FC), true, None)
                    .unwrap(),
                None,
                "clear token '{clear_tok}' should return Ok(None)"
            );
        }
    }

    #[test]
    fn test_parse_symbolic_offset_input_errors_bounds_and_zp_mode_enforcement() {
        let mut state = AppState::new();
        for (addr, name) in [(0x00FB, "zp_ptr"), (0x0100, "stack_tab")] {
            state.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::UserDefined,
                }],
            );
        }

        // 1. Target mismatch with actionable suggestion
        let mismatch_err = state
            .parse_symbolic_offset_input("zp_ptr + 2", Addr(0x00FC), true, None)
            .unwrap_err();
        assert!(mismatch_err.contains("$00FD"), "got: {mismatch_err}");
        assert!(mismatch_err.contains("$00FC"), "got: {mismatch_err}");
        assert!(mismatch_err.contains("zp_ptr + 1"), "got: {mismatch_err}");

        // 2. Unknown base symbol
        assert!(
            state
                .parse_symbolic_offset_input("missing_sym + 1", Addr(0x00FC), false, None)
                .is_err()
        );

        // 3. Non-wrapping 16-bit bounds (`base = effective_addr - offset`)
        assert!(
            state
                .parse_symbolic_offset_input("$0000 - 1", Addr(0xFFFF), false, None)
                .is_err()
        );
        assert!(
            state
                .parse_symbolic_offset_input("$FFFF + 1", Addr(0x0000), false, None)
                .is_err()
        );
        assert!(
            state
                .parse_symbolic_offset_input("+1", Addr(0x0000), false, None)
                .is_err(),
            "+1 at $0000 implies base = -1 (underflow)"
        );
        assert!(
            state
                .parse_symbolic_offset_input("-1", Addr(0xFFFF), false, None)
                .is_err(),
            "-1 at $FFFF implies base = $10000 (overflow)"
        );
        // Exact boundary hits remain valid
        assert_eq!(
            state
                .parse_symbolic_offset_input("-1", Addr(0x0000), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x0001),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("+1", Addr(0xFFFF), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0xFFFE),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );

        // 4. `i16` overflow (`diff = +65535`)
        assert!(
            state
                .parse_symbolic_offset_input("$0000", Addr(0xFFFF), false, None)
                .is_err()
        );

        // 5. ZP vs Forced-Absolute Rule (`stack_tab` = $0100 at effective $00FF)
        let zp_err = state
            .parse_symbolic_offset_input("stack_tab - 1", Addr(0x00FF), true, None)
            .unwrap_err();
        assert!(zp_err.contains("<= $00FF"), "got: {zp_err}");

        assert_eq!(
            state
                .parse_symbolic_offset_input("stack_tab - 1", Addr(0x00FF), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x0100),
                offset: -1,
                radix: OffsetRadix::Decimal,
            })
        );

        // 6. Multi-byte UTF-8 inputs must return Err cleanly without char-boundary slicing panics
        for utf8_input in ["ä + 1", "ä", "ä.member + 1", "🚀 - 1", "🚀.x"] {
            assert!(
                state
                    .parse_symbolic_offset_input(utf8_input, Addr(0x00FC), false, None)
                    .is_err(),
                "Multi-byte UTF-8 input '{utf8_input}' must return Err cleanly"
            );
        }
        assert!(
            state
                .apply_user_label_or_address_offset(Addr(0x00FC), "ä + 1", false, None)
                .is_err()
        );

        // 7. Cross-scope `LocalUserDefined` isolation (`irq_a` at $1000..=$1020 vs `irq_b` at $2000..=$2020)
        for (addr, name, ltype) in [
            (0x1000, "irq_a", LabelType::Subroutine),
            (0x1010, "inner_local", LabelType::LocalUserDefined),
            (0x2000, "irq_b", LabelType::Subroutine),
        ] {
            state.labels.insert(
                Addr(addr),
                vec![Label {
                    name: name.to_string(),
                    kind: LabelKind::User,
                    label_type: ltype,
                }],
            );
        }
        state
            .annotations
            .update(Addr(0x1000), |e| e.scope = Some(Addr(0x1020)));
        state
            .annotations
            .update(Addr(0x2000), |e| e.scope = Some(Addr(0x2020)));

        // Valid inside owning scope `irq_a` and via `irq_a.inner_local`
        assert_eq!(
            state
                .parse_symbolic_offset_input(
                    "_inner_local + 1",
                    Addr(0x1011),
                    false,
                    Some((Addr(0x1000), Addr(0x1020))),
                )
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x1010),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );
        assert_eq!(
            state
                .parse_symbolic_offset_input("irq_a.inner_local + 1", Addr(0x1011), false, None)
                .unwrap(),
            Some(SymbolicOffset {
                base: Addr(0x1010),
                offset: 1,
                radix: OffsetRadix::Decimal,
            })
        );

        // Must NOT leak into sibling scope `irq_b` or unscoped global lookup
        assert!(
            state
                .parse_symbolic_offset_input("irq_b.inner_local + 1", Addr(0x1011), false, None)
                .is_err(),
            "irq_a's inner_local must not leak through irq_b.inner_local"
        );
        assert!(
            state
                .parse_symbolic_offset_input(
                    "_inner_local + 1",
                    Addr(0x1011),
                    false,
                    Some((Addr(0x2000), Addr(0x2020))),
                )
                .is_err(),
            "irq_a's inner_local must not leak when current_scope is irq_b"
        );
        assert!(
            state
                .parse_symbolic_offset_input("_inner_local + 1", Addr(0x1011), false, None)
                .is_err(),
            "Scoped inner_local must not leak into unscoped lookup"
        );
    }

    #[test]
    fn test_effective_target_at_address_and_in_range_code_modes_and_relative_exclusion() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: A5 FB       LDA $FB          (ZeroPage -> is_zp = true)
        // $1002: B1 FB       LDA ($FB),Y      (IndirectY -> is_zp = true)
        // $1004: BD FF 1F    LDA $1FFF,X      (AbsoluteX -> is_zp = false)
        // $1007: 6C 00 C0    JMP ($C000)      (Indirect -> is_zp = false)
        // $100A: A9 42       LDA #$42         (plain Immediate -> excluded)
        // $100C: A9 FF       LDA #$FF         (LowByte($C0FF) -> is_zp = false)
        // $100E: CA          DEX              (Implied -> excluded)
        // $100F: 0A          ASL A            (Accumulator -> excluded)
        // $1010: D0 EE       BNE $1000        (Relative -> excluded per Landmine 9!)
        state.raw_data = vec![
            0xA5, 0xFB, // $1000
            0xB1, 0xFB, // $1002
            0xBD, 0xFF, 0x1F, // $1004
            0x6C, 0x00, 0xC0, // $1007
            0xA9, 0x42, // $100A
            0xA9, 0xFF, // $100C
            0xCA, // $100E
            0x0A, // $100F
            0xD0, 0xEE, // $1010
        ];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.annotations.update(Addr(0x100C), |e| {
            e.immediate_format = Some(ImmediateFormat::LowByte(Addr(0xC0FF)));
        });

        assert_eq!(
            state.effective_target_at_address(Addr(0x1000)),
            Some((Addr(0x1000), Addr(0x00FB), true))
        );
        assert_eq!(
            state.effective_target_at_address(Addr(0x1002)),
            Some((Addr(0x1002), Addr(0x00FB), true))
        );
        assert_eq!(
            state.effective_target_at_address(Addr(0x1004)),
            Some((Addr(0x1004), Addr(0x1FFF), false))
        );
        assert_eq!(
            state.effective_target_at_address(Addr(0x1007)),
            Some((Addr(0x1007), Addr(0xC000), false))
        );
        assert_eq!(state.effective_target_at_address(Addr(0x100A)), None);
        assert_eq!(
            state.effective_target_at_address(Addr(0x100C)),
            Some((Addr(0x100C), Addr(0xC0FF), false))
        );
        assert_eq!(state.effective_target_at_address(Addr(0x100E)), None);
        assert_eq!(state.effective_target_at_address(Addr(0x100F)), None);
        assert_eq!(state.effective_target_at_address(Addr(0x1010)), None);

        // Mid-instruction operand byte ($1005 inside LDA $1FFF,X) must NOT misdecode as an opcode
        assert_eq!(state.effective_target_at_address(Addr(0x1005)), None);

        let all = state.effective_targets_in_range(Addr(0x1000), Addr(0x1011));
        assert_eq!(
            all,
            vec![
                (Addr(0x1000), Addr(0x00FB), true),
                (Addr(0x1002), Addr(0x00FB), true),
                (Addr(0x1004), Addr(0x1FFF), false),
                (Addr(0x1007), Addr(0xC000), false),
                (Addr(0x100C), Addr(0xC0FF), false),
            ]
        );
    }

    #[test]
    fn test_effective_targets_in_range_address_and_split_tables_backward_scan_and_dedup() {
        let mut state = AppState::new();
        state.origin = Addr(0x2000);
        // 1. $2000..=$2005: BlockType::Address (3 words: $C0FF, $C1FF, $C2FF)
        // 2. $2006..=$2009: LoHiAddress Sub-block 1 (2 pairs: $D0FF, $D1FF)
        // 3. $200A..=$200F: LoHiAddress Sub-block 2 (splitter at $200A, 3 pairs: $E0FF, $E1FF, $E2FF)
        // 4. $2010..=$2013: HiLoAddress (2 pairs: $F0FF, $F1FF)
        state.raw_data = vec![
            0xFF, 0xC0, 0xFF, 0xC1, 0xFF, 0xC2, // $2000..=$2005 Address
            0xFF, 0xFF, 0xD0, 0xD1, // $2006..=$2009 LoHi Sub-block 1
            0xFF, 0xFF, 0xFF, 0xE0, 0xE1, 0xE2, // $200A..=$200F LoHi Sub-block 2
            0xF0, 0xF1, 0xFF, 0xFF, // $2010..=$2013 HiLo
        ];
        let mut block_types = vec![BlockType::Address; 6];
        block_types.extend(vec![BlockType::LoHiAddress; 10]);
        block_types.extend(vec![BlockType::HiLoAddress; 4]);
        state.block_types = block_types;
        state.splitters.insert(Addr(0x200A));

        // Add a midpoint label `table_hi` on the high-byte half ($200D) of Sub-block 2 to verify
        // standard 6502 `LDA table_hi,X` labels do not split `LoHiAddress` pairing.
        state.labels.insert(
            Addr(0x200D),
            vec![Label {
                name: "table_hi".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        // 1. Interior word of multi-word BlockType::Address
        assert_eq!(
            state.effective_targets_in_range(Addr(0x2002), Addr(0x2002)),
            vec![(Addr(0x2002), Addr(0xC1FF), false)]
        );

        // 2. Second-half backward scan on LoHiAddress Sub-block 2 (stopping at splitter $200A!)
        assert_eq!(
            state.effective_targets_in_range(Addr(0x200D), Addr(0x200F)),
            vec![
                (Addr(0x200A), Addr(0xE0FF), false),
                (Addr(0x200B), Addr(0xE1FF), false),
                (Addr(0x200C), Addr(0xE2FF), false),
            ]
        );
        assert_eq!(
            state.effective_target_at_address(Addr(0x200E)),
            Some((Addr(0x200B), Addr(0xE1FF), false))
        );

        // 3. Full-table deduplication across both halves ($200A..=$200F yields 3 pairs, NOT 6!)
        assert_eq!(
            state.effective_targets_in_range(Addr(0x200A), Addr(0x200F)),
            vec![
                (Addr(0x200A), Addr(0xE0FF), false),
                (Addr(0x200B), Addr(0xE1FF), false),
                (Addr(0x200C), Addr(0xE2FF), false),
            ]
        );

        // 4. HiLoAddress second-half backward scan ($2012..=$2013 -> canonical $2010..=$2011)
        assert_eq!(
            state.effective_targets_in_range(Addr(0x2012), Addr(0x2013)),
            vec![
                (Addr(0x2010), Addr(0xF0FF), false),
                (Addr(0x2011), Addr(0xF1FF), false),
            ]
        );
    }

    #[test]
    fn test_suggest_symbolic_offset_across_idioms_and_existing_tiers() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        // $1000: STA $FC (85 FC)
        // $1002: LDA $0428,X (BD 28 04)
        // $1005: LoHiAddress 1 pair ($C0FF -> FF C0)
        state.raw_data = vec![0x85, 0xFC, 0xBD, 0x28, 0x04, 0xFF, 0xC0];
        state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::Code,
            BlockType::LoHiAddress,
            BlockType::LoHiAddress,
        ];
        state.insert_user_labels(&[
            (0x00FB, "zp_ptr"),
            (0x0400, "SCREEN_RAM"),
            (0xC100, "sub_C100"),
        ]);

        // 1. ZP pointer high byte on Code instruction -> global true
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1000)), Addr(0x00FC), true),
            ("zp_ptr + 1".to_string(), true)
        );

        // 2. Screen stride on Code instruction -> global true
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1002)), Addr(0x0428), false),
            ("SCREEN_RAM + 40".to_string(), true)
        );

        // 3. RTS dispatch entry on LoHiAddress table -> global false
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1005)), Addr(0xC0FF), false),
            ("sub_C100 - 1".to_string(), false)
        );

        // 4. Existing Tier 1 overrides heuristic with global = false
        state.annotations.update(Addr(0x1002), |e| {
            e.operand_offset = Some(SymbolicOffset {
                base: Addr(0x0400),
                offset: 40,
                radix: OffsetRadix::Hex,
            });
        });
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1002)), Addr(0x0428), false),
            ("SCREEN_RAM + $28".to_string(), false)
        );
    }

    #[test]
    fn test_plain_user_label_validation_rejects_corrupt_syntax_and_normalizes_local_prefixes() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 0x30];
        state.block_types = vec![BlockType::Code; 0x30];
        state.insert_typed_user_labels(&[
            (0x00FB, "zp_ptr", LabelType::UserDefined),
            (0x1000, "irq_scope", LabelType::Subroutine),
            (0x1010, "local_base", LabelType::LocalUserDefined),
        ]);
        state
            .annotations
            .update(Addr(0x1000), |e| e.scope = Some(Addr(0x1020)));

        // 1. Corrupt plain global labels (`is_local = false`) rejected by both validator & mutator
        for bad_global in [
            "(foo)",
            "foo:bar",
            "a..b",
            "123",
            "foo!bar",
            "my label",
            "$0400",
            ".dot_global",
            "@at_global",
            "0abc",
        ] {
            assert!(
                state
                    .create_set_user_label_command(Addr(0x1004), bad_global, false)
                    .is_err(),
                "create_set_user_label_command should reject '{bad_global}'"
            );
            assert!(
                state
                    .validate_user_label_or_address_offset(Addr(0x1004), bad_global, false, None)
                    .is_err(),
                "validate_user_label_or_address_offset should reject '{bad_global}'"
            );
            assert!(
                state
                    .apply_user_label_or_address_offset(Addr(0x1004), bad_global, false, None)
                    .is_err(),
                "apply_user_label_or_address_offset should reject '{bad_global}'"
            );
            assert!(
                !state.labels.contains_key(&Addr(0x1004)),
                "Failed label '{bad_global}' must not mutate state.labels"
            );
        }

        // 2. Corrupt local labels (`is_local = true`) rejected even after single prefix strip
        for bad_local in [
            "(foo)", "foo:bar", "a..b", "123", "_1", ".1", "@1", "..loop", "@@loop", ".", "@", "_",
        ] {
            assert!(
                state
                    .create_set_user_label_command(Addr(0x1004), bad_local, true)
                    .is_err(),
                "Local label validation should reject '{bad_local}'"
            );
            assert!(
                state
                    .validate_user_label_or_address_offset(Addr(0x1004), bad_local, true, None)
                    .is_err(),
                "validate_user_label_or_address_offset(local) should reject '{bad_local}'"
            );
        }

        // 3. Duplicate label rejected by `validate_user_label_or_address_offset` without mutating
        assert!(
            state
                .validate_user_label_or_address_offset(Addr(0x1004), "zp_ptr", false, None)
                .is_err()
        );

        // 4. Valid local labels normalize optional single `_`, `.`, or `@` prefix to canonical name
        for raw_local in ["loop_1", "_loop_1", ".loop_1", "@loop_1"] {
            state
                .validate_user_label_or_address_offset(Addr(0x1004), raw_local, true, None)
                .expect("valid local label should pass validation");
            state
                .apply_user_label_or_address_offset(Addr(0x1004), raw_local, true, None)
                .expect("valid local label should apply");
            let applied = &state.labels.get(&Addr(0x1004)).unwrap()[0];
            assert_eq!(applied.name, "loop_1");
            assert_eq!(applied.label_type, LabelType::LocalUserDefined);
        }

        // 5. Clearing via `""` or whitespace `"   "` validates and clears cleanly
        state
            .validate_user_label_or_address_offset(Addr(0x1004), "", false, None)
            .unwrap();
        state
            .apply_user_label_or_address_offset(Addr(0x1004), "   ", false, None)
            .unwrap();
        assert!(!state.labels.contains_key(&Addr(0x1004)));

        // 6. `validate_user_label_or_address_offset` honors `site_addr` scope for offset expressions
        assert!(
            state
                .validate_user_label_or_address_offset(
                    Addr(0x1011),
                    "_local_base + 1",
                    false,
                    Some(Addr(0x1005)),
                )
                .is_ok()
        );
        assert!(
            state
                .validate_user_label_or_address_offset(
                    Addr(0x1025),
                    "_local_base + $15",
                    false,
                    None,
                )
                .is_err(),
            "Scoped _local_base must not validate outside scope when site_addr is None"
        );
    }

    #[test]
    fn test_is_symbolic_clear_input_balanced_and_unbalanced_parentheses() {
        for clear_tok in [
            "",
            "   ",
            "()",
            "( ( ) )",
            "0",
            "(0)",
            "( (0) )",
            "+0",
            "-0",
            "$0",
            "0x0",
            "0X0",
            "( + $0 )",
            "( - 0x0 )",
        ] {
            assert!(
                AppState::is_symbolic_clear_input(clear_tok),
                "Expected '{clear_tok}' to be recognized as symbolic clear input"
            );
        }

        let state = AppState::new();
        for non_clear in ["(0", "0)", "((0)", "(0) + (0)", "$0000", "zp_ptr"] {
            assert!(
                !AppState::is_symbolic_clear_input(non_clear),
                "'{non_clear}' must NOT be treated as a clear input"
            );
        }
        for unbalanced in ["(0", "0)", "((0)", "(0) + (0)"] {
            assert!(
                state
                    .parse_symbolic_offset_input(unbalanced, Addr(0x00FC), false, None)
                    .is_err(),
                "Unbalanced input '{unbalanced}' must return Err"
            );
        }
    }

    #[test]
    fn test_performance_64k_symbolic_offset_range_scopes_and_split_tables() {
        let state = AppState::build_64k_symbolic_offset_fixture();

        // Verify `O(log S)` `enclosing_scope_for_address` boundary exactness
        assert_eq!(state.enclosing_scope_for_address(Addr(0x0FFF)), None);
        assert_eq!(
            state.enclosing_scope_for_address(Addr(0x1000)),
            Some((Addr(0x1000), Addr(0x1020)))
        );
        assert_eq!(
            state.enclosing_scope_for_address(Addr(0x1020)),
            Some((Addr(0x1000), Addr(0x1020)))
        );
        assert_eq!(state.enclosing_scope_for_address(Addr(0x1021)), None);

        let split_targets = state.effective_targets_with_pair_in_range(Addr(0x4000), Addr(0x41FF));
        assert_eq!(split_targets.len(), 256);
        assert_eq!(
            split_targets[0],
            OperandTargetSite {
                site_addr: Addr(0x4000),
                raw_target: Addr(0x7FFF),
                is_zero_page_mode: false,
                second_pair_addr: Some(Addr(0x4100)),
            }
        );
        assert_eq!(
            split_targets[255],
            OperandTargetSite {
                site_addr: Addr(0x40FF),
                raw_target: Addr(0x8FEF),
                is_zero_page_mode: false,
                second_pair_addr: Some(Addr(0x41FF)),
            }
        );

        let mut cache = SymbolicBaseCache::default();
        let zero_scope = state.enclosing_scope_for_address(Addr(0x6000));
        assert_eq!(
            state
                .parse_symbolic_offset_input_cached(
                    "big_struct",
                    Addr(0x6000),
                    false,
                    zero_scope,
                    &mut cache,
                )
                .unwrap(),
            None
        );

        let mut resolved_fields = 0usize;
        for addr_u16 in 0x6001u16..=0x61FFu16 {
            let addr = Addr(addr_u16);
            let scope = state.enclosing_scope_for_address(addr);
            let parsed = state
                .parse_symbolic_offset_input_cached("big_struct", addr, false, scope, &mut cache)
                .unwrap();
            assert_eq!(
                parsed,
                Some(SymbolicOffset {
                    base: Addr(0x6000),
                    offset: (addr_u16 - 0x6000) as i16,
                    radix: OffsetRadix::Decimal,
                })
            );
            resolved_fields += 1;
        }
        assert_eq!(resolved_fields, 511);
    }

    #[test]
    fn test_system_labels_suggest_resolve_and_restore_on_offset_clear() {
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.settings.all_labels = true;
        state
            .settings
            .enabled_features
            .insert("Lower Page".to_string(), true);
        // $1000: STA $0428 (8D 28 04), $1003: JSR $FF84 (20 84 FF), $1006: RTS (60)
        state.raw_data = vec![0x8D, 0x28, 0x04, 0x20, 0x84, 0xFF, 0x60];
        state.block_types = vec![BlockType::Code; state.raw_data.len()];
        state.load_system_assets();
        let _ = state.perform_analysis();

        // Unreferenced `$0400` (`SCREEN_MEM`) and `$FF81` (`KERNAL_CINT`) stay out of `state.labels`.
        assert!(!state.labels.contains_key(&Addr(0x0400)));
        assert!(!state.labels.contains_key(&Addr(0xFF81)));
        assert_eq!(
            state
                .labels
                .get(&Addr(0xFF84))
                .and_then(|v| v.first())
                .map(|l| l.name.as_str()),
            Some("KERNAL_IOINIT")
        );

        // Unreferenced platform base (`KERNAL_CHROUT` at `$FFD2`) formats as `"KERNAL_CHROUT + 1"`
        // even when `$FFD3` has zero code XRefs so `$FFD2` remains only in `system_labels`.
        let unref_sym = state
            .apply_user_label_or_address_offset(Addr(0xFFD3), "KERNAL_CHROUT + 1", false, None)
            .unwrap()
            .unwrap();
        assert!(!state.labels.contains_key(&Addr(0xFFD2)));
        assert_eq!(
            state.format_symbolic_offset(unref_sym, None, false),
            "KERNAL_CHROUT + 1"
        );
        assert_eq!(
            state.suggest_symbolic_offset(None, Addr(0xFFD3), false),
            ("KERNAL_CHROUT + 1".to_string(), true)
        );
        state.apply_address_offsets(&[(Addr(0xFFD3), None)], None);

        // Suggest `SCREEN_MEM + 40` from `state.system_labels` on `STA $0428`.
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1000)), Addr(0x0428), false),
            ("SCREEN_MEM + 40".to_string(), true)
        );

        // Suggestion and base resolution skip excluded addresses.
        state.excluded_addresses.insert(Addr(0x0400));
        assert_eq!(
            state.suggest_symbolic_offset(Some(Addr(0x1000)), Addr(0x0428), false),
            ("SYS_TAPEBUF + 236".to_string(), true)
        );
        assert!(
            state
                .parse_symbolic_offset_input("SCREEN_MEM + 40", Addr(0x0428), false, None)
                .is_err()
        );
        state.excluded_addresses.remove(&Addr(0x0400));

        // Parse and apply `"SCREEN_MEM + 40"`, emitting `SCREEN_MEM = $0400` and `sta SCREEN_MEM + 40`.
        let parsed = state
            .parse_symbolic_offset_input("SCREEN_MEM + 40", Addr(0x0428), false, None)
            .unwrap();
        state.apply_address_offsets(&[(Addr(0x0428), parsed)], Some(Addr(0x1000)));
        assert!(state.disassembly.iter().any(|l| {
            l.external_label_address == Some(Addr(0x0400)) && l.mnemonic.contains("SCREEN_MEM")
        }));
        let sta_line = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(sta_line.operand, "SCREEN_MEM + 40");

        // Set `KERNAL_CINT + 3` on `$FF84`, then clear it and check that `KERNAL_IOINIT` returns.
        state
            .apply_user_label_or_address_offset(Addr(0xFF84), "KERNAL_CINT + 3", false, None)
            .unwrap();
        let jsr_ff84 = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(jsr_ff84.operand, "KERNAL_CINT + 3");

        state.apply_address_offsets(&[(Addr(0xFF84), None)], None);
        assert_eq!(
            state
                .labels
                .get(&Addr(0xFF84))
                .and_then(|v| v.first())
                .map(|l| (&l.name as &str, &l.kind)),
            Some(("KERNAL_IOINIT", &LabelKind::System))
        );
        let jsr_restored = state
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1003) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(jsr_restored.operand, "KERNAL_IOINIT");

        // With `auto_analyze = false`, binding `KERNAL_CHROUT + 1` materializes `KERNAL_CHROUT = $ffd2`
        // for disassembly and reverses cleanly on undo/redo.
        let mut manual = AppState::new();
        manual.origin = Addr(0x1000);
        manual.settings.all_labels = true;
        manual.raw_data = vec![0x20, 0xD3, 0xFF, 0x60]; // JSR $FFD3; RTS
        manual.block_types = vec![BlockType::Code; 4];
        manual.load_system_assets();
        let _ = manual.perform_analysis();
        manual.settings.auto_analyze = false;
        assert!(!manual.labels.contains_key(&Addr(0xFFD2)));

        let ffd3_sym = manual
            .parse_symbolic_offset_input("KERNAL_CHROUT + 1", Addr(0xFFD3), false, None)
            .unwrap();
        manual.apply_address_offsets(&[(Addr(0xFFD3), ffd3_sym)], Some(Addr(0x1000)));
        assert!(manual.disassembly.iter().any(|l| {
            l.external_label_address == Some(Addr(0xFFD2)) && l.mnemonic.contains("KERNAL_CHROUT")
        }));
        let manual_jsr = manual
            .disassembly
            .iter()
            .find(|l| l.address == Addr(0x1000) && !l.bytes.is_empty())
            .unwrap();
        assert_eq!(manual_jsr.operand, "KERNAL_CHROUT + 1");

        manual.undo_last_command();
        assert!(!manual.labels.contains_key(&Addr(0xFFD2)));
        manual.redo_last_command();
        assert!(manual.labels.contains_key(&Addr(0xFFD2)));
    }
}
