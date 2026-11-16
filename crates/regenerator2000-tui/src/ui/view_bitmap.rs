use crate::state::AppState;
use crate::terminal_graphics::{centered_image_rect, new_or_reused_protocol, theme_color_to_rgba};
use crate::ui::graphics_common::VIC_II_RGB;
use crate::ui_state::{ActivePane, AppAction, ScreenRamMode, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use image::{DynamicImage, Rgb, RgbImage};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{FilterType, Resize, StatefulImage, picker::ProtocolType};

use crate::ui::navigable::{
    AlignedGridRange, Navigable, handle_nav_input, jump_to_disassembly_at_address,
    right_pane_mouse_inner_area, sync_grid_item_to_disassembly,
};
use crate::ui::widget::{Widget, WidgetResult};

/// Immediately mirrors the bitmap cursor into the disassembly view's `cursor_index` and
/// clears any active disassembly selection so both views stay consistent within the same frame.
fn sync_bitmap_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    let grid = BitmapView::grid_range(app_state);
    sync_grid_item_to_disassembly(
        app_state,
        ui_state,
        &grid,
        ui_state.bitmap_cursor_index,
        None,
    );
}

/// Right-pane widget rendering C64 High-Res (320×200) and Multicolor (160×200) bitmaps.
pub struct BitmapView;

impl BitmapView {
    /// Active C64 bitmap data size in bytes (40×25 character cells × 8 bytes).
    const PAYLOAD_BYTES: usize = 8000;
    /// Scales `320×200` pixel art up to Retina pane dimensions with sharp pixel edges.
    ///
    /// Uses [`Resize::Scale`] instead of [`Resize::Fit`] because `Fit` never upscales beyond
    /// native pixel dimensions.
    const BITMAP_RESIZE: Resize = Resize::Scale(Some(FilterType::Nearest));

    #[must_use]
    fn grid_range(app_state: &AppState) -> AlignedGridRange {
        AlignedGridRange::bitmap(app_state)
    }
}

/// Calculates the Screen RAM address based on mode and bitmap address.
#[must_use]
fn calculate_screen_ram_addr(bitmap_addr: usize, mode: ScreenRamMode) -> usize {
    match mode {
        ScreenRamMode::AfterBitmap => bitmap_addr + 8000,
        ScreenRamMode::BankOffset(offset) => {
            // Determine VIC bank (0x0000, 0x4000, 0x8000, 0xC000)
            let vic_bank = (bitmap_addr / 0x4000) * 0x4000;
            // Add offset (0x0000, 0x0400, 0x0800, ..., 0x3C00)
            vic_bank + (offset as usize * 0x0400)
        }
    }
}

/// Returns `true` if a modal dialog or intersecting menu popup overlaps `target`.
///
/// Raw pixel protocols (`Kitty`, `Iterm2`, `Sixel`) cannot rely on Ratatui cell clearing
/// alone when overlays intersect `target`.
#[must_use]
fn is_occluded_by_overlay(ui_state: &UIState, full_frame_area: Rect, target: Rect) -> bool {
    // Guard empty rects before `Rect::intersects` because Ratatui returns `true` when a
    // zero-sized rect lies inside another rect's bounds.
    if target.is_empty() {
        return false;
    }
    // Check `active_dialog` instead of `active_dialog_area` because dialogs clear old bounds
    // on push and render after `BitmapView::render`.
    if ui_state.active_dialog.is_some() {
        return true;
    }
    if ui_state.menu.active {
        let bar_area = Rect::new(
            ui_state.menu_area.x,
            ui_state.menu_area.y,
            ui_state.menu_area.width,
            1,
        );
        let popup = ui_state.menu.popup_rect(bar_area, full_frame_area);
        // Reject empty popups for the same `Rect::intersects` zero-area reason.
        return !popup.is_empty() && popup.intersects(target);
    }
    false
}

impl Navigable for BitmapView {
    fn len(&self, app_state: &AppState) -> usize {
        Self::grid_range(app_state).item_count()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.bitmap_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let total = self.len(app_state);
        if total == 0 {
            return;
        }
        ui_state.bitmap_cursor_index =
            (ui_state.bitmap_cursor_index + amount).min(total.saturating_sub(1));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        ui_state.bitmap_cursor_index = ui_state.bitmap_cursor_index.saturating_sub(amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 5);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 5);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = self.len(app_state);
        ui_state.bitmap_cursor_index = index.min(total.saturating_sub(1));
    }

    fn item_name(&self) -> &'static str {
        "bitmap"
    }
}

impl Widget for BitmapView {
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if right_pane_mouse_inner_area(ui_state, &mouse).is_none() {
            return WidgetResult::Ignored;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                sync_bitmap_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                sync_bitmap_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }

    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let is_active = ui_state.active_pane == ActivePane::Bitmap;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let protocol_name = if let Some(picker) = &ui_state.picker {
            format!("{:?}", picker.protocol_type())
        } else {
            "None".to_string()
        };

        let title = if ui_state.bitmap_multicolor_mode {
            format!(" Bitmap (Multicolor 160×200) [{protocol_name}] ")
        } else {
            format!(" Bitmap (High-Res 320×200) [{protocol_name}] ")
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title)
            .style(
                Style::default()
                    .bg(ui_state.theme.background)
                    .fg(ui_state.theme.foreground),
            );
        let inner_area = block.inner(area);
        f.render_widget(block, area);

        if app_state.raw_data.is_empty() {
            return;
        }

        let grid = Self::grid_range(app_state);
        let bitmap_addr = grid.item_addr(ui_state.bitmap_cursor_index);
        if bitmap_addr >= grid.end_address {
            return;
        }

        // Calculate the actual data range within this 8000-byte bitmap.
        let bitmap_end = bitmap_addr + Self::PAYLOAD_BYTES;
        let data_start_in_bitmap = grid.origin.max(bitmap_addr).min(bitmap_end);
        let data_end_in_bitmap = grid.end_address.min(bitmap_end).max(data_start_in_bitmap);

