use crate::state::{AppState, ProjectSaveContext};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{List, ListItem, ListState, Paragraph},
};
use ratatui_textarea::{CursorMove, TextArea};
use std::path::PathBuf;

use crate::ui::widget::{Widget, WidgetResult};

#[derive(PartialEq, Eq)]
pub enum SaveAsFocus {
    Filename,
    DirectoryList,
}

pub struct SaveAsDialog {
    pub textarea: TextArea<'static>,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub history: Vec<PathBuf>,
    pub focus: SaveAsFocus,
}

impl Default for SaveAsDialog {
    fn default() -> Self {
        Self::new(None, std::env::current_dir().unwrap_or_default())
    }
}

impl SaveAsDialog {
    #[must_use]
    pub fn new(initial_filename: Option<String>, current_dir: PathBuf) -> Self {
        let mut textarea = TextArea::default();
        if let Some(filename) = initial_filename {
            textarea.insert_str(filename);
            textarea.move_cursor(CursorMove::End);
        }
        let mut dialog = Self {
            textarea,
            current_dir,
            files: Vec::new(),
            selected_index: 0,
            history: Vec::new(),
            focus: SaveAsFocus::Filename,
        };
        dialog.refresh_files();
        dialog
    }

    pub fn refresh_files(&mut self) {
        let filter_extensions = vec!["regen2000proj".to_string()];
        self.files = crate::utils::list_files(&self.current_dir, &filter_extensions);

        if self.current_dir.parent().is_some() {
            self.files.insert(0, self.current_dir.join(".."));
        }

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

    pub fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    pub fn page_down(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 10).min(self.files.len() - 1);
        }
    }
}

