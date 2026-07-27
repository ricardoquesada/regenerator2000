//! Disassembly view navigation operations, cursor movement, scrolling, and paging routines.
//!
//! This module implements cursor movement logic (taking multi-line instruction visual heights
//! into account), page up/down navigation, direct instruction index jumping, and G-command visual line jumping.

use crate::ui::view_disassembly::layout::{
    get_index_for_visual_line, get_visual_line_count_for_instruction,
};
use crate::ui_state::UIState;
use regenerator2000_core::state::AppState;

/// Default number of instruction lines scrolled during page up and page down actions.
pub const PAGE_SCROLL_AMOUNT: usize = 30;

/// Moves disassembly cursor down by `amount` visual steps.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `amount` - Number of visual line steps to move down.
pub fn move_cursor_down(app_state: &AppState, ui_state: &mut UIState, amount: usize) {
    if app_state.disassembly.is_empty() {
        return;
    }

    if ui_state.cursor_index >= app_state.disassembly.len() {
        ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1);
    }

    for _ in 0..amount {
        let line = &app_state.disassembly[ui_state.cursor_index];
        let visual_len = get_visual_line_count_for_instruction(line, app_state);

        if ui_state.sub_cursor_index + 1 < visual_len && ui_state.selection_start.is_none() {
            ui_state.sub_cursor_index += 1;
        } else {
            let mut next_idx = ui_state.cursor_index + 1;
            while next_idx < app_state.disassembly.len() {
                let next_line = &app_state.disassembly[next_idx];
                if !next_line.bytes.is_empty()
                    || next_line.is_collapsed
                    || next_line.label.is_some()
                    || !next_line.mnemonic.is_empty()
                {
                    break;
                }
                next_idx += 1;
            }

            if next_idx < app_state.disassembly.len() {
                ui_state.cursor_index = next_idx;
                ui_state.sub_cursor_index = 0;
            }
        }
    }
}

/// Moves disassembly cursor up by `amount` visual steps.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `amount` - Number of visual line steps to move up.
pub fn move_cursor_up(app_state: &AppState, ui_state: &mut UIState, amount: usize) {
    if app_state.disassembly.is_empty() {
        return;
    }

    if ui_state.cursor_index >= app_state.disassembly.len() {
        ui_state.cursor_index = app_state.disassembly.len().saturating_sub(1);
    }

    for _ in 0..amount {
        if ui_state.sub_cursor_index > 0 && ui_state.selection_start.is_none() {
            ui_state.sub_cursor_index -= 1;
        } else if ui_state.cursor_index > 0 {
            let mut prev_idx = ui_state.cursor_index - 1;
            while prev_idx > 0 {
                let prev_line = &app_state.disassembly[prev_idx];
                if !prev_line.bytes.is_empty()
                    || prev_line.is_collapsed
                    || prev_line.label.is_some()
                    || !prev_line.mnemonic.is_empty()
                {
                    break;
                }
                prev_idx -= 1;
            }

            let prev_line = &app_state.disassembly[prev_idx];
            if !prev_line.bytes.is_empty()
                || prev_line.is_collapsed
                || prev_line.label.is_some()
                || !prev_line.mnemonic.is_empty()
            {
                ui_state.cursor_index = prev_idx;
                let prev_counts = get_visual_line_count_for_instruction(prev_line, app_state);
                ui_state.sub_cursor_index = prev_counts.saturating_sub(1);
            }
        }
    }
}

/// Scrolls page down by [`PAGE_SCROLL_AMOUNT`] instructions.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `total_len` - Total count of disassembly instruction lines.
pub fn page_down(app_state: &AppState, ui_state: &mut UIState, total_len: usize) {
    if app_state.disassembly.is_empty() {
        return;
    }
    ui_state.cursor_index =
        (ui_state.cursor_index + PAGE_SCROLL_AMOUNT).min(total_len.saturating_sub(1));
    let line = &app_state.disassembly[ui_state.cursor_index];
    let max_sub = get_visual_line_count_for_instruction(line, app_state);
    if ui_state.sub_cursor_index >= max_sub {
        ui_state.sub_cursor_index = max_sub.saturating_sub(1);
    }
}

/// Scrolls page up by [`PAGE_SCROLL_AMOUNT`] instructions.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
pub fn page_up(app_state: &AppState, ui_state: &mut UIState) {
    if app_state.disassembly.is_empty() {
        return;
    }
    ui_state.cursor_index = ui_state.cursor_index.saturating_sub(PAGE_SCROLL_AMOUNT);
    let line = &app_state.disassembly[ui_state.cursor_index];
    let max_sub = get_visual_line_count_for_instruction(line, app_state);
    if ui_state.sub_cursor_index >= max_sub {
        ui_state.sub_cursor_index = max_sub.saturating_sub(1);
    }
}

