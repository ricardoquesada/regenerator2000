use crate::state::AppState;
use crate::ui_state::{ActivePane, MenuAction, UIState};
use crossterm::event::{KeyCode, KeyEvent};
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

// VIC-II RGB Palette for image generation
const VIC_II_RGB: [[u8; 3]; 16] = [
    [0, 0, 0],       // 0: Black
    [255, 255, 255], // 1: White
    [136, 0, 0],     // 2: Red
    [170, 255, 238], // 3: Cyan
    [204, 68, 204],  // 4: Purple
    [0, 204, 85],    // 5: Green
    [0, 0, 170],     // 6: Blue
    [238, 238, 119], // 7: Yellow
    [221, 136, 85],  // 8: Orange
    [102, 68, 0],    // 9: Brown
    [255, 119, 119], // 10: Light Red
    [51, 51, 51],    // 11: Dark Grey
    [119, 119, 119], // 12: Grey
    [170, 255, 102], // 13: Light Green
    [0, 136, 255],   // 14: Light Blue
    [187, 187, 187], // 15: Light Grey
];

impl Navigable for BitmapView {
    fn len(&self, app_state: &AppState) -> usize {
        // Bitmaps must be aligned to 8192-byte ($2000) boundaries
        let origin = app_state.origin as usize;
        let data_len = app_state.raw_data.len();

        let first_bitmap_offset =
            ((origin / 8192) * 8192 + if origin.is_multiple_of(8192) { 0 } else { 8192 }) - origin;

        if first_bitmap_offset >= data_len {
            return 1;
        }

        let remaining = data_len - first_bitmap_offset;
        (remaining / 8192).max(1)
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
            format!(" Bitmap (Multicolor 320×200) [{}] ", protocol_name)
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
        let first_aligned_addr =
            ((origin / 8192) * 8192) + if origin.is_multiple_of(8192) { 0 } else { 8192 };
        let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
        let bitmap_offset = bitmap_addr - origin;

        if bitmap_offset >= app_state.raw_data.len() {
            return;
        }

        let available_bytes = app_state.raw_data.len() - bitmap_offset;
        let bitmap_size = 8000.min(available_bytes);
        let screen_ram_size = if available_bytes > 8000 {
            1000.min(available_bytes - 8000)
        } else {
            0
        };

        f.render_widget(
            Paragraph::new(format!(
                "Bitmap @ ${:04X} ({} bytes)",
                bitmap_addr, bitmap_size
            ))
            .style(Style::default().fg(ui_state.theme.comment)),
            Rect::new(inner_area.x, inner_area.y, inner_area.width, 1),
        );

        // Calculate width based on image aspect ratio (320:200 = 8:5) and terminal cell ratio (~1:2)
        // For H rows displaying 200 logical pixels, we need W columns displaying 320 logical pixels
        // Terminal cells are roughly 1:2 (width:height), so W = H * (320/200) * 2 = H * 3.2
        let image_height = inner_area.height.saturating_sub(2);
        let image_width = ((image_height as f32) * 3.2) as u16;
        let image_width = image_width.min(inner_area.width);
        let image_area = Rect::new(inner_area.x, inner_area.y + 2, image_width, image_height);

        // --- ratatui-image integration ---

        let needs_update = match ui_state.bitmap_info {
            Some((addr, multi)) => addr != bitmap_addr || multi != ui_state.bitmap_multicolor_mode,
            None => true,
        };

        if needs_update || ui_state.bitmap_image.is_none() {
            let img = convert_to_dynamic_image(
                &app_state.raw_data[bitmap_offset..],
                bitmap_size,
                screen_ram_size,
                ui_state.bitmap_multicolor_mode,
            );
            ui_state.bitmap_image = Some(img);
            ui_state.bitmap_info = Some((bitmap_addr, ui_state.bitmap_multicolor_mode));
        }

        if let Some(img) = &ui_state.bitmap_image {
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
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(MenuAction::ToggleBitmapMulticolor)
            }
            KeyCode::Enter => {
                let origin = app_state.origin as usize;
                let first_aligned_addr =
                    ((origin / 8192) * 8192) + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
                let target_addr = bitmap_addr as u16;
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
    data: &[u8],
    bitmap_size: usize,
    screen_ram_size: usize,
    multicolor: bool,
) -> DynamicImage {
    // C64 resolution 320x200 scaled 2x to 640x400
    let mut rgb_img = RgbImage::new(640, 400);

    let bitmap_data = &data[..bitmap_size];
    let screen_ram = if screen_ram_size > 0 {
        Some(&data[8000..8000 + screen_ram_size])
    } else {
        None
    };

    if multicolor {
        // Multicolor Mode: 160x200 fat pixels
        // Each fat pixel (2x1 C64 pixels) maps to 2x1 pixels in 320x200 image
        for cell_y in 0..25 {
            for cell_x in 0..40 {
                let cell_idx = cell_y * 40 + cell_x;

                let (c1, c2, bg) = if let Some(screen) = screen_ram {
                    if cell_idx < screen.len() {
                        let val = screen[cell_idx];
                        (
                            VIC_II_RGB[(val >> 4) as usize],
                            VIC_II_RGB[(val & 0x0F) as usize],
                            VIC_II_RGB[0],
                        )
                    } else {
                        (VIC_II_RGB[1], VIC_II_RGB[3], VIC_II_RGB[0])
                    }
                } else {
                    (VIC_II_RGB[1], VIC_II_RGB[3], VIC_II_RGB[0])
                };
                let c3 = VIC_II_RGB[1];

                for row in 0..8 {
                    let offset = (cell_y * 320) + (cell_x * 8) + row;
                    let byte = if offset < bitmap_data.len() {
                        bitmap_data[offset]
                    } else {
                        0
                    };

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

                let (fg, bg) = if let Some(screen) = screen_ram {
                    if cell_idx < screen.len() {
                        let val = screen[cell_idx];
                        (
                            VIC_II_RGB[(val >> 4) as usize],
                            VIC_II_RGB[(val & 0x0F) as usize],
                        )
                    } else {
                        (VIC_II_RGB[1], VIC_II_RGB[0])
                    }
                } else {
                    (VIC_II_RGB[1], VIC_II_RGB[0])
                };

                for row in 0..8 {
                    let offset = (cell_y * 320) + (cell_x * 8) + row;
                    let byte = if offset < bitmap_data.len() {
                        bitmap_data[offset]
                    } else {
                        0
                    };

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
