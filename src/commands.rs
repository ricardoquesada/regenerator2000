use crate::state::{AddressType, AppState};

#[derive(Debug, Clone)]
pub enum Command {
    SetAddressType {
        range: std::ops::Range<usize>,
        new_type: AddressType,
        old_types: Vec<AddressType>,
    },
    SetLabel {
        address: u16,
        new_label: Option<crate::state::Label>,
        old_label: Option<crate::state::Label>,
    },
    SetLabels {
        labels: std::collections::HashMap<u16, Option<crate::state::Label>>, // Addr -> New Label (None to remove)
        old_labels: std::collections::HashMap<u16, Option<crate::state::Label>>,
    },
}

impl Command {
    pub fn apply(&self, state: &mut AppState) {
        match self {
            Command::SetAddressType {
                range,
                new_type,
                old_types: _,
            } => {
                let max_len = state.address_types.len();
                let start = range.start;
                let end = range.end.min(max_len);

                if start < end {
                    for i in start..end {
                        state.address_types[i] = *new_type;
                    }

                    // Re-analyze reference counts and labels
                    state.labels = crate::analyzer::analyze(state);
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
            Command::SetLabels {
                labels,
                old_labels: _,
            } => {
                for (address, label_opt) in labels {
                    if let Some(label) = label_opt {
                        state.labels.insert(*address, label.clone());
                    } else {
                        state.labels.remove(address);
                    }
                }
            }
        }
    }

    pub fn undo(&self, state: &mut AppState) {
        match self {
            Command::SetAddressType {
                range,
                new_type: _,
                old_types,
            } => {
                let max_len = state.address_types.len();
                let start = range.start;
                let end = range.end.min(max_len);

                if start < end {
                    for (i, old_type) in (start..end).zip(old_types.iter()) {
                        state.address_types[i] = *old_type;
                    }

                    // Re-analyze reference counts and labels
                    state.labels = crate::analyzer::analyze(state);
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
            Command::SetLabels {
                labels: _,
                old_labels,
            } => {
                for (address, label_opt) in old_labels {
                    if let Some(label) = label_opt {
                        state.labels.insert(*address, label.clone());
                    } else {
                        state.labels.remove(address);
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

    pub fn can_undo(&self) -> bool {
        self.pointer > 0
    }

    pub fn can_redo(&self) -> bool {
        self.pointer < self.commands.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AddressType, AppState};

    #[test]
    fn test_undo_stack_push_undo_redo() {
        let mut app_state = AppState::new();
        // Setup initial state: 10 lines of Code
        // We need to allow app_state to have some raw data to be valid for address types
        app_state.raw_data = vec![0xEA; 10]; // NOPs
        app_state.address_types = vec![AddressType::Code; 10];

        // Action: Change first 5 bytes to DataByte
        let range = 0..5;
        let old_types = vec![AddressType::Code; 5];
        let command = Command::SetAddressType {
            range: range.clone(),
            new_type: AddressType::DataByte,
            old_types,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        // Verify application
        for i in 0..5 {
            assert_eq!(app_state.address_types[i], AddressType::DataByte);
        }
        for i in 5..10 {
            assert_eq!(app_state.address_types[i], AddressType::Code);
        }

        // Undo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Undo
        for i in 0..10 {
            assert_eq!(app_state.address_types[i], AddressType::Code);
        }

        // Redo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        // Verify Redo
        for i in 0..5 {
            assert_eq!(app_state.address_types[i], AddressType::DataByte);
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
            refs: 0,
        };
        let command = Command::SetLabel {
            address,
            new_label: Some(label.clone()),
            old_label: None,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert_eq!(app_state.labels.get(&address), Some(&label));

        // Undo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(app_state.labels.get(&address).is_none());

        // Redo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert_eq!(app_state.labels.get(&address), Some(&label));
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
        app_state.address_types = vec![AddressType::Code; 6];

        // Initial Analysis
        app_state.labels = crate::analyzer::analyze(&app_state);

        // Assert label exists
        assert!(app_state.labels.get(&0x1005).is_some());
        assert_eq!(app_state.labels.get(&0x1005).unwrap().refs, 1);
        assert_eq!(
            app_state.labels.get(&0x1005).unwrap().kind,
            crate::state::LabelKind::Auto
        );

        // Action: Change JMP (3 bytes) to DataByte
        let range = 0..3;
        let old_types = vec![AddressType::Code; 3];
        let command = Command::SetAddressType {
            range: range.clone(),
            new_type: AddressType::DataByte,
            old_types,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        // Verify label is GONE because reference count dropped to 0
        assert!(app_state.labels.get(&0x1005).is_none());

        // Undo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, AppState::new().undo_stack);
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
        assert!(app_state.labels.get(&0x1005).is_some());
        assert_eq!(app_state.labels.get(&0x1005).unwrap().refs, 1);
    }
}
