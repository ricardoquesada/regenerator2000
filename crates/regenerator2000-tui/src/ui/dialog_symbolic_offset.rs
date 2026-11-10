//! Modal dialog (`o`) for editing Tier 1 (`operand_offset`) and Tier 2 (`address_offset`)
//! symbolic offsets.
//!
//! [`SymbolicOffsetDialog`] accepts binary expressions (`zp_ptr + 1`, `SCREEN_RAM + $28`), signed
//! shorthands (`+1`, `-1`), base symbols, and clear tokens (`""`, `"0"`). `Tab` moves focus to
//! the global scope checkbox, and `Space` toggles scope when unlocked. `Enter` checks submissions
//! via [`validate_symbolic_offset_submission`] before closing the dialog.

use crate::action_handlers::disassembly_handler::{
    SymbolicOffsetSubmission, validate_symbolic_offset_submission,
};
use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
use crate::ui::scoped_input::{ScopeLock, ScopedInputEvent, ScopedInputModal};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

const HINT_TEXT: &str =
    "Enter expression (e.g. zp_ptr + 1, sub - 1, SCREEN + $28), base name, or empty to clear";

/// Named configuration parameters for constructing a [`SymbolicOffsetDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolicOffsetDialogParams<'a> {
    /// Referencing instruction or table-entry address for an operand site, or `None` for global items.
    pub site_address: Option<Addr>,
    /// Primary effective target address under edit.
    pub target_address: Addr,
    /// Optional inclusive `(start, end)` address range for visual-selection edits.
    pub address_range: Option<(Addr, Addr)>,
    /// Initial expression or base symbol text pre-populated in the single-line input.
    pub initial_input: &'a str,
    /// `true` selects Tier 2 (`address_offset`) globally; `false` selects Tier 1 (`operand_offset`) at the site.
    pub is_global: bool,
    /// `true` locks the global scope checkbox so `Space` cannot toggle it.
    pub lock_global: bool,
    /// `true` checks 2-byte zero-page instruction base bounds (`base <= $00FF`).
    pub is_zero_page_mode: bool,
}

/// Modal dialog for editing Tier 1 (`operand_offset`) and Tier 2 (`address_offset`) symbolic
/// offsets.
pub struct SymbolicOffsetDialog {
    /// Shared single-line input editor, scope focus state, and inline error state.
    pub(crate) modal: ScopedInputModal,
    /// Referencing instruction or table-entry address for an operand site, or `None` for global items.
    pub site_address: Option<Addr>,
    /// Primary effective target address under edit.
    pub target_address: Addr,
    /// Optional inclusive `(start, end)` address range for visual-selection edits.
    pub address_range: Option<(Addr, Addr)>,
    /// `true` applies Tier 2 (`address_offset`) globally, while `false` applies Tier 1
    /// (`operand_offset`) at the site.
    pub is_global: bool,
    /// `true` when checking a 2-byte zero-page instruction operand (`base <= $00FF`).
    pub is_zero_page_mode: bool,
}

impl SymbolicOffsetDialog {
    /// Creates a [`SymbolicOffsetDialog`] from named [`SymbolicOffsetDialogParams`].
    #[must_use]
    pub fn new(params: SymbolicOffsetDialogParams<'_>) -> Self {
        let scope_lock = if params.lock_global {
            ScopeLock::Locked
        } else {
            ScopeLock::Unlocked
        };
        Self {
            modal: ScopedInputModal::new(Some(params.initial_input), scope_lock),
            site_address: params.site_address,
            target_address: params.target_address,
            address_range: params.address_range,
            is_global: params.lock_global || params.is_global,
            is_zero_page_mode: params.is_zero_page_mode,
        }
    }

    /// Formats the modal title bar showing the range, operand site, or target address.
    #[must_use]
    fn title(&self) -> String {
        if let Some((start, end)) = self.address_range
            && start < end
        {
            format!(" Symbolic Offset for ${start:04X}..=${end:04X} ")
        } else if let Some(site) = self.site_address {
            format!(
                " Symbolic Offset for ${:04X} (at ${site:04X}) ",
                self.target_address.0
            )
        } else {
            format!(" Symbolic Offset for ${:04X} ", self.target_address.0)
        }
    }

