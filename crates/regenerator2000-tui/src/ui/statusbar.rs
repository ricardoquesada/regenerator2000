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
    #[must_use]
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
            .map_or(crate::state::Addr(0), |l| l.address);

        let block_info = if let Some(offset) =
            (cursor_addr.0 as isize).checked_sub(app_state.origin.0 as isize)
        {
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
            "{} | {}Cursor: ${:04X} | File: {:?}{}",
            app_state.settings.system,
            block_info,
            cursor_addr,
            app_state
                .file_path
                .as_ref()
                .map(|p| p.file_name().unwrap_or_default())
                .unwrap_or_default(),
            if let Some(start) = ui_state.selection_start {
                let count = (ui_state.cursor_index as isize - start as isize).abs() + 1;
                format!(" | Selected: {count}")
            } else {
                String::new()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn test_status_bar_render_status_and_vim_search_with_selection() {
        let mut app_state = AppState::new();
        app_state.load_binary(Addr(0x1000), vec![0xEA; 4]).unwrap();
        app_state.block_types = vec![BlockType::Code; 4];
        app_state.file_path = Some(PathBuf::from("/tmp/demo.prg"));
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.status_bar.set_message("Custom status");
        ui_state.cursor_index = 2;
        ui_state.selection_start = Some(0);

        let backend = TestBackend::new(180, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut bar = StatusBar;

        terminal
            .draw(|f| {
                bar.render(f, Rect::new(0, 0, 180, 1), &app_state, &mut ui_state);
            })
            .unwrap();

        let row_str: String = (0..180)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map_or("", |c| c.symbol())
            })
            .collect();
        assert!(row_str.contains("Custom status"));
        assert!(row_str.contains("Code: $1000-$1003"));
        assert!(row_str.contains("Cursor: $1002"));
        assert!(row_str.contains("demo.prg"));
        assert!(row_str.contains("Selected: 3"));

        // Active vim search overrides left message with "/query".
        ui_state.vim_search_active = true;
        ui_state.vim_search_input = "lda".to_string();
        terminal
            .draw(|f| {
                bar.render(f, Rect::new(0, 0, 180, 1), &app_state, &mut ui_state);
            })
            .unwrap();
        let search_row: String = (0..180)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map_or("", |c| c.symbol())
            })
            .collect();
        assert!(search_row.contains("/lda"));

        assert_eq!(
            bar.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Ignored
        );
    }
}
