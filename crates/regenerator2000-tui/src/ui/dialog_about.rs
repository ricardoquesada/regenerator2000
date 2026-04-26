use crate::events::AppEvent;
use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, MouseButton, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui_image::StatefulImage;
use std::cell::RefCell;
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ── Tuning constants ──────────────────────────────────────────────────────────

/// Number of logo clicks required to trigger the easter egg.
const EASTER_EGG_CLICKS: u8 = 5;

/// The text that is "typed" character by character.
const TYPED_TEXT: &str = "SYS 64738";

/// Delay between each typed character.
const CHAR_INTERVAL: Duration = Duration::from_millis(120);

/// Cursor blink period.
const BLINK_PERIOD: Duration = Duration::from_millis(400);

/// How long to wait after the last character is typed before auto-closing.
const CLOSE_DELAY: Duration = Duration::from_secs(2);

/// Tick interval sent to the event loop to drive animation.
const TICK_INTERVAL: Duration = Duration::from_millis(80);

// ── Static C64 header lines (displayed immediately) ───────────────────────────

const C64_HEADER: &str = "\
    **** COMMODORE 64 BASIC V2 ****\n\
\n\
 64K RAM SYSTEM  38911 BASIC BYTES FREE\n\
\n\
READY.";

// ── Widget ────────────────────────────────────────────────────────────────────

pub struct AboutDialog {
    protocol: RefCell<Option<ratatui_image::protocol::StatefulProtocol>>,
    /// Sends `AppEvent::Tick` to drive animation re-renders.
    event_tx: mpsc::Sender<AppEvent>,
    /// Tracks left-click count on the logo area.
    logo_click_count: u8,
    /// Set to `Some(instant)` when the easter egg is activated.
    egg_start: Option<Instant>,
    /// The last known rendered logo area (used for click hit-testing).
    logo_area: RefCell<Rect>,
}

impl AboutDialog {
    /// Creates a new `AboutDialog`.
    #[must_use]
    pub fn new(ui_state: &mut UIState, event_tx: mpsc::Sender<AppEvent>) -> Self {
        let protocol = if let Some(logo) = &ui_state.logo
            && let Some(picker) = &ui_state.picker
        {
            Some(picker.new_resize_protocol(logo.clone()))
        } else {
            None
        };
        Self {
            protocol: RefCell::new(protocol),
            event_tx,
            logo_click_count: 0,
            egg_start: None,
            logo_area: RefCell::new(Rect::default()),
        }
    }

    // ── Easter egg helpers ────────────────────────────────────────────────────