    /// Formats the global scope checkbox label and appends `" (locked)"` when locked.
    #[must_use]
    fn scope_checkbox_label(&self) -> String {
        let mark = if self.is_global { "X" } else { " " };
        let lock_suffix = if self.modal.lock_scope {
            " (locked)"
        } else {
            ""
        };
        if let Some((start, end)) = self.address_range
            && start < end
        {
            format!("[{mark}] Apply globally to target addresses{lock_suffix}")
        } else {
            format!(
                "[{mark}] Apply globally to all ${:04X} references{lock_suffix}",
                self.target_address.0
            )
        }
    }
}

impl Widget for SymbolicOffsetDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        ui_state.active_dialog_area = self.modal.render(
            f,
            area,
            &ui_state.theme,
            &self.title(),
            &self.scope_checkbox_label(),
            HINT_TEXT,
        );
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match self.modal.handle_key(key) {
            ScopedInputEvent::Cancel => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            ScopedInputEvent::Submit => {
                let expr = self.modal.current_text().trim().to_string();
                let effective_global = self.modal.lock_scope || self.is_global;
                let submission = SymbolicOffsetSubmission {
                    site_address: self.site_address,
                    target_address: self.target_address,
                    address_range: self.address_range,
                    input: &expr,
                    is_global: effective_global,
                    is_zero_page_mode: self.is_zero_page_mode,
                };
                // Check the submission before emitting `AppAction::ApplySymbolicOffset`.
                // Returning `WidgetResult::Handled` on error keeps user text and `dialog_queue` intact.
                if let Err(err) = validate_symbolic_offset_submission(app_state, &submission) {
                    return self.modal.fail_validation(err, ui_state);
                }
                self.modal.error_message = None;
                WidgetResult::Action(AppAction::ApplySymbolicOffset {
                    site_address: self.site_address,
                    target_address: self.target_address,
                    address_range: self.address_range,
                    input: expr,
                    is_global: effective_global,
                    is_zero_page_mode: self.is_zero_page_mode,
                })
            }
            ScopedInputEvent::ToggleScope => {
                self.is_global = !self.is_global;
                WidgetResult::Handled
            }
            ScopedInputEvent::Handled | ScopedInputEvent::Ignored => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BlockType, Label, LabelKind, LabelType};
    use crate::ui::scoped_input::test_support::DialogHarness;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn state_with_zp_ptr() -> AppState {
        let mut app_state = AppState::new();
        app_state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state
    }

    #[test]
    fn test_symbolic_offset_dialog_input_filtering_scope_toggle_and_locked_global() {
        let mut h = DialogHarness::new(
            SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
                site_address: Some(Addr(0x1020)),
                target_address: Addr(0x00FC),
                address_range: None,
                initial_input: "",
                is_global: false,
                lock_global: false,
                is_zero_page_mode: true,
            }),
            AppState::new(),
        );

        // Check allowed characters and filter out disallowed punctuation (`!`, `#`, `%`, `;`).
        h.type_str("(irq.@zp_1::p + $1)!#%;");
        assert_eq!(h.dialog.modal.current_text(), "(irq.@zp_1::p + $1)");

        h.key(KeyCode::Tab);
        assert!(h.dialog.modal.editing_scope);
        h.key(KeyCode::Char(' '));
        assert!(h.dialog.is_global);
        h.key_mod(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert!(!h.dialog.modal.editing_scope);

        let rendered = h.render();
        assert!(rendered.contains(" Symbolic Offset for $00FC (at $1020) "));
        assert!(rendered.contains("[X] Apply globally to all $00FC references"));
        assert!(rendered.contains("Enter expression"));

        let mut locked = DialogHarness::new(
            SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
                site_address: None,
                target_address: Addr(0x00FC),
                address_range: None,
                initial_input: "zp_ptr + 1",
                is_global: true,
                lock_global: true,
                is_zero_page_mode: false,
            }),
            AppState::new(),
        );
        locked.key(KeyCode::Tab);
        assert!(locked.dialog.modal.editing_scope);
        locked.key(KeyCode::Char(' '));
        assert!(
            locked.dialog.is_global,
            "Space must not toggle is_global when lock_global is true"
        );
    }

    #[test]
    fn test_symbolic_offset_dialog_non_destructive_validation_and_closes_dialog() {
        let mut app_state = state_with_zp_ptr();
        app_state.labels.insert(
            Addr(0x0100),
            vec![Label {
                name: "table_0100".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        let mut h = DialogHarness::new(
            SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
                site_address: Some(Addr(0x1020)),
                target_address: Addr(0x00FC),
                address_range: None,
                initial_input: "",
                is_global: false,
                lock_global: false,
                is_zero_page_mode: true,
            }),
            app_state,
        );

        h.type_str("zp_ptr + 99");
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Handled);
        assert_eq!(h.dialog.modal.current_text(), "zp_ptr + 99");
        assert!(h.ui_state.status_message.as_deref().is_some_and(|m| {
            m.starts_with("Error: ") && m.contains("$00FC") && m.contains("zp_ptr + 1")
        }));

        h.clear();
        h.type_str("table_0100 - 4");
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Handled);
        assert_eq!(h.dialog.modal.current_text(), "table_0100 - 4");
        assert!(
            h.ui_state
                .status_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Error: ") && m.contains("<= $00FF"))
        );

        h.clear();
        h.type_str("zp_ptr + 1");
        let expected_action = AppAction::ApplySymbolicOffset {
            site_address: Some(Addr(0x1020)),
            target_address: Addr(0x00FC),
            address_range: None,
            input: "zp_ptr + 1".to_string(),
            is_global: false,
            is_zero_page_mode: true,
        };
        assert!(expected_action.closes_dialog());
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Action(expected_action));
    }

    #[test]
    fn test_symbolic_offset_dialog_ignores_ctrl_j_m_o_newlines() {
        let sanitized = SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
            site_address: Some(Addr(0x1020)),
            target_address: Addr(0x00FC),
            address_range: None,
            initial_input: "zp_ptr\r\n + 1",
            is_global: false,
            lock_global: false,
            is_zero_page_mode: true,
        });
        assert_eq!(sanitized.modal.textarea.lines().len(), 1);
        assert_eq!(sanitized.modal.current_text(), "zp_ptr + 1");

        let mut h = DialogHarness::new(
            SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
                site_address: Some(Addr(0x1020)),
                target_address: Addr(0x00FC),
                address_range: None,
                initial_input: "",
                is_global: false,
                lock_global: false,
                is_zero_page_mode: true,
            }),
            state_with_zp_ptr(),
        );
        h.type_str("zp_ptr + ");
        for ctrl_ch in ['j', 'm', 'o', 'J', 'M', 'O'] {
            h.key_mod(KeyCode::Char(ctrl_ch), KeyModifiers::CONTROL);
        }
        for alt_ch in ['a', 'j', 'm', 'x'] {
            h.key_mod(KeyCode::Char(alt_ch), KeyModifiers::ALT);
        }
        h.key(KeyCode::Char('\n'));
        h.type_str("1");

        assert_eq!(h.dialog.modal.textarea.lines().len(), 1);
        assert_eq!(h.dialog.modal.current_text(), "zp_ptr + 1");
        assert_eq!(
            h.key(KeyCode::Enter),
            WidgetResult::Action(AppAction::ApplySymbolicOffset {
                site_address: Some(Addr(0x1020)),
                target_address: Addr(0x00FC),
                address_range: None,
                input: "zp_ptr + 1".to_string(),
                is_global: false,
                is_zero_page_mode: true,
            })
        );
    }

    #[test]
    fn test_symbolic_offset_dialog_range_render_and_inline_error_footer() {
        let mut app_state = state_with_zp_ptr();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0x85, 0xFC, 0xEA, 0xEA, 0x85, 0xFC];
        app_state.block_types = vec![BlockType::Code; 6];
        app_state.disassemble();

        let mut h = DialogHarness::new(
            SymbolicOffsetDialog::new(SymbolicOffsetDialogParams {
                site_address: Some(Addr(0x2000)),
                target_address: Addr(0x00FC),
                address_range: Some((Addr(0x2000), Addr(0x2005))),
                initial_input: "",
                is_global: false,
                lock_global: false,
                is_zero_page_mode: true,
            }),
            app_state,
        );

        let initial_frame = h.render();
        assert!(initial_frame.contains(" Symbolic Offset for $2000..=$2005 "));
        assert!(initial_frame.contains("[ ] Apply globally to target addresses"));
        assert!(initial_frame.contains("Enter expression"));

        h.type_str("zp_ptr + 99");
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Handled);
        assert!(h.dialog.modal.error_message.is_some());

        let error_frame = h.render();
        assert!(error_frame.contains(" Symbolic Offset for $2000..=$2005 "));
        assert!(error_frame.contains("$00FC") && error_frame.contains("zp_ptr + 1"));
        assert!(!error_frame.contains("Enter expression"));

        h.key(KeyCode::Backspace);
        assert!(h.dialog.modal.error_message.is_none());
        assert!(h.render().contains("Enter expression"));
    }
}
