// TUI-specific utility functions. Pure-logic utilities live in regenerator_core::utils.
// Re-export core utils so `crate::utils::calculate_entropy` etc. keep working.
pub use regenerator_core::utils::*;

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
    let width = (r.width * percent_x / 100).max(width_min).min(r.width);
    let height = (r.height * percent_y / 100).max(height_min).min(r.height);

    let x = (r.width - width) / 2;
    let y = (r.height - height) / 2;

    Rect::new(r.x + x, r.y + y, width, height)
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
    let picker = Picker::from_fontsize(font_size);

    // Attempt to force Kitty for Ghostty
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "ghostty" {
        // This is a guess at the API since autodetection is failing.
        // We'll see if this compiles.
        // picker.protocol_type = ProtocolType::Kitty;
    }

    Some(picker)
}
