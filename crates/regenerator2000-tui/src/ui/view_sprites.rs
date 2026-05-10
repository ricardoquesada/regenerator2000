use crate::state::AppState;
use crate::ui_state::{ActivePane, AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

use crate::ui::navigable::{Navigable, handle_nav_input};

pub struct SpritesView;

impl Navigable for SpritesView {
    fn len(&self, app_state: &AppState) -> usize {
        let origin = app_state.origin.0 as usize;
        let aligned_origin = (origin / 64) * 64;
        let end_address = origin + app_state.raw_data.len();
        let total_bytes = end_address.saturating_sub(aligned_origin);
        total_bytes.div_ceil(64)
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.sprites_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.sprites_selection_start.is_none() {
                ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
            }
        } else {
            ui_state.sprites_selection_start = None;
        }
        let total = self.len(app_state);
        if total == 0 {
            return;
        }
        ui_state.sprites_cursor_index =
            (ui_state.sprites_cursor_index + amount).min(total.saturating_sub(1));
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        if ui_state.is_visual_mode {
            if ui_state.sprites_selection_start.is_none() {
                ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
            }
        } else {
            ui_state.sprites_selection_start = None;
        }
        ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(amount);
    }

    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_down(app_state, ui_state, 10);
    }

    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState) {
        self.move_up(app_state, ui_state, 10);
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = self.len(app_state);
        ui_state.sprites_cursor_index = index.min(total.saturating_sub(1));
    }

    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize) {
        let total = self.len(app_state);
        let target = if input == 0 {
            total.saturating_sub(1)
        } else {
            input.saturating_sub(1).min(total.saturating_sub(1))
        };
        ui_state.sprites_cursor_index = target;
    }

    fn item_name(&self) -> &'static str {
        "sprite"
    }
}

/// Renders a single sprite at the given column x-offset within `inner_area`.
///
/// Returns the number of vertical rows consumed (header + pixel rows).
#[allow(clippy::too_many_arguments)]
fn render_single_sprite(
    f: &mut Frame,
    app_state: &AppState,
    ui_state: &UIState,
    inner_area: Rect,
    y_offset: usize,
    sprite_index: usize,
    aligned_origin: usize,
    origin: usize,
    end_address: usize,
    _total_sprites: usize,
    x_offset: u16,
) -> usize {
    let visible_rows = inner_area.height as usize;
    let sprite_addr_start = aligned_origin + sprite_index * 64;

    if sprite_addr_start >= end_address {
        return 0;
    }

    let mut rows_used = 0;

    // Draw Sprite Header/Index
    let is_selected = if let Some(sel_start) = ui_state.sprites_selection_start {
        let (start, end) = if sel_start < ui_state.sprites_cursor_index {
            (sel_start, ui_state.sprites_cursor_index)
        } else {
            (ui_state.sprites_cursor_index, sel_start)
        };
        sprite_index >= start && sprite_index <= end
    } else {
        sprite_index == ui_state.sprites_cursor_index
    };
    let style = if is_selected {
        Style::default()
            .fg(ui_state.theme.highlight_fg)
            .bg(ui_state.theme.highlight_bg)
    } else {
        Style::default()
    };

    let sprite_num = (sprite_addr_start / 64) % 256;

    if y_offset + rows_used < visible_rows {
        // Use shorter header in 2-col mode to fit in narrower columns
        let header = if x_offset > 0 || inner_area.width > 40 {
            format!("Sprite  {sprite_num:03} / ${sprite_num:02X} @ ${sprite_addr_start:04X}")
        } else {
            format!("Spr {sprite_num:03}/${sprite_num:02X} @${sprite_addr_start:04X}")
        };
        let header_width = header
            .len()
            .min(inner_area.width.saturating_sub(x_offset) as usize);
        f.render_widget(
            Paragraph::new(header).style(style),
            Rect::new(
                inner_area.x + x_offset,
                inner_area.y + (y_offset + rows_used) as u16,
                header_width as u16,
                1,
            ),
        );
        rows_used += 1;
    }

    // Draw Sprite Data (21 lines)
    for row in 0..21 {
        if y_offset + rows_used >= visible_rows {
            break;
        }

        let row_addr_start = sprite_addr_start + row * 3;

        // Fetch 3 bytes for the row, handling alignment/padding
        let mut bytes = [0u8; 3];
        for (b_idx, b) in bytes.iter_mut().enumerate() {
            let addr = row_addr_start + b_idx;
            if addr >= origin && addr < end_address {
                *b = app_state.raw_data[addr - origin];
            }
        }

        let render_y = inner_area.y + (y_offset + rows_used) as u16;
        let render_x = inner_area.x + x_offset + 2;

        if row_addr_start < end_address {
            if ui_state.sprite_multicolor_mode {
                let mut spans = Vec::with_capacity(12);
                for b in &bytes {
                    for pair in (0..4).rev() {
                        let bits = (b >> (pair * 2)) & 0b11;
                        let (char_str, fg_color) = match bits {
                            0b00 => ("..", ui_state.theme.foreground),
                            0b01 => ("██", ui_state.theme.foreground),
                            0b10 => ("██", ui_state.theme.sprite_multicolor_1),
                            0b11 => ("██", ui_state.theme.sprite_multicolor_2),
                            _ => unreachable!(),
                        };
                        let pixel_style = if bits == 0b00 {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default().fg(fg_color)
                        };
                        spans.push(Span::styled(char_str, pixel_style));
                    }
                }
                f.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect::new(render_x, render_y, 24, 1),
                );
            } else {
                let mut line_str = String::with_capacity(24);
                for b in &bytes {
                    for bit in (0..8).rev() {
                        if (b >> bit) & 1 == 1 {
                            line_str.push('█');
                        } else {
                            line_str.push('.');
                        }
                    }
                }
                f.render_widget(
                    Paragraph::new(line_str),
                    Rect::new(render_x, render_y, 24, 1),
                );
            }
        } else {
            f.render_widget(
                Paragraph::new("                        "),
                Rect::new(render_x, render_y, 24, 1),
            );
        }

        rows_used += 1;
    }

    rows_used
}

