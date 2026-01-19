use crate::state::AppState;
use crate::theme::Theme;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use std::path::PathBuf;

pub struct OpenDialog {
    pub active: bool,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub filter_extensions: Vec<String>,
}

impl OpenDialog {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            active: false,
            current_dir,
            files: Vec::new(),
            selected_index: 0,
            filter_extensions: vec![
                "bin".to_string(),
                "prg".to_string(),
                "crt".to_string(),
                "vsf".to_string(),
                "t64".to_string(),
                "raw".to_string(),
                "regen2000proj".to_string(),
            ],
        }
    }

    pub fn open(&mut self) {
        self.active = true;
        self.refresh_files();
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.active = false;
    }

    pub fn refresh_files(&mut self) {
        self.files = crate::utils::list_files(&self.current_dir, &self.filter_extensions);
    }

    pub fn next(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.files.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.files.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.files.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

pub fn render(f: &mut Frame, area: Rect, dialog: &OpenDialog, theme: &Theme) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Open File (Space to Open, Backspace to Go Back, Esc to Cancel) ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let area = crate::utils::centered_rect(60, 50, area);
    f.render_widget(ratatui::widgets::Clear, area); // Clear background

    let items: Vec<ListItem> = dialog
        .files
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let name = if path.is_dir() {
                format!("{}/", name)
            } else {
                name.to_string()
            };

            ListItem::new(name)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(theme.menu_selected_bg)
                .fg(theme.menu_selected_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(dialog.selected_index));

    f.render_stateful_widget(list, area, &mut state);
}

pub fn handle_input(key: KeyEvent, app_state: &mut AppState, ui_state: &mut UIState) {
    let dialog = &mut ui_state.open_dialog;
    match key.code {
        KeyCode::Esc => {
            dialog.close();
            ui_state.set_status_message("Ready");
        }
        KeyCode::Down => dialog.next(),
        KeyCode::Up => dialog.previous(),
        KeyCode::Backspace => {
            // Go to parent dir
            if let Some(parent) = dialog.current_dir.parent().map(|p| p.to_path_buf()) {
                dialog.current_dir = parent;
                dialog.refresh_files();
                dialog.selected_index = 0;
            }
        }
        KeyCode::Enter => {
            if !dialog.files.is_empty() {
                let selected_path = dialog.files[dialog.selected_index].clone();
                if selected_path.is_dir() {
                    dialog.current_dir = selected_path;
                    dialog.refresh_files();
                    dialog.selected_index = 0;
                } else {
                    // Load file
                    // We need to close the dialog first or handle result, but `app_state.load_file` needs self methods?
                    // Actually app_state.load_file is a public method on AppState.
                    // But we are in a helper function.

                    // Moving logic here.
                    match app_state.load_file(selected_path.clone()) {
                        Err(e) => {
                            ui_state.set_status_message(format!("Error loading file: {}", e));
                        }
                        Ok(loaded_data) => {
                            ui_state.set_status_message(format!("Loaded: {:?}", selected_path));
                            ui_state.open_dialog.close();

                            let loaded_cursor = loaded_data.cursor_address;
                            let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
                            let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
                            let loaded_right_pane = loaded_data.right_pane_visible;
                            let loaded_charset_cursor = loaded_data.charset_cursor_address;

                            // Load new modes
                            ui_state.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
                            ui_state.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
                            ui_state.petscii_mode = loaded_data.petscii_mode;

                            if let Some(idx) = loaded_data.blocks_view_cursor {
                                ui_state.blocks_list_state.select(Some(idx));
                            }

                            // Auto-analyze if it's a binary file (not json)
                            let is_project = selected_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.eq_ignore_ascii_case("regen2000proj"))
                                .unwrap_or(false);

                            if !is_project {
                                app_state.perform_analysis();
                            }

                            // Move cursor
                            if let Some(cursor_addr) = loaded_cursor {
                                if let Some(idx) = app_state.get_line_index_for_address(cursor_addr)
                                {
                                    ui_state.cursor_index = idx;
                                }
                            } else {
                                // Default to origin
                                if let Some(idx) =
                                    app_state.get_line_index_for_address(app_state.origin)
                                {
                                    ui_state.cursor_index = idx;
                                }
                            }

                            if let Some(sprites_addr) = loaded_sprites_cursor {
                                // Calculate index from address
                                // Index = (addr - origin - padding) / 64
                                let origin = app_state.origin as usize;
                                let padding = (64 - (origin % 64)) % 64;
                                let addr = sprites_addr as usize;
                                if addr >= origin + padding {
                                    let offset = addr - (origin + padding);
                                    ui_state.sprites_cursor_index = offset / 64;
                                } else {
                                    ui_state.sprites_cursor_index = 0;
                                }
                            } else {
                                ui_state.sprites_cursor_index = 0;
                            }

                            if let Some(charset_addr) = loaded_charset_cursor {
                                let origin = app_state.origin as usize;
                                let base_alignment = 0x400;
                                let aligned_start_addr = (origin / base_alignment) * base_alignment;
                                let addr = charset_addr as usize;
                                if addr >= aligned_start_addr {
                                    let offset = addr - aligned_start_addr;
                                    ui_state.charset_cursor_index = offset / 8;
                                } else {
                                    ui_state.charset_cursor_index = 0;
                                }
                            } else {
                                ui_state.charset_cursor_index = 0;
                            }

                            if let Some(pane_str) = loaded_right_pane {
                                match pane_str.as_str() {
                                    "HexDump" => {
                                        ui_state.right_pane = crate::ui_state::RightPane::HexDump
                                    }
                                    "Sprites" => {
                                        ui_state.right_pane = crate::ui_state::RightPane::Sprites
                                    }
                                    "Charset" => {
                                        ui_state.right_pane = crate::ui_state::RightPane::Charset
                                    }
                                    "Blocks" => {
                                        ui_state.right_pane = crate::ui_state::RightPane::Blocks
                                    }
                                    "None" => {
                                        ui_state.right_pane = crate::ui_state::RightPane::None
                                    }
                                    _ => {}
                                }
                            }

                            // Restore Hex Cursor
                            // Restore or Reset Hex Cursor
                            if let Some(hex_addr) = loaded_hex_cursor
                                && !app_state.raw_data.is_empty()
                            {
                                let origin = app_state.origin as usize;
                                let alignment_padding = origin % 16;
                                let aligned_origin = origin - alignment_padding;
                                let target = hex_addr as usize;

                                if target >= aligned_origin {
                                    let offset = target - aligned_origin;
                                    let row = offset / 16;
                                    ui_state.hex_cursor_index = row;
                                } else {
                                    ui_state.hex_cursor_index = 0;
                                }
                            } else {
                                ui_state.hex_cursor_index = 0;
                            }

                            // Validate Hex Cursor Bounds
                            if !app_state.raw_data.is_empty() {
                                let origin = app_state.origin as usize;
                                let alignment_padding = origin % 16;
                                let total_len = app_state.raw_data.len() + alignment_padding;
                                let max_rows = total_len.div_ceil(16);
                                if ui_state.hex_cursor_index >= max_rows {
                                    ui_state.hex_cursor_index = 0;
                                }
                            } else {
                                ui_state.hex_cursor_index = 0;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
