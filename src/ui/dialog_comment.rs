use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentType {
    Side,
    Line,
}

pub struct CommentDialogState {
    pub active: bool,
    pub input: String,
    pub comment_type: CommentType,
}

impl CommentDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            comment_type: CommentType::Side,
        }
    }

    pub fn open(&mut self, current_comment: Option<&str>, comment_type: CommentType) {
        self.active = true;
        self.input = current_comment.unwrap_or("").to_string();
        self.comment_type = comment_type;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_comment_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &CommentDialogState,
    theme: &crate::theme::Theme,
) {
    let title = match dialog.comment_type {
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

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Esc => {
            ui_state.comment_dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let address = line.comment_address.unwrap_or(line.address);
                let new_comment = ui_state.comment_dialog.input.trim().to_string();
                let new_comment_opt = if new_comment.is_empty() {
                    None
                } else {
                    Some(new_comment)
                };

                let command = match ui_state.comment_dialog.comment_type {
                    crate::ui::dialog_comment::CommentType::Side => {
                        let old_comment = app_state.user_side_comments.get(&address).cloned();
                        crate::commands::Command::SetUserSideComment {
                            address,
                            new_comment: new_comment_opt,
                            old_comment,
                        }
                    }
                    crate::ui::dialog_comment::CommentType::Line => {
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
                ui_state.comment_dialog.close();
            }
        }
        KeyCode::Backspace => {
            ui_state.comment_dialog.input.pop();
        }
        KeyCode::Char(c) => {
            ui_state.comment_dialog.input.push(c);
        }
        _ => {}
    }
}
