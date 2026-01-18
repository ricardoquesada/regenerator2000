use crate::state::AppState;
use crate::theme::Theme;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct JumpToLineDialog {
    pub active: bool,
    pub input: String,
}

impl JumpToLineDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render(f: &mut Frame, area: Rect, dialog: &JumpToLineDialog, theme: &Theme) {
    if !dialog.active {
        return;
    }

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

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    let dialog = &mut ui_state.jump_to_line_dialog;
    match key.code {
        KeyCode::Esc => {
            dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            let input = dialog.input.clone();
            if let Ok(line_num) = input.parse::<usize>() {
                if line_num > 0 && line_num <= app_state.disassembly.len() {
                    ui_state
                        .navigation_history
                        .push((ActivePane::Disassembly, ui_state.cursor_index));
                    ui_state.cursor_index = line_num - 1;
                    ui_state.set_status_message(format!("Jumped to line {}", line_num));
                    ui_state.jump_to_line_dialog.close();
                } else {
                    ui_state.set_status_message("Line number out of range");
                }
            } else {
                ui_state.set_status_message("Invalid Line Number");
            }
        }
        KeyCode::Backspace => {
            dialog.input.pop();
        }
        KeyCode::Char(c) => {
            if c.is_ascii_digit() && dialog.input.len() < 10 {
                dialog.input.push(c);
            }
        }
        _ => {}
    }
}
