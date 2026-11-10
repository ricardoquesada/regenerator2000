//! Shared single-line modal input component with a toggleable scope checkbox.
//!
//! [`ScopedInputModal`] backs [`super::dialog_label::LabelDialog`] and
//! [`super::dialog_symbolic_offset::SymbolicOffsetDialog`]. It enforces single-line input, filters
//! characters, handles `Tab` and `Space` scope focus, and shows inline errors.

use crate::theme::Theme;
use crate::ui::widget::WidgetResult;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use ratatui_textarea::{CursorMove, TextArea};

/// Outcome of routing a [`KeyEvent`] through [`ScopedInputModal::handle_key`].
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopedInputEvent {
    /// User pressed `Esc` to cancel and close the dialog.
    Cancel,
    /// User pressed `Enter` to check and submit the input.
    Submit,
    /// User pressed `Space` on an unlocked scope checkbox.
    ToggleScope,
    /// Modal handled the key press internally.
    Handled,
    /// Modal did not recognize the key press.
    Ignored,
}

/// Controls whether the scope checkbox in [`ScopedInputModal`] can be toggled by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeLock {
    /// User can toggle the scope checkbox with `Space`.
    Unlocked,
    /// Scope checkbox is locked and ignores `Space`.
    Locked,
}

/// Shared 8-row modal editor with a single-line [`TextArea`], scope checkbox, and footer.
pub(crate) struct ScopedInputModal {
    /// Single-line text editor holding the candidate label or expression.
    pub textarea: TextArea<'static>,
    /// `true` when keyboard focus rests on the scope checkbox instead of [`Self::textarea`].
    pub editing_scope: bool,
    /// `true` when the scope checkbox is locked and ignores `Space`.
    pub lock_scope: bool,
    /// Inline validation error that the footer shows when `Enter` fails validation.
    pub error_message: Option<String>,
}

impl ScopedInputModal {
    /// Creates a [`ScopedInputModal`] focused on the text input and strips `\r`/`\n` from `initial_text`.
    #[must_use]
    pub fn new(initial_text: Option<&str>, scope_lock: ScopeLock) -> Self {
        let mut textarea = TextArea::default();
        if let Some(text) = initial_text {
            let sanitized = sanitize_single_line_input(text);
            if !sanitized.is_empty() {
                textarea.insert_str(&sanitized);
                textarea.move_cursor(CursorMove::End);
            }
        }
        Self {
            textarea,
            editing_scope: false,
            lock_scope: scope_lock == ScopeLock::Locked,
            error_message: None,
        }
    }

    /// Returns the current single-line text without trimming surrounding spaces.
    #[must_use]
    pub fn current_text(&self) -> String {
        self.textarea.lines().join("")
    }

    /// Stores `err` for the inline footer and shows `"Error: {err}"` in the status bar.
    #[must_use]
    pub fn fail_validation(&mut self, err: String, ui_state: &mut UIState) -> WidgetResult {
        ui_state.set_status_message(format!("Error: {err}"));
        self.error_message = Some(err);
        WidgetResult::Handled
    }

    /// Draws the centered 8-row modal dialog and returns its outer [`Rect`].
    #[must_use]
    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        theme: &Theme,
        title: &str,
        checkbox_label: &str,
        hint_text: &str,
    ) -> Rect {
        let block = crate::ui::widget::create_dialog_block(title, theme);
        // Height: 2 (dialog border) + 3 (input with border) + 1 (checkbox) + 2 (footer) = 8
        let dialog_area = crate::utils::centered_rect_adaptive(75, 92, 0, 8, area);
        f.render_widget(Clear, dialog_area);
        f.render_widget(block.clone(), dialog_area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(block.inner(dialog_area));

        let mut textarea = self.textarea.clone();
        let border_fg = if self.editing_scope {
            theme.dialog_border
        } else {
            theme.highlight_fg
        };
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_fg)),
        );
        textarea.set_style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        let cursor_style = if self.editing_scope {
            Style::default().fg(theme.dialog_fg)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
        };
        textarea.set_cursor_style(cursor_style);
        textarea.set_cursor_line_style(Style::default());
        f.render_widget(&textarea, layout[0]);

        let check_style = if self.editing_scope {
            let fg = if self.lock_scope {
                theme.menu_disabled_fg
            } else {
                theme.highlight_fg
            };
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else if self.lock_scope {
            Style::default().fg(theme.menu_disabled_fg)
        } else {
            Style::default().fg(theme.dialog_fg)
        };
        f.render_widget(Paragraph::new(checkbox_label).style(check_style), layout[1]);

        let (footer_text, footer_fg) = match &self.error_message {
            Some(err) => (err.as_str(), theme.error_fg),
            None => (hint_text, theme.comment),
        };
        f.render_widget(
            Paragraph::new(footer_text)
                .style(Style::default().fg(footer_fg))
                .wrap(Wrap { trim: true }),
            layout[2],
        );

        dialog_area
    }

    /// Routes `key` through single-line editing, scope focus, and submission rules.
    pub fn handle_key(&mut self, key: KeyEvent) -> ScopedInputEvent {
        match key.code {
            KeyCode::Esc => ScopedInputEvent::Cancel,
            KeyCode::Enter => ScopedInputEvent::Submit,
            KeyCode::Tab | KeyCode::BackTab => {
                self.editing_scope = !self.editing_scope;
                ScopedInputEvent::Handled
            }
            _ if self.editing_scope => {
                if key.code == KeyCode::Char(' ')
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && !self.lock_scope
                {
                    self.error_message = None;
                    return ScopedInputEvent::ToggleScope;
                }
                ScopedInputEvent::Handled
            }
            // Block Ctrl+J, Ctrl+M, and Ctrl+O so the editor stays strictly single-line.
            KeyCode::Char(c) if key.modifiers == KeyModifiers::CONTROL => {
                self.apply_ctrl_chord(c);
                ScopedInputEvent::Handled
            }
            KeyCode::Char(c) => {
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && is_allowed_input_char(c)
                {
                    self.error_message = None;
                    self.textarea.input(key);
                }
                ScopedInputEvent::Handled
            }
            KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End => {
                self.error_message = None;
                self.textarea.input(key);
                ScopedInputEvent::Handled
            }
            _ => ScopedInputEvent::Ignored,
        }
    }

    /// Applies supported single-line `Ctrl` cursor and delete chords.
    fn apply_ctrl_chord(&mut self, c: char) {
        match c.to_ascii_lowercase() {
            'a' => self.textarea.move_cursor(CursorMove::Head),
            'e' => self.textarea.move_cursor(CursorMove::End),
            'b' => self.textarea.move_cursor(CursorMove::Back),
            'f' => self.textarea.move_cursor(CursorMove::Forward),
            'h' => _ = self.textarea.delete_char(),
            'd' => _ = self.textarea.delete_next_char(),
            'k' => _ = self.textarea.delete_line_by_end(),
            'u' => _ = self.textarea.delete_line_by_head(),
            'w' => _ = self.textarea.delete_word(),
            _ => return,
        }
        self.error_message = None;
    }
}

