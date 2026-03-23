use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct MemoryDumpAddressDialog {
    pub input: String,
}

impl MemoryDumpAddressDialog {
    #[must_use]
    pub fn new(prefill: Option<u16>) -> Self {
        Self {
            input: prefill.map(|a| format!("{a:04X}")).unwrap_or_default(),
        }
    }
}

impl Widget for MemoryDumpAddressDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Memory Dump Address ", theme);

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 4, area);
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

        let para = Paragraph::new(vec![addr_line]).block(block);
        f.render_widget(para, area);

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
                if let Ok(addr) = u16::from_str_radix(&self.input, 16) {
                    WidgetResult::Action(
                        crate::state::actions::AppAction::ViceSetMemoryDumpAddress {
                            address: crate::state::Addr(addr),
                        },
                    )
                } else if self.input.is_empty() {
                    WidgetResult::Close
                } else {
                    ui_state.set_status_message("Invalid hex address");
                    WidgetResult::Handled
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) if c.is_ascii_hexdigit() && self.input.len() < 4 => {
                self.input.push(c.to_ascii_uppercase());
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
