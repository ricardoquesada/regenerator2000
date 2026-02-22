use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

pub struct ViceConnectDialog {
    pub input: String,
}

impl Default for ViceConnectDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ViceConnectDialog {
    pub fn new() -> Self {
        Self {
            input: "localhost:6502".to_string(),
        }
    }
}

impl Widget for ViceConnectDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Connect to VICE ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
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
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                let input = self.input.clone();
                if !input.is_empty() {
                    WidgetResult::Action(crate::ui::menu::MenuAction::ViceConnectAddress(input))
                } else {
                    WidgetResult::Close
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                if c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-' {
                    self.input.push(c);
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
