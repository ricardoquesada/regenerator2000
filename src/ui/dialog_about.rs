use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui_image::StatefulImage;
use std::cell::RefCell;

pub struct AboutDialog {
    protocol: RefCell<Option<ratatui_image::protocol::StatefulProtocol>>,
}

impl AboutDialog {
    pub fn new(ui_state: &mut UIState) -> Self {
        let protocol = if let Some(logo) = &ui_state.logo
            && let Some(picker) = &ui_state.picker
        {
            Some(picker.new_resize_protocol(logo.clone()))
        } else {
            None
        };
        Self {
            protocol: RefCell::new(protocol),
        }
    }
}

impl Widget for AboutDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        if let Some(logo) = &ui_state.logo
            && let Some(_picker) = &ui_state.picker
        {
            // Center popup
            let percent_x = 60;
            let percent_y = 75;
            let popup_width = area.width * percent_x / 100;
            let popup_height = area.height * percent_y / 100;
            let x = (area.width - popup_width) / 2;
            let y = (area.height - popup_height) / 2;

            let popup_area = Rect::new(x, y, popup_width, popup_height);
            ui_state.active_dialog_area = popup_area;

            f.render_widget(Clear, popup_area);

            let theme = &ui_state.theme;
            let block = crate::ui::widget::create_dialog_block(" About ", theme);
            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            // Split inner area: Top (Image), Bottom (Text)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(6)])
                .split(inner);

            // 1. Render Logo
            let img_area = chunks[0];

            // Calculate native size in cells (assuming 8x16 font)
            let font_width = 8.0;
            let font_height = 16.0;

            let native_width_cells = logo.width() as f64 / font_width;
            let native_height_cells = logo.height() as f64 / font_height;

            let avail_width_cells = img_area.width as f64;
            let avail_height_cells = img_area.height as f64;

            // Calculate scale to fit
            let scale_w = avail_width_cells / native_width_cells;
            let scale_h = avail_height_cells / native_height_cells;

            // Limit scale to 1.0 (don't upscale)
            let scale = scale_w.min(scale_h).min(1.0);

            let render_width = (native_width_cells * scale).max(1.0) as u16;
            let render_height = (native_height_cells * scale).max(1.0) as u16;

            let x = img_area.x + (img_area.width.saturating_sub(render_width)) / 2;
            let y = img_area.y + (img_area.height.saturating_sub(render_height)) / 2;

            let centered_area = Rect::new(x, y, render_width, render_height);

            // Use the original logo and let the library handle the downsampling into the target rect
            if let Some(protocol) = self.protocol.borrow_mut().as_mut() {
                let widget = StatefulImage::new();
                f.render_stateful_widget(widget, centered_area, protocol);
            }

            // 2. Render Text
            let text_area = chunks[1];
            let text = format!(
                "Regenerator 2000 v{}\nCommit: {} ({})\n(c) Ricardo Quesada 2026\nriq / L.I.A\nInspired by Regenerator, by Tom-Cat / Nostalgia",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
                option_env!("VERGEN_GIT_COMMIT_DATE").unwrap_or("unknown")
            );
            let paragraph = Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .block(Block::default());

            // Vertically center text in text_area
            let text_height = 5;
            let text_y = text_area.y + (text_area.height.saturating_sub(text_height)) / 2;
            let centered_text_area = Rect::new(text_area.x, text_y, text_area.width, text_height);

            f.render_widget(paragraph, centered_text_area);
        }
    }

    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_) = key.code {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        WidgetResult::Handled
    }
}
