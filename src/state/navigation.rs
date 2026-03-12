//! Navigation helpers that operate on `AppState` + `UIState`.
//!
//! Moved here from `ui/menu.rs` so that non-UI code (MCP handler,
//! future GUIs) can navigate without depending on the TUI layer.

use crate::state::AppState;
use crate::ui_state::{ActivePane, NavigationTarget, UIState};

/// Jump to `target_addr` and push the current cursor position onto the
/// navigation-history stack so the user can go back.
pub fn perform_jump_to_address(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) {
    // Push CURRENT state to history
    if let Some(current_line) = app_state.disassembly.get(ui_state.cursor_index) {
        ui_state.navigation_history.push((
            ActivePane::Disassembly,
            NavigationTarget::Address(current_line.address.0),
        ));
    } else {
        ui_state.navigation_history.push((
            ActivePane::Disassembly,
            NavigationTarget::Index(ui_state.cursor_index),
        ));
    }

    perform_jump_to_address_no_history(app_state, ui_state, target_addr);
}

/// Jump to `target_addr` *without* modifying navigation history.
pub fn perform_jump_to_address_no_history(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) {
    if let Some(mut idx) = app_state
        .get_line_index_containing_address(target_addr)
        .or_else(|| app_state.get_line_index_for_address(target_addr))
    {
        // Optimization: If we landed on a header/comment line (0 bytes)
        // but the next line is the actual code at the same address, advance to it.
        while idx + 1 < app_state.disassembly.len()
            && app_state.disassembly[idx].address == target_addr
            && app_state.disassembly[idx].bytes.is_empty()
            && app_state.disassembly[idx + 1].address == target_addr
        {
            idx += 1;
        }

        ui_state.cursor_index = idx;
        ui_state.scroll_index = idx; // Ensure we jump visually too
        ui_state.scroll_sub_index = 0;

        // Smart Jump: Select relevant sub-line if applicable
        if let Some(line) = app_state.disassembly.get(idx) {
            ui_state.sub_cursor_index =
                crate::ui::view_disassembly::DisassemblyView::get_sub_index_for_address(
                    line,
                    app_state,
                    target_addr.0,
                );
        } else {
            ui_state.sub_cursor_index = 0;
        }

        // Ensure active pane is Disassembly (important for MCP calls)
        ui_state.active_pane = ActivePane::Disassembly;

        ui_state.set_status_message(format!("Jumped to ${target_addr:04X}"));
    } else if !app_state.disassembly.is_empty() {
        ui_state.set_status_message(format!("Address ${target_addr:04X} not found"));
    }
}