    /// Start the easter egg: record the activation instant and spawn the tick thread.
    fn activate_egg(&mut self) {
        self.egg_start = Some(Instant::now());
        let tx = self.event_tx.clone();
        std::thread::spawn(move || {
            // Send ticks until the receiver is gone (dialog closed).
            // Total animation = header + typing + 2 s close delay.
            let total =
                CHAR_INTERVAL * TYPED_TEXT.len() as u32 + CLOSE_DELAY + Duration::from_secs(1);
            let steps = (total.as_millis() / TICK_INTERVAL.as_millis()) as u32 + 4;
            for _ in 0..steps {
                std::thread::sleep(TICK_INTERVAL);
                if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });
    }

    /// How many characters of `TYPED_TEXT` should be visible right now.
    fn visible_chars(&self) -> usize {
        let elapsed = self.egg_start.map_or(Duration::ZERO, |t| t.elapsed());
        let chars = elapsed.as_millis() / CHAR_INTERVAL.as_millis();
        (chars as usize).min(TYPED_TEXT.len())
    }

    /// Whether all characters have been typed.
    fn typing_done(&self) -> bool {
        self.visible_chars() >= TYPED_TEXT.len()
    }

    /// Whether the easter egg is active but the close delay has not yet expired.
    fn egg_alive(&self) -> bool {
        self.egg_start.is_some() && !self.should_close_now()
    }

    /// Whether enough time has passed since the last character was typed to auto-close.
    fn should_close_now(&self) -> bool {
        let Some(start) = self.egg_start else {
            return false;
        };
        let typing_done_at = CHAR_INTERVAL * TYPED_TEXT.len() as u32;
        let close_at = typing_done_at + CLOSE_DELAY;
        start.elapsed() >= close_at
    }

    /// Whether the cursor should be visible right now (blink logic).
    fn cursor_visible(&self) -> bool {
        let elapsed = self.egg_start.map_or(Duration::ZERO, |t| t.elapsed());
        // Blink only while typing or within 1 s after.
        let blink_end = CHAR_INTERVAL * TYPED_TEXT.len() as u32 + Duration::from_secs(1);
        if elapsed > blink_end {
            return false;
        }
        (elapsed.as_millis() / BLINK_PERIOD.as_millis()).is_multiple_of(2)
    }

    // ── Easter egg render ─────────────────────────────────────────────────────

    fn render_egg(&self, f: &mut Frame, popup_area: Rect) {
        // C64 colour palette approximations:
        //   border  → light blue / periwinkle  (Color::Rgb(100,100,220) ≈ #6464dc)
        //   screen  → darker blue              (Color::Rgb(0,0,170)     ≈ #0000aa)
        //   text    → light blue               (Color::Rgb(100,100,220))
        let border_bg = Color::Rgb(100, 100, 220);
        let screen_bg = Color::Rgb(0, 0, 170);
        let text_fg = Color::Rgb(100, 100, 220);

        // 1. Fill entire popup with border colour.
        let border_block = Block::default().style(Style::default().bg(border_bg));
        f.render_widget(border_block, popup_area);

        // 2. Center the "screen" (80 % wide, 80 % tall) within the popup.
        let screen_w = (popup_area.width * 80 / 100).max(20);
        let screen_h = (popup_area.height * 80 / 100).max(8);
        let screen_x = popup_area.x + (popup_area.width.saturating_sub(screen_w)) / 2;
        let screen_y = popup_area.y + (popup_area.height.saturating_sub(screen_h)) / 2;
        let screen_area = Rect::new(screen_x, screen_y, screen_w, screen_h);

        let screen_block = Block::default().style(Style::default().bg(screen_bg));
        f.render_widget(screen_block, screen_area);

        // 3. Text inside screen (1-cell padding on each side).
        let text_x = screen_area.x + 1;
        let text_y = screen_area.y + 1;
        let text_w = screen_area.width.saturating_sub(2);
        let text_h = screen_area.height.saturating_sub(2);
        if text_w == 0 || text_h == 0 {
            return;
        }
        let text_area = Rect::new(text_x, text_y, text_w, text_h);

        // Build lines: static header + blank line + typed portion.
        let base_style = Style::default().bg(screen_bg).fg(text_fg);
        let mut lines: Vec<Line> = C64_HEADER
            .lines()
            .map(|l| Line::styled(l.to_string(), base_style))
            .collect();

        // Typed line with optional blinking cursor.
        let n = self.visible_chars();
        let typed = &TYPED_TEXT[..n];
        let mut spans = vec![Span::styled(typed.to_string(), base_style)];
        if !self.typing_done() || self.cursor_visible() {
            spans.push(Span::styled(
                " ",
                base_style.add_modifier(Modifier::REVERSED),
            ));
        }
        lines.push(Line::from(spans));

        let paragraph = Paragraph::new(lines).style(base_style);
        f.render_widget(paragraph, text_area);
    }
}

impl Widget for AboutDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        if let Some(logo) = &ui_state.logo
            && let Some(_picker) = &ui_state.picker
        {
            // Center popup (same size as normal about dialog)
            let percent_x = 60;
            let percent_y = 75;
            let popup_width = area.width * percent_x / 100;
            let popup_height = area.height * percent_y / 100;
            let x = (area.width - popup_width) / 2;
            let y = (area.height - popup_height) / 2;
            let popup_area = Rect::new(x, y, popup_width, popup_height);
            ui_state.active_dialog_area = popup_area;
            f.render_widget(Clear, popup_area);

            // ── Easter egg mode ────────────────────────────────────────────
            if self.egg_alive() {
                self.render_egg(f, popup_area);
                return;
            }

            // ── Normal about dialog ────────────────────────────────────────
            let theme = &ui_state.theme;
            let block = crate::ui::widget::create_dialog_block(" About ", theme);
            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Min(0),
                    ratatui::layout::Constraint::Length(6),
                ])
                .split(inner);

            // Logo
            let img_area = chunks[0];
            let font_width = 8.0_f64;
            let font_height = 16.0_f64;
            let native_width_cells = f64::from(logo.width()) / font_width;
            let native_height_cells = f64::from(logo.height()) / font_height;
            let scale_w = f64::from(img_area.width) / native_width_cells;
            let scale_h = f64::from(img_area.height) / native_height_cells;
            let scale = scale_w.min(scale_h).min(1.0);
            let render_width = (native_width_cells * scale).max(1.0) as u16;
            let render_height = (native_height_cells * scale).max(1.0) as u16;
            let x = img_area.x + (img_area.width.saturating_sub(render_width)) / 2;
            let y = img_area.y + (img_area.height.saturating_sub(render_height)) / 2;
            let centered_area = Rect::new(x, y, render_width, render_height);
            *self.logo_area.borrow_mut() = centered_area;

            if let Some(protocol) = self.protocol.borrow_mut().as_mut() {
                f.render_stateful_widget(StatefulImage::new(), centered_area, protocol);
            }

            // Text
            let text_area = chunks[1];
            let text = format!(
                "Regenerator 2000 v{}\nCommit: {} ({})\n(c) Ricardo Quesada 2026\nriq / L.I.A\nA tribute to the original Regenerator, by Tom-Cat / Nostalgia",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
                option_env!("VERGEN_GIT_COMMIT_DATE").unwrap_or("unknown")
            );
            let text_height = 5u16;
            let text_y = text_area.y + (text_area.height.saturating_sub(text_height)) / 2;
            let centered_text_area = Rect::new(text_area.x, text_y, text_area.width, text_height);
            f.render_widget(
                Paragraph::new(text)
                    .alignment(ratatui::layout::Alignment::Center)
                    .block(Block::default()),
                centered_text_area,
            );
        }
    }

    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Auto-close once the timer has expired.
        if self.should_close_now() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        // Normal close keys (only when the egg is NOT active).
        if self.egg_start.is_none()
            && matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char(_))
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
        // Auto-close once the timer has expired.
        if self.should_close_now() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        // Only act on left-button down.
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return WidgetResult::Handled;
        }

        // A click while the egg is running closes it immediately.
        if self.egg_alive() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        // Hit-test against the logo area.
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
                self.logo_click_count = 0;
                self.activate_egg();
            }
        }

        WidgetResult::Handled
    }
}
