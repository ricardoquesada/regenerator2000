use crate::state::{AppState, ProjectSaveContext};
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct SaveAsDialog {
    pub input: String,
}

impl SaveAsDialog {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}

impl Widget for SaveAsDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
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

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                let filename = self.input.clone();
                if !filename.is_empty() {
                    // Determine path relative to open dialog's current directory
                    let mut path = ui_state.file_dialog_current_dir.join(filename);
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
                        WidgetResult::Handled
                    } else {
                        ui_state.set_status_message("Project saved");
                        WidgetResult::Close
                    }
                } else {
                    WidgetResult::Handled
                }
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
