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
    pub is_local: bool,
    pub editing_scope: bool,
}

impl LabelDialog {
    #[must_use]
    pub fn new(current_label: Option<&str>, address: Addr, is_local: bool) -> Self {
        let mut textarea = TextArea::default();
        if let Some(label) = current_label {
            textarea.insert_str(label);
            textarea.move_cursor(CursorMove::End);
        }
        Self {
            textarea,
            address,
            is_local,
            editing_scope: false,
        }
    }
}

impl Widget for LabelDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Enter Label Name ", theme);

        // Height: 2 (border) + 3 (input with border) + 1 (scope) = 6
        let area = crate::utils::centered_rect_adaptive(50, 40, 0, 6, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        use ratatui::layout::{Constraint, Direction, Layout};
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Input area (height 3 for border)
                Constraint::Length(1), // Scope Selector
            ])
            .split(inner);

        let input_area = layout[0];
        let scope_area = layout[1];

        let mut textarea = self.textarea.clone();

        let input_border_style = if !self.editing_scope {
            Style::default().fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.dialog_border)
        };

        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(input_border_style),
        );

        let style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);

        if !self.editing_scope {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_area);

        // Render Scope Selector
        let check = if self.is_local {
            "[X] Local Label"
        } else {
            "[ ] Local Label"
        };
        let check_style = if self.editing_scope {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        };

        use ratatui::widgets::Paragraph;
        f.render_widget(Paragraph::new(check).style(check_style), scope_area);
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
                    is_local: self.is_local,
                })
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.editing_scope = !self.editing_scope;
                WidgetResult::Handled
            }
            _ if self.editing_scope => {
                if let KeyCode::Char(' ') = key.code {
                    self.is_local = !self.is_local;
                }
                WidgetResult::Handled
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
