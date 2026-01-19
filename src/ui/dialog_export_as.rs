use crate::state::AppState;
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct ExportAsDialog {
    pub input: String,
}

impl Default for ExportAsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ExportAsDialog {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}

impl Widget for ExportAsDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Export Project As... ")
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
                let filename = self.input.clone();
                if !filename.is_empty() {
                    let mut path = ui_state.file_dialog_current_dir.join(filename);
                    if path.extension().is_none() {
                        path.set_extension("asm");
                    }
                    app_state.export_path = Some(path.clone());
                    if let Err(e) = crate::exporter::export_asm(app_state, &path) {
                        ui_state.set_status_message(format!("Error exporting: {}", e));
                        WidgetResult::Handled
                    } else {
                        ui_state.set_status_message("Project Exported");
                        WidgetResult::Close
                    }
                } else {
                    WidgetResult::Handled
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
