use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Paragraph,
};

#[derive(Debug, Clone)]
pub struct StatusBarState {
    pub message: String,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            message: "Ready".to_string(),
        }
    }
}

impl StatusBarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_message<S: Into<String>>(&mut self, message: S) {
        self.message = message.into();
    }
}

pub struct StatusBar;

impl Widget for StatusBar {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Message
                Constraint::Percentage(50), // Info
            ])
            .split(area);

        // Left: Status Message
        let (status_text, status_fg) = if ui_state.vim_search_active {
            (
                format!("/{}", ui_state.vim_search_input),
                ui_state.theme.highlight_fg,
            )
        } else {
            (
                format!(" {}", ui_state.status_bar.message),
                ui_state.theme.status_bar_fg,
            )
        };

        let status_msg = Paragraph::new(Span::styled(
            status_text,
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .style(
            Style::default()
                .bg(ui_state.theme.status_bar_bg)
                .fg(status_fg),
        );
        f.render_widget(status_msg, chunks[0]);

        // Right: Info
        let cursor_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map(|l| l.address)
            .unwrap_or(0);

        let block_info =
            if let Some(offset) = (cursor_addr as isize).checked_sub(app_state.origin as isize) {
                if offset >= 0 && (offset as usize) < app_state.block_types.len() {
                    let block_type = app_state.block_types[offset as usize];
                    if let Some((start, end)) = app_state.get_block_range(cursor_addr) {
                        format!(
                            "{} | {}: ${:04X}-${:04X} | ",
                            app_state.settings.assembler, block_type, start, end
                        )
                    } else {
                        format!("{} | {}: ??? | ", app_state.settings.assembler, block_type)
                    }
                } else {
                    format!("{} | ", app_state.settings.assembler)
                }
            } else {
                format!("{} | ", app_state.settings.assembler)
            };

        let info = format!(
            "{} | {}Cursor: {:04X} | Origin: {:04X} | File: {:?}{}",
            app_state.settings.platform,
            block_info,
            cursor_addr,
            app_state.origin,
            app_state
                .file_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default())
                .unwrap_or_default(),
            if let Some(start) = ui_state.selection_start {
                let count = (ui_state.cursor_index as isize - start as isize).abs() + 1;
                format!(" | Selected: {}", count)
            } else {
                "".to_string()
            }
        );

        let info_widget = Paragraph::new(info).alignment(Alignment::Right).style(
            Style::default()
                .bg(ui_state.theme.status_bar_bg)
                .fg(ui_state.theme.status_bar_fg),
        );
        f.render_widget(info_widget, chunks[1]);
    }

    fn handle_input(
        &mut self,
        _key: KeyEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        WidgetResult::Ignored
    }
}
