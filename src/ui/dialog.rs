use crate::state::AppState;
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

#[derive(Debug, PartialEq)]
pub enum DialogResult {
    KeepOpen,
    Close,
}

pub trait Dialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &UIState);
    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> DialogResult;
}
