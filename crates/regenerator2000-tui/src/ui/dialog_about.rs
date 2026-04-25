use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui_image::StatefulImage;
use std::cell::RefCell;
use std::time::Instant;

/// Number of logo clicks required to trigger the easter egg.
const EASTER_EGG_CLICKS: u8 = 5;

/// How long the easter egg screen is shown before the dialog auto-closes.
const EASTER_EGG_DURATION_SECS: u64 = 2;

/// The classic C64 boot screen text, faithfully reproduced.
const C64_BOOT_SCREEN: &str = "\
    **** COMMODORE 64 BASIC V2 ****\n\
\n\
 64K RAM SYSTEM  38911 BASIC BYTES FREE\n\
\n\
READY.\n\
SYS 64738";

pub struct AboutDialog {
    protocol: RefCell<Option<ratatui_image::protocol::StatefulProtocol>>,
    /// Tracks left-click count on the logo area.
    logo_click_count: u8,
    /// Set to `Some(instant)` when the easter egg is active.
    easter_egg_triggered_at: Option<Instant>,
    /// The last known rendered logo area (used for hit-testing).
    logo_area: RefCell<Rect>,
}

impl AboutDialog {
    /// Creates a new `AboutDialog`.
    #[must_use]
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
            logo_click_count: 0,
            easter_egg_triggered_at: None,
            logo_area: RefCell::new(Rect::default()),
        }
    }

    /// Returns `true` if the easter egg is currently active and still within its display window.
    fn is_easter_egg_active(&self) -> bool {
        self.easter_egg_triggered_at
            .is_some_and(|t| t.elapsed().as_secs() < EASTER_EGG_DURATION_SECS)
    }

    /// Returns `true` if the easter egg was triggered and its display time has expired.
    fn is_easter_egg_expired(&self) -> bool {
        self.easter_egg_triggered_at
            .is_some_and(|t| t.elapsed().as_secs() >= EASTER_EGG_DURATION_SECS)
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

            // --- Easter egg: replace dialog content with C64 boot screen ---
            if self.is_easter_egg_active() {
                // Draw a solid blue background (C64 default border + background colour)
                let blue_block =
                    Block::default().style(Style::default().bg(Color::Blue).fg(Color::LightBlue));
                f.render_widget(blue_block, popup_area);

                let inner = Rect::new(
                    popup_area.x + 1,
                    popup_area.y + 1,
                    popup_area.width.saturating_sub(2),
                    popup_area.height.saturating_sub(2),
                );

                let boot_text = Paragraph::new(C64_BOOT_SCREEN)
                    .style(Style::default().bg(Color::Blue).fg(Color::White))
                    .alignment(ratatui::layout::Alignment::Left);

                // Vertically center the boot text block (6 lines)
                let boot_lines = 6u16;
                let boot_y = inner.y + (inner.height.saturating_sub(boot_lines)) / 2;
                let boot_area = Rect::new(
                    inner.x + 2,
                    boot_y,
                    inner.width.saturating_sub(4),
                    boot_lines,
                );
                f.render_widget(boot_text, boot_area);
                return;
            }

            // --- Normal about dialog ---
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

            let native_width_cells = f64::from(logo.width()) / font_width;
            let native_height_cells = f64::from(logo.height()) / font_height;

            let avail_width_cells = f64::from(img_area.width);
            let avail_height_cells = f64::from(img_area.height);

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

            // Remember the rendered logo area for click hit-testing.
            *self.logo_area.borrow_mut() = centered_area;

            // Use the original logo and let the library handle the downsampling into the target rect
            if let Some(protocol) = self.protocol.borrow_mut().as_mut() {
                let widget = StatefulImage::new();
                f.render_stateful_widget(widget, centered_area, protocol);
            }

            // 2. Render Text
            let text_area = chunks[1];
            let text = format!(
                "Regenerator 2000 v{}\nCommit: {} ({})\n(c) Ricardo Quesada 2026\nriq / L.I.A\nA tribute to the original Regenerator, by Tom-Cat / Nostalgia",
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
        // Close immediately on any key press, whether the easter egg is showing or not.
        if self.is_easter_egg_expired()
            || matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_))
        {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        WidgetResult::Handled
    }

    fn handle_mouse(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // If the easter egg timer has expired, close on any further interaction.
        if self.is_easter_egg_expired() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        // Only count left-button down events.
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return WidgetResult::Handled;
        }

        // If easter egg is already active, a click closes it.
        if self.is_easter_egg_active() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        // Check whether the click landed on the logo area.
        let area = *self.logo_area.borrow();
        let col = mouse.column;
        let row = mouse.row;
        let on_logo = col >= area.x
            && col < area.x + area.width
            && row >= area.y
            && row < area.y + area.height;

        if on_logo {
            self.logo_click_count += 1;
            if self.logo_click_count >= EASTER_EGG_CLICKS {
                // Trigger the easter egg!
                self.easter_egg_triggered_at = Some(Instant::now());
                self.logo_click_count = 0;
            }
            return WidgetResult::Handled;
        }

        WidgetResult::Handled
    }
}
