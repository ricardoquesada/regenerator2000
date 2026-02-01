use crate::state::{AppState, BlockType, ImmediateFormat, Label};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Command {
    SetBlockType {
        range: std::ops::Range<usize>,
        new_type: BlockType,
        old_types: Vec<BlockType>,
    },
    SetLabel {
        address: u16,
        new_label: Option<Vec<crate::state::Label>>,
        old_label: Option<Vec<crate::state::Label>>,
    },
    SetAnalysisData {
        labels: BTreeMap<u16, Vec<Label>>,
        cross_refs: BTreeMap<u16, Vec<u16>>,
        old_labels: BTreeMap<u16, Vec<Label>>,
        old_cross_refs: BTreeMap<u16, Vec<u16>>,
    },
    SetUserSideComment {
        address: u16,
        new_comment: Option<String>,
        old_comment: Option<String>,
    },
    SetUserLineComment {
        address: u16,
        new_comment: Option<String>,
        old_comment: Option<String>,
    },
    ChangeOrigin {
        new_origin: u16,
        old_origin: u16,
    },
    SetImmediateFormat {
        address: u16,
        new_format: Option<ImmediateFormat>,
        old_format: Option<ImmediateFormat>,
    },
    CollapseBlock {
        range: (usize, usize),
    },
    UncollapseBlock {
        range: (usize, usize),
    },
    ToggleSplitter {
        address: u16,
    },
    ImportLabels {
        new_labels: Vec<(u16, crate::state::Label)>,
        old_labels: BTreeMap<u16, Vec<crate::state::Label>>,
    },
}

impl Command {
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
                    // Re-analyze reference counts and labels
                    let (new_labels, new_cross_refs) = crate::analyzer::analyze(state);
                    state.labels = new_labels;
                    state.cross_refs = new_cross_refs;
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
                // Complete replacement of the map (avoid clone by using reference)
                // Since labels/cross_refs are references from the command, we still need to clone
                // But this is unavoidable for undo/redo to work properly
                state.labels = labels.clone();
                state.cross_refs = cross_refs.clone();
            }
            Command::SetUserSideComment {
                address,
                new_comment,
                old_comment: _,
            } => {
                if let Some(comment) = new_comment {
                    state.user_side_comments.insert(*address, comment.clone());
                } else {
                    state.user_side_comments.remove(address);
                }
            }
            Command::SetUserLineComment {
                address,
                new_comment,
                old_comment: _,
            } => {
                if let Some(comment) = new_comment {
                    state.user_line_comments.insert(*address, comment.clone());
                } else {
                    state.user_line_comments.remove(address);
                }
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
                if let Some(format) = new_format {
                    state.immediate_value_formats.insert(*address, *format);
                } else {
                    state.immediate_value_formats.remove(address);
                }
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
        }
    }

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
                    // Re-analyze reference counts and labels
                    let (new_labels, new_cross_refs) = crate::analyzer::analyze(state);
                    state.labels = new_labels;
                    state.cross_refs = new_cross_refs;
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
                if let Some(comment) = old_comment {
                    state.user_side_comments.insert(*address, comment.clone());
                } else {
                    state.user_side_comments.remove(address);
                }
            }
            Command::SetUserLineComment {
                address,
                new_comment: _,
                old_comment,
            } => {
                if let Some(comment) = old_comment {
                    state.user_line_comments.insert(*address, comment.clone());
                } else {
                    state.user_line_comments.remove(address);
                }
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
                if let Some(format) = old_format {
                    state.immediate_value_formats.insert(*address, *format);
                } else {
                    state.immediate_value_formats.remove(address);
                }
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
                let affected_addrs: std::collections::HashSet<u16> =
                    new_labels.iter().map(|(a, _)| *a).collect();
                for addr in affected_addrs {
                    if let Some(labels) = old_labels.get(&addr) {
                        state.labels.insert(addr, labels.clone());
                    } else {
                        state.labels.remove(&addr);
                    }
                }
            }
        }
    }
}

