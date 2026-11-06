use crate::state::AppState;
use crate::ui_state::{ActivePane, AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::navigable::{
    AlignedGridRange, Navigable, apply_grid_mouse_click, compute_scroll_offset,
    handle_grid_selection_input, handle_nav_input, is_grid_item_selected, move_grid_cursor_down,
    move_grid_cursor_up, right_pane_mouse_inner_area, sync_grid_item_to_disassembly,
};
use crate::ui::widget::{Widget, WidgetResult};

/// Immediately mirror the sprite cursor / selection into the disassembly view's
/// `cursor_index` / `selection_start`.  Called eagerly so both views are
/// consistent within the same render frame.
fn sync_sprites_to_disassembly(app_state: &AppState, ui_state: &mut UIState) {
    let grid = SpritesView::grid_range(app_state);
    sync_grid_item_to_disassembly(
        app_state,
        ui_state,
        &grid,
        ui_state.sprites_cursor_index,
        ui_state.sprites_selection_start,
    );
}

pub struct SpritesView;

impl Navigable for SpritesView {
    fn len(&self, app_state: &AppState) -> usize {
        Self::grid_range(app_state).item_count()
    }

    fn current_index(&self, _app_state: &AppState, ui_state: &UIState) -> usize {
        ui_state.sprites_cursor_index
    }

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let total = self.len(app_state);
        let delta = amount * Self::cols(ui_state);
        move_grid_cursor_down(
            &mut ui_state.core.sprites_cursor_index,
            &mut ui_state.core.sprites_selection_start,
            ui_state.core.is_visual_mode,
            false,
            delta,
            total,
        );
    }

    fn move_up(&self, _app_state: &AppState, ui_state: &mut UIState, amount: usize) {
        let delta = amount * Self::cols(ui_state);
        move_grid_cursor_up(
            &mut ui_state.core.sprites_cursor_index,
            &mut ui_state.core.sprites_selection_start,
            ui_state.core.is_visual_mode,
            false,
            delta,
        );
    }

    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize) {
        let total = self.len(app_state);
        ui_state.sprites_cursor_index = index.min(total.saturating_sub(1));
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
    grid: &AlignedGridRange,
    x_offset: u16,
) -> usize {
    let visible_rows = inner_area.height as usize;
    let sprite_addr_start = grid.item_addr(sprite_index);

    if sprite_addr_start >= grid.end_address {
        return 0;
    }

    let mut rows_used = 0;

    // Draw Sprite Header/Index
    let is_selected = is_grid_item_selected(
        sprite_index,
        ui_state.sprites_cursor_index,
        ui_state.sprites_selection_start,
    );
    let style = if is_selected {
        Style::default()
            .fg(ui_state.theme.highlight_fg)
            .bg(ui_state.theme.highlight_bg)
    } else {
        Style::default()
    };

    let sprite_num = (sprite_addr_start / SpritesView::SPRITE_BYTES) % 256;

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
        let bytes = [
            grid.byte_at(&app_state.raw_data, row_addr_start),
            grid.byte_at(&app_state.raw_data, row_addr_start + 1),
            grid.byte_at(&app_state.raw_data, row_addr_start + 2),
        ];

        let render_y = inner_area.y + (y_offset + rows_used) as u16;
        let render_x = inner_area.x + x_offset + 2;

        if row_addr_start < grid.end_address {
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

    // --- Selection frame (left gutter + right edge) ---
    // Draws colored borders on both sides of the selected sprite to make it
    // clearly stand out, matching the Charset view's selection pattern.
    if is_selected && rows_used > 0 {
        let visible_height = rows_used.min(visible_rows.saturating_sub(y_offset));
        let gap_style = Style::default()
            .bg(ui_state.theme.selection_bg)
            .fg(ui_state.theme.selection_bg);
        let sprite_y = inner_area.y + y_offset as u16;
        let sprite_x = inner_area.x + x_offset;

        // Left gutter — fill the 2-char indent with selection color
        let left_lines: Vec<Line> = (0..visible_height).map(|_| Line::from("  ")).collect();
        f.render_widget(
            Paragraph::new(left_lines).style(gap_style),
            Rect::new(sprite_x, sprite_y, 2, visible_height as u16),
        );

        // Right edge — 1-char column after the 24-char pixel area
        let right_x = sprite_x + 26;
        if right_x < inner_area.x + inner_area.width {
            let right_lines: Vec<Line> = (0..visible_height).map(|_| Line::from(" ")).collect();
            f.render_widget(
                Paragraph::new(right_lines).style(gap_style),
                Rect::new(right_x, sprite_y, 1, visible_height as u16),
            );
        }
    }

    rows_used
}

impl SpritesView {
    const SPRITE_BYTES: usize = 64;
    /// Sprite layout constants (must stay in sync with `render`).
    const SPRITE_HEIGHT: usize = 22; // 1 header + 21 pixel rows
    const COL_WIDTH: u16 = 28; // 2 indent + 24 pixels + 2 gap

    #[must_use]
    fn grid_range(app_state: &AppState) -> AlignedGridRange {
        AlignedGridRange::uniform(app_state, Self::SPRITE_BYTES)
    }

    #[must_use]
    fn cols(ui_state: &UIState) -> usize {
        if ui_state.right_pane == crate::ui_state::RightPane::Sprites2Col {
            2
        } else {
            1
        }
    }

    /// Map a mouse position (column, row) inside `inner_area` to a sprite index,
    /// replicating the render loop's layout (scroll offset, 1-col / 2-col geometry).
    ///
    /// Returns `None` if the position doesn't land on a sprite cell.
    #[must_use]
    fn hit_test_sprite_index(
        &self,
        mouse_col: u16,
        mouse_row: u16,
        inner_area: Rect,
        app_state: &AppState,
        ui_state: &UIState,
    ) -> Option<usize> {
        if app_state.raw_data.is_empty() {
            return None;
        }

        let total_sprites = Self::grid_range(app_state).item_count();
        let cols = Self::cols(ui_state);
        let start_index = ui_state.sprites_scroll_index * cols;

        let click_rel_y = (mouse_row as usize).saturating_sub(inner_area.y as usize);
        let click_rel_x = (mouse_col as usize).saturating_sub(inner_area.x as usize);

        let visual_row_in_viewport = click_rel_y / Self::SPRITE_HEIGHT;
        let col_idx = usize::from(cols == 2 && click_rel_x >= Self::COL_WIDTH as usize);

        let sprite_idx = start_index + visual_row_in_viewport * cols + col_idx;
        (sprite_idx < total_sprites).then_some(sprite_idx)
    }
}

impl Widget for SpritesView {
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let Some(inner_area) = right_pane_mouse_inner_area(ui_state, &mouse) else {
            return WidgetResult::Ignored;
        };

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.move_down(app_state, ui_state, 3);
                sync_sprites_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            MouseEventKind::ScrollUp => {
                self.move_up(app_state, ui_state, 3);
                sync_sprites_to_disassembly(app_state, ui_state);
                WidgetResult::Handled
            }
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                let hit = self.hit_test_sprite_index(
                    mouse.column,
                    mouse.row,
                    inner_area,
                    app_state,
                    ui_state,
                );
                apply_grid_mouse_click(
                    &mouse,
                    ui_state.core.is_visual_mode,
                    &mut ui_state.core.sprites_cursor_index,
                    &mut ui_state.core.sprites_selection_start,
                    hit,
                );
                sync_sprites_to_disassembly(app_state, ui_state);
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

        let grid = Self::grid_range(app_state);
        let total_sprites = grid.item_count();
        let cols = Self::cols(ui_state);

        let visible_rows = inner_area.height as usize;
        let visible_visual_rows = visible_rows / Self::SPRITE_HEIGHT;
        let total_visual_rows = total_sprites.div_ceil(cols);

        let offset = compute_scroll_offset(
            ui_state.sprites_scroll_index,
            ui_state.sprites_cursor_index / cols,
            visible_visual_rows,
            total_visual_rows,
            1,
        );
        ui_state.sprites_scroll_index = offset;

        let start_index = offset * cols;
        let end_index = (start_index + visible_visual_rows * cols + cols).min(total_sprites);

        let mut y_offset = 0;
        let mut i = start_index;
        while i < end_index && y_offset < visible_rows {
            let left_height =
                render_single_sprite(f, app_state, ui_state, inner_area, y_offset, i, &grid, 0);
            let right_height = if cols == 2 && i + 1 < total_sprites {
                render_single_sprite(
                    f,
                    app_state,
                    ui_state,
                    inner_area,
                    y_offset,
                    i + 1,
                    &grid,
                    Self::COL_WIDTH,
                )
            } else {
                0
            };
            y_offset += left_height.max(right_height);
            i += cols;
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let WidgetResult::Handled = handle_nav_input(self, key, app_state, ui_state) {
            sync_sprites_to_disassembly(app_state, ui_state);
            return WidgetResult::Handled;
        }
        if key.code == KeyCode::Char('m') && key.modifiers.is_empty() {
            return WidgetResult::Action(AppAction::ToggleSpriteMulticolor);
        }

        let grid = Self::grid_range(app_state);
        let cols = Self::cols(ui_state);
        let cursor = ui_state.sprites_cursor_index;
        let selection = ui_state.sprites_selection_start;
        let result = handle_grid_selection_input(
            key,
            app_state,
            ui_state,
            &grid,
            cols,
            cursor,
            selection,
            |ui, new_cursor, new_sel| {
                ui.sprites_cursor_index = new_cursor;
                ui.sprites_selection_start = new_sel;
            },
        );
        if matches!(result, WidgetResult::Handled) {
            sync_sprites_to_disassembly(app_state, ui_state);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui_state::RightPane;
    use crossterm::event::KeyModifiers;
    use regenerator2000_core::state::Addr;

    #[test]
    fn test_sprites_view_2col_scroll_and_input_buffer_clear() {
        let mut app_state = AppState::new();
        app_state
            .load_binary(Addr(0x2000), vec![0xEA; 64 * 12])
            .unwrap();
        let mut ui_state = UIState::new(Theme::default());
        ui_state.right_pane = RightPane::Sprites2Col;
        ui_state.right_pane_area = Rect::new(0, 0, 60, 30);

        let mut view = SpritesView;

        // ScrollDown in 2-col mode moves by 3 * 2 = 6 sprites and keeps columns aligned.
        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 5,
            row: 5,
            modifiers: KeyModifiers::empty(),
        };
        assert_eq!(
            view.handle_mouse(scroll_down, &mut app_state, &mut ui_state),
            WidgetResult::Handled
        );
        assert_eq!(ui_state.sprites_cursor_index, 6);

        // 2-col directional keys clear any pending numeric prefix in input_buffer.
        ui_state.input_buffer.push('4');
        assert_eq!(
            view.handle_input(
                KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert!(ui_state.input_buffer.is_empty());
        assert_eq!(ui_state.sprites_cursor_index, 8);
    }
}
