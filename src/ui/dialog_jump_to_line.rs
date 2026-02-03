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

pub struct JumpToLineDialog {
    pub input: String,
}

impl Default for JumpToLineDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl JumpToLineDialog {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}

impl Widget for JumpToLineDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Jump To Line ", theme);

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
                let input = self.input.clone();
                if let Ok(line_num) = input.parse::<usize>() {
                    if let Some(target_idx) =
                        crate::ui::view_disassembly::DisassemblyView::get_index_for_visual_line(
                            app_state, line_num,
                        )
                    {
                        ui_state.navigation_history.push((
                            crate::ui_state::ActivePane::Disassembly,
                            crate::ui_state::NavigationTarget::Index(ui_state.cursor_index),
                        ));
                        ui_state.cursor_index = target_idx;
                        ui_state.set_status_message(format!("Jumped to visual line {}", line_num));
                    } else {
                        ui_state.set_status_message("Line number out of range");
                    }
                } else if !input.is_empty() {
                    ui_state.set_status_message("Invalid Line Number");
                }
                WidgetResult::Close
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                if c.is_ascii_digit() && self.input.len() < 10 {
                    self.input.push(c);
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