pub struct UndoStack {
    commands: Vec<Command>,
    pointer: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            pointer: 0,
        }
    }

    pub fn push(&mut self, command: Command) {
        // If we are not at the end, truncate the future
        if self.pointer < self.commands.len() {
            self.commands.truncate(self.pointer);
        }
        self.commands.push(command);
        self.pointer += 1;
    }

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

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        self.pointer < self.commands.len()
    }
    pub fn get_pointer(&self) -> usize {
        self.pointer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, BlockType};

    #[test]
    fn test_undo_stack_push_undo_redo() {
        let mut app_state = AppState::new();
        // Setup initial state: 10 lines of Code
        // We need to allow app_state to have some raw data to be valid for address types
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
        let address = 0x1000;

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
            app_state.labels.get(&address).map(|v| v.as_slice()),
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
            app_state.labels.get(&address).map(|v| v.as_slice()),
            Some(vec![label.clone()].as_slice())
        );
    }

    #[test]
    fn test_dynamic_label_update() {
        let mut app_state = AppState::new();
        app_state.origin = 0x1000;
        // JMP $1005 (4C 05 10)
        // NOP (EA)
        // NOP (EA)
        // Target: $1005 (EA)
        let data = vec![0x4C, 0x05, 0x10, 0xEA, 0xEA, 0xEA];
        app_state.raw_data = data;
        app_state.block_types = vec![BlockType::Code; 6];

        // Initial Analysis
        let (labels, cross_refs) = crate::analyzer::analyze(&app_state);
        app_state.labels = labels;
        app_state.cross_refs = cross_refs;

        // Assert label exists
        assert!(app_state.labels.contains_key(&0x1005));
        assert_eq!(app_state.cross_refs.get(&0x1005).unwrap().len(), 1);
        assert_eq!(
            app_state.labels.get(&0x1005).unwrap().first().unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Action: Change JMP (3 bytes) to DataByte
        let range = 0..3;
        let old_types = vec![BlockType::Code; 3];
        let command = Command::SetBlockType {
            range: range.clone(),
            new_type: BlockType::DataByte,
            old_types,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        // Verify label is GONE because reference count dropped to 0
        assert!(!app_state.labels.contains_key(&0x1005));

        // Undo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        // Wait, AppState::new() creates empty stack.
        // My previous test code used `UndoStack::new()`. I should respect imports.
        // But `UndoStack` is in this module (super).
        // Let's check imports in tests module. `use super::*;`.
        // So `UndoStack::new()` is valid.

        // Retrying with correct stack replacement logic
        // But wait, `app_state.undo_stack` is `UndoStack`.
        // `std::mem::replace` needs same type.
        // `UndoStack::new()` returns `UndoStack`.
        // So `stack` is `UndoStack`.
        // `stack.undo` needs `&mut AppState`.

        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify label is BACK
        assert!(app_state.labels.contains_key(&0x1005));
        assert_eq!(app_state.cross_refs.get(&0x1005).unwrap().len(), 1);
    }
    #[test]
    fn test_user_line_comment_undo_redo() {
        let mut app_state = AppState::new();
        let address = 0x1000;

        // Action: Set User Line Comment
        let comment = "Line Comment".to_string();
        let command = Command::SetUserLineComment {
            address,
            new_comment: Some(comment.clone()),
            old_comment: None,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert_eq!(app_state.user_line_comments.get(&address), Some(&comment));

        // Undo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(!app_state.user_line_comments.contains_key(&address));

        // Redo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert_eq!(app_state.user_line_comments.get(&address), Some(&comment));
    }

    #[test]
    fn test_import_labels_undo_redo() {
        let mut app_state = AppState::new();
        // Setup initial state: Label at 0x1000
        let address = 0x1000;
        let initial_label = crate::state::Label {
            name: "Initial".to_string(),
            kind: crate::state::LabelKind::User,
            label_type: crate::state::LabelType::UserDefined,
        };
        app_state
            .labels
            .insert(address, vec![initial_label.clone()]);

        // Action: Import Labels
        // 1. New label at 0x1000 (should merge if name different)
        // 2. New label at 0x2000
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

        let new_labels = vec![(0x1000, new_label1.clone()), (0x2000, new_label2.clone())];

        // Capture old state manually (as done in state.rs)
        let mut old_labels = BTreeMap::new();
        old_labels.insert(0x1000, vec![initial_label.clone()]);
        // 0x2000 has no old labels, so do we insert empty vec or nothing?
        // Logic in state.rs: if !old_labels_map.contains_key, insert current (empty).
        old_labels.insert(0x2000, Vec::new());

        let command = Command::ImportLabels {
            new_labels,
            old_labels,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        // Verify application
        // 0x1000 should have 2 labels
        let labels_1000 = app_state.labels.get(&0x1000).unwrap();
        assert_eq!(labels_1000.len(), 2);
        assert!(labels_1000.iter().any(|l| l.name == "Initial"));
        assert!(labels_1000.iter().any(|l| l.name == "New1"));

        // 0x2000 should have 1 label
        let labels_2000 = app_state.labels.get(&0x2000).unwrap();
        assert_eq!(labels_2000.len(), 1);
        assert_eq!(labels_2000[0].name, "New2");

        // Undo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Undo
        // 0x1000 should have 1 label (Initial)
        let labels_1000 = app_state.labels.get(&0x1000).unwrap();
        assert_eq!(labels_1000.len(), 1);
        assert_eq!(labels_1000[0].name, "Initial");

        // 0x2000 should be empty/removed
        // Note: our logic in undo inserts `Vec::new()` which means key exists but is empty.
        // OR we can check isEmpty.
        if let Some(l) = app_state.labels.get(&0x2000) {
            assert!(l.is_empty());
        } else {
            // also fine
        }

        // Redo
        let mut stack = std::mem::take(&mut app_state.undo_stack);
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Redo
        let labels_1000 = app_state.labels.get(&0x1000).unwrap();
        assert_eq!(labels_1000.len(), 2);

        let labels_2000 = app_state.labels.get(&0x2000).unwrap();
        assert_eq!(labels_2000.len(), 1);
    }
}