/// Jumps cursor directly to a target instruction index.
///
/// # Arguments
///
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `index` - Target instruction index to jump to.
/// * `total_len` - Total count of disassembly instruction lines.
pub fn jump_to(ui_state: &mut UIState, index: usize, total_len: usize) {
    let max = total_len.saturating_sub(1);
    ui_state.cursor_index = index.min(max);
    ui_state.sub_cursor_index = 0;

    let is_visible = ui_state.disassembly_viewport_height > 0
        && index >= ui_state.scroll_index
        && index < ui_state.scroll_index + ui_state.disassembly_viewport_height;

    if !is_visible {
        ui_state.scroll_index = ui_state.cursor_index;
        ui_state.scroll_sub_index = 0;
    }
}

/// Jumps cursor based on user G-input (visual line number).
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `input` - Target 1-indexed visual line number, or `0` for the last line.
/// * `total_len` - Total count of disassembly instruction lines.
pub fn jump_to_user_input(
    app_state: &AppState,
    ui_state: &mut UIState,
    input: usize,
    total_len: usize,
) {
    let new_cursor = if input == 0 {
        Some(total_len.saturating_sub(1))
    } else {
        get_index_for_visual_line(app_state, input)
    };

    if let Some(idx) = new_cursor {
        if ui_state.is_visual_mode && ui_state.selection_start.is_none() {
            ui_state.selection_start = Some(ui_state.cursor_index);
        }
        ui_state.cursor_index = idx;
        ui_state.sub_cursor_index = 0;
        let is_visible = ui_state.disassembly_viewport_height > 0
            && idx >= ui_state.scroll_index
            && idx < ui_state.scroll_index + ui_state.disassembly_viewport_height;

        if !is_visible {
            ui_state.scroll_index = idx;
            ui_state.scroll_sub_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regenerator2000_core::disassembler::DisassemblyLine;
    use regenerator2000_core::state::Addr;

    fn make_multiline_test_app_state() -> AppState {
        let mut app_state = AppState::new();
        let line0 = DisassemblyLine {
            address: Addr(0x1000),
            bytes: vec![0xEA],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: Some("Line comment 1\nLine comment 2".to_string()),
            label: None,
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };
        let line1 = DisassemblyLine {
            address: Addr(0x1001),
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
        };
        app_state.disassembly = vec![line0, line1];
        app_state
    }

    #[test]
    fn test_move_cursor_down_sub_cursor_step() {
        let app_state = make_multiline_test_app_state();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;
        ui_state.sub_cursor_index = 0;

        move_cursor_down(&app_state, &mut ui_state, 1);
        assert_eq!(ui_state.cursor_index, 0);
        assert_eq!(ui_state.sub_cursor_index, 1);

        move_cursor_down(&app_state, &mut ui_state, 1);
        assert_eq!(ui_state.cursor_index, 0);
        assert_eq!(ui_state.sub_cursor_index, 2);

        move_cursor_down(&app_state, &mut ui_state, 1);
        assert_eq!(ui_state.cursor_index, 1);
        assert_eq!(ui_state.sub_cursor_index, 0);
    }

    #[test]
    fn test_move_cursor_down_bounds_clamp() {
        let app_state = make_multiline_test_app_state();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;
        ui_state.sub_cursor_index = 0;

        move_cursor_down(&app_state, &mut ui_state, 100);
        assert_eq!(ui_state.cursor_index, 1);
    }

    #[test]
    fn test_move_cursor_up_from_top() {
        let app_state = make_multiline_test_app_state();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;
        ui_state.sub_cursor_index = 0;

        move_cursor_up(&app_state, &mut ui_state, 1);
        assert_eq!(ui_state.cursor_index, 0);
        assert_eq!(ui_state.sub_cursor_index, 0);
    }

    #[test]
    fn test_page_down_and_up_navigation() {
        let mut app_state = AppState::new();
        for i in 0..100 {
            app_state.disassembly.push(DisassemblyLine {
                address: Addr(0x1000 + i as u16),
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
        }
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;

        page_down(&app_state, &mut ui_state, app_state.disassembly.len());
        assert_eq!(ui_state.cursor_index, PAGE_SCROLL_AMOUNT);

        page_up(&app_state, &mut ui_state);
        assert_eq!(ui_state.cursor_index, 0);
    }

    #[test]
    fn test_navigation_with_active_selection() {
        let app_state = make_multiline_test_app_state();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.cursor_index = 0;
        ui_state.sub_cursor_index = 0;
        ui_state.selection_start = Some(0);

        move_cursor_down(&app_state, &mut ui_state, 1);
        assert_eq!(ui_state.cursor_index, 1);
        assert_eq!(ui_state.sub_cursor_index, 0);
    }
}
