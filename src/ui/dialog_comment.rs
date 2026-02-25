use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line as TextLine, Span},
    widgets::Paragraph,
};
use tui_textarea::{CursorMove, TextArea};

use crate::ui::widget::{Widget, WidgetResult};

const SEPARATOR_LEN: usize = 78;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentType {
    Side,
    Line,
}

pub struct CommentDialog {
    pub textarea: TextArea<'static>,
    pub comment_type: CommentType,
}

impl CommentDialog {
    pub fn new(current_comment: Option<&str>, comment_type: CommentType) -> Self {
        let textarea = if let Some(comment) = current_comment {
            let mut t = TextArea::from(comment.lines());
            // For existing comments, we assume user wants to edit them as is.
            // If it was single line, lines() works.
            // If empty string, lines() is empty, TextArea becomes empty.
            if comment.is_empty() {
                // Fallback to default logic if actually empty string passed (rare)
                Self::create_default_textarea(&comment_type)
            } else {
                t.move_cursor(CursorMove::Bottom);
                t.move_cursor(CursorMove::End);
                t
            }
        } else {
            Self::create_default_textarea(&comment_type)
        };

        Self {
            textarea,
            comment_type,
        }
    }

    fn create_default_textarea(comment_type: &CommentType) -> TextArea<'static> {
        match comment_type {
            CommentType::Line => {
                let default_text = "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-";
                let mut textarea = TextArea::from(vec![default_text.to_string(), "".to_string()]);
                textarea.move_cursor(CursorMove::Bottom);
                textarea.move_cursor(CursorMove::End);
                textarea
            }
            CommentType::Side => TextArea::default(),
        }
    }

    /// Insert a separator string.
    /// - If the current line is empty, replace it in-place.
    /// - If the current line has content, move to its end and insert below.
    ///
    /// In both cases the cursor ends up on the line following the separator.
    fn insert_separator(&mut self, sep: &str) {
        let (row, _) = self.textarea.cursor();
        let current_line_empty = self
            .textarea
            .lines()
            .get(row)
            .map(|l| l.is_empty())
            .unwrap_or(true);

        if current_line_empty {
            // Replace the empty line: just insert the text here.
            self.textarea.move_cursor(CursorMove::Head);
        } else {
            // Insert on a new line below.
            self.textarea.move_cursor(CursorMove::End);
            self.textarea.insert_newline();
        }
        self.textarea.insert_str(sep);
        // Leave cursor on the next row (create it if needed).
        self.textarea.insert_newline();
    }
}

