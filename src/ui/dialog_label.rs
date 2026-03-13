use crate::state::{Addr, AppState};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct LabelDialog {
    pub input: String,
    pub address: Addr,
}

impl LabelDialog {
    #[must_use]
    pub fn new(current_label: Option<&str>, address: Addr) -> Self {
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

        let area = crate::utils::centered_rect_adaptive(50, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);

        // Show blinking cursor at end of input
        f.set_cursor_position((area.x + 1 + self.input.len() as u16, area.y + 1));
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
            KeyCode::Enter => WidgetResult::Action(crate::state::actions::AppAction::ApplyLabel {
                address: self.address,
                name: self.input.clone(),
            }),
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
