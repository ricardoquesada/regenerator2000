use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::ui_state::MenuAction;

#[derive(Debug, PartialEq)]
pub enum WidgetResult {
    Ignored,
    Handled,
    Close,
    Action(MenuAction),
}

pub trait Widget {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState);
    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult;
}
