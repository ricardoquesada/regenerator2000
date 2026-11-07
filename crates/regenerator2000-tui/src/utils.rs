// TUI-specific utility functions. Pure-logic utilities live in regenerator2000_core::utils.
// Re-export core utils so `crate::utils::calculate_entropy` etc. keep working.
pub use regenerator2000_core::utils::*;

use image::DynamicImage;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui_image::picker::Picker;

#[must_use]
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[must_use]
pub fn centered_rect_adaptive(
    percent_x: u16,
    width_min: u16,
    percent_y: u16,
    height_min: u16,
    r: Rect,
) -> Rect {
    let width_pct =
        (u32::from(r.width) * u32::from(percent_x) / 100).min(u32::from(r.width)) as u16;
    let height_pct =
        (u32::from(r.height) * u32::from(percent_y) / 100).min(u32::from(r.height)) as u16;
    let width = width_pct.max(width_min).min(r.width);
    let height = height_pct.max(height_min).min(r.height);

    let x = (r.width - width) / 2;
    let y = (r.height - height) / 2;

    Rect::new(r.x.saturating_add(x), r.y.saturating_add(y), width, height)
}

#[must_use]
pub fn load_logo() -> Option<DynamicImage> {
    let logo_bytes = include_bytes!("../assets/regenerator2000_logo.png");
    if let Ok(img) = image::load_from_memory(logo_bytes) {
        return Some(img);
    }
    None
}
#[must_use]
pub fn create_picker() -> Option<Picker> {
    let font_size = (8, 16);
    // Force Kitty protocol for Ghostty if autodetection fails/blurs.
    // ratatui-image 0.9 Picker::new(font_size) might be available.
    #[allow(deprecated)]
    let picker = Picker::from_fontsize(font_size.into());

    // Attempt to force Kitty for Ghostty
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "ghostty" {
        // This is a guess at the API since autodetection is failing.
        // We'll see if this compiles.
        // picker.protocol_type = ProtocolType::Kitty;
    }

    Some(picker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_adaptive_handles_u16_max_and_wide_rects_without_overflow() {
        // UIState::terminal_area returns height == u16::MAX in headless tests when only menu_area is set.
        let full = Rect::new(0, 0, 1000, u16::MAX);
        let rect = centered_rect_adaptive(80, 60, 60, 20, full);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, (u32::from(u16::MAX) * 60 / 100) as u16);
        assert_eq!(rect.x, 100);
    }
}
