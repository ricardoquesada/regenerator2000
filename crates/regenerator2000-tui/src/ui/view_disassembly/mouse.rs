//! Mouse event handling, scroll wheel actions, and click/drag selection logic for the disassembly view.
//!
//! This module processes mouse scroll wheel inputs (moving cursor up/down), left clicks inside
//! the disassembly pane (positioning cursor and sub-cursor), and click-and-drag range selection events.

use crate::ui::view_disassembly::layout::get_visual_line_counts;
use crate::ui::widget::WidgetResult;
use crate::ui_state::UIState;
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use regenerator2000_core::state::AppState;

/// Handles mouse clicks, scrolling, and drag selections on the disassembly view with saturating math.
///
/// # Arguments
///
/// * `mouse` - The [`MouseEvent`] received from terminal input.
/// * `app_state` - Mutable reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
///
/// # Returns
///
/// [`WidgetResult::Handled`] if the mouse event was consumed inside the disassembly area,
/// or [`WidgetResult::Ignored`] if the click occurred outside bounds.
pub fn handle_mouse_events(
    mouse: MouseEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> WidgetResult {
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            crate::ui::view_disassembly::navigation::move_cursor_down(app_state, ui_state, 3);
            return WidgetResult::Handled;
        }
        MouseEventKind::ScrollUp => {
            crate::ui::view_disassembly::navigation::move_cursor_up(app_state, ui_state, 3);
            return WidgetResult::Handled;
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {}
        _ => return WidgetResult::Ignored,
    }

    let is_drag = matches!(mouse.kind, MouseEventKind::Drag(_));
    let area = ui_state.disassembly_area;

    let inner_area = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if mouse.column < inner_area.x
        || mouse.column >= inner_area.x.saturating_add(inner_area.width)
        || mouse.row < inner_area.y
        || mouse.row >= inner_area.y.saturating_add(inner_area.height)
    {
        return WidgetResult::Ignored;
    }

    let click_row = (mouse.row.saturating_sub(inner_area.y)) as usize;
    let visible_height = inner_area.height as usize;

    if app_state.disassembly.is_empty() {
        return WidgetResult::Handled;
    }

    let mut current_inst = ui_state.scroll_index;
    let mut current_sub = ui_state.scroll_sub_index;
    let mut current_y = 0;

    if current_inst >= app_state.disassembly.len() {
        current_inst = app_state.disassembly.len().saturating_sub(1);
        current_sub = 0;
    }

    while current_inst < app_state.disassembly.len() {
        let line = &app_state.disassembly[current_inst];
        let counts = get_visual_line_counts(line, app_state).total();
        let visible_part_count = counts.saturating_sub(current_sub);

        if click_row < current_y + visible_part_count {
            let shift_held = mouse.modifiers.contains(KeyModifiers::SHIFT);

            if is_drag || shift_held || ui_state.is_visual_mode {
                if ui_state.selection_start.is_none() {
                    ui_state.selection_start = Some(ui_state.cursor_index);
                }
            } else {
                ui_state.selection_start = None;
            }

            ui_state.cursor_index = current_inst;
            ui_state.sub_cursor_index =
                current_sub.saturating_add(click_row.saturating_sub(current_y));

            return WidgetResult::Handled;
        }

        current_y = current_y.saturating_add(visible_part_count);
        if current_y >= visible_height {
            break;
        }

        let mut next = current_inst + 1;
        while next < app_state.disassembly.len() {
            let l = &app_state.disassembly[next];
            if !l.bytes.is_empty() || l.is_collapsed || l.label.is_some() || !l.mnemonic.is_empty()
            {
                break;
            }
            next += 1;
        }
        current_inst = next;
        current_sub = 0;
    }

    WidgetResult::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use regenerator2000_core::disassembler::DisassemblyLine;
    use regenerator2000_core::state::Addr;

    #[test]
    fn test_mouse_click_to_line_index() {
        let mut app_state = AppState::new();
        app_state.disassembly.push(DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.disassembly_area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };

        let mouse_event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 1, // inside inner area (y=1)
            modifiers: KeyModifiers::NONE,
        };

        let result = handle_mouse_events(mouse_event, &mut app_state, &mut ui_state);
        assert_eq!(result, WidgetResult::Handled);
        assert_eq!(ui_state.cursor_index, 0);
    }

    #[test]
    fn test_mouse_drag_selection_saturating_sub() {
        let mut app_state = AppState::new();
        app_state.disassembly.push(DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        });

        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.disassembly_area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };

        // Click outside area bounds (column 0, row 0 -> border area)
        let mouse_event = MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };

        let result = handle_mouse_events(mouse_event, &mut app_state, &mut ui_state);
        assert_eq!(result, WidgetResult::Ignored);
    }
}