        // Show the first loaded address inside the bitmap when available.
        let display_addr =
            if data_start_in_bitmap < bitmap_end && data_start_in_bitmap >= bitmap_addr {
                data_start_in_bitmap
            } else {
                bitmap_addr
            };

        let padding_bytes = data_start_in_bitmap.saturating_sub(bitmap_addr);
        let actual_bytes = data_end_in_bitmap.saturating_sub(data_start_in_bitmap);
        let total_bytes = padding_bytes + actual_bytes;

        let screen_ram_addr =
            calculate_screen_ram_addr(bitmap_addr, ui_state.bitmap_screen_ram_mode);
        let sub_header = if padding_bytes > 0 {
            format!(
                "Bitmap @ ${display_addr:04X} (aligned ${bitmap_addr:04X}, {total_bytes} bytes: {padding_bytes} padded + {actual_bytes} data), Screen RAM @ ${screen_ram_addr:04X}"
            )
        } else {
            format!(
                "Bitmap @ ${display_addr:04X} ({total_bytes} bytes), Screen RAM @ ${screen_ram_addr:04X}"
            )
        };

        f.render_widget(
            Paragraph::new(sub_header).style(Style::default().fg(ui_state.theme.comment)),
            Rect::new(inner_area.x, inner_area.y, inner_area.width, 1),
        );

        let image_height = inner_area.height.saturating_sub(5);
        let available_image_area = Rect::new(
            inner_area.x,
            inner_area.y.saturating_add(2),
            inner_area.width,
            image_height,
        );

        // Screen RAM selector
        let vic_bank = (bitmap_addr / 0x4000) * 0x4000;
        let selector_text = match ui_state.bitmap_screen_ram_mode {
            ScreenRamMode::AfterBitmap => {
                format!("Screen RAM: ◄ After Bitmap (${screen_ram_addr:04X}) ►")
            }
            ScreenRamMode::BankOffset(offset) => {
                let offset_hex = offset as usize * 0x0400;
                format!(
                    "Screen RAM: ◄ ${screen_ram_addr:04X} (Bank ${vic_bank:04X}+${offset_hex:04X}) ►"
                )
            }
        };

        f.render_widget(
            Paragraph::new(selector_text).style(Style::default().fg(ui_state.theme.foreground)),
            Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(2),
                inner_area.width,
                1,
            ),
        );

        f.render_widget(
            Paragraph::new("[s] next • [S] prev • [x] after bitmap")
                .style(Style::default().fg(ui_state.theme.comment)),
            Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(1),
                inner_area.width,
                1,
            ),
        );

        // --- ratatui-image integration with lazy caching & zero-leak ID reuse ---
        let multicolor_mode = ui_state.bitmap_multicolor_mode;
        let cache_key: crate::ui_state::BitmapCacheKey =
            (grid.origin, bitmap_addr, multicolor_mode, screen_ram_addr);

        ui_state.bitmap_cache.entry(cache_key).or_insert_with(|| {
            convert_to_dynamic_image(
                &app_state.raw_data,
                &grid,
                bitmap_addr,
                screen_ram_addr,
                multicolor_mode,
            )
        });

        if let Some(picker) = &ui_state.picker {
            // Track theme background because `Resize::resize` pads sub-cell border pixels with it.
            // Rebuild before overlay suppression so Kitty keeps its transmission sequence ready
            // for the next unoccluded frame.
            let bg = theme_color_to_rgba(ui_state.theme.background);
            let needs_rebuild =
                ui_state
                    .bitmap_protocol
                    .as_ref()
                    .is_none_or(|(cached_key, proto)| {
                        *cached_key != Some(cache_key)
                            || proto.background_color() != bg
                            || proto.protocol_kind() != picker.protocol_type()
                    });
            if needs_rebuild && let Some(img) = ui_state.bitmap_cache.get(&cache_key) {
                let prev = ui_state.bitmap_protocol.take().map(|(_, p)| p);
                let protocol = new_or_reused_protocol(picker, prev, img.clone(), bg);
                ui_state.bitmap_protocol = Some((Some(cache_key), protocol));
            }

            // Skip image rendering on tiny terminals (`inner_area.height <= 5`) so footer rows
            // stay intact.
            if available_image_area.width > 0
                && available_image_area.height > 0
                && let Some((_, proto)) = ui_state.bitmap_protocol.as_ref()
            {
                let render_area =
                    centered_image_rect(proto, &Self::BITMAP_RESIZE, available_image_area);
                // Keep `Halfblocks` active under overlays because Ratatui layers regular cells
                // cleanly. Suppress `Kitty`, `Iterm2`, and `Sixel` so raw escapes cannot bleed
                // over overlays or lose Kitty's one-shot `(0, 0)` transmission string.
                let suppress_for_overlay = picker.protocol_type() != ProtocolType::Halfblocks
                    && is_occluded_by_overlay(ui_state, f.area(), render_area);
                if !suppress_for_overlay && let Some((_, proto)) = &mut ui_state.bitmap_protocol {
                    f.render_stateful_widget(
                        StatefulImage::new().resize(Self::BITMAP_RESIZE),
                        render_area,
                        proto,
                    );
                }
            }
        } else {
            f.render_widget(
                Paragraph::new("Image rendering not supported or no picker available"),
                available_image_area,
            );
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            if !matches!(key.code, KeyCode::Char(c) if c.is_ascii_digit()) {
                sync_bitmap_to_disassembly(app_state, ui_state);
            }
            return WidgetResult::Handled;
        }

        // All view-local single-character and `Enter` bindings must explicitly guard on
        // `key.modifiers.is_empty()` (or `NONE | SHIFT` for uppercase `'S'`) because
        // `handle_key_event_active_view` delegates to the active pane BEFORE `handle_global_input`.
        // Without these guards, `'s'`, `'x'`, and `'b'` would shadow global `Ctrl+S`/`Alt+S` (Save),
        // `Ctrl+X` (Find References), and `Ctrl+B`/`Alt+B` (Bookmarks).
        match key.code {
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                // Cycle forward through the 16 bank offsets
                ui_state.bitmap_screen_ram_mode = match ui_state.bitmap_screen_ram_mode {
                    ScreenRamMode::AfterBitmap => ScreenRamMode::BankOffset(0),
                    ScreenRamMode::BankOffset(offset) => {
                        ScreenRamMode::BankOffset((offset + 1) % 16)
                    }
                };
                WidgetResult::Handled
            }
            KeyCode::Char('S')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                // Cycle backward through the 16 bank offsets
                ui_state.bitmap_screen_ram_mode = match ui_state.bitmap_screen_ram_mode {
                    ScreenRamMode::AfterBitmap => ScreenRamMode::BankOffset(15),
                    ScreenRamMode::BankOffset(offset) => {
                        if offset == 0 {
                            ScreenRamMode::BankOffset(15)
                        } else {
                            ScreenRamMode::BankOffset(offset - 1)
                        }
                    }
                };
                WidgetResult::Handled
            }
            KeyCode::Char('x') if key.modifiers.is_empty() => {
                // Set to "After Bitmap" mode
                ui_state.bitmap_screen_ram_mode = ScreenRamMode::AfterBitmap;
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::ToggleBitmapMulticolor)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                let idx = ui_state.bitmap_cursor_index;
                Self::grid_range(app_state)
                    .item_range_byte_offsets(idx, idx)
                    .map_or(WidgetResult::Ignored, |(start, end)| {
                        WidgetResult::Action(AppAction::SetBytesBlockByOffset { start, end })
                    })
            }
            KeyCode::Enter if key.modifiers.is_empty() => Self::grid_range(app_state)
                .clamped_item_addr(ui_state.bitmap_cursor_index)
                .map_or(WidgetResult::Ignored, |addr| {
                    jump_to_disassembly_at_address(app_state, ui_state, addr)
                }),
            _ => WidgetResult::Ignored,
        }
    }
}

