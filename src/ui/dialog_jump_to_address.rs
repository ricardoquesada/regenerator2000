use crate::state::AppState;
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct JumpToAddressDialog {
    pub input: String,
}

impl Default for JumpToAddressDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpToAddressDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}

impl Widget for JumpToAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Jump To Address (Hex) ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let dollar = Style::default().fg(theme.comment);
        let addr_style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        let addr_line = Line::from(vec![
            Span::styled("$", dollar),
            Span::styled(self.input.clone(), addr_style),
        ]);
        let input = Paragraph::new(addr_line).block(block);
        f.render_widget(input, area);

        // Show blinking cursor at end of input
        // Show blinking cursor at end of input (after "$" prefix)
        f.set_cursor_position((area.x + 1 + 1 + self.input.len() as u16, area.y + 1));
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
                if let Ok(target_addr) = u16::from_str_radix(&input, 16) {
                    // Navigate manually and close
                    crate::ui::menu::execute_menu_action(
                        _app_state,
                        ui_state,
                        crate::ui::menu::MenuAction::NavigateToAddress(target_addr),
                    );
                } else if !input.is_empty() {
                    ui_state.set_status_message("Invalid Hex Address");
                }
                WidgetResult::Close
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                if c.is_ascii_hexdigit() && self.input.len() < 4 {
                    self.input.push(c.to_ascii_uppercase());
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
