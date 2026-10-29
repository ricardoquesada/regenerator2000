//! Executable command patterns and undo/redo infrastructure for Regenerator2000 state mutations.
//!
//! This module defines the [`Command`] enum representing all state mutations (setting labels, comments,
//! block types, scopes, immediate formats, enums, origin, and bookmarks) and the [`UndoStack`] manager
//! that facilitates bi-directional state transitions.

use crate::state::{Addr, AppState, BlockType, EnumDefinition, ImmediateFormat, Label};
use std::collections::BTreeMap;

/// Represents an executable and undoable state mutation command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Sets the [`BlockType`] classification for a range of memory indices.
    SetBlockType {
        /// Index range within the raw data buffer.
        range: std::ops::Range<usize>,
        /// The new block type classification to apply.
        new_type: BlockType,
        /// The previous block types before modification (for undo).
        old_types: Vec<BlockType>,
    },
    /// Sets or removes user-defined labels at a memory address [`Addr`].
    SetLabel {
        /// Target memory address [`Addr`].
        address: Addr,
        /// The new label vector to assign, or `None` to clear.
        new_label: Option<Vec<crate::state::Label>>,
        /// The previous label vector at this address (for undo).
        old_label: Option<Vec<crate::state::Label>>,
    },
    /// Bulk updates labels and cross-references resulting from an analysis pass.
    SetAnalysisData {
        /// New label mapping by address.
        labels: BTreeMap<Addr, Vec<Label>>,
        /// New cross-reference mapping by target address.
        cross_refs: BTreeMap<Addr, Vec<Addr>>,
        /// Previous label mapping (for undo).
        old_labels: BTreeMap<Addr, Vec<Label>>,
        /// Previous cross-reference mapping (for undo).
        old_cross_refs: BTreeMap<Addr, Vec<Addr>>,
    },
    /// Sets or removes an inline side comment on an instruction row.
    SetUserSideComment {
        /// Target memory address [`Addr`].
        address: Addr,
        /// The new comment string to set, or `None` to clear.
        new_comment: Option<String>,
        /// The previous comment string (for undo).
        old_comment: Option<String>,
    },
    /// Sets or removes a line comment appearing above an instruction row.
    SetUserLineComment {
        /// Target memory address [`Addr`].
        address: Addr,
        /// The new line comment string, or `None` to clear.
        new_comment: Option<String>,
        /// The previous line comment string (for undo).
        old_comment: Option<String>,
    },
    /// Changes the base memory origin address [`Addr`] of the disassembly.
    ChangeOrigin {
        /// The new 16-bit origin address.
        new_origin: Addr,
        /// The previous origin address (for undo).
        old_origin: Addr,
    },
    /// Configures immediate operand formatting [`ImmediateFormat`] at an instruction address.
    SetImmediateFormat {
        /// Target memory address [`Addr`].
        address: Addr,
        /// The new operand format option, or `None` to reset to default.
        new_format: Option<ImmediateFormat>,
        /// The previous operand format option (for undo).
        old_format: Option<ImmediateFormat>,
    },
    /// Collapses a contiguous block range in the visual disassembly view.
    CollapseBlock {
        /// Tuple containing start and end line indices of the collapsed range.
        range: (usize, usize),
    },
    /// Uncollapses a previously collapsed block range in the visual disassembly view.
    UncollapseBlock {
        /// Tuple containing start and end line indices of the block range to expand.
        range: (usize, usize),
    },
    /// Toggles a block splitter at an address [`Addr`] to control auto-merging.
    ToggleSplitter {
        /// Target memory address [`Addr`].
        address: Addr,
    },
    /// Imports a batch of labels into memory from symbol files or external sources.
    ImportLabels {
        /// Vector of (Address, Label) tuples to import.
        new_labels: Vec<(Addr, crate::state::Label)>,
        /// Mapping of previous labels before importing (for undo).
        old_labels: BTreeMap<Addr, Vec<crate::state::Label>>,
    },
    /// Defines a routine scope covering an address range [`Addr`].
    AddScope {
        /// Start address of the scope (inclusive).
        start: Addr,
        /// End address of the scope (inclusive).
        end: Addr,
        /// Previous scope end address if replacing an existing scope (for undo).
        old_end: Option<Addr>,
    },
    /// Removes an existing scope starting at an address [`Addr`].
    RemoveScope {
        /// Start address of the scope to remove.
        address: Addr,
        /// Previous scope end address (for undo).
        old_end: Addr,
    },
    /// Sets or clears a named bookmark at a memory address [`Addr`].
    SetBookmark {
        /// Target memory address [`Addr`].
        address: Addr,
        /// The new bookmark name string, or `None` to remove bookmark.
        new_name: Option<String>,
        /// The previous bookmark name string (for undo).
        old_name: Option<String>,
    },
    /// Adds or removes an address from the per-project user-excluded set.
    SetUserExcludedAddress {
        /// Target memory address [`Addr`].
        address: Addr,
        /// `true` if adding to exclusion list, `false` if removing.
        add: bool,
        /// Previous labels before exclusion (for undo).
        old_labels: BTreeMap<Addr, Vec<crate::state::Label>>,
        /// Previous cross-references before exclusion (for undo).
        old_cross_refs: BTreeMap<Addr, Vec<Addr>>,
    },
    /// Applies or clears an enum usage annotation [`EnumDefinition`] at an address.
    SetEnumUsage {
        /// Target memory address [`Addr`].
        address: Addr,
        /// Name of the enum to apply, or `None` to clear.
        new_enum: Option<String>,
        /// Name of the previous enum applied (for undo).
        old_enum: Option<String>,
    },
    /// Creates, updates, or deletes a project-specific enum definition [`EnumDefinition`].
    SetEnumDefinition {
        /// Unique name identifier of the enum.
        name: String,
        /// The new enum definition struct, or `None` if deleting.
        new_definition: Option<EnumDefinition>,
        /// The previous enum definition struct (for undo).
        old_definition: Option<EnumDefinition>,
    },
    /// Executes a series of commands sequentially as a single atomic undoable action.
    Batch(Vec<Command>),
}