impl Widget for SpritesView {
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
        let is_active = ui_state.active_pane == ActivePane::Sprites;
        let border_style = if is_active {
            Style::default().fg(ui_state.theme.border_active)
        } else {
            Style::default().fg(ui_state.theme.border_inactive)
        };

        let title = if ui_state.sprite_multicolor_mode {
            " Sprites (Multicolor) "
        } else {
            " Sprites (Single Color) "
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

        let origin = app_state.origin.0 as usize;
        let aligned_origin = (origin / 64) * 64;
        let end_address = origin + app_state.raw_data.len();
        let total_bytes = end_address.saturating_sub(aligned_origin);
        let total_sprites = total_bytes.div_ceil(64);

        let is_2col = ui_state.right_pane == crate::ui_state::RightPane::Sprites2Col;
        let cols = if is_2col { 2 } else { 1 };

        let sprite_height = 22; // 21 lines + 1 separator
        let visible_rows = inner_area.height as usize;
        let visible_visual_rows = visible_rows / sprite_height;
        let total_visual_rows = total_sprites.div_ceil(cols);

        // --- Scrolloff Logic (same pattern as HexDump) ---
        // Work in "visual row" units. In 2-col mode one visual row = 2 sprites.
        let cursor_visual_row = if is_2col {
            ui_state.sprites_cursor_index / 2
        } else {
            ui_state.sprites_cursor_index
        };
        let margin = 1usize.min(visible_visual_rows / 3);
        let mut offset = ui_state.sprites_scroll_index;

        // Constraint 1: cursor must be at least `margin` from top
        if cursor_visual_row < offset + margin {
            offset = cursor_visual_row.saturating_sub(margin);
        }

        // Constraint 2: cursor must be at least `margin` from bottom
        if cursor_visual_row >= offset + visible_visual_rows.saturating_sub(margin) {
            offset = cursor_visual_row
                .saturating_sub(visible_visual_rows.saturating_sub(margin).saturating_sub(1));
        }

        // Final bounds check
        let max_offset = total_visual_rows.saturating_sub(visible_visual_rows);
        offset = offset.min(max_offset);
        ui_state.sprites_scroll_index = offset;
        // --- Scrolloff Logic End ---

        let start_index = offset * cols;
        let end_index = (start_index + visible_visual_rows * cols + cols).min(total_sprites);

        let mut y_offset = 0;

        if is_2col {
            // 2-column mode: render sprites in pairs
            let col_width = 28u16; // 2 indent + 24 pixels + 2 gap
            let mut i = start_index;
            while i < end_index {
                if y_offset >= visible_rows {
                    break;
                }

                // Left sprite
                let left_height = render_single_sprite(
                    f,
                    app_state,
                    ui_state,
                    inner_area,
                    y_offset,
                    i,
                    aligned_origin,
                    origin,
                    end_address,
                    total_sprites,
                    0, // x_offset for left column
                );

                // Right sprite (if available)
                let right_height = if i + 1 < total_sprites {
                    render_single_sprite(
                        f,
                        app_state,
                        ui_state,
                        inner_area,
                        y_offset,
                        i + 1,
                        aligned_origin,
                        origin,
                        end_address,
                        total_sprites,
                        col_width, // x_offset for right column
                    )
                } else {
                    0
                };

                y_offset += left_height.max(right_height);
                i += 2;
            }
        } else {
            // 1-column mode: original layout
            for i in start_index..end_index {
                if y_offset >= visible_rows {
                    break;
                }
                let height = render_single_sprite(
                    f,
                    app_state,
                    ui_state,
                    inner_area,
                    y_offset,
                    i,
                    aligned_origin,
                    origin,
                    end_address,
                    total_sprites,
                    0,
                );
                y_offset += height;
            }
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let is_2col = ui_state.right_pane == crate::ui_state::RightPane::Sprites2Col;

        // In 2-column mode, intercept directional keys for grid navigation:
        // Up/Down move by 2 sprites (one visual row), Left/Right by 1 sprite.
        if is_2col {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                    self.move_down(app_state, ui_state, 2);
                    return WidgetResult::Handled;
                }
                KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                    self.move_up(app_state, ui_state, 2);
                    return WidgetResult::Handled;
                }
                KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                    if ui_state.is_visual_mode {
                        if ui_state.sprites_selection_start.is_none() {
                            ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                        }
                    } else {
                        ui_state.sprites_selection_start = None;
                    }
                    if ui_state.sprites_cursor_index > 0 {
                        ui_state.sprites_cursor_index -= 1;
                    }
                    return WidgetResult::Handled;
                }
                KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                    if ui_state.is_visual_mode {
                        if ui_state.sprites_selection_start.is_none() {
                            ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                        }
                    } else {
                        ui_state.sprites_selection_start = None;
                    }
                    let total = self.len(app_state);
                    if total > 0 && ui_state.sprites_cursor_index + 1 < total {
                        ui_state.sprites_cursor_index += 1;
                    }
                    return WidgetResult::Handled;
                }
                // Shift+Down: extend selection by one visual row (2 sprites)
                KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                    if ui_state.sprites_selection_start.is_none() {
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                    }
                    let selection_to_keep = ui_state.sprites_selection_start;
                    let total = self.len(app_state);
                    if total > 0 {
                        ui_state.sprites_cursor_index =
                            (ui_state.sprites_cursor_index + 2).min(total.saturating_sub(1));
                    }
                    ui_state.sprites_selection_start = selection_to_keep;
                    return WidgetResult::Handled;
                }
                // Shift+Up: extend selection by one visual row (2 sprites)
                KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                    if ui_state.sprites_selection_start.is_none() {
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                    }
                    let selection_to_keep = ui_state.sprites_selection_start;
                    ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(2);
                    ui_state.sprites_selection_start = selection_to_keep;
                    return WidgetResult::Handled;
                }
                // Shift+Left/Right: extend selection by 1 sprite
                KeyCode::Left if key.modifiers == KeyModifiers::SHIFT => {
                    if ui_state.sprites_selection_start.is_none() {
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                    }
                    let selection_to_keep = ui_state.sprites_selection_start;
                    ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(1);
                    ui_state.sprites_selection_start = selection_to_keep;
                    return WidgetResult::Handled;
                }
                KeyCode::Right if key.modifiers == KeyModifiers::SHIFT => {
                    if ui_state.sprites_selection_start.is_none() {
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                    }
                    let selection_to_keep = ui_state.sprites_selection_start;
                    let total = self.len(app_state);
                    if total > 0 {
                        ui_state.sprites_cursor_index =
                            (ui_state.sprites_cursor_index + 1).min(total.saturating_sub(1));
                    }
                    ui_state.sprites_selection_start = selection_to_keep;
                    return WidgetResult::Handled;
                }
                _ => {}
            }
        }

        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            return WidgetResult::Handled;
        }

        match key.code {
            // Escape cancels visual mode / selection
            KeyCode::Esc => {
                if ui_state.sprites_selection_start.is_some() || ui_state.is_visual_mode {
                    ui_state.sprites_selection_start = None;
                    ui_state.is_visual_mode = false;
                    ui_state.set_status_message("");
                    WidgetResult::Handled
                } else {
                    WidgetResult::Ignored
                }
            }
            // Visual mode toggle
            KeyCode::Char('V') if key.modifiers == KeyModifiers::SHIFT => {
                if !app_state.raw_data.is_empty() {
                    ui_state.is_visual_mode = !ui_state.is_visual_mode;
                    if ui_state.is_visual_mode {
                        ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                        ui_state.set_status_message("Visual Mode");
                    } else {
                        ui_state.sprites_selection_start = None;
                        ui_state.set_status_message("");
                    }
                }
                WidgetResult::Handled
            }
            // Shift+Down for selection (1-col mode only; 2-col handled above)
            KeyCode::Down if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.sprites_selection_start;
                if saved_selection.is_none() {
                    ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                }
                let selection_to_keep = ui_state.sprites_selection_start;
                // Move cursor
                let total = self.len(app_state);
                if total > 0 {
                    ui_state.sprites_cursor_index =
                        (ui_state.sprites_cursor_index + 1).min(total.saturating_sub(1));
                }
                // Restore selection for shift+arrow mode
                ui_state.sprites_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            // Shift+Up for selection (1-col mode only; 2-col handled above)
            KeyCode::Up if key.modifiers == KeyModifiers::SHIFT => {
                let saved_selection = ui_state.sprites_selection_start;
                if saved_selection.is_none() {
                    ui_state.sprites_selection_start = Some(ui_state.sprites_cursor_index);
                }
                let selection_to_keep = ui_state.sprites_selection_start;
                // Move cursor
                ui_state.sprites_cursor_index = ui_state.sprites_cursor_index.saturating_sub(1);
                // Restore selection for shift+arrow mode
                ui_state.sprites_selection_start = selection_to_keep;
                WidgetResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                WidgetResult::Action(AppAction::ToggleSpriteMulticolor)
            }
            KeyCode::Char('b') if key.modifiers.is_empty() => {
                // Convert selected sprites or current sprite to bytes block (64 bytes per sprite)
                let origin = app_state.origin.0 as usize;
                let aligned_origin = (origin / 64) * 64;
                let end_address = origin + app_state.raw_data.len();

                // Determine sprite range based on selection
                let (start_sprite, end_sprite) =
                    if let Some(sel_start) = ui_state.sprites_selection_start {
                        if sel_start < ui_state.sprites_cursor_index {
                            (sel_start, ui_state.sprites_cursor_index)
                        } else {
                            (ui_state.sprites_cursor_index, sel_start)
                        }
                    } else {
                        (ui_state.sprites_cursor_index, ui_state.sprites_cursor_index)
                    };

                let start_addr = aligned_origin + start_sprite * 64;
                let end_addr = aligned_origin + (end_sprite + 1) * 64 - 1;

                // Clamp to actual data range
                let start_offset = start_addr.saturating_sub(origin);

                let end_offset_abs = end_addr.min(end_address.saturating_sub(1));
                let end_offset = end_offset_abs.saturating_sub(origin);

                // Clear selection after action
                ui_state.sprites_selection_start = None;
                ui_state.is_visual_mode = false;

                if start_offset < app_state.raw_data.len() && start_offset <= end_offset {
                    WidgetResult::Action(AppAction::SetBytesBlockByOffset {
                        start: start_offset,
                        end: end_offset,
                    })
                } else {
                    WidgetResult::Ignored
                }
            }
            KeyCode::Enter => {
                let origin = app_state.origin.0 as usize;
                let aligned_origin = (origin / 64) * 64;
                let sprite_offset = ui_state.sprites_cursor_index * 64;
                let sprite_addr = aligned_origin + sprite_offset;

                // If this sprite contains the origin, jump to origin instead of the aligned boundary
                let target_addr = if origin >= sprite_addr && origin < sprite_addr + 64 {
                    origin as u16
                } else {
                    sprite_addr as u16
                };

                crate::ui::navigable::jump_to_disassembly_at_address(
                    app_state,
                    ui_state,
                    crate::state::Addr(target_addr),
                )
            }
            _ => WidgetResult::Ignored,
        }
    }
}
