//! Address jump navigation and project-save cursor context construction.

use crate::state::{Addr, AppState, ProjectSaveContext};
use crate::view_state::{ActivePane, CoreViewState, NavigationTarget, RightPane};

/// Resolves the disassembly line index for `target_addr` if the address exists in the document.
fn resolve_jump_line_index(app_state: &AppState, target_addr: Addr) -> Option<usize> {
    if let Some(idx) = app_state.get_line_index_containing_address(target_addr) {
        return Some(idx);
    }
    if let Some(idx) = app_state.disassembly.iter().position(|line| {
        line.address == target_addr || line.external_label_address == Some(target_addr)
    }) {
        return Some(idx);
    }
    if !app_state.is_external(target_addr) {
        return app_state.get_line_index_for_address(target_addr);
    }
    None
}

/// Jumps to `target_addr` and pushes the current cursor position onto the navigation history stack.
pub fn perform_jump_to_address(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
) {
    let Some(idx) = resolve_jump_line_index(app_state, target_addr) else {
        report_out_of_range(app_state, view_state, target_addr);
        return;
    };

    let active_pane = view_state.active_pane;
    let target = match active_pane {
        ActivePane::Disassembly => {
            if let Some(current_line) = app_state.disassembly.get(view_state.cursor_index) {
                let addr = current_line
                    .external_label_address
                    .unwrap_or(current_line.address);
                NavigationTarget::Address(addr.0)
            } else {
                NavigationTarget::Index(view_state.cursor_index)
            }
        }
        ActivePane::HexDump => NavigationTarget::Index(view_state.hex_cursor_index),
        ActivePane::Sprites => NavigationTarget::Index(view_state.sprites_cursor_index),
        ActivePane::Charset => NavigationTarget::Index(view_state.charset_cursor_index),
        ActivePane::Bitmap => NavigationTarget::Index(view_state.bitmap_cursor_index),
        ActivePane::Blocks => {
            NavigationTarget::Index(view_state.blocks_selected_index.unwrap_or(0))
        }
        ActivePane::Debugger => NavigationTarget::Index(view_state.cursor_index),
    };
    view_state.navigation_history.push((active_pane, target));

    apply_resolved_jump(app_state, view_state, target_addr, idx);
}

/// Jumps to `target_addr` without modifying the navigation history stack.
pub fn perform_jump_to_address_no_history(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
) {
    if let Some(idx) = resolve_jump_line_index(app_state, target_addr) {
        apply_resolved_jump(app_state, view_state, target_addr, idx);
    } else {
        report_out_of_range(app_state, view_state, target_addr);
    }
}

fn apply_resolved_jump(
    app_state: &AppState,
    view_state: &mut CoreViewState,
    target_addr: Addr,
    mut idx: usize,
) {
    // Advance past zero-byte header or comment lines when the next line shares `target_addr`.
    while idx + 1 < app_state.disassembly.len()
        && app_state.disassembly[idx].address == target_addr
        && app_state.disassembly[idx].bytes.is_empty()
        && app_state.disassembly[idx + 1].address == target_addr
    {
        idx += 1;
    }

    // Preserve the current scroll offset when `idx` is already inside the viewport.
    let is_visible = view_state.disassembly_viewport_height > 0
        && idx >= view_state.scroll_index
        && idx < view_state.scroll_index + view_state.disassembly_viewport_height;

    view_state.cursor_index = idx;
    if !is_visible {
        view_state.scroll_index = idx;
        view_state.scroll_sub_index = 0;
    }

    if let Some(line) = app_state.disassembly.get(idx) {
        view_state.sub_cursor_index = line.get_sub_index_for_address(app_state, target_addr);
    } else {
        view_state.sub_cursor_index = 0;
    }

    view_state.active_pane = ActivePane::Disassembly;
    view_state.status_message = Some(format!("Jumped to ${target_addr:04X}"));
}

fn report_out_of_range(app_state: &AppState, view_state: &mut CoreViewState, target_addr: Addr) {
    if !app_state.disassembly.is_empty() {
        let end = app_state
            .origin
            .wrapping_add(app_state.raw_data.len().saturating_sub(1) as u16);
        view_state.status_message = Some(format!(
            "Address ${target_addr:04X} out of range (${:04X}-${:04X})",
            app_state.origin.0, end.0
        ));
    }
}

