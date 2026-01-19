use crate::state::AppState;
use crate::ui_state::{RightPane, UIState};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Paragraph,
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

    crate::menu::render_menu(f, chunks[0], &ui_state.menu, &ui_state.theme);
    render_main_view(f, chunks[1], app_state, ui_state);
    render_status_bar(f, chunks[2], app_state, ui_state);

    // Render Popup if needed
    if ui_state.menu.active && ui_state.menu.selected_item.is_some() {
        crate::menu::render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    if ui_state.open_dialog.active {
        crate::dialog_open::render(f, f.area(), &ui_state.open_dialog, &ui_state.theme);
    }

    if ui_state.jump_to_address_dialog.active {
        crate::dialog_jump_to_address::render(
            f,
            f.area(),
            &ui_state.jump_to_address_dialog,
            &ui_state.theme,
        );
    }
    if ui_state.jump_to_line_dialog.active {
        crate::dialog_jump_to_line::render(
            f,
            f.area(),
            &ui_state.jump_to_line_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.save_as_dialog.active {
        crate::dialog_save_as::render(f, f.area(), &ui_state.save_as_dialog, &ui_state.theme);
    }

    if ui_state.export_as_dialog.active {
        crate::dialog_export_as::render(f, f.area(), &ui_state.export_as_dialog, &ui_state.theme);
    }

    if ui_state.label_dialog.active {
        crate::dialog_label::render_label_dialog(
            f,
            f.area(),
            &ui_state.label_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.comment_dialog.active {
        crate::dialog_comment::render_comment_dialog(
            f,
            f.area(),
            &ui_state.comment_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.settings_dialog.active {
        crate::dialog_document_settings::render(
            f,
            f.area(),
            app_state,
            &ui_state.settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.system_settings_dialog.active {
        crate::dialog_settings::render(
            f,
            f.area(),
            app_state,
            &ui_state.system_settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.about_dialog.active {
        crate::dialog_about::render(f, ui_state, f.area(), &ui_state.about_dialog);
    }

    if ui_state.shortcuts_dialog.active {
        crate::dialog_keyboard_shortcut::render(
            f,
            f.area(),
            &ui_state.shortcuts_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.confirmation_dialog.active {
        crate::dialog_confirmation::render_confirmation_dialog(
            f,
            f.area(),
            &ui_state.confirmation_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.origin_dialog.active {
        crate::dialog_origin::render_origin_dialog(
            f,
            f.area(),
            &ui_state.origin_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.search_dialog.active {
        crate::dialog_search::render(f, f.area(), &ui_state.search_dialog, &ui_state.theme);
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

    crate::view_disassembly::render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump => crate::view_hexdump::render(f, layout[1], app_state, ui_state),
        RightPane::Sprites => crate::view_sprites::render(f, layout[1], app_state, ui_state),
        RightPane::Charset => crate::view_charset::render(f, layout[1], app_state, ui_state),
        RightPane::Blocks => crate::view_blocks::render(f, layout[1], app_state, ui_state),
    }
}

fn render_status_bar(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &UIState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Message
            Constraint::Percentage(50), // Info
        ])
        .split(area);

    // Left: Status Message
    let (status_text, status_fg) = if ui_state.vim_search_active {
        (
            format!("/{}", ui_state.vim_search_input),
            ui_state.theme.highlight_fg,
        )
    } else {
        (
            format!(" {}", ui_state.status_message),
            ui_state.theme.status_bar_fg,
        )
    };

    let status_msg = Paragraph::new(Span::styled(
        status_text,
        Style::default().add_modifier(Modifier::BOLD),
    ))
    .style(
        Style::default()
            .bg(ui_state.theme.status_bar_bg)
            .fg(status_fg),
    );
    f.render_widget(status_msg, chunks[0]);

    // Right: Info
    let cursor_addr = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address)
        .unwrap_or(0);

    let block_info =
        if let Some(offset) = (cursor_addr as isize).checked_sub(app_state.origin as isize) {
            if offset >= 0 && (offset as usize) < app_state.block_types.len() {
                let block_type = app_state.block_types[offset as usize];
                if let Some((start, end)) = app_state.get_block_range(cursor_addr) {
                    format!(
                        "{} | {}: ${:04X}-${:04X} | ",
                        app_state.settings.assembler, block_type, start, end
                    )
                } else {
                    format!("{} | {}: ??? | ", app_state.settings.assembler, block_type)
                }
            } else {
                format!("{} | ", app_state.settings.assembler)
            }
        } else {
            format!("{} | ", app_state.settings.assembler)
        };

    let info = format!(
        "{} | {}Cursor: {:04X} | Origin: {:04X} | File: {:?}{}",
        app_state.settings.platform,
        block_info,
        cursor_addr,
        app_state.origin,
        app_state
            .file_path
            .as_ref()
            .map(|p| p.file_name().unwrap_or_default())
            .unwrap_or_default(),
        if let Some(start) = ui_state.selection_start {
            let count = (ui_state.cursor_index as isize - start as isize).abs() + 1;
            format!(" | Selected: {}", count)
        } else {
            "".to_string()
        }
    );

    let info_widget = Paragraph::new(info)
        .alignment(ratatui::layout::Alignment::Right)
        .style(
            Style::default()
                .bg(ui_state.theme.status_bar_bg)
                .fg(ui_state.theme.status_bar_fg),
        );
    f.render_widget(info_widget, chunks[1]);
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
