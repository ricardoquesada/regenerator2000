use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct OriginDialogState {
    pub active: bool,
    pub input: String,
    pub address: u16,
}

impl OriginDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            address: 0,
        }
    }

    pub fn open(&mut self, current_origin: u16) {
        self.active = true;
        self.input = format!("{:04X}", current_origin);
        self.address = current_origin;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_origin_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &OriginDialogState,
    theme: &crate::theme::Theme,
) {
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
            ui_state.origin_dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            if let Ok(new_origin) = u16::from_str_radix(&ui_state.origin_dialog.input, 16) {
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
                    ui_state.set_status_message(format!("Origin changed to ${:04X}", new_origin));
                    ui_state.origin_dialog.close();
                } else {
                    ui_state.set_status_message("Error: Origin + Size exceeds $FFFF");
                }
            } else {
                ui_state.set_status_message("Invalid Hex Address");
            }
        }
        KeyCode::Backspace => {
            ui_state.origin_dialog.input.pop();
        }
        KeyCode::Char(c) => {
            if c.is_ascii_hexdigit() && ui_state.origin_dialog.input.len() < 4 {
                ui_state.origin_dialog.input.push(c.to_ascii_uppercase());
            }
        }
        _ => {}
    }
}
