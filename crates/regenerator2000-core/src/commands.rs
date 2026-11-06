//! Undoable state mutation commands and the [`UndoStack`] history manager.
//!
//! Defines [`Command`] variants for all user-facing [`AppState`] edits (block types, labels,
//! comments, origin, immediate formats, collapsed blocks, splitters, scopes, bookmarks,
//! excluded addresses, and enums) and provides [`UndoStack`] for undo and redo transitions.

use crate::state::{Addr, AppState, BlockType, EnumDefinition, ImmediateFormat, Label, XRef};
use std::collections::BTreeMap;

/// Represents an executable and undoable state mutation command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    /// Sets the [`BlockType`] classification for a byte index range.
    SetBlockType {
        /// Byte offset range in `raw_data`.
        range: std::ops::Range<usize>,
        /// Block type to apply to `range`.
        new_type: BlockType,
        /// Previous block types in `range` for undo.
        old_types: Vec<BlockType>,
    },
    /// Sets or removes labels at `address`.
    SetLabel {
        /// Target 6502 memory address.
        address: Addr,
        /// Replacement label list, or `None` to remove all labels at `address`.
        new_label: Option<Vec<Label>>,
        /// Previous label list at `address` for undo.
        old_label: Option<Vec<Label>>,
    },
    /// Replaces labels and cross-references with the output of an analysis pass.
    SetAnalysisData {
        /// Replacement label map indexed by address.
        labels: BTreeMap<Addr, Vec<Label>>,
        /// Replacement cross-reference map indexed by target address.
        cross_refs: BTreeMap<Addr, Vec<XRef>>,
        /// Previous label map for undo.
        old_labels: BTreeMap<Addr, Vec<Label>>,
        /// Previous cross-reference map for undo.
        old_cross_refs: BTreeMap<Addr, Vec<XRef>>,
    },
    /// Sets or removes a user side comment at `address`.
    SetUserSideComment {
        /// Target 6502 memory address.
        address: Addr,
        /// New side comment text, or `None` to clear the comment.
        new_comment: Option<String>,
        /// Previous side comment text for undo.
        old_comment: Option<String>,
    },
    /// Sets or removes a user line comment above `address`.
    SetUserLineComment {
        /// Target 6502 memory address.
        address: Addr,
        /// New line comment text, or `None` to clear the comment.
        new_comment: Option<String>,
        /// Previous line comment text for undo.
        old_comment: Option<String>,
    },
    /// Changes the base load address (`origin`) of the binary.
    ChangeOrigin {
        /// New 16-bit origin address.
        new_origin: Addr,
        /// Previous 16-bit origin address for undo.
        old_origin: Addr,
    },
    /// Sets or clears the [`ImmediateFormat`] display override at `address`.
    SetImmediateFormat {
        /// Target instruction address.
        address: Addr,
        /// New immediate operand format, or `None` to use default formatting.
        new_format: Option<ImmediateFormat>,
        /// Previous immediate operand format for undo.
        old_format: Option<ImmediateFormat>,
    },
    /// Collapses a byte-offset block range in the disassembly view.
    CollapseBlock {
        /// Inclusive `(start_offset, end_offset)` byte range to collapse.
        range: (usize, usize),
    },
    /// Expands a previously collapsed byte-offset block range.
    UncollapseBlock {
        /// Inclusive `(start_offset, end_offset)` byte range to expand.
        range: (usize, usize),
    },
    /// Toggles a virtual block splitter at `address`.
    ToggleSplitter {
        /// Target 6502 memory address.
        address: Addr,
    },
    /// Imports a batch of `(Addr, Label)` pairs from an external symbol source.
    ImportLabels {
        /// Imported `(address, label)` pairs.
        new_labels: Vec<(Addr, Label)>,
        /// Snapshot of `state.labels` before the import for undo.
        old_labels: BTreeMap<Addr, Vec<Label>>,
    },
    /// Adds or updates a lexical scope spanning `start..=end`.
    AddScope {
        /// Inclusive start address of the scope.
        start: Addr,
        /// Inclusive end address of the scope.
        end: Addr,
        /// Previous end address at `start` for undo.
        old_end: Option<Addr>,
    },
    /// Removes the lexical scope that starts at `address`.
    RemoveScope {
        /// Start address of the scope to remove.
        address: Addr,
        /// Previous end address of the removed scope for undo.
        old_end: Addr,
    },
    /// Sets or clears a named bookmark at `address`.
    SetBookmark {
        /// Target 6502 memory address.
        address: Addr,
        /// Bookmark name, or `None` to remove the bookmark.
        new_name: Option<String>,
        /// Previous bookmark name for undo.
        old_name: Option<String>,
    },
    /// Adds or removes `address` from the user-excluded system label set.
    SetUserExcludedAddress {
        /// Target 6502 memory address.
        address: Addr,
        /// `true` to exclude `address`, or `false` to restore it.
        add: bool,
        /// Previous label map before exclusion for undo.
        old_labels: BTreeMap<Addr, Vec<Label>>,
        /// Previous cross-reference map before exclusion for undo.
        old_cross_refs: BTreeMap<Addr, Vec<XRef>>,
    },
    /// Sets or clears an [`EnumDefinition`] operand annotation at `address`.
    SetEnumUsage {
        /// Target instruction address.
        address: Addr,
        /// Enum name to apply, or `None` to clear.
        new_enum: Option<String>,
        /// Previous enum name at `address` for undo.
        old_enum: Option<String>,
    },
    /// Creates, updates, or deletes a project [`EnumDefinition`].
    SetEnumDefinition {
        /// Unique enum identifier.
        name: String,
        /// Replacement enum definition, or `None` to delete `name`.
        new_definition: Option<EnumDefinition>,
        /// Previous enum definition for undo.
        old_definition: Option<EnumDefinition>,
    },
    /// Executes a sequence of commands as one atomic undoable action.
    Batch(Vec<Command>),
}

