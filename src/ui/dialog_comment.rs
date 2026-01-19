use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::dialog::{Dialog, DialogResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentType {
    Side,
    Line,
}

pub struct CommentDialog {
    pub input: String,
    pub comment_type: CommentType,
}

impl CommentDialog {
    pub fn new(current_comment: Option<&str>, comment_type: CommentType) -> Self {
        Self {
            input: current_comment.unwrap_or("").to_string(),
            comment_type,
        }
    }
}

impl Dialog for CommentDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &UIState) {
        let theme = &ui_state.theme;
        let title = match self.comment_type {
            CommentType::Line => " Enter Line Comment ",
            CommentType::Side => " Enter Side Comment ",
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

        // Fixed height of 3 (Border + Input + Border)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(3),
                Constraint::Fill(1),
            ])
            .split(area);

        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(layout[1])[1];
        f.render_widget(ratatui::widgets::Clear, area);

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> DialogResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                DialogResult::Close
            }
            KeyCode::Enter => {
                if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                    let address = line.comment_address.unwrap_or(line.address);
                    let new_comment = self.input.trim().to_string();
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
                    DialogResult::Close
                } else {
                    DialogResult::KeepOpen
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                DialogResult::KeepOpen
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                DialogResult::KeepOpen
            }
            _ => DialogResult::KeepOpen,
        }
    }
}
