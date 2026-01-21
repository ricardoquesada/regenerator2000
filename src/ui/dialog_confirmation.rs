use crate::state::AppState;
use crate::ui::menu::{MenuAction, execute_menu_action};
use crate::ui_state::UIState;
use crate::utils::centered_rect;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct ConfirmationDialog {
    pub title: String,
    pub message: String,
    pub action: MenuAction,
}

impl ConfirmationDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>, action: MenuAction) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            action,
        }
    }
}

impl Widget for ConfirmationDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = format!(" {} ", self.title);
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = centered_rect(50, 7, area);
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Message
                Constraint::Length(1), // Gap
                Constraint::Length(1), // Instructions
            ])
            .split(inner);

        let message = Paragraph::new(self.message.clone())
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.dialog_fg)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(message, layout[0]);

        let instructions = Paragraph::new("Enter: Proceed  |  Esc: Cancel")
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(theme.highlight_fg));

        f.render_widget(instructions, layout[2]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => {
                ui_state.set_status_message("Action cancelled");
                WidgetResult::Close
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                // We need to clone the action because we can't move out of self in handle_input
                // But MenuAction should be Clone (it's an enum of simple types/strings?)
                // Let's assume MenuAction is Clone. If not I need to make it Clone.
                // Or I can use Option<MenuAction> in struct and take() it.
                // But handle_input takes &mut self.
                // I will use Option in struct for safety.
                execute_menu_action(app_state, ui_state, self.action.clone());
                WidgetResult::Close
            }
            _ => WidgetResult::Handled,
        }
    }
}
