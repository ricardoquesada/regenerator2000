//! Navigation helpers that operate on `AppState` + `UIState`.
//!
//! These live in the TUI crate (not regenerator-core) because they depend on
//! `UIState` and `DisassemblyView`, which are TUI-specific.

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

/// Build a [`ProjectSaveContext`] from the current app + UI state.
///
/// This is the single canonical implementation. Lives in the TUI crate
/// because it reads `UIState` fields (cursor positions, right pane, etc.).
#[must_use]
pub fn create_save_context(
    app_state: &AppState,
    ui_state: &UIState,
) -> crate::state::ProjectSaveContext {
    use crate::state::{Addr, ProjectSaveContext};

    let cursor_addr = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address);

    let hex_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let alignment_padding = origin % 16;
        let aligned_origin = origin - alignment_padding;
        let row_start_offset = ui_state.hex_cursor_index * 16;
        let addr = aligned_origin + row_start_offset;
        Some(Addr(addr as u16))
    };

    let sprites_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let padding = (64 - (origin % 64)) % 64;
        let sprite_offset = ui_state.sprites_cursor_index * 64;
        let addr = origin + padding + sprite_offset;
        Some(Addr(addr as u16))
    };

    let charset_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let char_offset = ui_state.charset_cursor_index * 8;
        let addr = aligned_start_addr + char_offset;
        Some(Addr(addr as u16))
    };

    let bitmap_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        // Bitmaps must be aligned to 8192-byte boundaries
        let first_aligned_addr =
            ((origin / 8192) * 8192) + if origin.is_multiple_of(8192) { 0 } else { 8192 };
        let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
        Some(Addr(bitmap_addr as u16))
    };

    let right_pane_str = format!("{:?}", ui_state.right_pane);

    ProjectSaveContext {
        cursor_address: cursor_addr,
        hex_dump_cursor_address: hex_addr,
        sprites_cursor_address: sprites_addr,
        right_pane_visible: Some(right_pane_str),
        charset_cursor_address: charset_addr,
        bitmap_cursor_address: bitmap_addr,
        sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
        charset_multicolor_mode: ui_state.charset_multicolor_mode,
        bitmap_multicolor_mode: ui_state.bitmap_multicolor_mode,
        hexdump_view_mode: ui_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: ui_state.blocks_list_state.selected(),
        bookmarks: app_state.bookmarks.clone(),
    }
}
