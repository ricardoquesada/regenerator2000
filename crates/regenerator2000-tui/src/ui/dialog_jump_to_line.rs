use crate::state::AppState;
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
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
    #[must_use]
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

        let area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);

        if area.width > 2 && area.height > 1 {
            let cursor_x = (area.x + 1 + self.input.chars().count() as u16)
                .min(area.right().saturating_sub(2));
            f.set_cursor_position((cursor_x, area.y + 1));
        }
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
                        let target_addr = app_state.disassembly[target_idx]
                            .external_label_address
                            .unwrap_or(app_state.disassembly[target_idx].address);
                        crate::ui::navigable::jump_to_disassembly_at_line_idx(
                            ui_state,
                            Some(target_idx),
                            target_addr,
                        );
                        ui_state.set_status_message(format!("Jumped to line {line_num}"));
                    } else {
                        ui_state.set_status_message(format!("Line {line_num} out of range"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_jump_to_line_updates_cursor_and_history() {
        let mut app_state = AppState::new();
        app_state.origin = crate::state::Addr(0x1000);
        app_state.raw_data = vec![0xEA; 10];
        app_state.block_types = vec![crate::state::BlockType::Code; 10];
        app_state.disassemble();

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;

        let mut dialog = JumpToLineDialog::new();
        dialog.input = "3".to_string();

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Close);
        assert_eq!(ui_state.cursor_index, 2);
        assert_eq!(ui_state.scroll_index, 2);
        assert_eq!(ui_state.navigation_history.len(), 1);
        assert_eq!(ui_state.status_message.as_deref(), Some("Jumped to line 3"));
    }
}
