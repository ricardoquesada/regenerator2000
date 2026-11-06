//! Disassembly view navigation operations, cursor movement, scrolling, and paging routines.
//!
//! This module implements cursor movement logic (taking multi-line instruction visual heights
//! into account), page up/down navigation, direct instruction index jumping, and G-command visual line jumping.

use crate::ui::view_disassembly::layout::{
    get_index_for_visual_line, get_line_visual_height, get_visual_line_count_for_instruction,
};
use crate::ui_state::UIState;
use regenerator2000_core::state::AppState;

/// Default number of instruction lines scrolled during page up and page down actions.
pub const PAGE_SCROLL_AMOUNT: usize = 30;

/// Synchronizes range selection state before a non-Shift cursor movement.
///
/// Anchors `selection_start` at `cursor_index` when visual mode is active, or clears
/// any transient `Shift+Arrow` selection when visual mode is inactive.
pub fn sync_selection_on_move(ui_state: &mut UIState) {
    if ui_state.is_visual_mode {
        if ui_state.selection_start.is_none() {
            ui_state.selection_start = Some(ui_state.cursor_index);
        }
    } else {
        ui_state.selection_start = None;
    }
}

/// Finds the closest index in `app_state.disassembly` where `line.is_navigable()` holds.
///
/// Searches in the preferred direction (`prefer_forward`) first, then searches in the
/// opposite direction if no navigable line exists in the preferred direction.
#[must_use]
pub fn nearest_navigable_index(app_state: &AppState, index: usize, prefer_forward: bool) -> usize {
    let len = app_state.disassembly.len();
    if len == 0 {
        return 0;
    }
    let clamped = index.min(len - 1);
    if app_state.disassembly[clamped].is_navigable() {
        return clamped;
    }

    if prefer_forward {
        if let Some(found) = (clamped + 1..len).find(|&i| app_state.disassembly[i].is_navigable()) {
            return found;
        }
        if let Some(found) = (0..clamped)
            .rev()
            .find(|&i| app_state.disassembly[i].is_navigable())
        {
            return found;
        }
    } else {
        if let Some(found) = (0..clamped)
            .rev()
            .find(|&i| app_state.disassembly[i].is_navigable())
        {
            return found;
        }
        if let Some(found) = (clamped + 1..len).find(|&i| app_state.disassembly[i].is_navigable()) {
            return found;
        }
    }

    clamped
}

/// Returns `true` when sub-row `0` of instruction `index` lies within the currently visible viewport.
#[must_use]
fn is_index_visually_visible(app_state: &AppState, ui_state: &UIState, index: usize) -> bool {
    let viewport_height = ui_state.disassembly_viewport_height;
    if viewport_height == 0 || index < ui_state.scroll_index || index >= app_state.disassembly.len()
    {
        return false;
    }
    if index == ui_state.scroll_index {
        return ui_state.scroll_sub_index == 0;
    }

    let mut visual_rows = 0usize;
    for (i, line) in app_state.disassembly[ui_state.scroll_index..index]
        .iter()
        .enumerate()
    {
        let height = get_line_visual_height(line, app_state);
        let used = if i == 0 {
            height.saturating_sub(ui_state.scroll_sub_index)
        } else {
            height
        };
        visual_rows = visual_rows.saturating_add(used);
        if visual_rows >= viewport_height {
            return false;
        }
    }
    visual_rows < viewport_height
}

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
                if next_line.is_navigable() {
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
                if prev_line.is_navigable() {
                    break;
                }
                prev_idx -= 1;
            }

            let prev_line = &app_state.disassembly[prev_idx];
            if prev_line.is_navigable() {
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
    sync_selection_on_move(ui_state);
    let raw_target = (ui_state.cursor_index + PAGE_SCROLL_AMOUNT).min(total_len.saturating_sub(1));
    ui_state.cursor_index = nearest_navigable_index(app_state, raw_target, true);
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
    sync_selection_on_move(ui_state);
    let raw_target = ui_state.cursor_index.saturating_sub(PAGE_SCROLL_AMOUNT);
    ui_state.cursor_index = nearest_navigable_index(app_state, raw_target, false);
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
/// * `app_state` - Reference to application state [`AppState`].
/// * `ui_state` - Mutable reference to UI state [`UIState`].
/// * `index` - Target instruction index to jump to.
/// * `total_len` - Total count of disassembly instruction lines.
pub fn jump_to(app_state: &AppState, ui_state: &mut UIState, index: usize, total_len: usize) {
    if app_state.disassembly.is_empty() {
        return;
    }
    sync_selection_on_move(ui_state);
    let max = total_len.saturating_sub(1);
    let clamped = index.min(max);
    let target_idx = nearest_navigable_index(app_state, clamped, clamped < max);
    ui_state.cursor_index = target_idx;
    ui_state.sub_cursor_index = 0;

    if !is_index_visually_visible(app_state, ui_state, target_idx) {
        ui_state.scroll_index = target_idx;
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
    if app_state.disassembly.is_empty() {
        return;
    }
    let new_cursor = if input == 0 {
        Some(nearest_navigable_index(
            app_state,
            total_len.saturating_sub(1),
            false,
        ))
    } else {
        get_index_for_visual_line(app_state, input)
            .map(|idx| nearest_navigable_index(app_state, idx, true))
    };

    if let Some(idx) = new_cursor {
        sync_selection_on_move(ui_state);
        ui_state.cursor_index = idx;
        ui_state.sub_cursor_index = 0;

        if !is_index_visually_visible(app_state, ui_state, idx) {
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

    #[test]
    fn test_navigation_skips_non_navigable_separator_lines() {
        let mut app_state = AppState::new();
        for i in 0..40 {
            let is_sep = i == 0 || i == PAGE_SCROLL_AMOUNT || i == 39;
            app_state.disassembly.push(DisassemblyLine {
                address: Addr(0x1000 + i as u16),
                bytes: if is_sep { vec![] } else { vec![0xEA] },
                mnemonic: if is_sep {
                    String::new()
                } else {
                    "NOP".to_string()
                },
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
        let len = app_state.disassembly.len();

        jump_to(&app_state, &mut ui_state, 0, len);
        assert_eq!(ui_state.cursor_index, 1);

        ui_state.cursor_index = 0;
        page_down(&app_state, &mut ui_state, len);
        assert_eq!(ui_state.cursor_index, PAGE_SCROLL_AMOUNT + 1);

        page_up(&app_state, &mut ui_state);
        assert_eq!(ui_state.cursor_index, 1);

        jump_to_user_input(&app_state, &mut ui_state, 0, len);
        assert_eq!(ui_state.cursor_index, 38);
    }

    #[test]
    fn test_jump_to_checks_visual_height_and_syncs_selection() {
        let app_state = make_multiline_test_app_state();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        ui_state.scroll_index = 0;
        ui_state.scroll_sub_index = 0;
        ui_state.disassembly_viewport_height = 2; // line 0 has visual height 3, so line 1 is off-screen
        ui_state.selection_start = Some(0);
        ui_state.is_visual_mode = false;

        jump_to(&app_state, &mut ui_state, 1, app_state.disassembly.len());
        assert_eq!(ui_state.cursor_index, 1);
        assert_eq!(ui_state.scroll_index, 1);
        assert_eq!(ui_state.selection_start, None);
    }
}
