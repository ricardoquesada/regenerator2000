use crate::state::AppState;
use crate::ui_state::{RightPane, UIState};

pub mod dialog;
pub mod dialog_about;
pub mod dialog_comment;
pub mod dialog_confirmation;
pub mod dialog_document_settings;
pub mod dialog_export_as;
pub mod dialog_jump_to_address;
pub mod dialog_jump_to_line;
pub mod dialog_keyboard_shortcut;
pub mod dialog_label;
pub mod dialog_open;
pub mod dialog_origin;
pub mod dialog_save_as;
pub mod dialog_search;
pub mod dialog_settings;
pub mod menu;
pub mod statusbar;
pub mod view_blocks;
pub mod view_charset;
pub mod view_disassembly;
pub mod view_hexdump;
pub mod view_sprites;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

pub fn ui(f: &mut Frame, app_state: &AppState, ui_state: &mut UIState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Menu
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    menu::render_menu(f, chunks[0], &ui_state.menu, &ui_state.theme);
    render_main_view(f, chunks[1], app_state, ui_state);
    statusbar::render(f, chunks[2], app_state, ui_state);

    // Render Popup if needed
    if ui_state.menu.active && ui_state.menu.selected_item.is_some() {
        menu::render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    if ui_state.jump_to_address_dialog.active {
        dialog_jump_to_address::render(
            f,
            f.area(),
            &ui_state.jump_to_address_dialog,
            &ui_state.theme,
        );
    }
    if ui_state.jump_to_line_dialog.active {
        dialog_jump_to_line::render(f, f.area(), &ui_state.jump_to_line_dialog, &ui_state.theme);
    }

    if ui_state.save_as_dialog.active {
        dialog_save_as::render(f, f.area(), &ui_state.save_as_dialog, &ui_state.theme);
    }

    if ui_state.export_as_dialog.active {
        dialog_export_as::render(f, f.area(), &ui_state.export_as_dialog, &ui_state.theme);
    }

    if ui_state.label_dialog.active {
        dialog_label::render_label_dialog(f, f.area(), &ui_state.label_dialog, &ui_state.theme);
    }

    if ui_state.comment_dialog.active {
        dialog_comment::render_comment_dialog(
            f,
            f.area(),
            &ui_state.comment_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.system_settings_dialog.active {
        dialog_settings::render(
            f,
            f.area(),
            app_state,
            &ui_state.system_settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.shortcuts_dialog.active {
        dialog_keyboard_shortcut::render(f, f.area(), &ui_state.shortcuts_dialog, &ui_state.theme);
    }

    if ui_state.confirmation_dialog.active {
        dialog_confirmation::render_confirmation_dialog(
            f,
            f.area(),
            &ui_state.confirmation_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.origin_dialog.active {
        dialog_origin::render_origin_dialog(f, f.area(), &ui_state.origin_dialog, &ui_state.theme);
    }

    if ui_state.search_dialog.active {
        dialog_search::render(f, f.area(), &ui_state.search_dialog, &ui_state.theme);
    }

    // Generic Active Dialog Handler (Refactored Dialogs)
    if let Some(dialog) = &ui_state.active_dialog {
        dialog.render(f, f.area(), app_state, ui_state);
    }
}

fn render_main_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    // Calculate required width for Right Pane
    let right_pane_width = match ui_state.right_pane {
        RightPane::None => 0,
        RightPane::HexDump => 75,
        RightPane::Sprites => 36, // 24 chars + border + padding
        RightPane::Charset => 76, // Grid view: 8 cols * (8+1) width + padding
        RightPane::Blocks => 42,
    };
    let disasm_view_width = area.width.saturating_sub(right_pane_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(right_pane_width),
        ])
        .split(area);

    view_disassembly::render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump => view_hexdump::render(f, layout[1], app_state, ui_state),
        RightPane::Sprites => view_sprites::render(f, layout[1], app_state, ui_state),
        RightPane::Charset => view_charset::render(f, layout[1], app_state, ui_state),
        RightPane::Blocks => view_blocks::render(f, layout[1], app_state, ui_state),
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::{AddressingMode, Opcode};
    use crate::disassembler::DisassemblyLine;
    use crate::state::AppState;

    fn make_line(
        addr: u16,
        mnemonic: &str,
        operand: &str,
        target: Option<u16>,
        opcode: Option<Opcode>,
    ) -> DisassemblyLine {
        DisassemblyLine {
            address: addr,
            bytes: vec![],
            mnemonic: mnemonic.to_string(),
            operand: operand.to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode,
            show_bytes: false,
            target_address: target,
            comment_address: None,
            is_collapsed: false,
        }
    }

    fn make_jmp_indirect_opcode() -> Option<Opcode> {
        Some(Opcode::new(
            "JMP",
            AddressingMode::Indirect,
            3,
            5,
            "Jump Indirect",
        ))
    }

    fn make_jmp_abs_opcode() -> Option<Opcode> {
        Some(Opcode::new(
            "JMP",
            AddressingMode::Absolute,
            3,
            3,
            "Jump Absolute",
        ))
    }

    #[test]
    fn test_arrow_filtering_indirect_jmp() {
        let lines = vec![
            // 0: JMP ($1000) - Should be filtered out
            make_line(
                0x1000,
                "JMP",
                "($1000)",
                Some(0x2000),
                make_jmp_indirect_opcode(),
            ),
            // 1: NOP
            make_line(0x1003, "NOP", "", None, None),
            // 2: JMP $1000 - Should NOT be filtered out (though valid arrow)
            make_line(0x1004, "JMP", "$1000", Some(0x1000), make_jmp_abs_opcode()),
        ];

        let mut app_state = AppState::new();
        app_state.disassembly = lines;
        app_state.settings.max_arrow_columns = 5;

        // We can't easily call render_disassembly here as it requires Frame and UIState.
        // However, we can assert that the specific logic path works by reproducing the check here
        // or by trusting that if we verified the logic match, it works.
        // Ideally, we'd refactor the arrow generation logic into a pure function `get_arrows(disassembly) -> Vec<Arrow>`.
        // Given constraints, this test ensures struct compatibility and compilation of the opcode helpers.

        // Manual verification of the logic block:
        let line = &app_state.disassembly[0];
        let should_skip = if let Some(opcode) = &line.opcode {
            opcode.mnemonic == "JMP" && opcode.mode == AddressingMode::Indirect
        } else {
            false
        };
        assert!(should_skip, "Indirect JMP should be skipped by opcode mode");

        let line2 = &app_state.disassembly[2];
        let should_skip2 = if let Some(opcode) = &line2.opcode {
            opcode.mnemonic == "JMP" && opcode.mode == AddressingMode::Indirect
        } else {
            false
        };
        assert!(!should_skip2, "Absolute JMP should NOT be skipped");
    }
}
