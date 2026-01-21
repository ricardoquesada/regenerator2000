use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct LabelDialog {
    pub input: String,
    pub address: u16,
}

impl LabelDialog {
    pub fn new(current_label: Option<&str>, address: u16) -> Self {
        Self {
            input: current_label.unwrap_or("").to_string(),
            address,
        }
    }
}

impl Widget for LabelDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Enter Label Name ", theme);

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
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                let address = self.address;
                let label_name = self.input.trim().to_string();

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
                    WidgetResult::Close
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
                        WidgetResult::Handled
                    } else {
                        let old_label_vec = app_state.labels.get(&address).cloned();

                        let mut new_label_vec = old_label_vec.clone().unwrap_or_default();

                        let new_label_entry = crate::state::Label {
                            name: label_name,
                            kind: crate::state::LabelKind::User,
                            label_type: crate::state::LabelType::UserDefined,
                        };

                        // If vector has items, we assume we are editing the first one (as that's what we showed).
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
                        WidgetResult::Close
                    }
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