/// Decodes an 8,000-byte C64 bitmap region and 1,000-byte Screen RAM into a `320×200` RGB image.
///
/// In multicolor mode, each `2×1` fat pixel writes two horizontal pixels so `ratatui-image`
/// can upscale directly from native `320×200` dimensions.
#[must_use]
fn convert_to_dynamic_image(
    raw_data: &[u8],
    grid: &AlignedGridRange,
    bitmap_addr: usize,
    screen_ram_addr: usize,
    multicolor: bool,
) -> DynamicImage {
    // Keep a native `320×200` canvas so `ratatui-image` handles pane upscaling with Nearest filter.
    let mut rgb_img = RgbImage::new(320, 200);
    let get_byte = |addr: usize| -> u8 { grid.byte_at(raw_data, addr) };

    if multicolor {
        // Multicolor Mode: 160x200 fat pixels
        // Each fat pixel (2x1 C64 pixels) maps to 2x1 pixels in the 320x200 image.
        for cell_y in 0..25 {
            for cell_x in 0..40 {
                let cell_idx = cell_y * 40 + cell_x;
                let current_screen_addr = screen_ram_addr + cell_idx;
                let val = get_byte(current_screen_addr);

                let (c1, c2, bg) = (
                    VIC_II_RGB[(val >> 4) as usize],
                    VIC_II_RGB[(val & 0x0F) as usize],
                    VIC_II_RGB[0],
                );
                let c3 = VIC_II_RGB[1];

                for row in 0..8 {
                    let offset = (cell_y * 320) + (cell_x * 8) + row;
                    let current_bitmap_addr = bitmap_addr + offset;
                    let byte = get_byte(current_bitmap_addr);
                    let y = (cell_y * 8 + row) as u32;

                    for fat_pix in 0..4 {
                        let shift = (3 - fat_pix) * 2;
                        let val = (byte >> shift) & 0b11;
                        let rgb = match val {
                            0b00 => bg,
                            0b01 => c1,
                            0b10 => c2,
                            0b11 => c3,
                            _ => unreachable!(),
                        };

                        let start_x = cell_x * 8 + fat_pix * 2;
                        for dx in 0..2 {
                            rgb_img.put_pixel((start_x + dx) as u32, y, Rgb(rgb));
                        }
                    }
                }
            }
        }
    } else {
        // High-Res Mode: 320x200 pixels
        // Each C64 pixel maps 1:1 to a pixel in the 320x200 image.
        for cell_y in 0..25 {
            for cell_x in 0..40 {
                let cell_idx = cell_y * 40 + cell_x;
                let current_screen_addr = screen_ram_addr + cell_idx;
                let val = get_byte(current_screen_addr);

                let (fg, bg) = (
                    VIC_II_RGB[(val >> 4) as usize],
                    VIC_II_RGB[(val & 0x0F) as usize],
                );

                for row in 0..8 {
                    let offset = (cell_y * 320) + (cell_x * 8) + row;
                    let current_bitmap_addr = bitmap_addr + offset;
                    let byte = get_byte(current_bitmap_addr);
                    let y = (cell_y * 8 + row) as u32;

                    for bit in 0..8 {
                        let shift = 7 - bit;
                        let val = (byte >> shift) & 1;
                        let rgb = if val == 1 { fg } else { bg };
                        let x = (cell_x * 8 + bit) as u32;
                        rgb_img.put_pixel(x, y, Rgb(rgb));
                    }
                }
            }
        }
    }

    DynamicImage::ImageRgb8(rgb_img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::terminal_graphics::{create_headless_picker, extract_kitty_transmit_id};
    use crate::theme::Theme;
    use crate::ui::dialog_keyboard_shortcut::ShortcutsDialog;
    use crate::ui_state::RightPane;
    use image::Rgba;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn make_bitmap_app_state(origin: u16, fill_pattern: impl Fn(usize) -> u8) -> AppState {
        let mut app_state = AppState::new();
        app_state.origin = Addr(origin);
        app_state.raw_data = (0..9000).map(fill_pattern).collect();
        app_state.block_types = vec![BlockType::DataByte; 9000];
        app_state
    }

    fn row_text(buffer: &Buffer, x_start: u16, x_end: u16, y: u16) -> String {
        (x_start..x_end)
            .map(|x| {
                buffer
                    .cell((x, y))
                    .map_or(String::new(), |c| c.symbol().to_string())
            })
            .collect()
    }

    fn area_contains_any_cell(
        buffer: &Buffer,
        area: Rect,
        predicate: impl Fn(&str) -> bool,
    ) -> bool {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buffer.cell((x, y))
                    && predicate(cell.symbol())
                {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn test_bitmap_view_origin_in_trailing_192_bytes_clamps_padding_and_ignores_b() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1F50);
        app_state.raw_data = vec![0xEA; 256];
        app_state.block_types = vec![BlockType::Code; 256];
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.bitmap_cursor_index = 0;

        let mut view = BitmapView;

        // Pressing 'b' on bitmap 0 (whose 8,000-byte payload ends at $1F40 <= origin $1F50)
        // must be ignored and must not mutate byte 0.
        let b_res = view.handle_input(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(b_res, WidgetResult::Ignored);

        // Rendering bitmap 0 must clamp padding to 8000 bytes and lazily cache only 1 entry.
        let area = Rect::new(0, 0, 90, 20);
        let backend = TestBackend::new(90, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                view.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        assert_eq!(
            ui_state.bitmap_cache.len(),
            1,
            "Render must lazily insert only the active screen-RAM configuration"
        );

        let buffer = terminal.backend().buffer();
        let header_row = row_text(buffer, 1, 89, 1);
        assert!(
            header_row.contains("8000 bytes: 8000 padded + 0 data"),
            "Expected padding clamped to 8000 bytes, got: '{header_row}'"
        );

        // Pressing Enter on bitmap 0 must clamp to origin ($1F50) and jump to Disassembly.
        let enter_res = view.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(enter_res, WidgetResult::Handled);
        assert_eq!(ui_state.active_pane, ActivePane::Disassembly);
        assert_eq!(
            app_state.disassembly[ui_state.cursor_index].address,
            Addr(0x1F50)
        );
    }

    #[test]
    fn test_convert_to_dynamic_image_is_native_320x200() {
        let mut app_state = make_bitmap_app_state(0x2000, |_| 0x00);
        // Cell (0, 0) row 0: bits 1,0,1,0,1,1,0,0 -> 0xAC; Screen RAM = 0x36 (Cyan / Blue)
        app_state.raw_data[0] = 0b1010_1100;
        app_state.raw_data[8000] = 0x36;

        // Cell (39, 24) row 7 (offset 7999): bits 0,0,0,1,1,0,1,1 -> 0x1B; Screen RAM[999] = 0x45
        app_state.raw_data[7999] = 0b0001_1011;
        app_state.raw_data[8999] = 0x45;

        let grid = BitmapView::grid_range(&app_state);
        let hires =
            convert_to_dynamic_image(&app_state.raw_data, &grid, 0x2000, 0x2000 + 8000, false);
        assert_eq!(hires.width(), 320);
        assert_eq!(hires.height(), 200);
        assert_eq!(hires.as_bytes().len(), 192_000);

        let hires_rgb = hires.to_rgb8();
        let expected_hires_0 = [
            VIC_II_RGB[3],
            VIC_II_RGB[6],
            VIC_II_RGB[3],
            VIC_II_RGB[6],
            VIC_II_RGB[3],
            VIC_II_RGB[3],
            VIC_II_RGB[6],
            VIC_II_RGB[6],
        ];
        for (bit, expected) in expected_hires_0.iter().enumerate() {
            assert_eq!(hires_rgb.get_pixel(bit as u32, 0), &Rgb(*expected));
        }

        let expected_hires_last = [
            VIC_II_RGB[5],
            VIC_II_RGB[5],
            VIC_II_RGB[5],
            VIC_II_RGB[4],
            VIC_II_RGB[4],
            VIC_II_RGB[5],
            VIC_II_RGB[4],
            VIC_II_RGB[4],
        ];
        for (bit, expected) in expected_hires_last.iter().enumerate() {
            assert_eq!(hires_rgb.get_pixel(312 + bit as u32, 199), &Rgb(*expected));
        }

        // Multicolor: byte 0 = 0b00_01_10_11 -> 0x1B (exercises all 4 bit pairs in one byte)
        app_state.raw_data[0] = 0b0001_1011;
        let multi =
            convert_to_dynamic_image(&app_state.raw_data, &grid, 0x2000, 0x2000 + 8000, true);
        assert_eq!(multi.width(), 320);
        assert_eq!(multi.height(), 200);
        assert_eq!(multi.as_bytes().len(), 192_000);

        let multi_rgb = multi.to_rgb8();
        // Scope lock: 0b00 -> VIC_II_RGB[0], 0b01 -> hi (3), 0b10 -> lo (6), 0b11 -> VIC_II_RGB[1]
        let expected_fat_0 = [VIC_II_RGB[0], VIC_II_RGB[3], VIC_II_RGB[6], VIC_II_RGB[1]];
        for (fat_pix, expected) in expected_fat_0.iter().enumerate() {
            let x = (fat_pix * 2) as u32;
            assert_eq!(multi_rgb.get_pixel(x, 0), &Rgb(*expected));
            assert_eq!(multi_rgb.get_pixel(x + 1, 0), &Rgb(*expected));
        }
        // Verify bottom-right cell (39, 24) row 7 uses Screen RAM[999] = 0x45 too
        let expected_fat_last = [VIC_II_RGB[0], VIC_II_RGB[4], VIC_II_RGB[5], VIC_II_RGB[1]];
        for (fat_pix, expected) in expected_fat_last.iter().enumerate() {
            let x = (312 + fat_pix * 2) as u32;
            assert_eq!(multi_rgb.get_pixel(x, 199), &Rgb(*expected));
            assert_eq!(multi_rgb.get_pixel(x + 1, 199), &Rgb(*expected));
        }
    }

    #[test]
    fn test_bitmap_view_retina_upscale_and_tiny_terminal_guard() {
        let app_state = make_bitmap_app_state(0x2000, |i| if i < 8000 { 0xAA } else { 0x10 });
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.picker = Some(picker);
        ui_state.bitmap_cursor_index = 0;

        let view = BitmapView;
        let large_area = Rect::new(0, 0, 120, 45);
        let mut large_term = Terminal::new(TestBackend::new(120, 45)).unwrap();
        large_term
            .draw(|f| view.render(f, large_area, &app_state, &mut ui_state))
            .unwrap();

        let available_image_area = Rect::new(1, 3, 118, 38);
        let proto = &ui_state
            .bitmap_protocol
            .as_ref()
            .expect("bitmap_protocol should be populated")
            .1;
        let render_area =
            centered_image_rect(proto, &BitmapView::BITMAP_RESIZE, available_image_area);
        assert_eq!(render_area.width, 118);
        assert_eq!(render_area.height, 37);

        let large_buf = large_term.backend().buffer();
        let title_row = row_text(large_buf, 0, 120, 0);
        assert!(
            title_row.contains("[Kitty]"),
            "Expected [Kitty] in pane title, got: '{title_row}'"
        );
        // Verify Kitty placeholder cells actually populate past the legacy 64x20 cap.
        assert!(
            large_buf[(100, 30)].symbol().contains('\u{10EEEE}'),
            "Expected upscaled Kitty placeholder beyond 64x20 at (100, 30)"
        );

        // Tiny terminal guard: inner_area.height == 4 -> image_height == 0; zero panic, footer intact.
        let tiny_area = Rect::new(0, 0, 40, 6);
        let mut tiny_term = Terminal::new(TestBackend::new(40, 6)).unwrap();
        tiny_term
            .draw(|f| view.render(f, tiny_area, &app_state, &mut ui_state))
            .unwrap();
        let tiny_buf = tiny_term.backend().buffer();
        assert!(row_text(tiny_buf, 1, 39, 3).contains("Screen RAM:"));
        assert!(row_text(tiny_buf, 1, 39, 4).contains("[s] next"));

        // Picker == None fallback message and [None] title.
        ui_state.picker = None;
        let mut fallback_term = Terminal::new(TestBackend::new(120, 45)).unwrap();
        fallback_term
            .draw(|f| view.render(f, large_area, &app_state, &mut ui_state))
            .unwrap();
        let fallback_buf = fallback_term.backend().buffer();
        assert!(row_text(fallback_buf, 0, 120, 0).contains("[None]"));
        assert!(
            row_text(fallback_buf, 1, 119, 3)
                .contains("Image rendering not supported or no picker available")
        );
    }

    #[test]
    fn test_bitmap_view_reactive_theme_invalidation_preserves_kitty_id() {
        let app_state = make_bitmap_app_state(0x2000, |i| if i < 8000 { 0x55 } else { 0x10 });
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let mut ui_state = UIState::new(Theme::from_name("Dracula"));
        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.picker = Some(picker);

        let view = BitmapView;
        let area = Rect::new(0, 0, 80, 30);
        let mut term1 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term1
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();

        let available = Rect::new(1, 3, 78, 23);
        let (render_area, id1, fg1) = {
            let proto = &ui_state
                .bitmap_protocol
                .as_ref()
                .expect("protocol should exist")
                .1;
            assert_eq!(proto.background_color(), Some(Rgba([40, 42, 54, 255])));
            let render_area = centered_image_rect(proto, &BitmapView::BITMAP_RESIZE, available);
            let buf1 = term1.backend().buffer();
            let sym1 = buf1[(render_area.x, render_area.y)].symbol();
            let id1 = extract_kitty_transmit_id(sym1)
                .expect("initial Kitty render should emit transmission escape");
            (render_area, id1, buf1[(render_area.x, render_area.y)].fg)
        };
        let (rx, ry) = (render_area.x, render_area.y);
        assert!(render_area.height > 1);

        // Live theme switch flips to `id1 ^ 1`, invalidating all rows in `Buffer::diff`.
        ui_state.theme = Theme::from_name("Solarized Light");
        let mut term2 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term2
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();

        assert_eq!(ui_state.bitmap_cache.len(), 1);
        assert_eq!(
            ui_state
                .bitmap_protocol
                .as_ref()
                .expect("protocol should exist")
                .1
                .background_color(),
            Some(Rgba([253, 246, 227, 255]))
        );
        let buf2 = term2.backend().buffer();
        let id2 = extract_kitty_transmit_id(buf2[(rx, ry)].symbol())
            .expect("rebuilt Kitty protocol should emit updated transmission");
        assert_ne!(buf2[(rx, ry)].fg, fg1);
        assert_eq!(id2, id1 ^ 1);
        let diff12 = term1.backend().buffer().diff(buf2);
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                assert!(
                    diff12.iter().any(|(dx, dy, _)| *dx == x && *dy == y),
                    "cell ({x}, {y}) must diff after Kitty ID flip"
                );
            }
        }

        // Toggle multicolor + Screen RAM offset: cache grows to 2 while flipping back to `id1`.
        ui_state.bitmap_multicolor_mode = true;
        ui_state.bitmap_screen_ram_mode = ScreenRamMode::BankOffset(2);
        let mut term3 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term3
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();

        assert_eq!(ui_state.bitmap_cache.len(), 2);
        let buf3 = term3.backend().buffer();
        let id3 = extract_kitty_transmit_id(buf3[(rx, ry)].symbol())
            .expect("mode switch should reuse bounded Kitty ID pair");
        assert_eq!(buf3[(rx, ry)].fg, fg1);
        assert_eq!(id3, id1);
        let diff23 = term2.backend().buffer().diff(buf3);
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                assert!(
                    diff23.iter().any(|(dx, dy, _)| *dx == x && *dy == y),
                    "cell ({x}, {y}) must diff on bounded Kitty ID cycle back"
                );
            }
        }
    }

    #[test]
    fn test_bitmap_view_origin_change_and_invalidation_preserve_kitty_id() {
        let mut app_state = make_bitmap_app_state(0x2000, |i| if i < 8000 { 0xAA } else { 0x10 });
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.picker = Some(picker);

        let view = BitmapView;
        let area = Rect::new(0, 0, 80, 30);
        let available = Rect::new(1, 3, 78, 23);

        let mut term1 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term1
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        let render_area = centered_image_rect(
            &ui_state.bitmap_protocol.as_ref().unwrap().1,
            &BitmapView::BITMAP_RESIZE,
            available,
        );
        let (rx, ry) = (render_area.x, render_area.y);
        let buf1 = term1.backend().buffer();
        let id1 = extract_kitty_transmit_id(buf1[(rx, ry)].symbol())
            .expect("initial Kitty render should emit transmission escape");
        let fg1 = buf1[(rx, ry)].fg;

        // Changing origin directly (as Undo/Redo or MCP does) keeps the same $2000 bucket key
        // prefix while differentiating cache_key by origin, re-encoding under `id1 ^ 1`.
        app_state.origin = Addr(0x1F50);
        let mut term2 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term2
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        assert_eq!(ui_state.bitmap_cache.len(), 2);
        let buf2 = term2.backend().buffer();
        let id2 = extract_kitty_transmit_id(buf2[(rx, ry)].symbol())
            .expect("origin change must invalidate and transmit updated Kitty image");
        assert_ne!(buf2[(rx, ry)].fg, fg1);
        assert_eq!(id2, id1 ^ 1);

        // Mutating raw_data at the same key and calling `invalidate_bitmap_cache()` forces
        // re-encoding and cycles back to `id1` without leaking GPU IDs.
        app_state.raw_data[0] = 0xFF;
        ui_state.invalidate_bitmap_cache();
        assert!(ui_state.bitmap_cache.is_empty());
        assert!(ui_state.bitmap_protocol.is_some());
        assert_eq!(ui_state.bitmap_protocol.as_ref().unwrap().0, None);

        let mut term3 = Terminal::new(TestBackend::new(80, 30)).unwrap();
        term3
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        assert_eq!(ui_state.bitmap_cache.len(), 1);
        assert_eq!(
            ui_state.bitmap_protocol.as_ref().unwrap().0,
            Some((0x1F50, 0x0000, false, 8000))
        );
        let buf3 = term3.backend().buffer();
        let id3 = extract_kitty_transmit_id(buf3[(rx, ry)].symbol())
            .expect("invalidation must transmit re-encoded Kitty image");
        assert_eq!(buf3[(rx, ry)].fg, fg1);
        assert_eq!(id3, id1);
    }

    #[test]
    fn test_bitmap_view_multicolor_toggle_kitty_diffs_all_rows() {
        let app_state = make_bitmap_app_state(0x2000, |i| if i < 8000 { 0xA5 } else { 0x36 });
        let mut picker = create_headless_picker();
        picker.set_protocol_type(ProtocolType::Kitty);

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Bitmap;
        ui_state.right_pane = RightPane::Bitmap;
        ui_state.picker = Some(picker);

        let view = BitmapView;
        let area = Rect::new(0, 0, 120, 40);
        let available = Rect::new(1, 3, 118, 33);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();

        // 1. Frame 1 (High-Res).
        terminal
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        let render_area = centered_image_rect(
            &ui_state.bitmap_protocol.as_ref().unwrap().1,
            &BitmapView::BITMAP_RESIZE,
            available,
        );
        assert!(render_area.height > 1);
        let buf_hires = terminal.backend().buffer().clone();
        let id1 = extract_kitty_transmit_id(buf_hires[(render_area.x, render_area.y)].symbol())
            .expect("High-Res frame must emit initial Kitty transmission");

        // 2. Toggle Multicolor ('m') on the same persistent Terminal: every row/col must diff.
        ui_state.bitmap_multicolor_mode = true;
        terminal
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        let buf_multi = terminal.backend().buffer().clone();
        let id2 = extract_kitty_transmit_id(buf_multi[(render_area.x, render_area.y)].symbol())
            .expect("Multicolor frame must emit Kitty transmission");
        assert_eq!(id2, id1 ^ 1);
        let diff1 = buf_hires.diff(&buf_multi);
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                assert!(
                    diff1.iter().any(|(dx, dy, _)| *dx == x && *dy == y),
                    "High-Res -> Multicolor diff missing cell ({x}, {y})"
                );
            }
        }

        // 3. Toggle back to High-Res ('m' again): cycles back to `id1` and diffs every cell.
        ui_state.bitmap_multicolor_mode = false;
        terminal
            .draw(|f| view.render(f, area, &app_state, &mut ui_state))
            .unwrap();
        let buf_hires_again = terminal.backend().buffer();
        let id3 =
            extract_kitty_transmit_id(buf_hires_again[(render_area.x, render_area.y)].symbol())
                .expect("High-Res toggle back must emit Kitty transmission");
        assert_eq!(id3, id1);
        let diff2 = buf_multi.diff(buf_hires_again);
        for y in render_area.top()..render_area.bottom() {
            for x in render_area.left()..render_area.right() {
                assert!(
                    diff2.iter().any(|(dx, dy, _)| *dx == x && *dy == y),
                    "Multicolor -> High-Res diff missing cell ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn test_bitmap_view_overlay_occlusion_detection() {
        let mut ui_state = UIState::new(Theme::default());
        ui_state.menu_area = Rect::new(0, 0, 120, 1);
        let full_frame = Rect::new(0, 0, 120, 30);
        let target = Rect::new(45, 4, 70, 20);

        // (1) Closed menu & no dialog -> false
        assert!(!is_occluded_by_overlay(&ui_state, full_frame, target));

        // (2) Active dialog -> true
        ui_state.active_dialog = Some(Box::new(ShortcutsDialog::new()));
        assert!(is_occluded_by_overlay(&ui_state, full_frame, target));

        // (3) Open "File" menu on left pane (x < 45) -> false
        ui_state.active_dialog = None;
        ui_state.menu.active = true;
        ui_state.menu.selected_category = 0; // File
        assert!(!is_occluded_by_overlay(&ui_state, full_frame, target));

        // (4) Open "View" (4) or "Help" (6) overlapping right-pane target -> true
        ui_state.menu.selected_category = 4; // View
        assert!(is_occluded_by_overlay(&ui_state, full_frame, target));
        ui_state.menu.selected_category = 6; // Help
        assert!(is_occluded_by_overlay(&ui_state, full_frame, target));

        // (5) Empty target inside open "File" popup (and with active dialog) -> false
        ui_state.menu.selected_category = 0;
        assert!(!is_occluded_by_overlay(
            &ui_state,
            full_frame,
            Rect::new(10, 5, 0, 0)
        ));
        assert!(!is_occluded_by_overlay(
            &ui_state,
            full_frame,
            Rect::new(10, 5, 0, 10)
        ));
        assert!(!is_occluded_by_overlay(
            &ui_state,
            full_frame,
            Rect::new(10, 5, 10, 0)
        ));
        ui_state.active_dialog = Some(Box::new(ShortcutsDialog::new()));
        assert!(!is_occluded_by_overlay(
            &ui_state,
            full_frame,
            Rect::new(10, 5, 0, 0)
        ));

        // (6) Empty popup_rect (0x0 full_frame or 1-row-tall full_frame) with non-empty target -> false
        ui_state.active_dialog = None;
        assert!(!is_occluded_by_overlay(
            &ui_state,
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 10, 10)
        ));
        assert!(!is_occluded_by_overlay(
            &ui_state,
            Rect::new(0, 0, 20, 1),
            Rect::new(0, 0, 20, 10)
        ));
    }

    #[test]
    fn test_bitmap_view_overlay_suppression_vs_halfblocks_non_suppression() {
        let app_state = make_bitmap_app_state(0x2000, |i| {
            if i < 8000 {
                if i % 2 == 0 { 0xFF } else { 0x00 }
            } else {
                0x10
            }
        });
        let view = BitmapView;
        let full_area = Rect::new(0, 0, 120, 30);
        let available_image_area = Rect::new(1, 3, 118, 23);

        // Part A1: Kitty suppresses on initial frame when active_dialog is open, without
        // consuming the one-shot transmit string; resumes immediately when closed.
        let mut kitty_picker = create_headless_picker();
        kitty_picker.set_protocol_type(ProtocolType::Kitty);
        let mut ui_kitty = UIState::new(Theme::default());
        ui_kitty.active_pane = ActivePane::Bitmap;
        ui_kitty.picker = Some(kitty_picker);
        ui_kitty.menu_area = Rect::new(0, 0, 120, 1);
        ui_kitty.active_dialog = Some(Box::new(ShortcutsDialog::new()));

        let mut kitty_suppressed_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        kitty_suppressed_term
            .draw(|f| view.render(f, full_area, &app_state, &mut ui_kitty))
            .unwrap();
        let kitty_suppressed_buf = kitty_suppressed_term.backend().buffer();
        assert!(!area_contains_any_cell(
            kitty_suppressed_buf,
            available_image_area,
            |s| s.contains('\u{10EEEE}')
                || s.contains("]1337;File=inline=1;")
                || s.contains('\x1b')
        ));

        ui_kitty.active_dialog = None;
        let mut kitty_resumed_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        kitty_resumed_term
            .draw(|f| view.render(f, full_area, &app_state, &mut ui_kitty))
            .unwrap();
        let kitty_resumed_buf = kitty_resumed_term.backend().buffer();
        let kitty_render_area = centered_image_rect(
            &ui_kitty.bitmap_protocol.as_ref().unwrap().1,
            &BitmapView::BITMAP_RESIZE,
            available_image_area,
        );
        let resumed_sym = kitty_resumed_buf[(kitty_render_area.x, kitty_render_area.y)].symbol();
        assert!(resumed_sym.contains('\u{10EEEE}'));
        assert!(
            extract_kitty_transmit_id(resumed_sym).is_some(),
            "One-shot Kitty transmit escape must remain intact until first unoccluded frame"
        );

        // Part A2: Iterm2 & Sixel suppress raw escape payloads under active_dialog and emit once closed.
        for proto_type in [ProtocolType::Iterm2, ProtocolType::Sixel] {
            let mut picker = create_headless_picker();
            picker.set_protocol_type(proto_type);
            let mut ui_state = UIState::new(Theme::default());
            ui_state.active_pane = ActivePane::Bitmap;
            ui_state.picker = Some(picker);
            ui_state.menu_area = Rect::new(0, 0, 120, 1);
            ui_state.active_dialog = Some(Box::new(ShortcutsDialog::new()));

            let mut suppressed_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
            suppressed_term
                .draw(|f| view.render(f, full_area, &app_state, &mut ui_state))
                .unwrap();
            assert!(!area_contains_any_cell(
                suppressed_term.backend().buffer(),
                available_image_area,
                |s| s.contains('\u{10EEEE}')
                    || s.contains("]1337;File=inline=1;")
                    || s.contains('\x1b')
            ));

            ui_state.active_dialog = None;
            let mut visible_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
            visible_term
                .draw(|f| view.render(f, full_area, &app_state, &mut ui_state))
                .unwrap();
            let buf = visible_term.backend().buffer();
            let render_area = centered_image_rect(
                &ui_state.bitmap_protocol.as_ref().unwrap().1,
                &BitmapView::BITMAP_RESIZE,
                available_image_area,
            );
            let top_left_sym = buf[(render_area.x, render_area.y)].symbol();
            if proto_type == ProtocolType::Iterm2 {
                assert!(top_left_sym.contains("]1337;File=inline=1;"));
                assert!(!top_left_sym.contains("px;"));
                assert!(top_left_sym.contains(&format!(
                    ";width={};height={};preserveAspectRatio=0;doNotMoveCursor=1:",
                    render_area.width, render_area.height
                )));
            } else {
                assert!(top_left_sym.contains('\x1b'));
            }
        }

        // Part A3: Overlapping Help menu suppresses Kitty; non-overlapping File menu allows Kitty.
        let mut kitty_menu_picker = create_headless_picker();
        kitty_menu_picker.set_protocol_type(ProtocolType::Kitty);
        let mut ui_menu = UIState::new(Theme::default());
        ui_menu.dismiss_logo = true;
        ui_menu.right_pane = RightPane::Bitmap;
        ui_menu.active_pane = ActivePane::Bitmap;
        ui_menu.picker = Some(kitty_menu_picker);
        ui_menu.menu.active = true;
        ui_menu.menu.selected_category = 6; // Help (overlaps right pane BitmapView)

        let mut help_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        help_term
            .draw(|f| crate::ui::ui(f, &app_state, &mut ui_menu))
            .unwrap();
        let right_inner = Rect::new(41, 5, 78, 20);
        assert!(!area_contains_any_cell(
            help_term.backend().buffer(),
            right_inner,
            |s| s.contains('\u{10EEEE}')
        ));

        ui_menu.menu.selected_category = 0; // File (stays on left pane, x < 41)
        let mut file_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        file_term
            .draw(|f| crate::ui::ui(f, &app_state, &mut ui_menu))
            .unwrap();
        assert!(area_contains_any_cell(
            file_term.backend().buffer(),
            right_inner,
            |s| s.contains('\u{10EEEE}')
        ));

        // Part B: Halfblocks is NEVER suppressed under active_dialog, and full-frame ui::ui
        // overwrites the centered modal region while unoccluded Halfblocks columns stay visible.
        let mut ui_hb = UIState::new(Theme::default());
        ui_hb.dismiss_logo = true;
        ui_hb.right_pane = RightPane::Bitmap;
        ui_hb.active_pane = ActivePane::Bitmap;
        ui_hb.picker = Some(create_headless_picker());
        ui_hb.menu_area = Rect::new(0, 0, 120, 1);
        ui_hb.active_dialog = Some(Box::new(ShortcutsDialog::new()));

        let mut hb_direct_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        hb_direct_term
            .draw(|f| view.render(f, full_area, &app_state, &mut ui_hb))
            .unwrap();
        let hb_direct_buf = hb_direct_term.backend().buffer();
        assert!(area_contains_any_cell(
            hb_direct_buf,
            available_image_area,
            |s| s == "▀" || s == "▄"
        ));

        let mut hb_full_term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        hb_full_term
            .draw(|f| crate::ui::ui(f, &app_state, &mut ui_hb))
            .unwrap();
        let hb_full_buf = hb_full_term.backend().buffer();
        let dialog_area = ui_hb.active_dialog_area;
        assert!(dialog_area.width > 0 && dialog_area.height > 0);
        // Modal center was cleared and painted with dialog border/title (no halfblocks inside dialog).
        assert!(!area_contains_any_cell(hb_full_buf, dialog_area, |s| {
            s == "▀" || s == "▄"
        }));
        assert!(
            row_text(
                hb_full_buf,
                dialog_area.x,
                dialog_area.right(),
                dialog_area.y
            )
            .contains("Keyboard Shortcuts")
        );
        // Unoccluded right-hand columns of BitmapView retain Halfblocks cells.
        let unoccluded_right = Rect::new(
            dialog_area.right(),
            5,
            119u16.saturating_sub(dialog_area.right()),
            20,
        );
        assert!(unoccluded_right.width > 0);
        assert!(area_contains_any_cell(hb_full_buf, unoccluded_right, |s| s
            == "▀"
            || s == "▄"));
    }

    #[test]
    fn test_bitmap_view_navigation_and_scroll_sync_disassembly() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x0801), vec![0xEA; 0x5000])
            .unwrap();

        for sync_enabled in [false, true] {
            app_state.system_config.sync_bitmap_view = sync_enabled;

            let mut ui_state = UIState::new(Theme::default());
            ui_state.active_pane = ActivePane::Bitmap;
            ui_state.right_pane = RightPane::Bitmap;
            ui_state.right_pane_area = Rect::new(0, 0, 80, 30);

            let mut view = BitmapView;
            let press =
                |view: &mut BitmapView, code: KeyCode, app: &mut AppState, ui: &mut UIState| {
                    let res = view.handle_input(KeyEvent::new(code, KeyModifiers::NONE), app, ui);
                    assert_eq!(res, WidgetResult::Handled);
                };
            let disasm_addr =
                |app: &AppState, ui: &UIState| -> Addr { app.disassembly[ui.cursor_index].address };

            // Start at slot 0 ($0000 clamped to $0801); verify selection_start clears on nav.
            ui_state.selection_start = Some(5);
            press(&mut view, KeyCode::Down, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 1);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x2000));
            assert_eq!(ui_state.selection_start, None);

            press(&mut view, KeyCode::Char('j'), &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 2);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x4000));

            press(&mut view, KeyCode::Up, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 1);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x2000));

            press(&mut view, KeyCode::Char('k'), &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 0);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x0801));

            press(&mut view, KeyCode::PageDown, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 2);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x4000));

            press(&mut view, KeyCode::PageUp, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 0);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x0801));

            press(&mut view, KeyCode::End, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 2);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x4000));

            press(&mut view, KeyCode::Home, &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 0);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x0801));

            // Typing numeric prefix '2' must NOT move Disassembly before 'G' is pressed.
            ui_state.cursor_index = app_state
                .get_line_index_containing_address(Addr(0x4000))
                .unwrap();
            press(&mut view, KeyCode::Char('2'), &mut app_state, &mut ui_state);
            assert_eq!(ui_state.input_buffer, "2");
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x4000));

            // Pressing 'G' completes "2G" -> slot 1 ($2000) and syncs Disassembly.
            press(&mut view, KeyCode::Char('G'), &mut app_state, &mut ui_state);
            assert_eq!(ui_state.bitmap_cursor_index, 1);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x2000));

            // Mouse wheel ScrollUp / ScrollDown syncs Disassembly as well.
            ui_state.bitmap_cursor_index = 0;
            let scroll_down = MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 10,
                row: 10,
                modifiers: KeyModifiers::empty(),
            };
            assert_eq!(
                view.handle_mouse(scroll_down, &mut app_state, &mut ui_state),
                WidgetResult::Handled
            );
            assert_eq!(ui_state.bitmap_cursor_index, 2);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x4000));

            let scroll_up = MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 10,
                row: 10,
                modifiers: KeyModifiers::empty(),
            };
            assert_eq!(
                view.handle_mouse(scroll_up, &mut app_state, &mut ui_state),
                WidgetResult::Handled
            );
            assert_eq!(ui_state.bitmap_cursor_index, 0);
            assert_eq!(disasm_addr(&app_state, &ui_state), Addr(0x0801));
        }
    }
}
