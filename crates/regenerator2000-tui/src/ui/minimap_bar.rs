use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect, style::Style, text::Span, widgets::Paragraph};

pub struct MinimapBar;

impl Widget for MinimapBar {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        if app_state.block_types.is_empty() || area.width == 0 {
            return;
        }

        let total_bytes = app_state.block_types.len();
        let width = area.width as usize;
        let render_width = width.min(total_bytes);
        if render_width == 0 {
            return;
        }

        let cursor_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map_or(app_state.origin.0, |l| l.address.0);
        let cursor_offset = cursor_addr.saturating_sub(app_state.origin.0) as usize;
        let clamped_cursor_offset = cursor_offset.min(total_bytes.saturating_sub(1));
        let cursor_x = (clamped_cursor_offset * render_width) / total_bytes;

        let mut spans = Vec::with_capacity(render_width);

        for x in 0..render_width {
            let start = (x * total_bytes) / render_width;
            let end = (((x + 1) * total_bytes) / render_width)
                .max(start + 1)
                .min(total_bytes);

            // Pick the first non-undefined block type in the chunk, or the start byte's type.
            let block_type = app_state.block_types[start..end]
                .iter()
                .find(|&&b| b != crate::state::BlockType::Undefined)
                .copied()
                .unwrap_or(app_state.block_types[start]);

            // Map block type to color foreground (filled block character █).
            let fg_color = match block_type {
                crate::state::BlockType::Code => ui_state.theme.block_code_fg,
                crate::state::BlockType::DataByte => ui_state.theme.block_data_byte_fg,
                crate::state::BlockType::DataWord => ui_state.theme.block_data_word_fg,
                crate::state::BlockType::Address => ui_state.theme.block_address_fg,
                crate::state::BlockType::PetsciiText => ui_state.theme.block_petscii_text_fg,
                crate::state::BlockType::ScreencodeText => ui_state.theme.block_screencode_text_fg,
                crate::state::BlockType::LoHiAddress => ui_state.theme.block_lohi_fg,
                crate::state::BlockType::HiLoAddress => ui_state.theme.block_hilo_fg,
                crate::state::BlockType::LoHiWord => ui_state.theme.block_lohi_fg,
                crate::state::BlockType::HiLoWord => ui_state.theme.block_hilo_fg,
                crate::state::BlockType::ExternalFile => ui_state.theme.block_external_file_fg,
                crate::state::BlockType::Undefined => ui_state.theme.block_undefined_fg,
            };

            let is_cursor = x == cursor_x && cursor_offset < total_bytes;

            let span = if is_cursor {
                let chunk_len = (end - start).max(1);
                let local_offset = clamped_cursor_offset.saturating_sub(start);
                let segment = (local_offset * 3) / chunk_len;
                let tick_char = match segment {
                    0 => "▏",
                    1 => "│",
                    2 => "▕",
                    _ => "│",
                };

                Span::styled(
                    tick_char,
                    Style::default()
                        .fg(ui_state.theme.minimap_cursor_fg)
                        .bg(fg_color),
                )
            } else {
                Span::styled(
                    "█", // Filled block
                    Style::default().fg(fg_color),
                )
            };

            spans.push(span);
        }

        let paragraph = Paragraph::new(ratatui::text::Line::from(spans));
        f.render_widget(paragraph, area);
    }

    fn handle_input(
        &mut self,
        _key: KeyEvent,
        _app_state: &mut AppState,
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        WidgetResult::Ignored
    }

    fn handle_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if mouse.kind != crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        {
            return WidgetResult::Ignored;
        }

        if app_state.block_types.is_empty() || ui_state.minimap_area.width == 0 {
            return WidgetResult::Ignored;
        }

        let total_bytes = app_state.block_types.len();
        let width = ui_state.minimap_area.width as usize;
        let render_width = width.min(total_bytes);
        if render_width == 0 {
            return WidgetResult::Ignored;
        }

        let rel_x = (mouse.column.saturating_sub(ui_state.minimap_area.x)) as usize;
        if rel_x >= render_width {
            return WidgetResult::Ignored;
        }

        let byte_offset = (rel_x * total_bytes) / render_width;
        let addr = (app_state.origin.0 as usize + byte_offset).min(0xFFFF);

        crate::ui::navigable::jump_to_disassembly_at_address(
            app_state,
            ui_state,
            crate::state::Addr(addr as u16),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_minimap_239_bytes_in_120_columns_covers_all_bytes_and_renders_last_cursor() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 239];
        app_state.block_types = vec![BlockType::Undefined; 239];
        // Mark the very last byte (index 238) as Code to verify that column 119 covers byte 238.
        app_state.block_types[238] = BlockType::Code;
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.cursor_index = app_state.disassembly.len() - 1;

        let area = Rect::new(0, 0, 120, 1);
        ui_state.minimap_area = area;
        let backend = TestBackend::new(120, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        let bar = MinimapBar;
        terminal
            .draw(|f| {
                bar.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let last_cell = buffer.cell((119, 0)).unwrap();
        assert!(
            matches!(last_cell.symbol(), "▏" | "│" | "▕"),
            "Expected cursor tick at last column 119 for byte 238, got '{}'",
            last_cell.symbol()
        );
        assert_eq!(
            last_cell.bg, ui_state.theme.block_code_fg,
            "Expected last column 119 to reflect Code block at byte 238"
        );
    }

    #[test]
    fn test_minimap_click_jumps_and_records_navigation_history() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        // 239 bytes of 3-byte JMP $1000 instructions so clicking lands inside multi-byte instructions.
        let mut raw = Vec::with_capacity(239);
        while raw.len() + 3 <= 239 {
            raw.extend_from_slice(&[0x4C, 0x00, 0x10]);
        }
        while raw.len() < 239 {
            raw.push(0xEA);
        }
        app_state.raw_data = raw;
        app_state.block_types = vec![BlockType::Code; 239];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.minimap_area = Rect::new(0, 0, 120, 1);
        ui_state.cursor_index = 0;

        let mut bar = MinimapBar;
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 60,
            row: 0,
            modifiers: KeyModifiers::empty(),
        };
        let res = bar.handle_mouse(mouse, &mut app_state, &mut ui_state);
        assert_eq!(res, WidgetResult::Handled);
        assert_eq!(ui_state.core.navigation_history.len(), 1);
        assert!(ui_state.cursor_index > 0);
    }

    #[test]
    fn test_minimap_sub_cell_tick_segments_and_empty_guards() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        // 30 bytes rendered into 10 columns -> each column covers 3 bytes (chunk_len = 3),
        // so offsets 0, 1, 2 in column 0 map to segments 0 ('▏'), 1 ('│'), 2 ('▕').
        app_state.raw_data = vec![0xEA; 30];
        app_state.block_types = vec![BlockType::Code; 30];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        let area = Rect::new(0, 0, 10, 1);
        let backend = TestBackend::new(10, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let bar = MinimapBar;

        for (cursor_idx, expected_tick) in [(0, "▏"), (1, "│"), (2, "▕")] {
            ui_state.cursor_index = cursor_idx;
            terminal
                .draw(|f| {
                    bar.render(f, area, &app_state, &mut ui_state);
                })
                .unwrap();
            let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
            assert_eq!(cell.symbol(), expected_tick);
        }

        // Empty block_types or zero-width area does not panic and ignores mouse clicks.
        let empty_state = AppState::new();
        terminal
            .draw(|f| {
                bar.render(f, area, &empty_state, &mut ui_state);
                bar.render(f, Rect::new(0, 0, 0, 1), &app_state, &mut ui_state);
            })
            .unwrap();
    }
}
