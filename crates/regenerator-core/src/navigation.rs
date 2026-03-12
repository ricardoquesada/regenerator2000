use crate::state::Addr;
use crate::state::AppState;
use crate::view_state::{ActivePane, CoreViewState, NavigationTarget};

/// Jump to `target_addr` and push the current cursor position onto the
/// navigation-history stack so the user can go back.
pub fn perform_jump_to_address(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
) {
    // Push CURRENT state to history
    if let Some(current_line) = app_state.disassembly.get(view_state.cursor_index) {
        view_state.navigation_history.push((
            ActivePane::Disassembly,
            NavigationTarget::Address(current_line.address.0),
        ));
    } else {
        view_state.navigation_history.push((
            ActivePane::Disassembly,
            NavigationTarget::Index(view_state.cursor_index),
        ));
    }

    perform_jump_to_address_no_history(app_state, view_state, target_addr);
}

/// Jump to `target_addr` *without* modifying navigation history.
pub fn perform_jump_to_address_no_history(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
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

        view_state.cursor_index = idx;
        view_state.scroll_index = idx; // Ensure we jump visually too
        view_state.scroll_sub_index = 0;

        // Smart Jump: Select relevant sub-line if applicable
        if let Some(line) = app_state.disassembly.get(idx) {
            view_state.sub_cursor_index = line.get_sub_index_for_address(app_state, target_addr.0);
        } else {
            view_state.sub_cursor_index = 0;
        }

        // Ensure active pane is Disassembly (important for MCP calls)
        view_state.active_pane = ActivePane::Disassembly;

        view_state.status_message = Some(format!("Jumped to ${target_addr:04X}"));
    } else if !app_state.disassembly.is_empty() {
        view_state.status_message = Some(format!("Address ${target_addr:04X} not found"));
    }
}

/// Build a [`crate::state::ProjectSaveContext`] from the current app + UI state.
#[must_use]
pub fn create_save_context(
    app_state: &AppState,
    view_state: &CoreViewState,
) -> crate::state::ProjectSaveContext {
    use crate::state::ProjectSaveContext;

    let cursor_addr = app_state
        .disassembly
        .get(view_state.cursor_index)
        .map(|l| l.address);

    let hex_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let alignment_padding = origin % 16;
        let aligned_origin = origin - alignment_padding;
        let row_start_offset = view_state.hex_cursor_index * 16;
        let addr = aligned_origin + row_start_offset;
        Some(Addr(addr as u16))
    };

    let sprites_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let padding = (64 - (origin % 64)) % 64;
        let sprite_offset = view_state.sprites_cursor_index * 64;
        let addr = origin + padding + sprite_offset;
        Some(Addr(addr as u16))
    };

    let charset_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let char_offset = view_state.charset_cursor_index * 8;
        let addr = aligned_start_addr + char_offset;
        Some(Addr(addr as u16))
    };

    let bitmap_addr = if app_state.raw_data.is_empty() {
        None
    } else {
        let origin = app_state.origin.0 as usize;
        // Bitmaps must be aligned to 8192-byte boundaries
        let first_aligned_addr = origin.div_ceil(8192) * 8192;
        let bitmap_addr = first_aligned_addr + (view_state.bitmap_cursor_index * 8192);
        Some(Addr(bitmap_addr as u16))
    };

    let right_pane_str = format!("{:?}", view_state.right_pane);

    ProjectSaveContext {
        cursor_address: cursor_addr,
        hex_dump_cursor_address: hex_addr,
        sprites_cursor_address: sprites_addr,
        right_pane_visible: Some(right_pane_str),
        charset_cursor_address: charset_addr,
        bitmap_cursor_address: bitmap_addr,
        sprite_multicolor_mode: view_state.sprite_multicolor_mode,
        charset_multicolor_mode: view_state.charset_multicolor_mode,
        bitmap_multicolor_mode: view_state.bitmap_multicolor_mode,
        hexdump_view_mode: view_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: view_state.blocks_selected_index,
        bookmarks: app_state.bookmarks.clone(),
    }
}
