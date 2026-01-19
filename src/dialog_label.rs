use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct LabelDialogState {
    pub active: bool,
    pub input: String,
    pub address: Option<u16>,
}

impl LabelDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            address: None,
        }
    }

    pub fn open(&mut self, current_label: Option<&str>, address: u16) {
        self.active = true;
        self.input = current_label.unwrap_or("").to_string();
        self.address = Some(address);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_label_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &LabelDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Enter Label Name ")
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
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    match key.code {
        KeyCode::Esc => {
            ui_state.label_dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            // Get address from dialog state
            if let Some(address) = ui_state.label_dialog.address {
                let label_name = ui_state.label_dialog.input.trim().to_string();

                if label_name.is_empty() {
                    // Remove label
                    let old_label = app_state.labels.get(&address).cloned();

                    let command = crate::commands::Command::SetLabel {
                        address,
                        new_label: None,
                        old_label,
                    };

                    command.apply(app_state);
                    app_state.push_command(command);

                    ui_state.set_status_message("Label removed");
                    app_state.disassemble();
                    ui_state.label_dialog.close();
                } else {
                    // Check for duplicates (exclude current address in case of rename/edit)
                    let exists = app_state.labels.iter().any(|(addr, label_vec)| {
                        *addr != address && label_vec.iter().any(|l| l.name == label_name)
                    });

                    if exists {
                        ui_state.set_status_message(format!(
                            "Error: Label '{}' already exists",
                            label_name
                        ));
                        // Do not close dialog, let user correct it
                    } else {
                        let old_label_vec = app_state.labels.get(&address).cloned();

                        let mut new_label_vec = old_label_vec.clone().unwrap_or_default();

                        let new_label_entry = crate::state::Label {
                            name: label_name,
                            kind: crate::state::LabelKind::User,
                            label_type: crate::state::LabelType::UserDefined,
                        };

                        // If vector has items, we assume we are editing the first one (as that's what we showed).
                        // If we want to SUPPORT multiple, we need a better UI.
                        // For now, replace 0 or push.
                        if !new_label_vec.is_empty() {
                            new_label_vec[0] = new_label_entry;
                        } else {
                            new_label_vec.push(new_label_entry);
                        }

                        let command = crate::commands::Command::SetLabel {
                            address,
                            new_label: Some(new_label_vec),
                            old_label: old_label_vec,
                        };

                        command.apply(app_state);
                        app_state.push_command(command);

                        ui_state.set_status_message("Label set");
                        app_state.disassemble();
                        ui_state.label_dialog.close();
                    }
                }
            }
        }
        KeyCode::Backspace => {
            ui_state.label_dialog.input.pop();
        }
        KeyCode::Char(c) => {
            ui_state.label_dialog.input.push(c);
        }
        _ => {}
    }
}
