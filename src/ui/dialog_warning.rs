use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::utils::centered_rect;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

pub struct WarningDialog {
    title: String,
    message: String,
}

impl WarningDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn show_if_needed(entropy: Option<f32>, ui_state: &mut UIState) {
        if let Some(entropy_val) = entropy {
            ui_state.active_dialog = Some(Box::new(Self::new(
                "High Entropy Detected",
                format!(
                    "The loaded file has high entropy ({:.2}).\nIt is likely compressed.\n\nYou might want to uncompress it with tools like Unp64, and reload the uncompressed file.",
                    entropy_val
                ),
            )));
        }
    }
}

impl Widget for WarningDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = format!(" {} ", self.title);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg));

        let area = centered_rect(50, 20, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Message
                Constraint::Length(1), // Gap
                Constraint::Length(1), // Footer
            ])
            .split(inner);

        let message = Paragraph::new(self.message.clone())
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .style(Style::default().fg(theme.dialog_fg));

        f.render_widget(message, layout[0]);

        let footer = Paragraph::new("Press Enter or Esc to continue")
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(footer, layout[2]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            _ => WidgetResult::Handled,
        }
    }
}
