use crate::state::{Addr, AppState};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};
use ratatui_textarea::{CursorMove, TextArea};

use crate::ui::widget::{Widget, WidgetResult};

pub struct LabelDialog {
    pub textarea: TextArea<'static>,
    pub address: Addr,
}

impl LabelDialog {
    #[must_use]
    pub fn new(current_label: Option<&str>, address: Addr) -> Self {
        let mut textarea = TextArea::default();
        if let Some(label) = current_label {
            textarea.insert_str(label);
            textarea.move_cursor(CursorMove::End);
        }
        Self { textarea, address }
    }
}

impl Widget for LabelDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Enter Label Name ", theme);

        let area = crate::utils::centered_rect_adaptive(50, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let mut textarea = self.textarea.clone();
        textarea.set_block(block);

        let style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);
        textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, area);
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
                let lines = self.textarea.lines();
                let label_name = lines.join("").trim().to_string();
                WidgetResult::Action(crate::state::actions::AppAction::ApplyLabel {
                    address: self.address,
                    name: label_name,
                })
            }
            KeyCode::Char(c) => {
                if c.is_ascii_alphanumeric() || c == '_' {
                    self.textarea.input(key);
                }
                WidgetResult::Handled
            }
            _ => {
                self.textarea.input(key);
                WidgetResult::Handled
            }
        }
    }
}
