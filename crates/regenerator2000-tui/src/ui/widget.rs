use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::state::actions::AppAction;

#[derive(Debug, PartialEq)]
pub enum WidgetResult {
    Ignored,
    Handled,
    Close,
    Action(AppAction),
}

pub trait Widget {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState);
    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult;

    fn handle_mouse(
        &mut self,
        _mouse: crossterm::event::MouseEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        WidgetResult::Ignored
    }

    /// Called on every [`AppEvent::Tick`].  Dialogs that need to auto-close
    /// based on a timer should override this and return [`WidgetResult::Close`]
    /// when their timer expires.  The default implementation is a no-op.
    fn handle_tick(&mut self, _app_state: &mut AppState, _ui_state: &mut UIState) -> WidgetResult {
        WidgetResult::Handled
    }
}

#[must_use]
pub fn create_dialog_block<'a>(
    title: &'a str,
    theme: &crate::theme::Theme,
) -> ratatui::widgets::Block<'a> {
    use ratatui::style::Style;
    use ratatui::widgets::{Block, Borders};

    use ratatui::layout::Alignment;
    use ratatui::text::Line;

    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_top(Line::from("[x]").alignment(Alignment::Right))
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg))
}
