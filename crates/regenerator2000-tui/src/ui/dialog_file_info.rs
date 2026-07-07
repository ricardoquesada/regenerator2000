use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

pub struct FileInfoDialog;

impl Default for FileInfoDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl FileInfoDialog {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Widget for FileInfoDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = " File Info ";

        let block = crate::ui::widget::create_dialog_block(title, theme);

        let area = crate::utils::centered_rect_adaptive(56, 12, 20, 10, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),    // Info lines
                Constraint::Length(1), // Gap
                Constraint::Length(1), // Footer
            ])
            .split(inner);

        let info = app_state.file_info();

        let label_style = Style::default().fg(theme.dialog_fg);
        let val_style = Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD);

        // 1. Start / End Address & Size
        let range_str = format!(
            "${:04X} - ${:04X}  ({} bytes)",
            info.start_addr.0, info.end_addr.0, info.size
        );
        let line_range = Line::from(vec![
            Span::styled("Start / End Address: ", label_style),
            Span::styled(range_str, val_style),
        ]);

        // 2. Entry Point Address
        let entry_str = match info.entry_point {
            Some(ep) => format!("${:04X}", ep.0),
            None => "N/A".to_string(),
        };
        let line_entry = Line::from(vec![
            Span::styled("Entry Point Address: ", label_style),
            Span::styled(entry_str, val_style),
        ]);

        // 3. Entropy
        let entropy_note = if info.entropy > app_state.system_config.entropy_threshold {
            " (High / Likely Compressed)"
        } else {
            " (Normal)"
        };
        let entropy_str = format!("{:.2} / 8.00{}", info.entropy, entropy_note);
        let entropy_style = if info.entropy > app_state.system_config.entropy_threshold {
            Style::default()
                .fg(theme.error_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            val_style
        };
        let line_entropy = Line::from(vec![
            Span::styled("Entropy:             ", label_style),
            Span::styled(entropy_str, entropy_style),
        ]);

        // 4. Packer
        let (packer_str, packer_style) = if let Some(name) = info.packer_name {
            (
                name.to_string(),
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD),
            )
        } else if info.entropy > app_state.system_config.entropy_threshold {
            (
                "unknown".to_string(),
                Style::default()
                    .fg(theme.error_fg)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            ("none".to_string(), val_style)
        };
        let line_packed = Line::from(vec![
            Span::styled("Packer:              ", label_style),
            Span::styled(packer_str, packer_style),
        ]);

        let text = vec![
            Line::from(""),
            line_range,
            line_entry,
            line_entropy,
            line_packed,
        ];

        let paragraph = Paragraph::new(text).alignment(Alignment::Left);
        f.render_widget(paragraph, layout[0]);

        let footer = Paragraph::new("Press Enter or Esc to continue")
            .alignment(Alignment::Center)
            .style(val_style);

        f.render_widget(footer, layout[2]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Enter
            | KeyCode::Esc
            | KeyCode::Char(' ')
            | KeyCode::Char('q')
            | KeyCode::Char('Q') => WidgetResult::Close,
            _ => WidgetResult::Handled,
        }
    }
}
