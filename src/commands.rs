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
        new_label: Option<String>,
        old_label: Option<String>,
    },
    SetLabels {
        labels: std::collections::HashMap<u16, Option<String>>, // Addr -> New Label (None to remove)
        old_labels: std::collections::HashMap<u16, Option<String>>,
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
        let command = Command::SetLabel {
            address,
            new_label: Some("Start".to_string()),
            old_label: None,
        };

        command.apply(&mut app_state);
        app_state.undo_stack.push(command);

        assert_eq!(app_state.labels.get(&address), Some(&"Start".to_string()));

        // Undo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.undo(&mut app_state);
        app_state.undo_stack = stack;

        assert!(app_state.labels.get(&address).is_none());

        // Redo
        let mut stack = std::mem::replace(&mut app_state.undo_stack, UndoStack::new());
        stack.redo(&mut app_state);
        app_state.undo_stack = stack;

        assert_eq!(app_state.labels.get(&address), Some(&"Start".to_string()));
    }
}
