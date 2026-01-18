use crate::state::AppState;
use crate::theme::Theme;
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct JumpToAddressDialog {
    pub active: bool,
    pub input: String,
}

impl JumpToAddressDialog {
    pub fn new() -> Self {
        Self {
            active: false,
            input: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
        self.input.clear();
    }
}

pub fn render(f: &mut Frame, area: Rect, dialog: &JumpToAddressDialog, theme: &Theme) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Jump To Address (Hex) ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Percentage(35),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    let dialog = &mut ui_state.jump_to_address_dialog;
    match key.code {
        KeyCode::Esc => {
            dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            let input = dialog.input.clone();
            if let Ok(target_addr) = u16::from_str_radix(&input, 16) {
                match ui_state.active_pane {
                    ActivePane::Disassembly => {
                        if let Some(idx) = app_state.get_line_index_containing_address(target_addr)
                        {
                            ui_state
                                .navigation_history
                                .push((ActivePane::Disassembly, ui_state.cursor_index));
                            ui_state.cursor_index = idx;
                            ui_state.set_status_message(format!("Jumped to ${:04X}", target_addr));
                        } else if !app_state.disassembly.is_empty() {
                            ui_state
                                .navigation_history
                                .push((ActivePane::Disassembly, ui_state.cursor_index));
                            ui_state.cursor_index = app_state.disassembly.len() - 1;
                            ui_state.set_status_message("Jumped to end");
                        }
                    }
                    ActivePane::HexDump => {
                        let origin = app_state.origin as usize;
                        let target = target_addr as usize;
                        let data_len = app_state.raw_data.len();
                        let end_addr = origin + data_len;

                        if target >= origin && target < end_addr {
                            let alignment_padding = origin % 16;
                            let aligned_origin = origin - alignment_padding;
                            let offset = target - aligned_origin;
                            let row = offset / 16;
                            ui_state.hex_cursor_index = row;
                            ui_state.set_status_message(format!("Jumped to ${:04X}", target_addr));
                        } else {
                            ui_state.set_status_message("Address out of range");
                        }
                    }
                    ActivePane::Sprites => {
                        let origin = app_state.origin as usize;
                        let target = target_addr as usize;

                        let padding = (64 - (origin % 64)) % 64;
                        let aligned_start = origin + padding;

                        if target >= aligned_start && target < origin + app_state.raw_data.len() {
                            let offset = target - aligned_start;
                            let sprite_idx = offset / 64;
                            let sprite_num = (target / 64) % 256;

                            ui_state.sprites_cursor_index = sprite_idx;
                            ui_state.set_status_message(format!(
                                "Jumped to sprite {} (${:04X})",
                                sprite_num, target_addr
                            ));
                        } else {
                            ui_state.set_status_message("Address out of range or unaligned area");
                        }
                    }
                    ActivePane::Charset => {
                        let origin = app_state.origin as usize;
                        let target = target_addr as usize;
                        let base_alignment = 0x400;
                        let aligned_start_addr = (origin / base_alignment) * base_alignment;

                        let end_addr = origin + app_state.raw_data.len();

                        if target >= aligned_start_addr && target < end_addr {
                            let offset = target - aligned_start_addr;
                            let char_idx = offset / 8;

                            ui_state.charset_cursor_index = char_idx;
                            ui_state.set_status_message(format!(
                                "Jumped to char index {} (${:04X})",
                                char_idx, target_addr
                            ));
                        } else {
                            ui_state.set_status_message("Address out of range");
                        }
                    }
                    ActivePane::Blocks => {
                        ui_state.set_status_message("Jump to address not supported in Blocks view");
                    }
                }

                ui_state.jump_to_address_dialog.close();
            } else {
                ui_state.set_status_message("Invalid Hex Address");
            }
        }
        KeyCode::Backspace => {
            dialog.input.pop();
        }
        KeyCode::Char(c) => {
            if c.is_ascii_hexdigit() && dialog.input.len() < 4 {
                dialog.input.push(c.to_ascii_uppercase());
            }
        }
        _ => {}
    }
}
