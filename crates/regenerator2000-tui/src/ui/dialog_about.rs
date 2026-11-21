//! Animated C64-themed About dialog with logo rendering and keyboard easter egg.

use crate::events::AppEvent;
use crate::state::AppState;
use crate::terminal_graphics::{centered_image_rect, new_or_reused_protocol, theme_color_to_rgba};
use crate::ui::widget::{self, Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui_image::{FilterType, Resize, StatefulImage};
use std::cell::RefCell;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

// ── Tuning constants ──────────────────────────────────────────────────────────

/// Number of logo clicks required to trigger the easter egg.
const EASTER_EGG_CLICKS: u8 = 5;

/// The text that is "typed" character by character.
const TYPED_TEXT: &str = "SYS 64738";

/// Delay before the first character starts appearing.
/// Sized to exactly 3 full cursor blink cycles (6 × `BLINK_PERIOD`).
const TYPING_DELAY: Duration = Duration::from_millis(2400);

/// Delay between each typed character.
const CHAR_INTERVAL: Duration = Duration::from_millis(240);

/// Cursor blink period.
const BLINK_PERIOD: Duration = Duration::from_millis(400);

/// How long to wait after the last character is typed before auto-closing.
const CLOSE_DELAY: Duration = Duration::from_secs(2);

/// Tick interval sent to the event loop to drive animation.
const TICK_INTERVAL: Duration = Duration::from_millis(80);

// ── Static C64 header lines (displayed immediately) ───────────────────────────

const C64_HEADER: &str =
    "    **** COMMODORE 64 BASIC V2 ****\n\n 64K RAM SYSTEM  38911 BASIC BYTES FREE\n\nREADY.";

// ── Widget ────────────────────────────────────────────────────────────────────

/// Modal "About" dialog displaying the application logo, version info, and credits.
pub struct AboutDialog {
    /// Sends `AppEvent::Tick` to drive animation re-renders.
    event_tx: mpsc::Sender<AppEvent>,
    /// Tracks left-click count on the logo area.
    logo_click_count: u8,
    /// Set to `Some(instant)` when the easter egg is activated.
    egg_start: Option<Instant>,
    /// The last known rendered logo area (used for click hit-testing).
    logo_area: RefCell<Rect>,
}

/// Rebuilds [`UIState::about_logo_protocol`] for the current logo, picker, and `dialog_bg`.
///
/// Reuses any existing slot so `Kitty` double-buffers its image ID pair (`id` / `id ^ 1`) while
/// resetting its transmit hash so opening `Help -> About` re-sends the texture cleanly (e.g.
/// after `tmux` reattach or terminal eviction).
fn rebuild_logo_protocol(ui_state: &mut UIState) {
    let (Some(logo), Some(picker)) = (&ui_state.logo, &ui_state.picker) else {
        ui_state.about_logo_protocol = None;
        return;
    };

    // Pre-composite transparent PNG pixels onto `dialog_bg` before downsampling to
    // prevent `#000000` boxes in Halfblocks and dark edge halos in Kitty/Iterm2/Sixel.
    let bg = theme_color_to_rgba(ui_state.theme.dialog_bg);
    let prev = ui_state.about_logo_protocol.take();
    ui_state.about_logo_protocol = Some(new_or_reused_protocol(picker, prev, logo.clone(), bg));
}

/// Synchronizes [`UIState::about_logo_protocol`] with the current logo, picker, and `dialog_bg`.
fn sync_logo_protocol(ui_state: &mut UIState) {
    let (Some(_), Some(picker)) = (&ui_state.logo, &ui_state.picker) else {
        ui_state.about_logo_protocol = None;
        return;
    };

    let bg = theme_color_to_rgba(ui_state.theme.dialog_bg);
    let needs_rebuild = ui_state.about_logo_protocol.as_ref().is_none_or(|proto| {
        proto.background_color() != bg || proto.protocol_kind() != picker.protocol_type()
    });
    if needs_rebuild {
        rebuild_logo_protocol(ui_state);
    }
}

impl AboutDialog {
    /// Uses `Lanczos3` downsampling because `StatefulImage::new()` defaults to `Nearest`,
    /// which creates jagged edges on the anti-aliased 869x553 logo.
    const LOGO_RESIZE: Resize = Resize::Fit(Some(FilterType::Lanczos3));

    /// Creates a new [`AboutDialog`] and re-arms the cached logo protocol for transmission while
    /// preserving its bounded Kitty double-buffer ID pair.
    #[must_use]
    pub fn new(ui_state: &mut UIState, event_tx: mpsc::Sender<AppEvent>) -> Self {
        rebuild_logo_protocol(ui_state);
        Self {
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
        thread::spawn(move || {
            // Send ticks until the receiver is gone (dialog closed).
            // Total = pre-delay + typing + 2 s close delay.
            let total = TYPING_DELAY
                + CHAR_INTERVAL * TYPED_TEXT.len() as u32
                + CLOSE_DELAY
                + Duration::from_secs(1);
            let steps = (total.as_millis() / TICK_INTERVAL.as_millis()) as u32 + 4;
            for _ in 0..steps {
                thread::sleep(TICK_INTERVAL);
                if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });
    }

    /// How many characters of `TYPED_TEXT` should be visible right now.
    fn visible_chars(&self) -> usize {
        let elapsed = self.egg_start.map_or(Duration::ZERO, |t| t.elapsed());
        // Characters only start appearing after TYPING_DELAY.
        let typing_elapsed = elapsed.saturating_sub(TYPING_DELAY);
        let chars = typing_elapsed.as_millis() / CHAR_INTERVAL.as_millis();
        (chars as usize).min(TYPED_TEXT.len())
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
        let typing_done_at = TYPING_DELAY + CHAR_INTERVAL * TYPED_TEXT.len() as u32;
        let close_at = typing_done_at + CLOSE_DELAY;
        start.elapsed() >= close_at
    }

    /// Whether the cursor should be visible right now (blink logic).
    fn cursor_visible(&self) -> bool {
        let elapsed = self.egg_start.map_or(Duration::ZERO, |t| t.elapsed());
        // Blink from egg activation until 1 s after typing finishes.
        let blink_end =
            TYPING_DELAY + CHAR_INTERVAL * TYPED_TEXT.len() as u32 + Duration::from_secs(1);
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

        // 3. Text inside screen: flush left (no padding), 1-cell top/bottom gap.
        let text_x = screen_area.x;
        let text_y = screen_area.y + 1;
        let text_w = screen_area.width;
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
        if self.cursor_visible() {
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
        let percent_x = 60;
        let percent_y = 75;
        let popup_width = area.width * percent_x / 100;
        let popup_height = area.height * percent_y / 100;
        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
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
        let block = widget::create_dialog_block(" About ", theme);
        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)])
            .split(inner);

        let img_area = chunks[0];
        sync_logo_protocol(ui_state);
        if let Some(protocol) = ui_state.about_logo_protocol.as_mut()
            && img_area.width > 0
            && img_area.height > 0
        {
            // Compute bounds with `picker` cell metrics so `logo_area` matches on Retina displays.
            let centered_area = centered_image_rect(protocol, &Self::LOGO_RESIZE, img_area);
            *self.logo_area.borrow_mut() = centered_area;
            f.render_stateful_widget(
                StatefulImage::new().resize(Self::LOGO_RESIZE),
                centered_area,
                protocol,
            );
        } else {
            // Clear stale bounds so hidden or tiny terminals ignore old logo coordinates.
            *self.logo_area.borrow_mut() = Rect::default();
        }

        let text_area = chunks[1];
        let text = format!(
            "Regenerator 2000 v{}\nCommit: {} ({})\n(c) Ricardo Quesada 2026\nriq / L.I.A\nA tribute to the original Regenerator, by Tom-Cat / Nostalgia",
            env!("CARGO_PKG_VERSION"),
            option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
            option_env!("VERGEN_GIT_COMMIT_DATE").unwrap_or("unknown")
        );
        let text_height = 5u16.min(text_area.height);
        if text_area.width > 0 && text_height > 0 {
            let text_y = text_area.y + (text_area.height.saturating_sub(text_height)) / 2;
            let centered_text_area = Rect::new(text_area.x, text_y, text_area.width, text_height);
            f.render_widget(
                Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .block(Block::default()),
                centered_text_area,
            );
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if self.should_close_now() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        if key.code == KeyCode::Esc
            || (self.egg_start.is_none() && matches!(key.code, KeyCode::Enter | KeyCode::Char(_)))
        {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        WidgetResult::Handled
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
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

        // While the egg animation is running, swallow all clicks silently.
        if self.egg_alive() {
            return WidgetResult::Handled;
        }

        // Hit-test against the logo area.
        let area = *self.logo_area.borrow();
        let on_logo = area.contains(Position::new(mouse.column, mouse.row));

        if on_logo {
            self.logo_click_count += 1;
            if self.logo_click_count >= EASTER_EGG_CLICKS {
                self.logo_click_count = 0;
                self.activate_egg();
            }
        }

        WidgetResult::Handled
    }

    fn handle_tick(&mut self, _app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult {
        if self.should_close_now() {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }
        WidgetResult::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_graphics::{
        TerminalImageProtocol, TmuxMode, create_headless_picker, extract_kitty_transmit_id,
    };
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;
    use image::Rgba;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;
    use ratatui_image::FontSize;
    use ratatui_image::picker::ProtocolType;

    fn about_dialog_chunks(area: Rect, theme: &Theme) -> Vec<Rect> {
        let popup_width = area.width * 60 / 100;
        let popup_height = area.height * 75 / 100;
        let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(x, y, popup_width, popup_height);
        let block = widget::create_dialog_block(" About ", theme);
        let inner = block.inner(popup_area);
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)])
            .split(inner)
            .to_vec()
    }

    fn left_click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn test_about_dialog_renders_text_when_logo_is_none() {
        let app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.logo = None;
        ui_state.picker = None;
        let (tx, _rx) = mpsc::channel();
        let dialog = AboutDialog::new(&mut ui_state, tx);

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw AboutDialog without logo");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
            .collect();
        assert!(rendered.contains("Regenerator 2000"));
        assert!(ui_state.active_dialog_area.width > 0);
        assert_eq!(*dialog.logo_area.borrow(), Rect::default());
    }

    #[test]
    fn test_about_dialog_esc_closes_during_easter_egg() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let (tx, _rx) = mpsc::channel();
        let mut dialog = AboutDialog::new(&mut ui_state, tx);
        dialog.egg_start = Some(Instant::now());

        let enter_res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(enter_res, WidgetResult::Handled);

        let esc_res = dialog.handle_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(esc_res, WidgetResult::Close);
    }

    #[test]
    fn test_about_dialog_renders_logo_into_non_empty_hit_area() {
        let mut app_state = AppState::new();
        let theme = Theme::from_name("Solarized Light");
        let mut ui_state = UIState::new(theme.clone());
        assert!(ui_state.logo.is_some());
        assert!(ui_state.picker.is_some());

        let (tx, _rx) = mpsc::channel();
        let mut dialog = AboutDialog::new(&mut ui_state, tx);

        let expected_bg = theme_color_to_rgba(theme.dialog_bg);
        assert_eq!(expected_bg, Some(Rgba([238, 232, 213, 255])));
        assert_eq!(
            ui_state
                .about_logo_protocol
                .as_ref()
                .expect("about_logo_protocol should be initialized")
                .background_color(),
            expected_bg
        );

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("create terminal");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw AboutDialog with logo");

        let chunks = about_dialog_chunks(Rect::new(0, 0, 100, 30), &theme);
        let img_area = chunks[0];
        let text_area = chunks[1];
        let logo_area = *dialog.logo_area.borrow();

        assert!(logo_area.width > 0 && logo_area.height > 0);
        assert!(logo_area.x >= img_area.x);
        assert!(logo_area.y >= img_area.y);
        assert!(logo_area.right() <= img_area.right());
        assert!(logo_area.bottom() <= img_area.bottom());

        let expected_logo_area = centered_image_rect(
            ui_state
                .about_logo_protocol
                .as_ref()
                .expect("about_logo_protocol should be populated after render"),
            &AboutDialog::LOGO_RESIZE,
            img_area,
        );
        assert_eq!(logo_area, expected_logo_area);

        // Click outside the logo area (on the footer text row) does not increment count.
        assert_eq!(
            dialog.handle_mouse(
                left_click(text_area.x, text_area.y),
                &mut app_state,
                &mut ui_state
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.logo_click_count, 0);
        assert!(dialog.egg_start.is_none());

        // Five clicks inside logo_area trigger the easter egg.
        for expected_clicks in 1..EASTER_EGG_CLICKS {
            assert_eq!(
                dialog.handle_mouse(
                    left_click(logo_area.x, logo_area.y),
                    &mut app_state,
                    &mut ui_state
                ),
                WidgetResult::Handled
            );
            assert_eq!(dialog.logo_click_count, expected_clicks);
            assert!(dialog.egg_start.is_none());
        }

        assert_eq!(
            dialog.handle_mouse(
                left_click(logo_area.x, logo_area.y),
                &mut app_state,
                &mut ui_state
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.logo_click_count, 0);
        assert!(dialog.egg_start.is_some());
    }

    #[test]
    fn test_about_dialog_respects_retina_font_size_for_logo_area() {
        let app_state = AppState::new();
        let theme = Theme::default();
        let mut ui_state = UIState::new(theme.clone());
        let logo = ui_state.logo.clone().expect("logo should load");
        let bg = theme_color_to_rgba(ui_state.theme.dialog_bg);

        let (tx, _rx) = mpsc::channel();
        let dialog = AboutDialog::new(&mut ui_state, tx);
        ui_state.about_logo_protocol = Some(TerminalImageProtocol::new_halfblocks_with_font_size(
            logo.clone(),
            FontSize::new(18, 40),
            bg,
        ));

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create retina terminal");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw retina AboutDialog");

        let img_area = about_dialog_chunks(Rect::new(0, 0, 120, 40), &theme)[0];
        let expected_retina = centered_image_rect(
            ui_state
                .about_logo_protocol
                .as_ref()
                .expect("about_logo_protocol should be populated"),
            &AboutDialog::LOGO_RESIZE,
            img_area,
        );
        let legacy_8x16_proto =
            TerminalImageProtocol::new_halfblocks_with_font_size(logo, FontSize::new(8, 16), bg);
        let legacy_8x16_rect =
            centered_image_rect(&legacy_8x16_proto, &AboutDialog::LOGO_RESIZE, img_area);

        let rendered_logo_area = *dialog.logo_area.borrow();
        assert_eq!(rendered_logo_area, expected_retina);
        assert_ne!(rendered_logo_area, legacy_8x16_rect);
    }

    #[test]
    fn test_about_dialog_iterm2_uses_centered_cell_dimensions() {
        let app_state = AppState::new();
        let theme = Theme::from_name("Catppuccin Mocha");
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Iterm2);

        let mut ui_state = UIState::new(theme.clone());
        ui_state.picker = Some(picker);
        let logo = ui_state.logo.clone().expect("logo should load");
        let bg = theme_color_to_rgba(ui_state.theme.dialog_bg);

        let (tx, _rx) = mpsc::channel();
        let dialog = AboutDialog::new(&mut ui_state, tx);
        ui_state.about_logo_protocol = Some(TerminalImageProtocol::new_iterm2_with_font_size(
            logo,
            FontSize::new(7, 17),
            bg,
            TmuxMode::Direct,
        ));

        let mut terminal =
            Terminal::new(TestBackend::new(210, 50)).expect("create iterm2 terminal");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw iterm2 AboutDialog");

        let img_area = about_dialog_chunks(Rect::new(0, 0, 210, 50), &theme)[0];
        let logo_area = *dialog.logo_area.borrow();
        let expected = centered_image_rect(
            ui_state
                .about_logo_protocol
                .as_ref()
                .expect("about_logo_protocol should remain populated"),
            &AboutDialog::LOGO_RESIZE,
            img_area,
        );
        assert_eq!(logo_area, expected);

        let left_margin = logo_area.x - img_area.x;
        let right_margin = img_area.right() - logo_area.right();
        assert!(left_margin.abs_diff(right_margin) <= 1);

        let sym = terminal.backend().buffer()[(logo_area.x, logo_area.y)].symbol();
        assert!(!sym.contains("px;"));
        assert!(sym.contains(&format!(
            ";width={};height={};preserveAspectRatio=0;doNotMoveCursor=1:",
            logo_area.width, logo_area.height
        )));
    }

    #[test]
    fn test_about_dialog_resets_logo_area_when_unavailable_or_empty() {
        let mut app_state = AppState::new();
        let theme = Theme::default();
        let mut ui_state = UIState::new(theme.clone());
        assert!(ui_state.logo.is_some());
        assert!(ui_state.picker.is_some());

        let (tx, _rx) = mpsc::channel();
        let mut dialog = AboutDialog::new(&mut ui_state, tx);

        // Initial render populates a non-empty logo_area.
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("create terminal");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw initial AboutDialog");
        let initial_logo_area = *dialog.logo_area.borrow();
        assert!(initial_logo_area.width > 0 && initial_logo_area.height > 0);

        // (a) Shrinking terminal so chunks[0].height == 0 resets logo_area without clamping panic.
        let tiny_chunks = about_dialog_chunks(Rect::new(0, 0, 60, 8), &theme);
        assert_eq!(tiny_chunks[0].height, 0);
        let mut tiny_terminal =
            Terminal::new(TestBackend::new(60, 8)).expect("create tiny terminal");
        tiny_terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw tiny AboutDialog");
        assert_eq!(*dialog.logo_area.borrow(), Rect::default());

        // Restore a populated logo_area first so (b) also verifies clearing from non-empty.
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("re-draw populated AboutDialog");
        assert_eq!(*dialog.logo_area.borrow(), initial_logo_area);

        // (b) Clearing picker and re-rendering drops about_logo_protocol, resets logo_area, and
        // ignores stale clicks.
        ui_state.picker = None;
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw AboutDialog after clearing picker");
        assert!(ui_state.about_logo_protocol.is_none());
        assert_eq!(*dialog.logo_area.borrow(), Rect::default());

        assert_eq!(
            dialog.handle_mouse(
                left_click(initial_logo_area.x, initial_logo_area.y),
                &mut app_state,
                &mut ui_state
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.logo_click_count, 0);
        assert!(dialog.egg_start.is_none());
    }

    #[test]
    fn test_about_dialog_reopening_reuses_cached_protocol_and_kitty_id() {
        let app_state = AppState::new();
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let mut ui_state = UIState::new(Theme::from_name("Dracula"));
        ui_state.picker = Some(picker);

        let (tx1, _rx1) = mpsc::channel();
        let dialog1 = AboutDialog::new(&mut ui_state, tx1);
        let mut term1 = Terminal::new(TestBackend::new(100, 30)).expect("create term1");
        term1
            .draw(|f| {
                let area = f.area();
                dialog1.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw term1");

        let logo_area = *dialog1.logo_area.borrow();
        let sym1 = term1.backend().buffer()[(logo_area.x, logo_area.y)].symbol();
        let id1 = extract_kitty_transmit_id(sym1)
            .expect("first AboutDialog Kitty render should emit transmission escape");
        let fg1 = term1.backend().buffer()[(logo_area.x, logo_area.y)].fg;

        // A second render while the same dialog stays open does not re-transmit.
        let mut term1_again = Terminal::new(TestBackend::new(100, 30)).expect("create term1_again");
        term1_again
            .draw(|f| {
                let area = f.area();
                dialog1.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw term1_again");
        let sym1_again = term1_again.backend().buffer()[(logo_area.x, logo_area.y)].symbol();
        assert!(sym1_again.contains('\u{10EEEE}'));
        assert!(
            !sym1_again.contains(",a=T,U=1"),
            "subsequent frame of same open dialog should reuse transmitted Kitty texture"
        );
        assert_eq!(
            term1_again.backend().buffer()[(logo_area.x, logo_area.y)].fg,
            fg1
        );
        drop(dialog1);

        // Second open with identical theme flips to `id1 ^ 1` so `Buffer::diff` updates all rows,
        // while keeping GPU storage bounded to the 2-slot double-buffer pair.
        let (tx2, _rx2) = mpsc::channel();
        let dialog2 = AboutDialog::new(&mut ui_state, tx2);
        let mut term2 = Terminal::new(TestBackend::new(100, 30)).expect("create term2");
        term2
            .draw(|f| {
                let area = f.area();
                dialog2.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw term2");
        let buf2 = term2.backend().buffer();
        let sym2 = buf2[(logo_area.x, logo_area.y)].symbol();
        assert!(sym2.contains('\u{10EEEE}'));
        let id2 = extract_kitty_transmit_id(sym2)
            .expect("reopening AboutDialog should re-transmit logo under flipped Kitty ID");
        assert_eq!(id2, id1 ^ 1);
        assert_ne!(buf2[(logo_area.x, logo_area.y)].fg, fg1);
        drop(dialog2);

        // Switching theme invalidates background_color and cycles back to `id1` (zero ID leak).
        ui_state.theme = Theme::from_name("Solarized Light");
        let (tx3, _rx3) = mpsc::channel();
        let dialog3 = AboutDialog::new(&mut ui_state, tx3);
        assert_eq!(
            ui_state
                .about_logo_protocol
                .as_ref()
                .expect("about_logo_protocol should remain populated")
                .background_color(),
            Some(Rgba([238, 232, 213, 255]))
        );
        let mut term3 = Terminal::new(TestBackend::new(100, 30)).expect("create term3");
        term3
            .draw(|f| {
                let area = f.area();
                dialog3.render(f, area, &app_state, &mut ui_state);
            })
            .expect("draw term3");
        let buf3 = term3.backend().buffer();
        let id3 = extract_kitty_transmit_id(buf3[(logo_area.x, logo_area.y)].symbol())
            .expect("theme switch reopen should emit updated Kitty transmission");
        assert_eq!(buf3[(logo_area.x, logo_area.y)].fg, fg1);
        assert_eq!(id3, id1);
    }
}
