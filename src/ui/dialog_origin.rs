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

pub struct OriginDialog {
    pub input: String,
}

impl OriginDialog {
    pub fn new(current_origin: u16) -> Self {
        Self {
            input: format!("{:04X}", current_origin),
        }
    }
}

impl Dialog for OriginDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &UIState) {
        let theme = &ui_state.theme;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Change Origin (Hex) ")
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
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
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
                if let Ok(new_origin) = u16::from_str_radix(&self.input, 16) {
                    let size = app_state.raw_data.len();
                    // Check for overflow
                    if (new_origin as usize) + size <= 0x10000 {
                        let old_origin = app_state.origin;
                        let command = crate::commands::Command::ChangeOrigin {
                            new_origin,
                            old_origin,
                        };
                        command.apply(app_state);
                        app_state.push_command(command);

                        app_state.disassemble();
                        ui_state
                            .set_status_message(format!("Origin changed to ${:04X}", new_origin));
                        DialogResult::Close
                    } else {
                        ui_state.set_status_message("Error: Origin + Size exceeds $FFFF");
                        DialogResult::KeepOpen
                    }
                } else {
                    ui_state.set_status_message("Invalid Hex Address");
                    DialogResult::KeepOpen
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                DialogResult::KeepOpen
            }
            KeyCode::Char(c) => {
                if c.is_ascii_hexdigit() && self.input.len() < 4 {
                    self.input.push(c.to_ascii_uppercase());
                }
                DialogResult::KeepOpen
            }
            _ => DialogResult::KeepOpen,
        }
    }
}
