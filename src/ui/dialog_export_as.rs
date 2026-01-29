use crate::state::AppState;
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct ExportAsDialog {
    pub input: String,
}

impl Default for ExportAsDialog {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ExportAsDialog {
    pub fn new(initial_filename: Option<String>) -> Self {
        Self {
            input: initial_filename.unwrap_or_default(),
        }
    }
}

impl Widget for ExportAsDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Export Project As... ", theme);

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
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        f.render_widget(block.clone(), area);
        let inner = block.inner(area);

        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(5), // ".asm" + 1 padding
            ])
            .split(inner);

        let input = Paragraph::new(self.input.clone())
            .block(
                ratatui::widgets::Block::default()
                    .style(Style::default().bg(theme.menu_selected_bg)),
            )
            .style(
                Style::default()
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(input, input_layout[0]);

        let extension = Paragraph::new(".asm").style(Style::default().fg(Color::Gray));
        f.render_widget(extension, input_layout[1]);
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
                    let mut path = ui_state.file_dialog_current_dir.join(&filename);
                    if path.extension().is_none() {
                        path.set_extension("asm");
                    }
                    app_state.export_path = Some(path.clone());
                    if let Err(e) = crate::exporter::export_asm(app_state, &path) {
                        ui_state.set_status_message(format!("Error exporting: {}", e));
                        WidgetResult::Handled
                    } else {
                        app_state.last_export_asm_filename = Some(filename.clone());
                        let saved_filename = path.file_name().unwrap_or_default().to_string_lossy();
                        ui_state.set_status_message(format!("Exported: {}", saved_filename));
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
