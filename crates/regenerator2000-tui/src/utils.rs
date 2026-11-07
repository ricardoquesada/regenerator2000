// TUI-specific utility functions. Pure-logic utilities live in regenerator2000_core::utils.
// Re-export core utils so `crate::utils::calculate_entropy` etc. keep working.
pub use regenerator2000_core::utils::*;

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::picker::Picker;

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
    image::load_from_memory(logo_bytes).ok()
}

#[must_use]
pub fn create_picker() -> Option<Picker> {
    let font_size = (8, 16);
    #[allow(deprecated)]
    let picker = Picker::from_fontsize(font_size.into());
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

        // Minimum dimensions clamp up when percentage is smaller, and clamp down to outer rect when smaller than min.
        let medium = Rect::new(10, 5, 100, 40);
        let clamped_min = centered_rect_adaptive(20, 50, 20, 16, medium);
        assert_eq!(clamped_min, Rect::new(35, 17, 50, 16));

        let tiny = Rect::new(0, 0, 30, 10);
        let clamped_outer = centered_rect_adaptive(80, 50, 80, 20, tiny);
        assert_eq!(clamped_outer, Rect::new(0, 0, 30, 10));
    }
}
