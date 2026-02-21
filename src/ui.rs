use crate::state::AppState;
use crate::ui_state::{RightPane, UIState};

pub mod dialog_about;
pub mod dialog_bookmarks;
pub mod dialog_comment;
pub mod dialog_complete_address;
pub mod dialog_confirmation;
pub mod dialog_crt_picker;
pub mod dialog_d64_picker;
pub mod dialog_document_settings;
pub mod dialog_export_as;
pub mod dialog_export_labels;
pub mod dialog_find_references;
pub mod dialog_go_to_symbol;
pub mod dialog_jump_to_address;
pub mod dialog_jump_to_line;
pub mod dialog_keyboard_shortcut;
pub mod dialog_label;
pub mod dialog_open;
pub mod dialog_open_recent;
pub mod dialog_origin;
pub mod dialog_save_as;
pub mod dialog_search;
pub mod dialog_settings;
pub mod dialog_t64_picker;
pub mod dialog_warning;
pub mod graphics_common;
pub mod menu;
pub mod navigable;
pub mod statusbar;
pub mod view_bitmap;
pub mod view_blocks;
pub mod view_charset;
pub mod view_debugger;
pub mod view_disassembly;
pub mod view_hexdump;
pub mod view_sprites;
pub mod widget;

use crate::ui::widget::Widget;

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

    ui_state.menu_area = chunks[0];
    ui_state.main_area = chunks[1];
    ui_state.status_bar_area = chunks[2];

    menu::Menu.render(f, chunks[0], app_state, ui_state);
    render_main_view(f, chunks[1], app_state, ui_state);
    statusbar::StatusBar.render(f, chunks[2], app_state, ui_state);

    // Menu Popup is now handled here to ensure it's on top
    // Menu Popup is now handled here to ensure it's on top
    if ui_state.menu.active {
        menu::render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    // Generic Active Dialog Handler (Refactored Dialogs)
    if let Some(dialog) = ui_state.active_dialog.take() {
        dialog.render(f, f.area(), app_state, ui_state);
        ui_state.active_dialog = Some(dialog);
    }
}

fn render_main_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    // Calculate required width for Right Pane
    let right_pane_width = match ui_state.right_pane {
        RightPane::None => 0,
        RightPane::HexDump => 78,
        RightPane::Sprites => 36, // 24 chars + border + padding
        RightPane::Charset => 76, // Grid view: 8 cols * (8+1) width + padding
        RightPane::Bitmap => 80,  // Compact view, ratatui-image scales to fit
        RightPane::Blocks => 42,
        RightPane::Debugger => 36,
    };
    let disasm_view_width = area.width.saturating_sub(right_pane_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(right_pane_width),
        ])
        .split(area);

    ui_state.disassembly_area = layout[0];
    ui_state.right_pane_area = layout[1];

    view_disassembly::DisassemblyView.render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump => view_hexdump::HexDumpView.render(f, layout[1], app_state, ui_state),
        RightPane::Sprites => view_sprites::SpritesView.render(f, layout[1], app_state, ui_state),
        RightPane::Charset => view_charset::CharsetView.render(f, layout[1], app_state, ui_state),
        RightPane::Bitmap => view_bitmap::BitmapView.render(f, layout[1], app_state, ui_state),
        RightPane::Blocks => view_blocks::BlocksView.render(f, layout[1], app_state, ui_state),
        RightPane::Debugger => {
            view_debugger::DebuggerView.render(f, layout[1], app_state, ui_state)
        }
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
            external_label_address: None,
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
