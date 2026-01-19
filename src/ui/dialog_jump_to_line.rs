use crate::state::AppState;
// Theme import removed
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::dialog::{Dialog, DialogResult};

pub struct JumpToLineDialog {
    pub input: String,
}

impl JumpToLineDialog {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}

impl Dialog for JumpToLineDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &UIState) {
        let theme = &ui_state.theme;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Jump To Line ")
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

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
                Constraint::Percentage(35),
                Constraint::Percentage(30),
                Constraint::Percentage(35),
            ])
            .split(layout[1])[1];
        f.render_widget(ratatui::widgets::Clear, area);

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(Color::Yellow)
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
                let input = self.input.clone();
                if let Ok(line_num) = input.parse::<usize>() {
                    if line_num > 0 && line_num <= app_state.disassembly.len() {
                        ui_state
                            .navigation_history
                            .push((ActivePane::Disassembly, ui_state.cursor_index));
                        ui_state.cursor_index = line_num - 1;
                        ui_state.set_status_message(format!("Jumped to line {}", line_num));
                        DialogResult::Close
                    } else {
                        ui_state.set_status_message("Line number out of range");
                        DialogResult::KeepOpen
                    }
                } else {
                    ui_state.set_status_message("Invalid Line Number");
                    DialogResult::KeepOpen
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                DialogResult::KeepOpen
            }
            KeyCode::Char(c) => {
                if c.is_ascii_digit() && self.input.len() < 10 {
                    self.input.push(c);
                }
                DialogResult::KeepOpen
            }
            _ => DialogResult::KeepOpen,
        }
    }
}
