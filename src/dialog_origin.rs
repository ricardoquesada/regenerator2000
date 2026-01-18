use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct OriginDialogState {
    pub active: bool,
    pub input: String,
    pub address: u16,
}

impl OriginDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            address: 0,
        }
    }

    pub fn open(&mut self, current_origin: u16) {
        self.active = true;
        self.input = format!("{:04X}", current_origin);
        self.address = current_origin;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_origin_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &OriginDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Change Origin (Hex) ")
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
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}
