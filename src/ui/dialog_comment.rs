use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
};
use tui_textarea::TextArea;

use crate::ui::widget::{Widget, WidgetResult};

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
            let t = TextArea::from(comment.lines());
            // For existing comments, we assume user wants to edit them as is.
            // If it was single line, lines() works.
            // If empty string, lines() is empty, TextArea becomes empty.
            if comment.is_empty() {
                // Fallback to default logic if actually empty string passed (rare)
                Self::create_default_textarea(&comment_type)
            } else {
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
                TextArea::from(vec![default_text.to_string()])
            }
            CommentType::Side => TextArea::default(),
        }
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
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(10),
                Constraint::Fill(1),
            ])
            .split(area);

        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(15),
                Constraint::Percentage(70),
                Constraint::Percentage(15),
            ])
            .split(layout[1])[1];
        f.render_widget(ratatui::widgets::Clear, area);

        let mut textarea = self.textarea.clone();
        textarea.set_block(block);

        let style = Style::default().fg(theme.highlight_fg);
        textarea.set_style(style);

        // Also set cursor style if needed, but default is usually inverse of style
        textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, area);
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
                    let address = line.comment_address.unwrap_or(line.address);

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
