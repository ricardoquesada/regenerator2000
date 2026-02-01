use crate::state::AppState;
use crate::ui::graphics_common::VIC_II_RGB;
use crate::ui_state::{ActivePane, MenuAction, ScreenRamMode, UIState};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use image::{DynamicImage, Rgb, RgbImage};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::StatefulImage;

use crate::ui::navigable::{Navigable, handle_nav_input};
use crate::ui::widget::{Widget, WidgetResult};

pub struct BitmapView;

/// Calculate screen RAM address based on mode and bitmap address
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

impl Navigable for BitmapView {
    fn len(&self, app_state: &AppState) -> usize {
        // Bitmaps must be aligned to 8192-byte ($2000) boundaries
        // We align to the floor to support partial bitmaps (padding with zeros before origin)
        let origin = app_state.origin as usize;
        let aligned_origin = (origin / 8192) * 8192;
        let end_address = origin + app_state.raw_data.len();

        let total_bytes = end_address.saturating_sub(aligned_origin);
        (total_bytes as f64 / 8192.0).ceil() as usize
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

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let total = self.len(app_state);
        let target = if input == 0 {
            total.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(total.saturating_sub(1))
        };
        ui_state.bitmap_cursor_index = target;
    }

    fn item_name(&self) -> &str {
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
        let area = ui_state.right_pane_area;
        let inner_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        if mouse.column < inner_area.x
            || mouse.column >= inner_area.x + inner_area.width
            || mouse.row < inner_area.y
            || mouse.row >= inner_area.y + inner_area.height
        {
            return WidgetResult::Ignored;
        }

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
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
            format!(" Bitmap (Multicolor 160×200) [{}] ", protocol_name)
        } else {
            format!(" Bitmap (High-Res 320×200) [{}] ", protocol_name)
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

        let origin = app_state.origin as usize;
        // Align to floor boundary to support partial bitmaps (bytes before origin are zeros)
        let aligned_origin = (origin / 8192) * 8192;
        let bitmap_addr = aligned_origin + (ui_state.bitmap_cursor_index * 8192);

        let buffer_end_address = origin + app_state.raw_data.len();
        if bitmap_addr >= buffer_end_address {
            return;
        }

        // Calculate the actual data range within this bitmap
        let bitmap_end = bitmap_addr + 8000;
        let data_start_in_bitmap = origin.max(bitmap_addr);
        let data_end_in_bitmap = buffer_end_address.min(bitmap_end);

        // If this bitmap contains actual data, show where it starts
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
                "Bitmap @ ${:04X} (aligned ${:04X}, {} bytes: {} padded + {} data), Screen RAM @ ${:04X}",
                display_addr,
                bitmap_addr,
                total_bytes,
                padding_bytes,
                actual_bytes,
                screen_ram_addr
            )
        } else {
            format!(
                "Bitmap @ ${:04X} ({} bytes), Screen RAM @ ${:04X}",
                display_addr, total_bytes, screen_ram_addr
            )
        };

        f.render_widget(
            Paragraph::new(sub_header).style(Style::default().fg(ui_state.theme.comment)),
            Rect::new(inner_area.x, inner_area.y, inner_area.width, 1),
        );

        // Calculate width based on image aspect ratio (320:200 = 8:5) and terminal cell ratio (~1:2)
        // For H rows displaying 200 logical pixels, we need W columns displaying 320 logical pixels
        // Terminal cells are roughly 1:2 (width:height), so W = H * (320/200) * 2 = H * 3.2
        let image_height = inner_area.height.saturating_sub(6);
        let image_width = ((image_height as f32) * 3.2) as u16;
        let image_width = image_width.min(inner_area.width);
        let image_area = Rect::new(inner_area.x, inner_area.y + 2, image_width, image_height);

        // Screen RAM selector
        let vic_bank = (bitmap_addr / 0x4000) * 0x4000;
        let selector_text = match ui_state.bitmap_screen_ram_mode {
            ScreenRamMode::AfterBitmap => {
                format!("Screen RAM: ◄ After Bitmap (${:04X}) ►", screen_ram_addr)
            }
            ScreenRamMode::BankOffset(offset) => {
                let offset_hex = offset as usize * 0x0400;
                format!(
                    "Screen RAM: ◄ ${:04X} (Bank ${:04X}+${:04X}) ►",
                    screen_ram_addr, vic_bank, offset_hex
                )
            }
        };

        f.render_widget(
            Paragraph::new(selector_text).style(Style::default().fg(ui_state.theme.foreground)),
            Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(3),
                inner_area.width,
                1,
            ),
        );

        f.render_widget(
            Paragraph::new("[s] next • [S] prev • [x] after bitmap")
                .style(Style::default().fg(ui_state.theme.comment)),
            Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(2),
                inner_area.width,
                1,
            ),
        );

        f.render_widget(
            Paragraph::new("WARNING: This view makes the application less responsive.")
                .style(Style::default().fg(ui_state.theme.error_fg)),
            Rect::new(
                inner_area.x,
                inner_area.bottom().saturating_sub(1),
                inner_area.width,
                1,
            ),
        );

        // --- ratatui-image integration with caching ---

        // Pre-cache all 17 possible screen RAM configurations for instant switching
        // (16 bank offsets + 1 after bitmap)
        for offset in 0..16 {
            let test_screen_ram =
                calculate_screen_ram_addr(bitmap_addr, ScreenRamMode::BankOffset(offset));
            let test_key = (
                bitmap_addr,
                ui_state.bitmap_multicolor_mode,
                test_screen_ram,
            );
            if !ui_state.bitmap_cache.contains_key(&test_key) {
                let img = convert_to_dynamic_image(
                    &app_state.raw_data,
                    origin,
                    bitmap_addr,
                    test_screen_ram,
                    ui_state.bitmap_multicolor_mode,
                );
                ui_state.bitmap_cache.insert(test_key, img);
            }
        }

        // Also pre-cache "after bitmap" mode
        let after_bitmap_screen_ram =
            calculate_screen_ram_addr(bitmap_addr, ScreenRamMode::AfterBitmap);
        let after_bitmap_key = (
            bitmap_addr,
            ui_state.bitmap_multicolor_mode,
            after_bitmap_screen_ram,
        );
        if !ui_state.bitmap_cache.contains_key(&after_bitmap_key) {
            let img = convert_to_dynamic_image(
                &app_state.raw_data,
                origin,
                bitmap_addr,
                after_bitmap_screen_ram,
                ui_state.bitmap_multicolor_mode,
            );
            ui_state.bitmap_cache.insert(after_bitmap_key, img);
        }

        // Create cache key from bitmap address, multicolor mode, and screen RAM address
        let cache_key = (
            bitmap_addr,
            ui_state.bitmap_multicolor_mode,
            screen_ram_addr,
        );

        // Get the cached image (guaranteed to exist now)
        if let Some(img) = ui_state.bitmap_cache.get(&cache_key) {
            if let Some(picker) = &ui_state.picker {
                let mut protocol = picker.new_resize_protocol(img.clone());
                let widget = StatefulImage::new();
                f.render_stateful_widget(widget, image_area, &mut protocol);
            } else {
                f.render_widget(
                    Paragraph::new("Image rendering not supported or no picker available"),
                    image_area,
                );
            }
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            KeyCode::Char('s') => {
                // Cycle forward through the 16 bank offsets
                ui_state.bitmap_screen_ram_mode = match ui_state.bitmap_screen_ram_mode {
                    ScreenRamMode::AfterBitmap => ScreenRamMode::BankOffset(0),
                    ScreenRamMode::BankOffset(offset) => {
                        ScreenRamMode::BankOffset((offset + 1) % 16)
                    }
                };
                WidgetResult::Handled
            }
            KeyCode::Char('S') => {
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
            KeyCode::Char('x') => {
                // Set to "After Bitmap" mode
                ui_state.bitmap_screen_ram_mode = ScreenRamMode::AfterBitmap;
                WidgetResult::Handled
            }
            KeyCode::Char('m') => WidgetResult::Action(MenuAction::ToggleBitmapMulticolor),
            KeyCode::Char('B') => {
                // Convert current bitmap to bytes block (8000 bytes per bitmap)
                let origin = app_state.origin as usize;
                // Align to floor boundary to support partial bitmaps
                let aligned_origin = (origin / 8192) * 8192;
                let bitmap_addr = aligned_origin + (ui_state.bitmap_cursor_index * 8192);
                let end_address = origin + app_state.raw_data.len();

                // Calculate the byte offset range within raw_data (8000 bytes for bitmap data)
                let start_addr = bitmap_addr;
                let end_addr = bitmap_addr + 7999;

                let start_offset = start_addr.saturating_sub(origin);
                let end_offset = if end_addr < origin {
                    0
                } else {
                    end_addr.min(end_address.saturating_sub(1)) - origin
                };

                if start_offset < app_state.raw_data.len() && start_offset <= end_offset {
                    WidgetResult::Action(MenuAction::SetBytesBlockByOffset {
                        start: start_offset,
                        end: end_offset,
                    })
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Enter => {
                let origin = app_state.origin as usize;
                // Align to floor boundary to support partial bitmaps
                let aligned_origin = (origin / 8192) * 8192;
                let bitmap_addr = aligned_origin + (ui_state.bitmap_cursor_index * 8192);

                // Calculate the actual displayed address (consistent with header logic)
                let bitmap_end = bitmap_addr + 8000;
                let data_start_in_bitmap = origin.max(bitmap_addr);

                let display_addr =
                    if data_start_in_bitmap < bitmap_end && data_start_in_bitmap >= bitmap_addr {
                        data_start_in_bitmap
                    } else {
                        bitmap_addr
                    };

                let target_addr = display_addr as u16;
                crate::ui::navigable::jump_to_disassembly_at_address(
                    app_state,
                    ui_state,
                    target_addr,
                )
            }
            _ => WidgetResult::Ignored,
        }
    }
}

fn convert_to_dynamic_image(
    raw_data: &[u8],
    origin: usize,
    bitmap_addr: usize,
    screen_ram_addr: usize,
    multicolor: bool,
) -> DynamicImage {
    // C64 resolution 320x200 scaled 2x to 640x400
    let mut rgb_img = RgbImage::new(640, 400);

    let end_address = origin + raw_data.len();

    // Helper to get byte at absolute address
    let get_byte = |addr: usize| -> u8 {
        if addr >= origin && addr < end_address {
            raw_data[addr - origin]
        } else {
            0
        }
    };

    if multicolor {
        // Multicolor Mode: 160x200 fat pixels
        // Each fat pixel (2x1 C64 pixels) maps to 2x1 pixels in 320x200 image
        for cell_y in 0..25 {
            for cell_x in 0..40 {
                let cell_idx = cell_y * 40 + cell_x;

                // Fetch colors from Screen RAM
                // Logical address of screen ram byte
                let current_screen_addr = screen_ram_addr + cell_idx;
                // If Screen RAM is "valid" (within observed range or just virtual 0s)
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

                        // Render fat pixel: C64 (2x1) -> Image (4x2) for 2x scale
                        let start_x = (cell_x * 8 + fat_pix * 2) * 2;
                        let start_y = (cell_y * 8 + row) * 2;

                        for dy in 0..2 {
                            for dx in 0..4 {
                                rgb_img.put_pixel(
                                    (start_x + dx) as u32,
                                    (start_y + dy) as u32,
                                    Rgb(rgb),
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        // High-Res Mode: 320x200 pixels
        // Each pixel maps to 1x1 pixels in 320x200 image
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

                    for bit in 0..8 {
                        let shift = 7 - bit;
                        let val = (byte >> shift) & 1;
                        let rgb = if val == 1 { fg } else { bg };

                        // Render pixel: C64 (1x1) -> Image (2x2) for 2x scale
                        let start_x = (cell_x * 8 + bit) * 2;
                        let start_y = (cell_y * 8 + row) * 2;

                        for dy in 0..2 {
                            for dx in 0..2 {
                                rgb_img.put_pixel(
                                    (start_x + dx) as u32,
                                    (start_y + dy) as u32,
                                    Rgb(rgb),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    DynamicImage::ImageRgb8(rgb_img)
}
