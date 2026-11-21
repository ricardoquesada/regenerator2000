//! Single-row status bar widget showing transient messages or search input and cursor context.

use crate::state::{Addr, AppState};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use regenerator2000_core::navigation::AlignedGridRange;

/// Persistent status bar message buffer rendered on the left side of [`StatusBar`].
#[derive(Debug, Clone)]
pub struct StatusBarState {
    /// Current status message shown when incremental search is inactive.
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
    /// Creates a [`StatusBarState`] initialized to `"Ready"`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the displayed status bar message.
    pub fn set_message<S: Into<String>>(&mut self, message: S) {
        self.message = message.into();
    }
}

/// Bottom status bar widget splitting status feedback on the left from active pane telemetry on the right.
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
            .map_or(Addr(0), |l| l.address);

        let block_info = if let Some(offset) =
            (cursor_addr.0 as isize).checked_sub(app_state.origin.0 as isize)
        {
            if offset >= 0 && (offset as usize) < app_state.block_types.len() {
                let block_type = app_state.block_types[offset as usize];
                if let Some(span) = app_state.block_range(cursor_addr) {
                    format!(
                        "{} | {}: ${:04X}-${:04X} | ",
                        app_state.settings.assembler, block_type, span.start, span.end
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

        let file_name = app_state
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // Read only the active pane's selection anchor so unsynced Disassembly selections do not
        // leak into companion status counts.
        let selection_count = match ui_state.active_pane {
            ActivePane::HexDump => ui_state.hex_selection_start.map(|start_row| {
                let stride = AlignedGridRange::hex_dump(app_state, ui_state.right_pane).stride;
                let max_col = stride.saturating_sub(1);
                let start_pos = start_row
                    .saturating_mul(stride)
                    .saturating_add(ui_state.hex_selection_start_col.min(max_col));
                let cursor_pos = ui_state
                    .hex_cursor_index
                    .saturating_mul(stride)
                    .saturating_add(ui_state.hex_col_cursor.min(max_col));
                cursor_pos.abs_diff(start_pos) + 1
            }),
            ActivePane::Sprites => ui_state
                .sprites_selection_start
                .map(|start| ui_state.sprites_cursor_index.abs_diff(start) + 1),
            ActivePane::Charset => ui_state
                .charset_selection_start
                .map(|start| ui_state.charset_cursor_index.abs_diff(start) + 1),
            ActivePane::Bitmap => ui_state
                .bitmap_selection_start
                .map(|start| ui_state.bitmap_cursor_index.abs_diff(start) + 1),
            _ => ui_state
                .selection_start
                .map(|start| ui_state.cursor_index.abs_diff(start) + 1),
        };

        let info = format!(
            "{} | {}Cursor: ${:04X} | File: {file_name}{}",
            app_state.settings.system,
            block_info,
            cursor_addr,
            if let Some(count) = selection_count {
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
    use crate::state::BlockType;
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn test_status_bar_render_status_and_vim_search_with_selection() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x1000), vec![0xEA; 4])
            .expect("loading test binary at $1000 should succeed");
        app_state.block_types = vec![BlockType::Code; 4];
        app_state.file_path = Some(PathBuf::from("/tmp/demo.prg"));
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.status_bar.set_message("Custom status");
        ui_state.cursor_index = 2;
        ui_state.selection_start = Some(0);

        let backend = TestBackend::new(180, 1);
        let mut terminal = Terminal::new(backend).expect("creating test terminal should succeed");
        let mut bar = StatusBar;

        terminal
            .draw(|f| {
                bar.render(f, Rect::new(0, 0, 180, 1), &app_state, &mut ui_state);
            })
            .expect("drawing StatusBar should succeed");

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
        assert!(row_str.contains("File: demo.prg"));
        assert!(!row_str.contains("File: \"demo.prg\""));
        assert!(row_str.contains("Selected: 3"));

        // Right-pane HexDump selection count is reflected when HexDump is active.
        ui_state.active_pane = ActivePane::HexDump;
        ui_state.selection_start = None;
        ui_state.hex_selection_start = Some(0);
        ui_state.hex_selection_start_col = 1;
        ui_state.hex_cursor_index = 0;
        ui_state.hex_col_cursor = 4;
        terminal
            .draw(|f| {
                bar.render(f, Rect::new(0, 0, 180, 1), &app_state, &mut ui_state);
            })
            .expect("drawing StatusBar in HexDump pane should succeed");
        let hex_row: String = (0..180)
            .map(|x| {
                terminal
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map_or("", |c| c.symbol())
            })
            .collect();
        assert!(hex_row.contains("Selected: 4"));

        // Active vim search overrides left message with "/query".
        ui_state.vim_search_active = true;
        ui_state.vim_search_input = "lda".to_string();
        terminal
            .draw(|f| {
                bar.render(f, Rect::new(0, 0, 180, 1), &app_state, &mut ui_state);
            })
            .expect("drawing StatusBar with active vim search should succeed");
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

    #[test]
    fn test_status_bar_companion_selection_and_sync_false_isolation() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x0800), vec![0xEA; 0x4200])
            .expect("load 16.5KB binary at $0800");
        app_state.block_types = vec![BlockType::Code; 0x4200];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let mut terminal = Terminal::new(TestBackend::new(180, 1)).expect("create 180x1 terminal");
        let bar = StatusBar;

        let render_row =
            |terminal: &mut Terminal<TestBackend>, app: &AppState, ui: &mut UIState| -> String {
                terminal
                    .draw(|f| bar.render(f, Rect::new(0, 0, 180, 1), app, ui))
                    .expect("draw status bar row");
                (0..180)
                    .map(|x| {
                        terminal
                            .backend()
                            .buffer()
                            .cell((x, 0))
                            .map_or("", |cell| cell.symbol())
                    })
                    .collect()
            };

        // 1. Tier 1 passive focus: no "Selected:" badge on Sprites, Charset, or Bitmap.
        for pane in [ActivePane::Sprites, ActivePane::Charset, ActivePane::Bitmap] {
            ui_state.active_pane = pane;
            let row = render_row(&mut terminal, &app_state, &mut ui_state);
            assert!(
                !row.contains("Selected:"),
                "Tier 1 passive focus in {pane:?} must not show Selected:, got: '{row}'"
            );
        }

        // 2. Tier 2 active selection: Selected: N per pane.
        ui_state.active_pane = ActivePane::Sprites;
        ui_state.sprites_selection_start = Some(2);
        ui_state.sprites_cursor_index = 1;
        assert!(render_row(&mut terminal, &app_state, &mut ui_state).contains("Selected: 2"));

        ui_state.active_pane = ActivePane::Charset;
        ui_state.charset_selection_start = Some(3);
        ui_state.charset_cursor_index = 3;
        assert!(render_row(&mut terminal, &app_state, &mut ui_state).contains("Selected: 1"));

        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.bitmap_selection_start = Some(0);
        ui_state.bitmap_cursor_index = 1;
        assert!(render_row(&mut terminal, &app_state, &mut ui_state).contains("Selected: 2"));

        // 3. sync_*_view == false isolation: Disassembly selection must NOT leak into companion panes.
        ui_state.selection_start = Some(0);
        ui_state.cursor_index = 4;
        ui_state.sprites_selection_start = None;
        ui_state.charset_selection_start = None;
        ui_state.bitmap_selection_start = None;
        ui_state.hex_selection_start = None;

        for pane in [
            ActivePane::Sprites,
            ActivePane::Charset,
            ActivePane::Bitmap,
            ActivePane::HexDump,
        ] {
            ui_state.active_pane = pane;
            let row = render_row(&mut terminal, &app_state, &mut ui_state);
            assert!(
                !row.contains("Selected:"),
                "Pane {pane:?} must not leak Disassembly selection count, got: '{row}'"
            );
        }

        ui_state.active_pane = ActivePane::Disassembly;
        assert!(render_row(&mut terminal, &app_state, &mut ui_state).contains("Selected: 5"));
    }
}
