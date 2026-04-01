use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::Paragraph,
};

pub struct MinimapBar;

impl Widget for MinimapBar {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        if app_state.block_types.is_empty() || area.width == 0 {
            return;
        }

        let total_bytes = app_state.block_types.len();
        let width = area.width as usize;
        let chunk_size = total_bytes / width;
        let chunk_size = if chunk_size == 0 { 1 } else { chunk_size };

        let cursor_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map_or(app_state.origin.0, |l| l.address.0);
        let cursor_offset = cursor_addr.saturating_sub(app_state.origin.0) as usize;
        let cursor_x = cursor_offset / chunk_size;

        let mut spans = Vec::new();

        for x in 0..width {
            let start = x * chunk_size;

            if start >= total_bytes {
                break;
            }

            // Pick the first block type in the chunk
            let block_type = app_state.block_types[start];

            // Map block type to color foreground (filled block character █)
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
                let local_offset = cursor_offset % chunk_size;
                let segment = (local_offset * 3) / chunk_size;
                let tick_char = match segment {
                    0 => "▏",
                    1 => "│",
                    2 => "▕",
                    _ => "│",
                };

                Span::styled(tick_char, Style::default().fg(Color::White).bg(fg_color))
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
        let chunk_size = total_bytes / width;
        let chunk_size = if chunk_size == 0 { 1 } else { chunk_size };

        let rel_x = (mouse.column - ui_state.minimap_area.x) as usize;
        let byte_offset = rel_x * chunk_size;
        let addr = app_state.origin.0 as usize + byte_offset;

        if let Some(idx) = app_state.get_line_index_for_address(crate::state::Addr(addr as u16)) {
            ui_state.cursor_index = idx;
            ui_state.scroll_index = idx;
            ui_state.sub_cursor_index = 0;
            ui_state.scroll_sub_index = 0;
            ui_state.active_pane = crate::ui_state::ActivePane::Disassembly;
            return WidgetResult::Handled;
        }

        WidgetResult::Ignored
    }
}
