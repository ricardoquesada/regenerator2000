use crate::state::{AppState, ProjectSaveContext};
use crate::theme::Theme;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

pub struct SaveAsDialog {
    pub active: bool,
    pub input: String,
}

impl SaveAsDialog {
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

pub fn render(f: &mut Frame, area: Rect, dialog: &SaveAsDialog, theme: &Theme) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save Project As... ")
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
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
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
    let dialog = &mut ui_state.save_as_dialog;
    match key.code {
        KeyCode::Esc => {
            dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Enter => {
            let filename = dialog.input.clone();
            if !filename.is_empty() {
                // Determine path relative to open dialog's current directory
                let mut path = ui_state.open_dialog.current_dir.join(filename);
                if path.extension().is_none() {
                    path.set_extension("regen2000proj");
                }
                app_state.project_path = Some(path);

                // Collect context for saving
                let cursor_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|l| l.address);

                let hex_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let row_start_offset = ui_state.hex_cursor_index * 16;
                    let addr = aligned_origin + row_start_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                let sprites_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    let sprite_offset = ui_state.sprites_cursor_index * 64;
                    let addr = origin + padding + sprite_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                let charset_addr = if !app_state.raw_data.is_empty() {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    let char_offset = ui_state.charset_cursor_index * 8;
                    let addr = aligned_start_addr + char_offset;
                    Some(addr as u16)
                } else {
                    None
                };

                let right_pane_str = format!("{:?}", ui_state.right_pane);

                if let Err(e) = app_state.save_project(
                    ProjectSaveContext {
                        cursor_address: cursor_addr,
                        hex_dump_cursor_address: hex_addr,
                        sprites_cursor_address: sprites_addr,
                        right_pane_visible: Some(right_pane_str),
                        charset_cursor_address: charset_addr,
                        sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
                        charset_multicolor_mode: ui_state.charset_multicolor_mode,
                        petscii_mode: ui_state.petscii_mode,
                        splitters: app_state.splitters.clone(),
                        blocks_view_cursor: ui_state.blocks_list_state.selected(),
                    },
                    true,
                ) {
                    ui_state.set_status_message(format!("Error saving: {}", e));
                } else {
                    ui_state.set_status_message("Project saved");
                    ui_state.save_as_dialog.close();
                }
            }
        }
        KeyCode::Backspace => {
            dialog.input.pop();
        }
        KeyCode::Char(c) => {
            dialog.input.push(c);
        }
        _ => {}
    }
}
