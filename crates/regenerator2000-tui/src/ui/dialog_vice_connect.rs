use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
};

use ratatui_textarea::{CursorMove, TextArea};

pub struct ViceConnectDialog {
    pub textarea: TextArea<'static>,
}

impl Default for ViceConnectDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ViceConnectDialog {
    #[must_use]
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        textarea.insert_str("localhost:6502");
        textarea.move_cursor(CursorMove::End);
        Self { textarea }
    }
}

impl Widget for ViceConnectDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Connect to VICE ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let mut textarea = self.textarea.clone();
        textarea.set_block(block);
        textarea.set_style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
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
                let input = self.textarea.lines().join("").trim().to_string();
                if input.is_empty() {
                    WidgetResult::Close
                } else {
                    WidgetResult::Action(crate::state::actions::AppAction::ViceConnectAddress(
                        input,
                    ))
                }
            }
            KeyCode::Char(c) => {
                if c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-' {
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
