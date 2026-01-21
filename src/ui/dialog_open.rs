use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};
use std::path::PathBuf;

pub struct OpenDialog {
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub filter_extensions: Vec<String>,
}

impl OpenDialog {
    pub fn new(current_dir: PathBuf) -> Self {
        let mut dialog = Self {
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
        };
        dialog.refresh_files();
        dialog
    }

    pub fn refresh_files(&mut self) {
        self.files = crate::utils::list_files(&self.current_dir, &self.filter_extensions);
        // Reset selection if out of bounds
        if self.selected_index >= self.files.len() {
            self.selected_index = 0;
        }
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

impl Widget for OpenDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(
            " Open File (Space to Open, Backspace to Go Back, Esc to Cancel) ",
            theme,
        );

        let area = crate::utils::centered_rect(60, 50, area);
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let items: Vec<ListItem> = self
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
        state.select(Some(self.selected_index));

        f.render_stateful_widget(list, area, &mut state);
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
            KeyCode::Down => {
                self.next();
                WidgetResult::Handled
            }
            KeyCode::Up => {
                self.previous();
                WidgetResult::Handled
            }
            KeyCode::Backspace => {
                // Go to parent dir
                if let Some(parent) = self.current_dir.parent().map(|p| p.to_path_buf()) {
                    self.current_dir = parent;
                    self.refresh_files();
                    self.selected_index = 0;
                    // Persist to UIState
                    ui_state.file_dialog_current_dir = self.current_dir.clone();
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if !self.files.is_empty() {
                    let selected_path = self.files[self.selected_index].clone();
                    if selected_path.is_dir() {
                        self.current_dir = selected_path;
                        self.refresh_files();
                        self.selected_index = 0;
                        // Persist to UIState
                        ui_state.file_dialog_current_dir = self.current_dir.clone();
                        WidgetResult::Handled
                    } else {
                        match app_state.load_file(selected_path.clone()) {
                            Err(e) => {
                                ui_state.set_status_message(format!("Error loading file: {}", e));
                                WidgetResult::Handled // Or close? User might want to retry
                            }
                            Ok(loaded_data) => {
                                ui_state.set_status_message(format!("Loaded: {:?}", selected_path));

                                let loaded_cursor = loaded_data.cursor_address;
                                let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
                                let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
                                let loaded_right_pane = loaded_data.right_pane_visible;
                                let loaded_charset_cursor = loaded_data.charset_cursor_address;

                                // Load new modes
                                ui_state.sprite_multicolor_mode =
                                    loaded_data.sprite_multicolor_mode;
                                ui_state.charset_multicolor_mode =
                                    loaded_data.charset_multicolor_mode;
                                ui_state.hexdump_view_mode = loaded_data.hexdump_view_mode;

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
                                    if let Some(idx) =
                                        app_state.get_line_index_for_address(cursor_addr)
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
                                    let aligned_start_addr =
                                        (origin / base_alignment) * base_alignment;
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
                                            ui_state.right_pane =
                                                crate::ui_state::RightPane::HexDump
                                        }
                                        "Sprites" => {
                                            ui_state.right_pane =
                                                crate::ui_state::RightPane::Sprites
                                        }
                                        "Charset" => {
                                            ui_state.right_pane =
                                                crate::ui_state::RightPane::Charset
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

                                WidgetResult::Close
                            }
                        }
                    }
                } else {
                    WidgetResult::Handled
                }
            }
            _ => WidgetResult::Handled,
        }
    }
}
