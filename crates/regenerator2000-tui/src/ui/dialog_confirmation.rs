use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::ui_state::UIState;
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
    pub action: AppAction,
}

impl ConfirmationDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>, action: AppAction) -> Self {
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

        let area = crate::utils::centered_rect_adaptive(50, 40, 40, 5, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Top padding (flexible)
                Constraint::Length(1), // Message
                Constraint::Length(1), // Gap
                Constraint::Length(1), // Instructions
                Constraint::Min(0),    // Bottom padding (flexible)
            ])
            .split(inner);

        let message = Paragraph::new(self.message.clone())
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.dialog_fg)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(message, layout[1]);

        let instructions = Paragraph::new("Enter: Proceed  |  Esc: Cancel")
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(theme.highlight_fg));

        f.render_widget(instructions, layout[3]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Action cancelled");
                WidgetResult::Close
            }
            KeyCode::Char('n' | 'N')
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT,
                ) =>
            {
                ui_state.set_status_message("Action cancelled");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                WidgetResult::Action(AppAction::Confirmed(Box::new(self.action.clone())))
            }
            KeyCode::Char('y' | 'Y')
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT,
                ) =>
            {
                WidgetResult::Action(AppAction::Confirmed(Box::new(self.action.clone())))
            }
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_confirmation_dialog_render_confirm_and_cancel() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog =
            ConfirmationDialog::new("Confirm Exit", "Discard unsaved changes?", AppAction::Exit);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains("Discard unsaved changes?"));

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Action(AppAction::Confirmed(Box::new(AppAction::Exit)))
        );

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Action(AppAction::Confirmed(Box::new(AppAction::Exit)))
        );

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
        assert_eq!(ui_state.status_message.as_deref(), Some("Action cancelled"));
    }
}