impl Command {
    /// Applies the state modification represented by this command to [`AppState`].
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to the application state [`AppState`].
    pub fn apply(&self, state: &mut AppState) {
        match self {
            Command::CollapseBlock { range } => {
                state.collapsed_blocks.push(*range);
            }
            Command::UncollapseBlock { range } => {
                state.collapsed_blocks.retain(|r| r != range);
            }
            Command::SetBlockType {
                range,
                new_type,
                old_types: _,
            } => {
                let max_len = state.block_types.len();
                let start = range.start;
                let end = range.end.min(max_len);

                if start < end {
                    for i in start..end {
                        state.block_types[i] = *new_type;
                    }

                    // Re-analyze reference counts and labels
                    let result = crate::analyzer::analyze(state);
                    state.labels = result.labels;
                    state.cross_refs = result.cross_refs;
                }
            }
            Command::SetLabel {
                address,
                new_label,
                old_label: _,
            } => {
                if let Some(label) = new_label {
                    state.labels.insert(*address, label.clone());
                } else {
                    state.labels.remove(address);
                }
            }
            Command::SetAnalysisData {
                labels,
                cross_refs,
                old_labels: _,
                old_cross_refs: _,
            } => {
                state.labels = labels.clone();
                state.cross_refs = cross_refs.clone();
            }
            Command::SetUserSideComment {
                address,
                new_comment,
                old_comment: _,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.user_side_comment = new_comment.clone());
            }
            Command::SetUserLineComment {
                address,
                new_comment,
                old_comment: _,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.user_line_comment = new_comment.clone());
            }
            Command::ChangeOrigin {
                new_origin,
                old_origin: _,
            } => {
                state.origin = *new_origin;
            }
            Command::SetImmediateFormat {
                address,
                new_format,
                old_format: _,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.immediate_format = *new_format);
            }
            Command::ToggleSplitter { address } => {
                state.toggle_splitter(*address);
            }
            Command::ImportLabels {
                new_labels,
                old_labels: _,
            } => {
                for (addr, label) in new_labels {
                    let labels = state.labels.entry(*addr).or_default();
                    if !labels
                        .iter()
                        .any(|l| l.name == label.name && l.kind == label.kind)
                    {
                        labels.push(label.clone());
                    }
                }
            }
            Command::SetBookmark {
                address,
                new_name,
                old_name: _,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.bookmark = new_name.clone());
            }
            Command::SetUserExcludedAddress {
                address,
                add,
                old_labels: _,
                old_cross_refs: _,
            } => {
                if *add {
                    state.user_excluded_addresses.insert(*address);
                } else {
                    state.user_excluded_addresses.remove(address);
                }
                state.load_system_assets();
                let result = crate::analyzer::analyze(state);
                state.labels = result.labels;
                state.cross_refs = result.cross_refs;
                state.disassemble();
            }
            Command::AddScope {
                start,
                end,
                old_end: _,
            } => {
                state.annotations.update(*start, |e| e.scope = Some(*end));
            }
            Command::RemoveScope {
                address,
                old_end: _,
            } => {
                state.annotations.update(*address, |e| e.scope = None);
            }
            Command::SetEnumUsage {
                address,
                new_enum,
                old_enum: _,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.enum_usage = new_enum.clone());
            }
            Command::SetEnumDefinition {
                name,
                new_definition,
                old_definition: _,
            } => {
                if let Some(def) = new_definition {
                    state.enums.insert(name.clone(), def.clone());
                } else {
                    state.enums.remove(name);
                }
            }
            Command::Batch(commands) => {
                for command in commands {
                    command.apply(state);
                }
            }
        }
    }

    /// Reverts the state modification represented by this command on [`AppState`].
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to the application state [`AppState`].
    pub fn undo(&self, state: &mut AppState) {
        match self {
            Command::SetBlockType {
                range,
                new_type: _,
                old_types,
            } => {
                let max_len = state.block_types.len();
                let start = range.start;
                let end = range.end.min(max_len);

                if start < end {
                    for (i, old_type) in (start..end).zip(old_types.iter()) {
                        state.block_types[i] = *old_type;
                    }

                    // Re-analyze reference counts and labels
                    let result = crate::analyzer::analyze(state);
                    state.labels = result.labels;
                    state.cross_refs = result.cross_refs;
                }
            }
            Command::SetLabel {
                address,
                new_label: _,
                old_label,
            } => {
                if let Some(label) = old_label {
                    state.labels.insert(*address, label.clone());
                } else {
                    state.labels.remove(address);
                }
            }
            Command::SetAnalysisData {
                labels: _,
                cross_refs: _,
                old_labels,
                old_cross_refs,
            } => {
                state.labels = old_labels.clone();
                state.cross_refs = old_cross_refs.clone();
            }
            Command::SetUserSideComment {
                address,
                new_comment: _,
                old_comment,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.user_side_comment = old_comment.clone());
            }
            Command::SetUserLineComment {
                address,
                new_comment: _,
                old_comment,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.user_line_comment = old_comment.clone());
            }
            Command::ChangeOrigin {
                new_origin: _,
                old_origin,
            } => {
                state.origin = *old_origin;
            }
            Command::SetImmediateFormat {
                address,
                new_format: _,
                old_format,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.immediate_format = *old_format);
            }
            Command::CollapseBlock { range } => {
                state.collapsed_blocks.retain(|r| r != range);
            }
            Command::UncollapseBlock { range } => {
                state.collapsed_blocks.push(*range);
            }
            Command::ToggleSplitter { address } => {
                state.toggle_splitter(*address);
            }
            Command::ImportLabels {
                new_labels,
                old_labels,
            } => {
                let affected_addrs: std::collections::HashSet<Addr> =
                    new_labels.iter().map(|(a, _)| *a).collect();
                for addr in affected_addrs {
                    if let Some(labels) = old_labels.get(&addr) {
                        state.labels.insert(addr, labels.clone());
                    } else {
                        state.labels.remove(&addr);
                    }
                }
            }
            Command::SetBookmark {
                address,
                new_name: _,
                old_name,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.bookmark = old_name.clone());
            }
            Command::SetUserExcludedAddress {
                address,
                add,
                old_labels,
                old_cross_refs,
            } => {
                // Reverse the exclusion set change
                if *add {
                    state.user_excluded_addresses.remove(address);
                } else {
                    state.user_excluded_addresses.insert(*address);
                }
                state.load_system_assets();
                // Restore the exact pre-command labels and xrefs
                state.labels = old_labels.clone();
                state.cross_refs = old_cross_refs.clone();
                state.disassemble();
            }
            Command::AddScope {
                start,
                end: _,
                old_end,
            } => {
                state.annotations.update(*start, |e| e.scope = *old_end);
            }
            Command::RemoveScope { address, old_end } => {
                state
                    .annotations
                    .update(*address, |e| e.scope = Some(*old_end));
            }
            Command::SetEnumUsage {
                address,
                new_enum: _,
                old_enum,
            } => {
                state
                    .annotations
                    .update(*address, |e| e.enum_usage = old_enum.clone());
            }
            Command::SetEnumDefinition {
                name,
                new_definition: _,
                old_definition,
            } => {
                if let Some(def) = old_definition {
                    state.enums.insert(name.clone(), def.clone());
                } else {
                    state.enums.remove(name);
                }
            }
            Command::Batch(commands) => {
                for command in commands.iter().rev() {
                    command.undo(state);
                }
            }
        }
    }
}