/// Builds a [`ProjectSaveContext`] from the current application state and view state.
#[must_use]
pub fn create_save_context(app_state: &AppState, view_state: &CoreViewState) -> ProjectSaveContext {
    let cursor_addr = app_state
        .disassembly
        .get(view_state.cursor_index)
        .map(|l| l.external_label_address.unwrap_or(l.address));

    let origin = app_state.origin.0 as usize;
    let has_data = !app_state.raw_data.is_empty();

    let hex_addr = has_data.then(|| {
        let bytes_per_row = match view_state.right_pane {
            RightPane::HexDump8 => 8usize,
            _ => 16usize,
        };
        let aligned_origin = (origin / bytes_per_row) * bytes_per_row;
        Addr((aligned_origin + view_state.hex_cursor_index * bytes_per_row) as u16)
    });

    let sprites_addr = has_data.then(|| {
        let aligned_origin = (origin / 64) * 64;
        Addr((aligned_origin + view_state.sprites_cursor_index * 64) as u16)
    });

    let charset_addr = has_data.then(|| {
        let aligned_origin = (origin / 0x400) * 0x400;
        Addr((aligned_origin + view_state.charset_cursor_index * 8) as u16)
    });

    let bitmap_addr = has_data.then(|| {
        let aligned_origin = (origin / 8192) * 8192;
        Addr((aligned_origin + view_state.bitmap_cursor_index * 8192) as u16)
    });

    ProjectSaveContext {
        cursor_address: cursor_addr,
        hex_dump_cursor_address: hex_addr,
        sprites_cursor_address: sprites_addr,
        right_pane_visible: Some(format!("{:?}", view_state.right_pane)),
        charset_cursor_address: charset_addr,
        bitmap_cursor_address: bitmap_addr,
        sprite_multicolor_mode: view_state.sprite_multicolor_mode,
        charset_multicolor_mode: view_state.charset_multicolor_mode,
        bitmap_multicolor_mode: view_state.bitmap_multicolor_mode,
        hexdump_view_mode: view_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: view_state.blocks_selected_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState, BlockType, Label, LabelKind, LabelType};
    use crate::view_state::CoreViewState;

    #[test]
    fn test_perform_jump_to_address_pushes_external_label_address() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA];
        app_state.block_types = vec![BlockType::Code; 1];

        app_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        app_state.settings.all_labels = true;
        app_state.disassemble();

        let ext_label_idx = app_state
            .get_line_index_for_address(Addr(0xD020))
            .expect("External label should be in disassembly");

        let mut view_state = CoreViewState {
            cursor_index: ext_label_idx,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        assert!(!view_state.navigation_history.is_empty());
        let (_pane, target) = view_state.navigation_history.last().unwrap();
        match target {
            NavigationTarget::Address(addr) => {
                assert_eq!(
                    *addr, 0xD020,
                    "Should have pushed $D020, but pushed ${addr:04X}"
                );
            }
            _ => panic!("Expected Address target, found {target:?}"),
        }
    }

    #[test]
    fn test_navigate_back_to_external_label() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA];
        app_state.block_types = vec![BlockType::Code; 1];

        app_state.labels.insert(
            Addr(0xD020),
            vec![Label {
                name: "VIC_BORDER".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Field,
            }],
        );

        app_state.settings.all_labels = true;
        app_state.disassemble();

        let ext_label_idx = app_state.get_line_index_for_address(Addr(0xD020)).unwrap();

        let mut view_state = CoreViewState {
            cursor_index: ext_label_idx,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        let jumped_idx = app_state.get_line_index_for_address(Addr(0x0801)).unwrap();
        assert_eq!(view_state.cursor_index, jumped_idx);

        let (_, target) = view_state.navigation_history.pop().unwrap();
        match target {
            NavigationTarget::Address(addr) => {
                perform_jump_to_address_no_history(&app_state, &mut view_state, Addr(addr));
            }
            _ => panic!("Expected Address target"),
        }

        assert_eq!(
            view_state.cursor_index, ext_label_idx,
            "Should have returned to external label index {ext_label_idx}, instead got {}",
            view_state.cursor_index
        );
    }

    #[test]
    fn test_perform_jump_to_address_invalid_does_not_push_history() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA, 0xEA];
        app_state.block_types = vec![BlockType::Code; 2];
        app_state.disassemble();

        let mut view_state = CoreViewState {
            cursor_index: 1,
            ..Default::default()
        };

        // Jump to an address above the loaded range.
        perform_jump_to_address(&app_state, &mut view_state, Addr(0xC000));
        assert!(view_state.navigation_history.is_empty());
        assert_eq!(view_state.cursor_index, 1);

        // Jump to an address below `origin` ($0400 < $0801) with no external label.
        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0400));
        assert!(
            view_state.navigation_history.is_empty(),
            "Below-origin invalid jump must not push history or jump to origin"
        );
        assert_eq!(view_state.cursor_index, 1);
        assert!(
            view_state
                .status_message
                .as_deref()
                .is_some_and(|m| m.contains("out of range"))
        );
    }

    #[test]
    fn test_perform_jump_to_address_from_non_disassembly_pane() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 64];
        app_state.block_types = vec![BlockType::Code; 64];
        app_state.disassemble();

        let mut view_state = CoreViewState {
            active_pane: ActivePane::HexDump,
            hex_cursor_index: 2,
            ..Default::default()
        };

        perform_jump_to_address(&app_state, &mut view_state, Addr(0x0801));

        assert_eq!(view_state.navigation_history.len(), 1);
        assert_eq!(
            view_state.navigation_history[0],
            (ActivePane::HexDump, NavigationTarget::Index(2))
        );
    }

    #[test]
    fn test_create_save_context_uses_floor_alignment_for_sprites_and_bitmap() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0801);
        app_state.raw_data = vec![0xEA; 16384];
        app_state.block_types = vec![BlockType::Code; 16384];
        app_state.disassemble();

        let view_state = CoreViewState {
            sprites_cursor_index: 2,
            bitmap_cursor_index: 1,
            charset_cursor_index: 3,
            hex_cursor_index: 4,
            ..Default::default()
        };

        let ctx = create_save_context(&app_state, &view_state);
        assert_eq!(ctx.sprites_cursor_address, Some(Addr(0x0800 + 2 * 64)));
        assert_eq!(ctx.bitmap_cursor_address, Some(Addr(8192)));
        assert_eq!(ctx.charset_cursor_address, Some(Addr(0x0800 + 3 * 8)));
        assert_eq!(ctx.hex_dump_cursor_address, Some(Addr(0x0800 + 4 * 16)));
    }
}
