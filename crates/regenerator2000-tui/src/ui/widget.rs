//! Shared Ratatui widget trait and dialog frame helpers for TUI panes and modals.

use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::theme::Theme;
use crate::ui_state::UIState;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders};

/// Outcome returned by widget key and mouse event handlers.
#[derive(Debug, PartialEq)]
pub enum WidgetResult {
    /// Event was not consumed by this widget and should bubble to outer handlers.
    Ignored,
    /// Event was consumed internally without closing the dialog or dispatching an action.
    Handled,
    /// Modal dialog requested to close and promote any queued dialog.
    Close,
    /// Widget produced an [`AppAction`] to be executed by the application loop.
    Action(AppAction),
}

/// Interactive Ratatui pane or modal dialog component.
pub trait Widget {
    /// Renders the widget into `area`.
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState);

    /// Handles a keyboard event directed at this widget.
    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult;

    /// Handles a mouse event directed at this widget.
    fn handle_mouse(
        &mut self,
        _mouse: MouseEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        WidgetResult::Ignored
    }

    /// Called on every [`AppEvent::Tick`](crate::events::AppEvent::Tick).
    ///
    /// Dialogs that auto-close after a timer should override this and return
    /// [`WidgetResult::Close`] when their timer expires. The default implementation is a no-op.
    fn handle_tick(&mut self, _app_state: &mut AppState, _ui_state: &mut UIState) -> WidgetResult {
        WidgetResult::Handled
    }

    /// Refreshes cached or derived dialog data after [`regenerator2000_core::Core::apply_action`]
    /// mutates [`AppState`] while the dialog remains open.
    fn refresh(&mut self, _app_state: &AppState) {}
}

/// Builds a bordered modal dialog block with a right-aligned `[x]` close affordance.
#[must_use]
pub fn create_dialog_block<'title>(title: &'title str, theme: &Theme) -> Block<'title> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title_top(Line::from("[x]").alignment(Alignment::Right))
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_create_dialog_block_renders_title_and_close_button() {
        let theme = Theme::default();
        let block = create_dialog_block(" Test Dialog ", &theme);
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("creating test terminal should succeed");

        terminal
            .draw(|f| {
                f.render_widget(block, Rect::new(0, 0, 40, 5));
            })
            .expect("drawing dialog block should succeed");

        let top_row: String = (0..40)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map_or("", |c| c.symbol())
            })
            .collect();
        assert!(top_row.contains("Test Dialog"));
        assert!(top_row.contains("[x]"));
    }
}