/// Stack manager for tracking applied commands and performing undo/redo state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStack {
    /// History buffer of executed commands.
    commands: Vec<Command>,
    /// Pointer pointing to the next command index for execution.
    pointer: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Constructs a new empty [`UndoStack`].
    ///
    /// # Returns
    ///
    /// An [`UndoStack`] initialized with an empty command history vector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pointer: 0,
        }
    }

    /// Pushes a new [`Command`] onto the stack, clearing any existing redo history.
    ///
    /// # Arguments
    ///
    /// * `command` - The executed [`Command`] to append to history.
    pub fn push(&mut self, command: Command) {
        // If we are not at the end, truncate the future
        if self.pointer < self.commands.len() {
            self.commands.truncate(self.pointer);
        }
        self.commands.push(command);
        self.pointer += 1;
    }

    /// Reverts the previous command on the stack.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to application state [`AppState`].
    ///
    /// # Returns
    ///
    /// `Some("Undone")` if a command was undone, or `None` if the stack is at the beginning.
    pub fn undo(&mut self, state: &mut AppState) -> Option<String> {
        if self.pointer > 0 {
            self.pointer -= 1;
            let command = &self.commands[self.pointer];
            command.undo(state);
            state.disassemble(); // Refresh view
            Some("Undone".to_string())
        } else {
            None
        }
    }

    /// Re-applies the next undone command on the stack.
    ///
    /// # Arguments
    ///
    /// * `state` - Mutable reference to application state [`AppState`].
    ///
    /// # Returns
    ///
    /// `Some("Redone")` if a command was redone, or `None` if no undone commands remain.
    pub fn redo(&mut self, state: &mut AppState) -> Option<String> {
        if self.pointer < self.commands.len() {
            let command = &self.commands[self.pointer];
            command.apply(state);
            self.pointer += 1;
            state.disassemble(); // Refresh view
            Some("Redone".to_string())
        } else {
            None
        }
    }

    /// Checks if there are commands available to undo.
    ///
    /// # Returns
    ///
    /// `true` if `pointer > 0`, otherwise `false`.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    /// Checks if there are undone commands available to redo.
    ///
    /// # Returns
    ///
    /// `true` if `pointer < commands.len()`, otherwise `false`.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.pointer < self.commands.len()
    }

    /// Checks if the command history is completely empty.
    ///
    /// # Returns
    ///
    /// `true` if no commands have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the current position index of the stack pointer.
    ///
    /// # Returns
    ///
    /// `usize` index pointing to the position in the command history.
    #[must_use]
    pub fn get_pointer(&self) -> usize {
        self.pointer
    }

    /// Returns the slice of currently active (applied, non-undone) commands on the stack.
    #[must_use]
    pub fn active_commands(&self) -> &[Command] {
        &self.commands[..self.pointer]
    }

    /// Removes and returns the top active command if it satisfies `predicate`.
    pub(crate) fn pop_if_matches(
        &mut self,
        predicate: impl FnOnce(&Command) -> bool,
    ) -> Option<Command> {
        if self.pointer > 0 && predicate(&self.commands[self.pointer - 1]) {
            self.pointer -= 1;
            self.commands.truncate(self.pointer + 1);
            self.commands.pop()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState, BlockType};

    #[test]
    fn test_undo_stack_push_undo_redo() {
        let mut app_state = AppState::new();
        // Setup initial state: 10 lines of Code
        app_state.raw_data = vec![0xEA; 10]; // NOPs
        app_state.block_types = vec![BlockType::Code; 10];

        // Action: Change first 5 bytes to DataByte
        let range = 0..5;
        let old_types = vec![BlockType::Code; 5];
        let command = Command::SetBlockType {
            range: range.clone(),
            new_type: BlockType::DataByte,
            old_types,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        // Verify application
        for i in 0..5 {
            assert_eq!(app_state.block_types[i], BlockType::DataByte);
        }
        for i in 5..10 {
            assert_eq!(app_state.block_types[i], BlockType::Code);
        }

        // Undo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Undo
        for i in 0..10 {
            assert_eq!(app_state.block_types[i], BlockType::Code);
        }

        // Redo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Redo
        for i in 0..5 {
            assert_eq!(app_state.block_types[i], BlockType::DataByte);
        }
    }

    #[test]
    fn test_label_undo_redo() {
        let mut app_state = AppState::new();
        let address = Addr(0x1000);

        // Action 1: Set Label
        let label = crate::state::Label {
            name: "Start".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::UserDefined,
        };
        let command = Command::SetLabel {
            address,
            new_label: Some(vec![label.clone()]),
            old_label: None,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert_eq!(
            app_state.labels.get(&address).map(std::vec::Vec::as_slice),
            Some(vec![label.clone()].as_slice())
        );

        // Undo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(!app_state.labels.contains_key(&address));

        // Redo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert_eq!(
            app_state.labels.get(&address).map(std::vec::Vec::as_slice),
            Some(vec![label.clone()].as_slice())
        );
    }

    #[test]
    fn test_dynamic_label_update() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        let data = vec![0x4C, 0x05, 0x10, 0xEA, 0xEA, 0xEA];
        app_state.raw_data = data;
        app_state.block_types = vec![BlockType::Code; 6];

        let result = crate::analyzer::analyze(&app_state);
        app_state.labels = result.labels;
        app_state.cross_refs = result.cross_refs;

        assert!(app_state.labels.contains_key(&Addr(0x1005)));
        assert_eq!(app_state.cross_refs.get(&Addr(0x1005)).unwrap().len(), 1);
        assert_eq!(
            app_state
                .labels
                .get(&Addr(0x1005))
                .unwrap()
                .first()
                .unwrap()
                .kind,
            crate::state::LabelKind::Auto
        );

        let range = 0..3;
        let old_types = vec![BlockType::Code; 3];
        let command = Command::SetBlockType {
            range: range.clone(),
            new_type: BlockType::DataByte,
            old_types,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert!(!app_state.labels.contains_key(&Addr(0x1005)));

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(app_state.labels.contains_key(&Addr(0x1005)));
        assert_eq!(app_state.cross_refs.get(&Addr(0x1005)).unwrap().len(), 1);
    }

    #[test]
    fn test_user_line_comment_undo_redo() {
        let mut app_state = AppState::new();
        let address = Addr(0x1000);

        let comment = "Line Comment".to_string();
        let command = Command::SetUserLineComment {
            address,
            new_comment: Some(comment.clone()),
            old_comment: None,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert_eq!(
            app_state
                .annotations
                .get(address)
                .and_then(|e| e.user_line_comment.as_ref()),
            Some(&comment)
        );

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(app_state.annotations.get(address).is_none());

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert_eq!(
            app_state
                .annotations
                .get(address)
                .and_then(|e| e.user_line_comment.as_ref()),
            Some(&comment)
        );
    }

    #[test]
    fn test_import_labels_undo_redo() {
        let mut app_state = AppState::new();
        let address = Addr(0x1000);
        let initial_label = crate::state::Label {
            name: "Initial".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::UserDefined,
        };
        app_state
            .labels
            .insert(address, vec![initial_label.clone()]);

        let new_label1 = crate::state::Label {
            name: "New1".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::UserDefined,
        };
        let new_label2 = crate::state::Label {
            name: "New2".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::UserDefined,
        };

        let new_labels = vec![
            (Addr(0x1000), new_label1.clone()),
            (Addr(0x2000), new_label2.clone()),
        ];

        let mut old_labels = BTreeMap::new();
        old_labels.insert(Addr(0x1000), vec![initial_label.clone()]);
        old_labels.insert(Addr(0x2000), Vec::new());

        let command = Command::ImportLabels {
            new_labels,
            old_labels,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        let labels_1000 = app_state.labels.get(&Addr(0x1000)).unwrap();
        assert_eq!(labels_1000.len(), 2);
        assert!(labels_1000.iter().any(|l| l.name == "Initial"));
        assert!(labels_1000.iter().any(|l| l.name == "New1"));

        let labels_2000 = app_state.labels.get(&Addr(0x2000)).unwrap();
        assert_eq!(labels_2000.len(), 1);
        assert_eq!(labels_2000[0].name, "New2");

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        let labels_1000 = app_state.labels.get(&Addr(0x1000)).unwrap();
        assert_eq!(labels_1000.len(), 1);
        assert_eq!(labels_1000[0].name, "Initial");

        if let Some(l) = app_state.labels.get(&Addr(0x2000)) {
            assert!(l.is_empty());
        }

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        let labels_1000 = app_state.labels.get(&Addr(0x1000)).unwrap();
        assert_eq!(labels_1000.len(), 2);

        let labels_2000 = app_state.labels.get(&Addr(0x2000)).unwrap();
        assert_eq!(labels_2000.len(), 1);
    }

    #[test]
    fn test_set_user_excluded_address_undo_redo() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        let data = vec![0x4C, 0x00, 0xE5];
        app_state.raw_data = data;
        app_state.block_types = vec![BlockType::Code; 3];

        let result = crate::analyzer::analyze(&app_state);
        app_state.labels = result.labels;
        app_state.cross_refs = result.cross_refs;
        app_state.disassemble();

        assert!(
            app_state.labels.contains_key(&Addr(0xE500)),
            "Label at $E500 should exist before exclusion"
        );
        assert!(
            app_state.user_excluded_addresses.is_empty(),
            "user_excluded_addresses should be empty initially"
        );

        let old_labels = app_state.labels.clone();
        let old_cross_refs = app_state.cross_refs.clone();
        let command = Command::SetUserExcludedAddress {
            address: Addr(0xE500),
            add: true,
            old_labels,
            old_cross_refs,
        };
        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert!(
            app_state.user_excluded_addresses.contains(&Addr(0xE500)),
            "user_excluded_addresses should contain $E500 after apply"
        );
        assert!(
            !app_state.labels.contains_key(&Addr(0xE500)),
            "Label at $E500 should be gone after exclusion"
        );

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(
            !app_state.user_excluded_addresses.contains(&Addr(0xE500)),
            "user_excluded_addresses should NOT contain $E500 after undo"
        );
        assert!(
            app_state.labels.contains_key(&Addr(0xE500)),
            "Label at $E500 should be restored after undo"
        );

        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(
            app_state.user_excluded_addresses.contains(&Addr(0xE500)),
            "user_excluded_addresses should contain $E500 after redo"
        );
        assert!(
            !app_state.labels.contains_key(&Addr(0xE500)),
            "Label at $E500 should be gone after redo"
        );
    }

    #[test]
    fn test_user_excluded_addresses_merged_in_load_system_assets() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA];
        app_state.block_types = vec![BlockType::Code; 1];

        app_state.settings.exclude_well_known_labels = false;
        app_state.user_excluded_addresses.insert(Addr(0xFFD2));
        app_state.load_system_assets();

        assert!(
            app_state.excluded_addresses.contains(&Addr(0xFFD2)),
            "User-excluded $FFD2 should be in excluded_addresses even when \
             exclude_well_known_labels is off"
        );
    }

    #[test]
    fn test_undo_stack_initial_state() {
        let stack = UndoStack::new();
        assert!(
            !stack.can_undo(),
            "New UndoStack must have can_undo() == false"
        );
        assert!(
            !stack.can_redo(),
            "New UndoStack must have can_redo() == false"
        );
        assert_eq!(stack.get_pointer(), 0);
    }

    #[test]
    fn test_undo_stack_can_undo_after_push() {
        let mut stack = UndoStack::new();
        stack.push(Command::ToggleSplitter {
            address: Addr(0x1000),
        });
        assert!(
            stack.can_undo(),
            "UndoStack must have can_undo() == true after push"
        );
        assert!(
            !stack.can_redo(),
            "UndoStack must have can_redo() == false after fresh push"
        );
        assert_eq!(stack.get_pointer(), 1);
    }

    #[test]
    fn test_undo_stack_can_redo_after_undo() {
        let mut stack = UndoStack::new();
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 256];
        state.block_types = vec![BlockType::Code; 256];
        state.disassemble();

        stack.push(Command::ToggleSplitter {
            address: Addr(0x1000),
        });
        let _ = stack.undo(&mut state);
        assert!(
            !stack.can_undo(),
            "can_undo() must be false after undoing sole command"
        );
        assert!(
            stack.can_redo(),
            "can_redo() must be true after undoing command"
        );
        assert_eq!(stack.get_pointer(), 0);

        let _ = stack.redo(&mut state);
        assert!(
            stack.can_undo(),
            "can_undo() must be true after redoing command"
        );
        assert!(
            !stack.can_redo(),
            "can_redo() must be false after redoing last command"
        );
        assert_eq!(stack.get_pointer(), 1);
    }

    #[test]
    fn test_undo_stack_push_truncates_redo_history() {
        let mut stack = UndoStack::new();
        let mut state = AppState::new();
        state.origin = Addr(0x1000);
        state.raw_data = vec![0xEA; 256];
        state.block_types = vec![BlockType::Code; 256];
        state.disassemble();

        stack.push(Command::ToggleSplitter {
            address: Addr(0x1000),
        });
        stack.push(Command::ToggleSplitter {
            address: Addr(0x2000),
        });

        let _ = stack.undo(&mut state);
        assert!(stack.can_redo());

        stack.push(Command::ToggleSplitter {
            address: Addr(0x3000),
        });
        assert!(stack.can_undo());
        assert!(
            !stack.can_redo(),
            "Pushing new command must clear redo stack"
        );
        assert_eq!(stack.get_pointer(), 2);
    }
}