impl Command {
    /// Applies this command to `state`.
    pub fn apply(&self, state: &mut AppState) {
        match self {
            Command::CollapseBlock { range } => {
                if !state.collapsed_blocks.contains(range) {
                    state.collapsed_blocks.push(*range);
                }
            }
            Command::UncollapseBlock { range } => {
                state.collapsed_blocks.retain(|r| r != range);
            }
            Command::SetBlockType {
                range,
                new_type,
                old_types: _,
            } => {
                let start = range.start;
                let end = range.end.min(state.block_types.len());

                if start < end {
                    state.block_types[start..end].fill(*new_type);

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
                if let Some(label) = new_label.as_ref().filter(|v| !v.is_empty()) {
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

    /// Reverts this command on `state`.
    pub fn undo(&self, state: &mut AppState) {
        match self {
            Command::SetBlockType {
                range,
                new_type: _,
                old_types,
            } => {
                let start = range.start;
                let end = range.end.min(state.block_types.len());

                if start < end {
                    for (slot, old_type) in state.block_types[start..end].iter_mut().zip(old_types)
                    {
                        *slot = *old_type;
                    }

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
                if let Some(label) = old_label.as_ref().filter(|v| !v.is_empty()) {
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
                if !state.collapsed_blocks.contains(range) {
                    state.collapsed_blocks.push(*range);
                }
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
                    if let Some(labels) = old_labels.get(&addr).filter(|v| !v.is_empty()) {
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
                if *add {
                    state.user_excluded_addresses.remove(address);
                } else {
                    state.user_excluded_addresses.insert(*address);
                }
                state.load_system_assets();
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

/// Manages the command history buffer and tracks the current undo/redo cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoStack {
    /// Ordered list of applied commands.
    commands: Vec<Command>,
    /// Index of the next command to redo (`0..=commands.len()`).
    pointer: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Creates an empty [`UndoStack`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pointer: 0,
        }
    }

    /// Appends `command` to the stack and discards any undone commands after `pointer`.
    pub fn push(&mut self, command: Command) {
        if self.pointer < self.commands.len() {
            self.commands.truncate(self.pointer);
        }
        self.commands.push(command);
        self.pointer += 1;
    }

    /// Reverts the most recent applied command and refreshes the disassembly.
    ///
    /// Returns `Some("Undone")` when a command is reverted, or `None` when the stack is at index 0.
    pub fn undo(&mut self, state: &mut AppState) -> Option<String> {
        if self.pointer > 0 {
            self.pointer -= 1;
            let command = &self.commands[self.pointer];
            command.undo(state);
            state.disassemble();
            Some("Undone".to_string())
        } else {
            None
        }
    }

    /// Reapplies the next undone command and refreshes the disassembly.
    ///
    /// Returns `Some("Redone")` when a command is reapplied, or `None` when no undone commands remain.
    pub fn redo(&mut self, state: &mut AppState) -> Option<String> {
        if self.pointer < self.commands.len() {
            let command = &self.commands[self.pointer];
            command.apply(state);
            self.pointer += 1;
            state.disassemble();
            Some("Redone".to_string())
        } else {
            None
        }
    }

    /// Returns `true` when at least one command can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    /// Returns `true` when at least one undone command can be redone.
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
