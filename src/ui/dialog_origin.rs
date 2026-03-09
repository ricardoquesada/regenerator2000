use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct OriginDialog {
    pub input: String,
}

impl OriginDialog {
    #[must_use]
    pub fn new(current_origin: u16) -> Self {
        Self {
            input: format!("{current_origin:04X}"),
        }
    }
}

impl Widget for OriginDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Change Origin (Hex) ", theme);

        let area = crate::utils::centered_rect_adaptive(40, 40, 0, 3, area);
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
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                if let Ok(new_origin) = u16::from_str_radix(&self.input, 16) {
                    let size = app_state.raw_data.len();
                    // Check for overflow
                    if (new_origin as usize) + size <= 0x10000 {
                        let old_origin = app_state.origin;
                        let command = crate::commands::Command::ChangeOrigin {
                            new_origin,
                            old_origin,
                        };
                        command.apply(app_state);
                        app_state.push_command(command);

                        app_state.disassemble();
                        ui_state.set_status_message(format!("Origin changed to ${new_origin:04X}"));
                        WidgetResult::Close
                    } else {
                        ui_state.set_status_message("Error: Origin + Size exceeds $FFFF");
                        WidgetResult::Handled
                    }
                } else {
                    ui_state.set_status_message("Invalid Hex Address");
                    WidgetResult::Handled
                }
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