impl Widget for SaveAsDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = " Save Project As (Tab to Switch Focus, Esc to Cancel) ";
        let block = crate::ui::widget::create_dialog_block(title, theme);

        let area = crate::utils::centered_rect_adaptive(60, 40, 60, 18, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        f.render_widget(block.clone(), area);
        let inner = block.inner(area);

        let dialog_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(inner);

        let parent_path = self.current_dir.join("..");
        let items: Vec<ListItem> = self
            .files
            .iter()
            .map(|path| {
                let name = if path == &parent_path {
                    "..".to_string()
                } else {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if path.is_dir() {
                        format!("{name}/")
                    } else {
                        name.to_string()
                    }
                };
                ListItem::new(name)
            })
            .collect();

        let list_block = ratatui::widgets::Block::default()
            .title(format!(
                " Directory: {} ",
                self.current_dir.to_string_lossy()
            ))
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(if self.focus == SaveAsFocus::DirectoryList {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let mut state = ListState::default();
        state.select(Some(self.selected_index));
        f.render_stateful_widget(list, dialog_layout[0], &mut state);

        let input_block = ratatui::widgets::Block::default()
            .title(" Filename ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(if self.focus == SaveAsFocus::Filename {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let input_inner = input_block.inner(dialog_layout[2]);
        f.render_widget(input_block, dialog_layout[2]);

        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(16)])
            .split(input_inner);

        let mut textarea = self.textarea.clone();
        let style = Style::default()
            .fg(if self.focus == SaveAsFocus::Filename {
                theme.menu_selected_fg
            } else {
                Color::Reset
            })
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);

        if self.focus == SaveAsFocus::Filename {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_layout[0]);

        let extension = Paragraph::new(".regen2000proj").style(Style::default().fg(Color::Gray));
        f.render_widget(extension, input_layout[1]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if key.code == KeyCode::Esc {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                SaveAsFocus::Filename => SaveAsFocus::DirectoryList,
                SaveAsFocus::DirectoryList => SaveAsFocus::Filename,
            };
            return WidgetResult::Handled;
        }

        match self.focus {
            SaveAsFocus::DirectoryList => match key.code {
                KeyCode::Down => {
                    self.next();
                    WidgetResult::Handled
                }
                KeyCode::Up => {
                    self.previous();
                    WidgetResult::Handled
                }
                KeyCode::PageUp => {
                    self.page_up();
                    WidgetResult::Handled
                }
                KeyCode::PageDown => {
                    self.page_down();
                    WidgetResult::Handled
                }
                KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                    self.page_up();
                    WidgetResult::Handled
                }
                KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
                    self.page_down();
                    WidgetResult::Handled
                }
                KeyCode::Backspace => {
                    if let Some(prev_dir) = self.history.pop() {
                        self.current_dir = prev_dir;
                        self.refresh_files();
                        self.selected_index = 0;
                    }
                    WidgetResult::Handled
                }
                KeyCode::Enter => {
                    if self.files.is_empty() {
                        WidgetResult::Handled
                    } else {
                        let selected_path = self.files[self.selected_index].clone();
                        if selected_path.is_dir() {
                            self.history.push(self.current_dir.clone());
                            self.current_dir = selected_path;
                            self.refresh_files();
                            self.selected_index = 0;
                        } else if let Some(name) = selected_path.file_stem() {
                            self.textarea = TextArea::default();
                            self.textarea.insert_str(name.to_string_lossy());
                            self.textarea.move_cursor(CursorMove::End);
                            self.focus = SaveAsFocus::Filename;
                        }
                        WidgetResult::Handled
                    }
                }
                _ => WidgetResult::Handled,
            },
            SaveAsFocus::Filename => match key.code {
                KeyCode::Enter => {
                    let filename = self.textarea.lines().join("").trim().to_string();
                    if filename.is_empty() {
                        WidgetResult::Handled
                    } else {
                        let mut path = self.current_dir.join(&filename);
                        if path.extension().is_none() {
                            path.set_extension("regen2000proj");
                        }
                        let saved_filename = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        app_state.project_path = Some(path);

                        let cursor_addr = app_state
                            .disassembly
                            .get(ui_state.cursor_index)
                            .map(|l| l.address);

                        let hex_addr = if app_state.raw_data.is_empty() {
                            None
                        } else {
                            let origin = app_state.origin.0 as usize;
                            let alignment_padding = origin % 16;
                            let aligned_origin = origin - alignment_padding;
                            let row_start_offset = ui_state.hex_cursor_index * 16;
                            let addr = aligned_origin + row_start_offset;
                            Some(crate::state::Addr(addr as u16))
                        };

                        let sprites_addr = if app_state.raw_data.is_empty() {
                            None
                        } else {
                            let origin = app_state.origin.0 as usize;
                            let padding = (64 - (origin % 64)) % 64;
                            let sprite_offset = ui_state.sprites_cursor_index * 64;
                            let addr = origin + padding + sprite_offset;
                            Some(crate::state::Addr(addr as u16))
                        };

                        let charset_addr = if app_state.raw_data.is_empty() {
                            None
                        } else {
                            let origin = app_state.origin.0 as usize;
                            let base_alignment = 0x400;
                            let aligned_start_addr = (origin / base_alignment) * base_alignment;
                            let char_offset = ui_state.charset_cursor_index * 8;
                            let addr = aligned_start_addr + char_offset;
                            Some(crate::state::Addr(addr as u16))
                        };

                        let bitmap_addr = if app_state.raw_data.is_empty() {
                            None
                        } else {
                            let origin = app_state.origin.0 as usize;
                            let first_aligned_addr = ((origin / 8192) * 8192)
                                + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                            let bitmap_addr =
                                first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
                            Some(crate::state::Addr(bitmap_addr as u16))
                        };

                        let right_pane_str = format!("{:?}", ui_state.right_pane);

                        if let Err(e) = app_state.save_project(
                            ProjectSaveContext {
                                cursor_address: cursor_addr,
                                hex_dump_cursor_address: hex_addr,
                                sprites_cursor_address: sprites_addr,
                                right_pane_visible: Some(right_pane_str),
                                charset_cursor_address: charset_addr,
                                bitmap_cursor_address: bitmap_addr,
                                sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
                                charset_multicolor_mode: ui_state.charset_multicolor_mode,
                                bitmap_multicolor_mode: ui_state.bitmap_multicolor_mode,
                                hexdump_view_mode: ui_state.hexdump_view_mode,
                                splitters: app_state.splitters.clone(),
                                blocks_view_cursor: ui_state.blocks_list_state.selected(),
                                bookmarks: app_state.bookmarks.clone(),
                            },
                            true,
                        ) {
                            ui_state.set_status_message(format!("Error saving: {e}"));
                            WidgetResult::Handled
                        } else {
                            app_state.last_save_as_filename = Some(filename.clone());
                            ui_state.set_status_message(format!("Project saved: {saved_filename}"));
                            WidgetResult::Close
                        }
                    }
                }
                _ => {
                    self.textarea.input(key);
                    WidgetResult::Handled
                }
            },
        }
    }
}
