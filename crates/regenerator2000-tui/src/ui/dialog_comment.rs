use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line as TextLine, Span},
    widgets::Paragraph,
};
use ratatui_textarea::{CursorMove, DataCursor, TextArea};

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
    pub address: crate::state::Addr,
}

impl CommentDialog {
    #[must_use]
    pub fn new(
        current_comment: Option<&str>,
        comment_type: CommentType,
        address: crate::state::Addr,
    ) -> Self {
        let textarea = if let Some(comment) = current_comment {
            let mut t = TextArea::from(comment.lines());
            if comment.is_empty() {
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
            address,
        }
    }

    fn create_default_textarea(comment_type: &CommentType) -> TextArea<'static> {
        match comment_type {
            CommentType::Line => TextArea::default(),
            CommentType::Side => TextArea::default(),
        }
    }

    /// Insert a separator string.
    /// - If the current line is empty, replace it in-place.
    /// - If the current line has content, move to its end and insert below.
    ///
    /// In both cases the cursor ends up on the line following the separator.
    fn insert_separator(&mut self, sep: &str) {
        let DataCursor(row, _) = self.textarea.cursor();
        let current_line_empty = self
            .textarea
            .lines()
            .get(row)
            .is_none_or(std::string::String::is_empty);

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
            ui_state.comment_textarea_area = chunks[0];

            // Render the hint footer.
            let dim = Style::default().add_modifier(Modifier::DIM);
            let sep_style = Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::DIM);
            let hint = Paragraph::new(TextLine::from(vec![
                Span::styled(
                    " Enter",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":save", dim),
                Span::styled(
                    "  Shift+Enter/Alt+Enter",
                    Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
                ),
                Span::styled(":line", dim),
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
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                match self.comment_type {
                    CommentType::Line => {
                        if key
                            .modifiers
                            // Shift+Enter is the most intuitive, but doesn't work in terminals with tmux.
                            // Alt+Enter is less intuitive but works in more terminals.
                            // We accept both.
                            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT)
                        {
                            self.textarea.insert_newline();
                            WidgetResult::Handled
                        } else {
                            let lines = self.textarea.lines();
                            let full_comment = lines.join("\n");
                            WidgetResult::Action(crate::state::actions::AppAction::ApplyComment {
                                address: self.address,
                                text: full_comment,
                                kind: crate::state::types::CommentKind::Line,
                            })
                        }
                    }
                    // Side comment: Enter submits.
                    CommentType::Side => {
                        let lines = self.textarea.lines();
                        let full_comment = lines.join(" ");
                        WidgetResult::Action(crate::state::actions::AppAction::ApplyComment {
                            address: self.address,
                            text: full_comment,
                            kind: crate::state::types::CommentKind::Side,
                        })
                    }
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
            // PageDown: jump to the last line of the textarea.
            // (The default ratatui-textarea handler uses the stored viewport height, which is
            // always 0 here because rendering uses a clone — causing it to snap to row 0.)
            KeyCode::PageDown => {
                self.textarea.move_cursor(CursorMove::Bottom);
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

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Mouse support is only available in multi-line (Line) comment mode.
        if self.comment_type != CommentType::Line {
            return WidgetResult::Ignored;
        }

        let is_down = mouse.kind == MouseEventKind::Down(MouseButton::Left);
        let is_drag = mouse.kind == MouseEventKind::Drag(MouseButton::Left);

        if !is_down && !is_drag {
            return WidgetResult::Ignored;
        }

        let ta_area = ui_state.comment_textarea_area;
        let col = mouse.column;
        let row = mouse.row;

        // Clamp drag coordinates to the textarea bounds so dragging outside
        // the widget still moves the cursor to the nearest edge.
        let clamped_col = col.clamp(ta_area.x, ta_area.x + ta_area.width.saturating_sub(1));
        let clamped_row = row.clamp(ta_area.y, ta_area.y + ta_area.height.saturating_sub(1));

        // For a plain click, only handle it when the cursor is inside the area.
        if is_down {
            let inside = col >= ta_area.x
                && col < ta_area.x + ta_area.width
                && row >= ta_area.y
                && row < ta_area.y + ta_area.height;
            if !inside {
                return WidgetResult::Ignored;
            }
        }

        let rel_row = clamped_row.saturating_sub(ta_area.y);
        let rel_col = clamped_col.saturating_sub(ta_area.x);

        if is_down {
            // Cancel any previous selection, position cursor, then begin a new selection.
            self.textarea.cancel_selection();
            self.textarea
                .move_cursor(CursorMove::Jump(rel_row, rel_col));
            self.textarea.start_selection();
        } else {
            // Drag: move cursor to extend the ongoing selection.
            self.textarea
                .move_cursor(CursorMove::Jump(rel_row, rel_col));
        }

        WidgetResult::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_position_at_end() {
        let comment = "Hello\nWorld";
        let dialog =
            CommentDialog::new(Some(comment), CommentType::Side, crate::state::Addr(0x1000));
        let cursor = dialog.textarea.cursor();
        // cursor is (row, col)
        // Rows are 0-indexed. Hello is row 0. World is row 1.
        // World has length 5. Cursor should be at (1, 5).
        assert_eq!(cursor, DataCursor(1, 5));
    }

    #[test]
    fn test_cursor_position_single_line() {
        let comment = "Hello";
        let dialog =
            CommentDialog::new(Some(comment), CommentType::Side, crate::state::Addr(0x1000));
        let cursor = dialog.textarea.cursor();
        assert_eq!(cursor, DataCursor(0, 5));
    }

    #[test]
    fn test_cursor_position_default_line_comment() {
        let dialog = CommentDialog::new(None, CommentType::Line, crate::state::Addr(0x1000));
        let cursor = dialog.textarea.cursor();
        // Dialog now starts empty; cursor should be at (0, 0).
        assert_eq!(cursor, DataCursor(0, 0));
    }

    #[test]
    fn test_insert_separator_dashes() {
        let mut dialog = CommentDialog::new(None, CommentType::Line, crate::state::Addr(0x1000));
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
        let mut dialog = CommentDialog::new(None, CommentType::Line, crate::state::Addr(0x1000));
        dialog.insert_separator(&"=".repeat(SEPARATOR_LEN));
        let lines = dialog.textarea.lines();
        let sep_line = &lines[lines.len() - 2];
        assert_eq!(sep_line.len(), SEPARATOR_LEN);
        assert!(sep_line.chars().all(|c| c == '='));
        assert_eq!(dialog.textarea.cursor().0, lines.len() - 1);
    }
}
