use crate::action_handlers::{
    ActionContext, DebugActionHandler, DisassemblyActionHandler, DomainActionHandler,
    FileActionHandler, NavigationActionHandler,
};
use crate::event::CoreEvent;
use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::view_state::CoreViewState;

/// The central engine of Regenerator 2000.
///
/// Manages persistent state ([`AppState`]) and transient view state ([`CoreViewState`]).
/// Frontends interact with this via [`Core::apply_action`].
pub struct Core {
    /// Persistent application state (memory, disassembly, labels, configuration).
    pub state: AppState,
    /// Transient UI view state (cursor indices, active pane, scroll offsets).
    pub view: CoreViewState,
}

impl Core {
    /// Creates a new default [`Core`] engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            view: CoreViewState::new(),
        }
    }

    /// Handles a semantic [`AppAction`] and returns a list of emitted [`CoreEvent`]s for the frontend.
    pub fn apply_action(&mut self, action: AppAction) -> Vec<CoreEvent> {
        let mut events = Vec::new();

        // Check for unsaved changes on destructive actions
        if action.is_destructive() && self.state.is_dirty() {
            events.push(CoreEvent::DialogRequested(
                crate::event::DialogType::Confirmation {
                    title: "Unsaved Changes".to_string(),
                    message: "You have unsaved changes. Proceed anyway?".to_string(),
                    action,
                },
            ));
            return events;
        }

        // Unwrap Confirmed for processing
        let action = match action {
            AppAction::Confirmed(inner) => *inner,
            other => other,
        };

        if action.requires_document() && self.state.raw_data.is_empty() {
            events.push(CoreEvent::StatusMessage("No open document".to_string()));
            return events;
        }

        if action == AppAction::FindReferences
            && self.view.active_pane != crate::view_state::ActivePane::Disassembly
        {
            events.push(CoreEvent::StatusMessage(
                "Action only available in Disassembly View".to_string(),
            ));
            return events;
        }

        let mut ctx = ActionContext {
            state: &mut self.state,
            view: &mut self.view,
            events: &mut events,
        };

        let handlers: [&dyn DomainActionHandler; 4] = [
            &FileActionHandler::new(),
            &DisassemblyActionHandler::new(),
            &DebugActionHandler::new(),
            &NavigationActionHandler::new(),
        ];

        let mut handled = false;
        for handler in handlers {
            match handler.handle_action(&action, &mut ctx) {
                Ok(true) => {
                    handled = true;
                    break;
                }
                Ok(false) => {}
                Err(err) => {
                    ctx.events
                        .push(CoreEvent::StatusMessage(format!("Error: {err}")));
                    handled = true;
                    break;
                }
            }
        }

        if !handled {
            ctx.events.push(CoreEvent::StatusMessage(format!(
                "Unhandled action: {action:?}"
            )));
        }

        events
    }

    /// Creates a project save context from current core state.
    #[must_use]
    pub fn create_save_context(&self) -> crate::state::ProjectSaveContext {
        crate::navigation::create_save_context(&self.state, &self.view)
    }

    /// Returns the default filename stem derived from the loaded file path.
    #[must_use]
    pub fn get_default_filename_stem(&self) -> Option<String> {
        self.state
            .file_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string())
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Addr;
    use crate::view_state::ActivePane;

    #[test]
    fn test_handle_add_scope_default_label() {
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let code_data = vec![0xEA, 0xEA, 0xEA, 0xEA]; // A few NOPs

        // Use state.load_binary which handles setup
        core.state.load_binary(origin, code_data).unwrap();

        core.view.active_pane = ActivePane::Disassembly;
        core.view.cursor_index = 0; // Pointing to first instruction

        core.apply_action(AppAction::Scope);

        // Origin doesn't have a label by default, so it should generate "scope_1000"
        let label = core
            .state
            .labels
            .get(&origin)
            .expect("Expected label at start of scope");
        let first_label = label.first().expect("Expected at least one label");

        assert_eq!(first_label.name, format!("scope_{:04X}", origin.0));
        assert_eq!(first_label.kind, crate::state::LabelKind::User);
        assert_eq!(first_label.label_type, crate::state::LabelType::UserDefined);
    }

    #[test]
    fn test_handle_add_scope_overlapping() {
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let code_data = vec![0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA];

        core.state.load_binary(origin, code_data).unwrap();

        core.state.scopes.insert(Addr(0x1001), Addr(0x1002));

        core.view.active_pane = ActivePane::Disassembly;
        core.view.is_visual_mode = true;

        core.view.selection_start = Some(0);
        core.view.cursor_index = 2;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 1);
        assert_eq!(core.state.scopes.get(&Addr(0x1001)), Some(&Addr(0x1002)));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );

        core.view.is_visual_mode = true;
        core.view.selection_start = Some(2);
        core.view.cursor_index = 4;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 1);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );

        core.view.is_visual_mode = true;
        core.view.selection_start = Some(3);
        core.view.cursor_index = 4;
        let events = core.apply_action(AppAction::Scope);

        assert_eq!(core.state.scopes.len(), 2);
        assert_eq!(core.state.scopes.get(&Addr(0x1003)), Some(&Addr(0x1004)));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("overlaps")))
        );
    }

    #[test]
    fn test_apply_block_type_with_splitter_even_check() {
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let data = vec![
            0x54, 0xA1, 0xDA, 0x19, 0x19, 0x19, // 6 bytes
            0x51, 0x9E, 0xD7, 0x19, 0x19, 0x19, // 6 bytes
        ];
        core.state.load_binary(origin, data).unwrap();

        // Set block types to DataByte so they show up as individual bytes (1 line per byte)
        core.state.block_types = vec![crate::state::BlockType::DataByte; 12];

        // Disassemble to populate disassembly vector
        core.state.disassemble();

        // Add splitter at $1006
        core.state.toggle_splitter(Addr(0x1006));

        // Re-disassemble to include splitter in disassembly vector
        core.state.disassemble();

        core.view.active_pane = ActivePane::Disassembly;
        core.view.is_visual_mode = true;

        core.view.selection_start = Some(0); // $1000
        core.view.cursor_index = 12; // $100B (last byte, line index 12 because of splitter at index 6)

        let events = core.apply_action(crate::state::AppAction::SetLoHiAddress);

        // Verify that we did NOT get the error message
        let has_error = events.iter().any(|e| match e {
            CoreEvent::StatusMessage(msg) => msg.contains("requires even number of bytes"),
            _ => false,
        });

        assert!(!has_error, "Expected no even byte error, but got one");
    }

    #[test]
    fn test_cycle_immediate_format() {
        use crate::state::types::ImmediateFormat;
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let code_data = vec![0xA9, 0x05]; // LDA #$05

        core.state.load_binary(origin, code_data).unwrap();

        core.view.active_pane = ActivePane::Disassembly;
        core.view.cursor_index = 0; // Pointing to the LDA instruction

        // Initial state: None (effectively Hex)
        assert_eq!(core.state.immediate_value_formats.get(&origin), None);

        // Cycle forward
        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::InvertedHex)
        );

        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::Decimal)
        );

        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::NegativeDecimal)
        );

        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::Binary)
        );

        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::InvertedBinary)
        );

        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::Hex)
        );

        // Loop back to InvertedHex
        core.apply_action(AppAction::NextImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::InvertedHex)
        );

        // Test backward cycling from InvertedHex
        core.apply_action(AppAction::PreviousImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::Hex)
        );

        core.apply_action(AppAction::PreviousImmediateFormat);
        assert_eq!(
            core.state.immediate_value_formats.get(&origin),
            Some(&ImmediateFormat::InvertedBinary)
        );
    }

    #[test]
    fn test_handle_jump_unexplored() {
        use crate::state::types::BlockType;
        let mut core = Core::new();
        let origin = Addr(0x1000);
        let data = vec![0xEA; 10];
        core.state.load_binary(origin, data).unwrap();

        // Pattern: C C U U C C U U C C (C=Code, U=Undefined)
        core.state.block_types = vec![
            BlockType::Code,
            BlockType::Code,
            BlockType::Undefined,
            BlockType::Undefined,
            BlockType::Code,
            BlockType::Code,
            BlockType::Undefined,
            BlockType::Undefined,
            BlockType::Code,
            BlockType::Code,
        ];

        core.state.disassemble();

        core.view.active_pane = ActivePane::Disassembly;

        // Start at beginning ($1000)
        core.view.cursor_index = 0;

        // Jump Next -> should go to $1002
        core.apply_action(AppAction::JumpNextUnexplored);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address,
            Addr(0x1002)
        );

        // Jump Next -> should go to $1006
        core.apply_action(AppAction::JumpNextUnexplored);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address,
            Addr(0x1006)
        );

        // Jump Next -> no more, should stay at $1006
        core.apply_action(AppAction::JumpNextUnexplored);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address,
            Addr(0x1006)
        );

        // Jump Prev -> should go back to $1002
        core.apply_action(AppAction::JumpPrevUnexplored);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address,
            Addr(0x1002)
        );

        // Jump Prev -> no more, should stay at $1002
        core.apply_action(AppAction::JumpPrevUnexplored);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address,
            Addr(0x1002)
        );
    }

    #[test]
    fn test_analyze_preserves_cursor_on_external_label_definition() {
        use crate::state::project::Label;
        use crate::state::types::{BlockType, LabelKind, LabelType};

        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x0801), vec![0xA9, 0x00, 0x85, 0x02, 0x60])
            .unwrap();
        core.state.block_types = vec![BlockType::Code; 5];

        // External label outside the loaded range
        core.state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        core.state.settings.all_labels = true;
        core.state.disassemble();

        // Find the line for VIC_BORDER
        let ext_idx = core
            .state
            .disassembly
            .iter()
            .position(|l| l.external_label_address == Some(Addr(0xD020)))
            .expect("Should have external label line for $D020");

        core.view.cursor_index = ext_idx;
        core.view.active_pane = ActivePane::Disassembly;

        // Run Analyze
        core.apply_action(AppAction::Analyze);

        // Cursor must land back on the VIC_BORDER definition line
        let line = &core.state.disassembly[core.view.cursor_index];
        assert_eq!(
            line.address,
            Addr(0xD020),
            "Definition line should carry the real label address"
        );
        assert_eq!(
            line.external_label_address,
            Some(Addr(0xD020)),
            "Cursor should stay on the external label definition after Analyze"
        );
    }

    /// Regression test: Ctrl+K (ToggleCollapsedBlock) on a DataByte block that
    /// has a splitter in the middle must collapse only the sub-block the cursor
    /// is in, not the entire merged block.  Previously `toggle_collapsed_block`
    /// used `get_compressed_blocks()` which ignores splitters.
    #[test]
    fn test_toggle_collapse_respects_splitter() {
        use crate::state::types::BlockType;

        let mut core = Core::new();
        // 10 data bytes at $1000–$1009
        core.state
            .load_binary(Addr(0x1000), vec![0x00; 10])
            .unwrap();
        core.state.block_types = vec![BlockType::DataByte; 10];

        // Splitter at $1005 → two virtual sub-blocks: $1000–$1004 and $1005–$1009
        core.state.toggle_splitter(Addr(0x1005));
        core.state.disassemble();

        // Place cursor on the first byte ($1000) and collapse
        core.view.active_pane = ActivePane::Disassembly;
        core.view.cursor_index = 0;

        let events = core.apply_action(AppAction::ToggleCollapsedBlock);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CoreEvent::StatusMessage(msg) if msg.contains("Collapsed"))),
            "Should emit 'Collapsed block' status"
        );

        // Only the first sub-block ($1000–$1004, offsets 0–4) should be collapsed
        assert_eq!(
            core.state.collapsed_blocks.len(),
            1,
            "Exactly one collapsed range expected"
        );
        assert_eq!(
            core.state.collapsed_blocks[0],
            (0, 4),
            "Collapsed range should be offsets 0–4 (the first sub-block)"
        );

        // The second sub-block ($1005–$1009) must still be fully visible in
        // the disassembly — look for a non-collapsed line at $1005.
        let has_1005 = core
            .state
            .disassembly
            .iter()
            .any(|l| l.address == Addr(0x1005) && !l.is_collapsed);
        assert!(
            has_1005,
            "Address $1005 should still be visible (not collapsed)"
        );
    }

    #[test]
    fn test_analyze_preserves_cursor_on_external_label_header() {
        use crate::state::project::Label;
        use crate::state::types::{BlockType, LabelKind, LabelType};

        let mut core = Core::new();
        core.state
            .load_binary(Addr(0x0801), vec![0xA9, 0x00, 0x60])
            .unwrap();
        core.state.block_types = vec![BlockType::Code; 3];

        // Add an external label so the header/separator lines are generated
        core.state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        core.state.settings.all_labels = true;
        core.state.disassemble();

        // Put cursor on the header line (the "; FIELDS" comment line, index 0)
        // Header lines have external_label_address == None and address == Addr::ZERO
        assert!(
            core.state.disassembly[0].external_label_address.is_none(),
            "First line should be a header"
        );
        core.view.cursor_index = 0;
        core.view.active_pane = ActivePane::Disassembly;

        core.apply_action(AppAction::Analyze);

        assert_eq!(
            core.view.cursor_index, 0,
            "Cursor should remain at header line after Analyze"
        );
    }

    #[test]
    fn test_preserve_cursor_various_actions() {
        use crate::state::types::{BlockType, EnumDefinition};
        use crate::view_state::ActivePane;

        let mut core = Core::new();
        // Load a small binary: 10 bytes
        core.state
            .load_binary(
                Addr(0x1000),
                vec![0xA9, 0x01, 0x8D, 0x20, 0xD0, 0xA9, 0x00, 0x8D, 0x21, 0xD0],
            )
            .unwrap();
        core.state.block_types = vec![BlockType::Code; 10];
        core.state.settings.all_labels = false;
        core.state.disassemble();

        // Find the index of line with address 0x1002 (STA $D020)
        let addr_1002 = Addr(0x1002);
        let idx = core.state.get_line_index_for_address(addr_1002).unwrap();
        core.view.cursor_index = idx;
        core.view.active_pane = ActivePane::Disassembly;

        // 1. Analyze
        core.apply_action(AppAction::Analyze);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after Analyze"
        );

        // 2. Add Enum Definition
        let enum_def = EnumDefinition {
            name: "MyEnum".to_string(),
            description: None,
            source_file: None,
            variants: std::collections::BTreeMap::from([(1, "ONE".to_string())]),
        };
        core.apply_action(AppAction::ApplyEnumDefinition {
            name: "MyEnum".to_string(),
            definition: Some(enum_def),
            rename_from: None,
        });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after adding enum definition"
        );

        // 3. Apply Enum Usage
        core.apply_action(AppAction::ApplyEnumUsage {
            address: addr_1002,
            enum_name: Some("MyEnum".to_string()),
        });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after applying enum usage"
        );

        // 4. Remove Enum Definition
        core.apply_action(AppAction::ApplyEnumDefinition {
            name: "MyEnum".to_string(),
            definition: None,
            rename_from: None,
        });
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after removing enum definition"
        );

        // 5. Change Document Settings (all_labels = true) and Analyze
        core.state.settings.all_labels = true;
        // Need an external label to see effect
        use crate::state::project::Label;
        use crate::state::types::{LabelKind, LabelType};
        core.state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );
        core.apply_action(AppAction::Analyze);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after Analyze with all_labels=true"
        );

        // 6. Undo/Redo
        core.apply_action(AppAction::Undo);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after Undo"
        );
        core.apply_action(AppAction::Redo);
        assert_eq!(
            core.state.disassembly[core.view.cursor_index].address, addr_1002,
            "Cursor should remain at 0x1002 after Redo"
        );
    }

    #[test]
    fn test_set_bytes_block_by_offset() {
        let mut core = Core::new();
        core.state.origin = Addr(0x1000);
        core.state.raw_data = vec![0xEA; 5];
        core.state.block_types = vec![crate::state::BlockType::Code; 5];
        core.state.disassemble();

        // Convert offset 1 through 3 to bytes
        core.apply_action(AppAction::SetBytesBlockByOffset { start: 1, end: 3 });

        assert_eq!(core.state.block_types[0], crate::state::BlockType::Code);
        assert_eq!(core.state.block_types[1], crate::state::BlockType::DataByte);
        assert_eq!(core.state.block_types[2], crate::state::BlockType::DataByte);
        assert_eq!(core.state.block_types[3], crate::state::BlockType::DataByte);
        assert_eq!(core.state.block_types[4], crate::state::BlockType::Code);
    }
}
