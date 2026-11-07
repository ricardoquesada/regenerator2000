use crate::state::AppState;
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
use ratatui_image::StatefulImage;

use crate::ui::navigable::{
    AlignedGridRange, Navigable, handle_nav_input, jump_to_disassembly_at_address,
    right_pane_mouse_inner_area,
};
use crate::ui::widget::{Widget, WidgetResult};

pub struct BitmapView;

impl BitmapView {
    /// Bitmaps are aligned to 8,192-byte (`$2000`) boundaries with an 8,000-byte active payload.
    const ALIGNMENT: usize = 8192;
    const PAYLOAD_BYTES: usize = 8000;

    #[must_use]
    fn grid_range(app_state: &AppState) -> AlignedGridRange {
        AlignedGridRange::new(
            app_state,
            Self::ALIGNMENT,
            Self::ALIGNMENT,
            Self::PAYLOAD_BYTES,
        )
    }
}

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

        // Calculate width based on image aspect ratio (320:200 = 8:5) and terminal cell ratio (~1:2)
        // For H rows displaying 200 logical pixels, we need W columns displaying 320 logical pixels
        // Terminal cells are roughly 1:2 (width:height), so W = H * (320/200) * 2 = H * 3.2
        let image_height = inner_area.height.saturating_sub(5);
        let image_width = (f32::from(image_height) * 3.2) as u16;
        let image_width = image_width.min(inner_area.width);
        let image_area = Rect::new(inner_area.x, inner_area.y + 2, image_width, image_height);

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

        // --- ratatui-image integration with lazy caching ---
        let multicolor_mode = ui_state.bitmap_multicolor_mode;
        let cache_key = (bitmap_addr, multicolor_mode, screen_ram_addr);

        ui_state.bitmap_cache.entry(cache_key).or_insert_with(|| {
            convert_to_dynamic_image(
                &app_state.raw_data,
                &grid,
                bitmap_addr,
                screen_ram_addr,
                multicolor_mode,
            )
        });

        if ui_state.picker.is_some() {
            let needs_rebuild = ui_state
                .bitmap_protocol
                .as_ref()
                .is_none_or(|(cached_key, _)| *cached_key != cache_key);
            if needs_rebuild
                && let Some(picker) = &ui_state.picker
                && let Some(img) = ui_state.bitmap_cache.get(&cache_key)
            {
                let protocol = picker.new_resize_protocol(img.clone());
                ui_state.bitmap_protocol = Some((cache_key, protocol));
            }
            if let Some((_, proto)) = &mut ui_state.bitmap_protocol {
                let image_widget = StatefulImage::new();
                f.render_stateful_widget(image_widget, image_area, proto);
            }
        } else {
            f.render_widget(
                Paragraph::new("Image rendering not supported or no picker available"),
                image_area,
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

fn convert_to_dynamic_image(
    raw_data: &[u8],
    grid: &AlignedGridRange,
    bitmap_addr: usize,
    screen_ram_addr: usize,
    multicolor: bool,
) -> DynamicImage {
    // C64 resolution 320x200 scaled 2x to 640x400
    let mut rgb_img = RgbImage::new(640, 400);
    let get_byte = |addr: usize| -> u8 { grid.byte_at(raw_data, addr) };

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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
        let header_row: String = (1..89)
            .map(|x| {
                buffer
                    .cell((x, 1))
                    .map_or(String::new(), |c| c.symbol().to_string())
            })
            .collect();
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
}
