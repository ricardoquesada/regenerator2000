use crate::ui_state::MenuAction;
use crate::utils::centered_rect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct ConfirmationDialogState {
    pub active: bool,
    pub title: String,
    pub message: String,
    pub action_on_confirm: Option<MenuAction>,
}

impl ConfirmationDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            title: String::new(),
            message: String::new(),
            action_on_confirm: None,
        }
    }

    pub fn open(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        action: MenuAction,
    ) {
        self.active = true;
        self.title = title.into();
        self.message = message.into();
        self.action_on_confirm = Some(action);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.action_on_confirm = None;
    }
}

pub fn render_confirmation_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &ConfirmationDialogState,
    theme: &crate::theme::Theme,
) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", dialog.title))
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

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

    let message = Paragraph::new(dialog.message.clone())
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