impl Widget for CommentDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = match self.comment_type {
            CommentType::Line => " Enter Line Comment ",
            CommentType::Side => " Enter Side Comment ",
        };

        let block = crate::ui::widget::create_dialog_block(title, theme);

        // Fixed height of 10 for multi-line
        let area = match self.comment_type {
            CommentType::Side => crate::utils::centered_rect_adaptive(70, 40, 0, 3, area),
            CommentType::Line => crate::utils::centered_rect_adaptive(70, 40, 40, 12, area),
        };
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        if self.comment_type == CommentType::Line {
            // Render the border block separately so we can split the inner area.
            let inner = block.inner(area);
            f.render_widget(block, area);

            // Split inner into textarea + 1-row hint footer.
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            // Render textarea without its own border (block already drawn above).
            let mut textarea = self.textarea.clone();
            let style = Style::default().fg(theme.highlight_fg);
            textarea.set_style(style);
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_cursor_line_style(Style::default());
            f.render_widget(&textarea, chunks[0]);

            // Render the hint footer.
            let dim = Style::default().add_modifier(Modifier::DIM);
            let sep_style = Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::DIM);
            let hint = Paragraph::new(TextLine::from(vec![
                Span::styled(
                    " Ctrl+Enter",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":new line", dim),
                Span::styled(
                    "  Alt+-",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":", dim),
                Span::styled("────", sep_style),
                Span::styled(
                    "  Alt+=",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":", dim),
                Span::styled("════", sep_style),
                Span::styled(
                    "  Alt+\\",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":", dim),
                Span::styled("-=-=-", sep_style),
            ]));
            f.render_widget(hint, chunks[1]);
        } else {
            let mut textarea = self.textarea.clone();
            textarea.set_block(block);

            let style = Style::default().fg(theme.highlight_fg);
            textarea.set_style(style);

            // Also set cursor style if needed, but default is usually inverse of style
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_cursor_line_style(Style::default());

            f.render_widget(&textarea, area);
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    if self.comment_type == CommentType::Line {
                        self.textarea.insert_newline();
                    }
                    WidgetResult::Handled
                } else if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                    let address = line.external_label_address.unwrap_or(line.address);

                    let lines = self.textarea.lines();
                    // Join with newline for Line comments, space for Side comments
                    let full_comment = match self.comment_type {
                        CommentType::Line => lines.join("\n"),
                        CommentType::Side => lines.join(" "),
                    };
                    let new_comment = full_comment.trim().to_string();

                    let new_comment_opt = if new_comment.is_empty() {
                        None
                    } else {
                        Some(new_comment)
                    };

                    let command = match self.comment_type {
                        CommentType::Side => {
                            let old_comment = app_state.user_side_comments.get(&address).cloned();
                            crate::commands::Command::SetUserSideComment {
                                address,
                                new_comment: new_comment_opt,
                                old_comment,
                            }
                        }
                        CommentType::Line => {
                            let old_comment = app_state.user_line_comments.get(&address).cloned();
                            crate::commands::Command::SetUserLineComment {
                                address,
                                new_comment: new_comment_opt,
                                old_comment,
                            }
                        }
                    };

                    command.apply(app_state);
                    app_state.push_command(command);

                    ui_state.set_status_message("Comment set");
                    app_state.disassemble();
                    WidgetResult::Close
                } else {
                    WidgetResult::Handled
                }
            }
            // Separator shortcuts (only in multi-line / Line comment mode)
            KeyCode::Char('-') if key.modifiers.contains(KeyModifiers::ALT) => {
                if self.comment_type == CommentType::Line {
                    self.insert_separator(&"-".repeat(SEPARATOR_LEN));
                }
                WidgetResult::Handled
            }
            KeyCode::Char('=') if key.modifiers.contains(KeyModifiers::ALT) => {
                if self.comment_type == CommentType::Line {
                    self.insert_separator(&"=".repeat(SEPARATOR_LEN));
                }
                WidgetResult::Handled
            }
            // Alt+\ : on macOS, Option+\ produces « (U+00AB) directly.
            // Catch both forms.
            KeyCode::Char('\\') if key.modifiers.contains(KeyModifiers::ALT) => {
                if self.comment_type == CommentType::Line {
                    let sep = "-=".repeat(SEPARATOR_LEN / 2);
                    self.insert_separator(&sep);
                }
                WidgetResult::Handled
            }
            KeyCode::Char('\u{00AB}') => {
                // macOS Option+\ produces « (U+00AB) directly
                if self.comment_type == CommentType::Line {
                    let sep = "-=".repeat(SEPARATOR_LEN / 2);
                    self.insert_separator(&sep);
                }
                WidgetResult::Handled
            }
            _ => {
                // Determine if key is editing key or navigation
                // tui-textarea handles input(key) and returns true if changed.
                // We just pass it through.
                self.textarea.input(key);
                WidgetResult::Handled
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_position_at_end() {
        let comment = "Hello\nWorld";
        let dialog = CommentDialog::new(Some(comment), CommentType::Side);
        let cursor = dialog.textarea.cursor();
        // cursor is (row, col)
        // Rows are 0-indexed. Hello is row 0. World is row 1.
        // World has length 5. Cursor should be at (1, 5).
        assert_eq!(cursor, (1, 5));
    }

    #[test]
    fn test_cursor_position_single_line() {
        let comment = "Hello";
        let dialog = CommentDialog::new(Some(comment), CommentType::Side);
        let cursor = dialog.textarea.cursor();
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_cursor_position_default_line_comment() {
        let dialog = CommentDialog::new(None, CommentType::Line);
        let cursor = dialog.textarea.cursor();
        // Default line comment has 2 lines. Cursor should be at the start of the second line (index 1).
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_insert_separator_dashes() {
        let mut dialog = CommentDialog::new(None, CommentType::Line);
        dialog.insert_separator(&"-".repeat(SEPARATOR_LEN));
        let lines = dialog.textarea.lines();
        // Last line is empty (cursor row). Second-to-last is the separator.
        let sep_line = &lines[lines.len() - 2];
        assert_eq!(sep_line.len(), SEPARATOR_LEN);
        assert!(sep_line.chars().all(|c| c == '-'));
        // Cursor must be on the empty line after the separator.
        assert_eq!(dialog.textarea.cursor().0, lines.len() - 1);
    }

    #[test]
    fn test_insert_separator_equals() {
        let mut dialog = CommentDialog::new(None, CommentType::Line);
        dialog.insert_separator(&"=".repeat(SEPARATOR_LEN));
        let lines = dialog.textarea.lines();
        let sep_line = &lines[lines.len() - 2];
        assert_eq!(sep_line.len(), SEPARATOR_LEN);
        assert!(sep_line.chars().all(|c| c == '='));
        assert_eq!(dialog.textarea.cursor().0, lines.len() - 1);
    }
}
