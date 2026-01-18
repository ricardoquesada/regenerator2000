use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentType {
    Side,
    Line,
}

pub struct CommentDialogState {
    pub active: bool,
    pub input: String,
    pub comment_type: CommentType,
}

impl CommentDialogState {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
            comment_type: CommentType::Side,
        }
    }

    pub fn open(&mut self, current_comment: Option<&str>, comment_type: CommentType) {
        self.active = true;
        self.input = current_comment.unwrap_or("").to_string();
        self.comment_type = comment_type;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render_comment_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &CommentDialogState,
    theme: &crate::theme::Theme,
) {
    let title = match dialog.comment_type {
        CommentType::Line => " Enter Line Comment ",
        CommentType::Side => " Enter Side Comment ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
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
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}
