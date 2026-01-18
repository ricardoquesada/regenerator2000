use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct LabelDialogState {
    pub active: bool,
    pub input: String,
    pub address: Option<u16>,
}

impl LabelDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            address: None,
        }
    }

    pub fn open(&mut self, current_label: Option<&str>, address: u16) {
        self.active = true;
        self.input = current_label.unwrap_or("").to_string();
        self.address = Some(address);
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_label_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &LabelDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Enter Label Name ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}