/// Checks whether `c` belongs to the identifier and symbolic-offset character set.
fn is_allowed_input_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | '+' | '-' | '$' | ' ' | '.' | ':' | '@' | '(' | ')')
}

/// Deletes `\r` and `\n` characters so `text` stays on one line.
fn sanitize_single_line_input(text: &str) -> String {
    text.chars().filter(|&c| c != '\n' && c != '\r').collect()
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::state::AppState;
    use crate::ui::widget::Widget;
    use ratatui::{Terminal, backend::TestBackend};

    /// Renders `widget` into a 100x20 [`TestBackend`] buffer and returns the concatenated symbols.
    pub(crate) fn render_widget_text<W: Widget + ?Sized>(
        widget: &W,
        app_state: &AppState,
        ui_state: &mut UIState,
    ) -> String {
        let mut terminal = Terminal::new(TestBackend::new(100, 20)).unwrap();
        terminal
            .draw(|f| widget.render(f, f.area(), app_state, ui_state))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    /// Headless unit-test driver pairing a modal [`Widget`] with [`AppState`] and [`UIState`].
    pub(crate) struct DialogHarness<W> {
        pub dialog: W,
        pub app_state: AppState,
        pub ui_state: UIState,
    }

    impl<W: Widget> DialogHarness<W> {
        pub(crate) fn new(dialog: W, app_state: AppState) -> Self {
            Self {
                dialog,
                app_state,
                ui_state: UIState::new(Theme::default()),
            }
        }

        pub(crate) fn key(&mut self, code: KeyCode) -> WidgetResult {
            self.key_mod(code, KeyModifiers::NONE)
        }

        pub(crate) fn key_mod(&mut self, code: KeyCode, mods: KeyModifiers) -> WidgetResult {
            self.dialog.handle_input(
                KeyEvent::new(code, mods),
                &mut self.app_state,
                &mut self.ui_state,
            )
        }

        pub(crate) fn type_str(&mut self, text: &str) {
            for ch in text.chars() {
                self.key(KeyCode::Char(ch));
            }
        }

        pub(crate) fn clear(&mut self) {
            self.key_mod(KeyCode::Char('e'), KeyModifiers::CONTROL);
            self.key_mod(KeyCode::Char('u'), KeyModifiers::CONTROL);
        }

        pub(crate) fn render(&mut self) -> String {
            render_widget_text(&self.dialog, &self.app_state, &mut self.ui_state)
        }
    }

    #[test]
    fn scoped_input_modal_handles_ctrl_chords_and_reports_ignored_keys() {
        let mut modal = ScopedInputModal::new(Some("ab\r\ncd ef"), ScopeLock::Unlocked);
        assert_eq!(modal.current_text(), "abcd ef");

        modal.error_message = Some("err".to_string());
        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            ScopedInputEvent::Ignored
        );
        assert!(modal.error_message.is_some());

        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            ScopedInputEvent::Handled
        );
        assert!(modal.error_message.is_none());
        assert_eq!(modal.current_text(), "abcd ");

        for c in ['a', 'f', 'd', 'h'] {
            assert_eq!(
                modal.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
                ScopedInputEvent::Handled
            );
        }
        assert_eq!(modal.current_text(), "cd ");
        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            ScopedInputEvent::Handled
        );
        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            ScopedInputEvent::Handled
        );
        assert_eq!(modal.current_text(), "");

        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ScopedInputEvent::Cancel
        );
        assert_eq!(
            modal.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            ScopedInputEvent::Submit
        );
    }
}
