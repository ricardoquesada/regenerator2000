//! Modal dialog (`l`) for editing user labels and Tier 2 symbolic offsets.
//!
//! [`LabelDialog`] accepts plain assembler identifiers (`[A-Za-z_][A-Za-z0-9_]*`) and Tier 2
//! global symbolic offset expressions (`zp_ptr + 1`, `SCREEN_RAM + $28`). `Tab` and `Space` toggle
//! local scope. `Enter` checks input via [`AppState::validate_user_label_or_address_offset`] so
//! invalid names never close the dialog or delete typed input.

use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
use crate::ui::scoped_input::{ScopeLock, ScopedInputEvent, ScopedInputModal};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

const HINT_TEXT: &str =
    "Enter label name (e.g. init_screen), offset (e.g. zp_ptr + 1), or empty to clear";

/// Modal dialog for setting or deleting a user label or Tier 2 symbolic offset at `address`.
pub struct LabelDialog {
    /// Shared single-line input editor, scope focus state, and inline error state.
    pub(crate) modal: ScopedInputModal,
    /// Target 6502 memory address under edit.
    pub address: Addr,
    /// `true` creates a [`LabelType::LocalUserDefined`](crate::state::LabelType::LocalUserDefined)
    /// local label for plain identifiers.
    pub is_local: bool,
}

impl LabelDialog {
    /// Creates a [`LabelDialog`] for `address` and pre-populates `current_label` when present.
    #[must_use]
    pub fn new(current_label: Option<&str>, address: Addr, is_local: bool) -> Self {
        Self {
            modal: ScopedInputModal::new(current_label, ScopeLock::Unlocked),
            address,
            is_local,
        }
    }
}

impl Widget for LabelDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let checkbox_label = if self.is_local {
            "[X] Local Label"
        } else {
            "[ ] Local Label"
        };
        ui_state.active_dialog_area = self.modal.render(
            f,
            area,
            &ui_state.theme,
            " Enter Label Name ",
            checkbox_label,
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
                let label_name = self.modal.current_text().trim().to_string();
                let site_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .filter(|l| {
                        l.external_label_address.is_none() && !app_state.is_external(l.address)
                    })
                    .map(|l| l.address);
                // Check input before emitting `AppAction::ApplyLabel`.
                // Returning `WidgetResult::Handled` on error keeps user text and `dialog_queue` intact.
                if let Err(err) = app_state.validate_user_label_or_address_offset(
                    self.address,
                    &label_name,
                    self.is_local,
                    site_addr,
                ) {
                    return self.modal.fail_validation(err, ui_state);
                }
                self.modal.error_message = None;
                WidgetResult::Action(AppAction::ApplyLabel {
                    address: self.address,
                    name: label_name,
                    is_local: self.is_local,
                })
            }
            ScopedInputEvent::ToggleScope => {
                self.is_local = !self.is_local;
                WidgetResult::Handled
            }
            ScopedInputEvent::Handled | ScopedInputEvent::Ignored => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::process_key_event;
    use crate::state::{BlockType, Label, LabelKind, LabelType};
    use crate::theme::Theme;
    use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
    use crate::ui::scoped_input::test_support::{DialogHarness, render_widget_text};
    use crossterm::event::{KeyCode, KeyModifiers};
    use regenerator2000_core::Core;

    fn render_active_dialog_text(core: &Core, ui_state: &mut UIState) -> String {
        let Some(dialog) = ui_state.active_dialog.take() else {
            return String::new();
        };
        let rendered = render_widget_text(dialog.as_ref(), &core.state, ui_state);
        ui_state.active_dialog = Some(dialog);
        rendered
    }

    #[test]
    fn test_label_dialog_input_filtering_scope_toggle_and_submit() {
        let mut h = DialogHarness::new(
            LabelDialog::new(Some("init"), Addr(0x1000), false),
            AppState::new(),
        );

        // Accept `_` and alphanumeric characters, and filter out `!`, `#`, and `%`.
        h.type_str("_1!#%");
        assert_eq!(h.dialog.modal.current_text(), "init_1");

        // `Tab` moves focus to the local-label checkbox, and `Space` toggles `is_local`.
        h.key(KeyCode::Tab);
        assert!(h.dialog.modal.editing_scope);
        h.key(KeyCode::Char(' '));
        assert!(h.dialog.is_local);

        let rendered = h.render();
        assert!(rendered.contains("init_1"));
        assert!(rendered.contains("[X] Local Label"));
        assert!(rendered.contains("Enter label name"));

        assert_eq!(
            h.key(KeyCode::Enter),
            WidgetResult::Action(AppAction::ApplyLabel {
                address: Addr(0x1000),
                name: "init_1".to_string(),
                is_local: true,
            })
        );
    }

    #[test]
    fn test_label_dialog_symbolic_offset_expression_validation_and_submit() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x2000);
        app_state.raw_data = vec![0xEA; 8];
        app_state.block_types = vec![BlockType::Code; 8];
        app_state
            .annotations
            .update(Addr(0x2000), |e| e.scope = Some(Addr(0x2003)));
        app_state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        app_state.labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "loc_base".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::LocalUserDefined,
            }],
        );
        app_state.disassemble();

        let mut h = DialogHarness::new(LabelDialog::new(None, Addr(0x00FC), false), app_state);
        h.type_str("zp_ptr + 99");
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Handled);
        assert_eq!(h.dialog.modal.current_text(), "zp_ptr + 99");
        assert!(
            h.dialog
                .modal
                .error_message
                .as_deref()
                .is_some_and(|m| m.contains("$00FC") && m.contains("zp_ptr + 1"))
        );
        assert!(h.ui_state.status_message.as_deref().is_some_and(|m| {
            m.starts_with("Error: ") && m.contains("$00FC") && m.contains("zp_ptr + 1")
        }));
        assert!(h.render().contains("zp_ptr + 1"));

        // `Backspace` deletes the inline error and restores the hint text.
        h.key(KeyCode::Backspace);
        assert!(h.dialog.modal.error_message.is_none());
        assert!(h.render().contains("Enter label name"));

        h.key(KeyCode::Backspace);
        h.type_str("1");
        assert_eq!(
            h.key(KeyCode::Enter),
            WidgetResult::Action(AppAction::ApplyLabel {
                address: Addr(0x00FC),
                name: "zp_ptr + 1".to_string(),
                is_local: false,
            })
        );

        h.dialog = LabelDialog::new(None, Addr(0x00FC), false);
        h.type_str("zp_ptr + $1");
        assert_eq!(
            h.key(KeyCode::Enter),
            WidgetResult::Action(AppAction::ApplyLabel {
                address: Addr(0x00FC),
                name: "zp_ptr + $1".to_string(),
                is_local: false,
            })
        );

        // Cursor outside `$2000..=$2003` rejects `loc_base`, while cursor inside resolves it.
        h.ui_state.cursor_index = h
            .app_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x2005) && l.external_label_address.is_none())
            .expect("instruction at $2005 must exist");
        h.dialog = LabelDialog::new(Some("loc_base + 16"), Addr(0x2010), false);
        assert_eq!(h.key(KeyCode::Enter), WidgetResult::Handled);

        h.ui_state.cursor_index = h
            .app_state
            .disassembly
            .iter()
            .position(|l| l.address == Addr(0x2001) && l.external_label_address.is_none())
            .expect("instruction at $2001 must exist");
        h.dialog = LabelDialog::new(Some("loc_base + 16"), Addr(0x2010), false);
        assert_eq!(
            h.key(KeyCode::Enter),
            WidgetResult::Action(AppAction::ApplyLabel {
                address: Addr(0x2010),
                name: "loc_base + 16".to_string(),
                is_local: false,
            })
        );
    }

    #[test]
    fn test_label_dialog_non_destructive_enter_retains_input_and_dialog_queue() {
        let mut core = Core::new();
        core.state.origin = Addr(0x1000);
        core.state.raw_data = vec![0xEA; 16];
        core.state.block_types = vec![BlockType::Code; 16];
        core.state.labels.insert(
            Addr(0x00FB),
            vec![Label {
                name: "zp_ptr".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::UserDefined,
            }],
        );
        core.state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_dialog = Some(Box::new(LabelDialog::new(None, Addr(0x00FC), false)));
        ui_state.dialog_queue.push(Box::new(ShortcutsDialog::new()));

        let press = |core: &mut Core, ui: &mut UIState, code: KeyCode, mods: KeyModifiers| {
            process_key_event(core, ui, KeyEvent::new(code, mods));
        };
        let clear_all = |core: &mut Core, ui: &mut UIState| {
            press(core, ui, KeyCode::Char('e'), KeyModifiers::CONTROL);
            press(core, ui, KeyCode::Char('u'), KeyModifiers::CONTROL);
        };
        let type_str = |text: &str, core: &mut Core, ui: &mut UIState| {
            for ch in text.chars() {
                press(core, ui, KeyCode::Char(ch), KeyModifiers::NONE);
            }
        };

        let invalid_inputs = [
            ("zp_ptr", "already exists"),
            ("(foo)", "Invalid label name"),
            ("foo:bar", "Invalid label name"),
            ("123", "Invalid label name"),
            ("my label", "Invalid label name"),
            ("$0400", "Invalid label name"),
            ("zp_ptr + 99", "$00FC"),
        ];

        for (bad_input, expected_err_substr) in invalid_inputs {
            clear_all(&mut core, &mut ui_state);
            assert!(
                render_active_dialog_text(&core, &mut ui_state).contains("Enter label name"),
                "Editing must clear inline error before typing '{bad_input}'"
            );

            type_str(bad_input, &mut core, &mut ui_state);
            press(&mut core, &mut ui_state, KeyCode::Enter, KeyModifiers::NONE);

            assert_eq!(
                ui_state.dialog_queue.len(),
                1,
                "Invalid Enter on '{bad_input}' must NOT close LabelDialog or promote dialog_queue"
            );
            let frame = render_active_dialog_text(&core, &mut ui_state);
            assert!(frame.contains("Enter Label Name") && frame.contains(bad_input));
            assert!(frame.contains(expected_err_substr));
            assert!(
                ui_state.status_bar.message.starts_with("Error: ")
                    && ui_state.status_bar.message.contains(expected_err_substr)
            );
        }

        // Toggle `[X] Local Label` to clear the error and accept `".local_ok"` as `"local_ok"`.
        clear_all(&mut core, &mut ui_state);
        type_str(".local_ok", &mut core, &mut ui_state);
        press(&mut core, &mut ui_state, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(ui_state.dialog_queue.len(), 1);
        assert!(render_active_dialog_text(&core, &mut ui_state).contains("Invalid label name"));

        press(&mut core, &mut ui_state, KeyCode::Tab, KeyModifiers::NONE);
        press(
            &mut core,
            &mut ui_state,
            KeyCode::Char(' '),
            KeyModifiers::NONE,
        );
        let toggled_frame = render_active_dialog_text(&core, &mut ui_state);
        assert!(toggled_frame.contains("[X] Local Label"));
        assert!(toggled_frame.contains("Enter label name"));

        press(&mut core, &mut ui_state, KeyCode::Enter, KeyModifiers::NONE);
        assert!(ui_state.dialog_queue.is_empty());
        assert!(render_active_dialog_text(&core, &mut ui_state).contains("Keyboard Shortcuts"));
        assert!(
            core.state
                .labels
                .get(&Addr(0x00FC))
                .is_some_and(|v| v.iter().any(|l| l.name == "local_ok"))
        );
    }

    #[test]
    fn test_label_dialog_ignores_ctrl_j_m_o_newlines() {
        let sanitized = LabelDialog::new(Some("my\r\n_label"), Addr(0x00FC), false);
        assert_eq!(sanitized.modal.textarea.lines().len(), 1);
        assert_eq!(sanitized.modal.current_text(), "my_label");

        let mut h =
            DialogHarness::new(LabelDialog::new(None, Addr(0x00FC), false), AppState::new());
        h.type_str("zp_ptr + 1");
        for ctrl_ch in ['j', 'm', 'o', 'J', 'M', 'O'] {
            h.key_mod(KeyCode::Char(ctrl_ch), KeyModifiers::CONTROL);
        }
        for alt_ch in ['a', 'j', 'm', 'x'] {
            h.key_mod(KeyCode::Char(alt_ch), KeyModifiers::ALT);
        }
        assert_eq!(h.dialog.modal.textarea.lines().len(), 1);
        assert_eq!(h.dialog.modal.current_text(), "zp_ptr + 1");

        // `Ctrl+U` deletes the line, and subsequent typing keeps 1 line.
        h.key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL);
        for ctrl_ch in ['j', 'm', 'o'] {
            h.key_mod(KeyCode::Char(ctrl_ch), KeyModifiers::CONTROL);
        }
        h.type_str("foo");
        assert_eq!(h.dialog.modal.textarea.lines().len(), 1);
        assert_eq!(h.dialog.modal.current_text(), "foo");
    }
}
