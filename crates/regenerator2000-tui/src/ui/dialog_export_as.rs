use crate::state::AppState;
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

#[derive(Debug, PartialEq, Eq)]
pub enum ExportAsFocus {
    Filename,
    DirectoryList,
}

pub struct ExportAsDialog {
    pub textarea: TextArea<'static>,
    pub format: crate::event::ExportFormat,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub history: Vec<PathBuf>,
    pub focus: ExportAsFocus,
    pub sub_dialog: Option<Box<dyn Widget>>,
}

impl Default for ExportAsDialog {
    fn default() -> Self {
        Self::new(
            None,
            crate::event::ExportFormat::Asm,
            std::env::current_dir().unwrap_or_default(),
        )
    }
}

impl ExportAsDialog {
    #[must_use]
    pub fn new(
        initial_filename: Option<String>,
        format: crate::event::ExportFormat,
        current_dir: PathBuf,
    ) -> Self {
        let mut textarea = TextArea::default();
        if let Some(filename) = initial_filename {
            textarea.insert_str(filename);
            textarea.move_cursor(CursorMove::End);
        }
        let mut dialog = Self {
            textarea,
            format,
            current_dir,
            files: Vec::new(),
            selected_index: 0,
            history: Vec::new(),
            focus: ExportAsFocus::Filename,
            sub_dialog: None,
        };
        dialog.refresh_files();
        dialog
    }

    pub fn refresh_files(&mut self) {
        let ext = match self.format {
            crate::event::ExportFormat::Asm => "asm",
            crate::event::ExportFormat::Html => "html",
        };
        let filter_extensions = vec![ext.to_string()];
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

impl Widget for ExportAsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        if let Some(sub) = &self.sub_dialog {
            self.render_background(f, area, app_state, ui_state);
            sub.render(f, area, app_state, ui_state);
            return;
        }
        self.render_background(f, area, app_state, ui_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if let Some(sub) = &mut self.sub_dialog {
            let res = sub.handle_input(key, app_state, ui_state);
            match res {
                WidgetResult::Close => {
                    self.sub_dialog = None;
                    return WidgetResult::Handled;
                }
                WidgetResult::Action(action) => {
                    self.sub_dialog = None;
                    if let crate::state::actions::AppAction::Confirmed(inner) = &action {
                        match **inner {
                            crate::state::actions::AppAction::ExportAsmAs
                            | crate::state::actions::AppAction::ExportHtmlAs => {
                                return self.execute_export(app_state, ui_state);
                            }
                            _ => {}
                        }
                    }
                    return WidgetResult::Action(action);
                }
                WidgetResult::Handled => return WidgetResult::Handled,
                WidgetResult::Ignored => return WidgetResult::Ignored,
            }
        }
        if key.code == KeyCode::Esc {
            ui_state.set_status_message("Ready");
            return WidgetResult::Close;
        }

        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                ExportAsFocus::Filename => ExportAsFocus::DirectoryList,
                ExportAsFocus::DirectoryList => ExportAsFocus::Filename,
            };
            return WidgetResult::Handled;
        }

        match self.focus {
            ExportAsFocus::DirectoryList => match key.code {
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
                            self.focus = ExportAsFocus::Filename;
                        }
                        WidgetResult::Handled
                    }
                }
                _ => WidgetResult::Handled,
            },
            ExportAsFocus::Filename => match key.code {
                KeyCode::Enter => {
                    let filename = self.textarea.lines().join("").trim().to_string();
                    if filename.is_empty() {
                        WidgetResult::Handled
                    } else {
                        let mut path = self.current_dir.join(&filename);
                        if path.extension().is_none() {
                            let ext = match self.format {
                                crate::event::ExportFormat::Asm => "asm",
                                crate::event::ExportFormat::Html => "html",
                            };
                            path.set_extension(ext);
                        }

                        if path.exists() {
                            let action = match self.format {
                                crate::event::ExportFormat::Asm => {
                                    crate::state::actions::AppAction::ExportAsmAs
                                }
                                crate::event::ExportFormat::Html => {
                                    crate::state::actions::AppAction::ExportHtmlAs
                                }
                            };
                            self.sub_dialog = Some(Box::new(
                                crate::ui::dialog_confirmation::ConfirmationDialog::new(
                                    "File Exists",
                                    format!("Are you sure you want to overwrite '{}'?", filename),
                                    action,
                                ),
                            ));
                            WidgetResult::Handled
                        } else {
                            self.execute_export(app_state, ui_state)
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

impl ExportAsDialog {
    fn render_background(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let block_title = match self.format {
            crate::event::ExportFormat::Asm => " Export to .asm as... (Tab to Switch Focus) ",
            crate::event::ExportFormat::Html => " Export to .html as... (Tab to Switch Focus) ",
        };
        let block = crate::ui::widget::create_dialog_block(block_title, theme);

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
            .border_style(if self.focus == ExportAsFocus::DirectoryList {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let list = List::new(items)
            .block(list_block)
            .highlight_style(if self.focus == ExportAsFocus::DirectoryList {
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .highlight_symbol(if self.focus == ExportAsFocus::DirectoryList {
                ">> "
            } else {
                "   "
            });

        let mut state = ListState::default();
        state.select(Some(self.selected_index));
        f.render_stateful_widget(list, dialog_layout[0], &mut state);

        let input_block = ratatui::widgets::Block::default()
            .title(" Filename ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(if self.focus == ExportAsFocus::Filename {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let input_inner = input_block.inner(dialog_layout[2]);
        f.render_widget(input_block, dialog_layout[2]);

        let ext_text = match self.format {
            crate::event::ExportFormat::Asm => ".asm",
            crate::event::ExportFormat::Html => ".html",
        };

        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length((ext_text.len() + 1) as u16),
            ])
            .split(input_inner);

        let mut textarea = self.textarea.clone();
        let style = Style::default()
            .fg(if self.focus == ExportAsFocus::Filename {
                theme.menu_selected_fg
            } else {
                Color::Reset
            })
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);

        if self.focus == ExportAsFocus::Filename {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_layout[0]);

        let extension = Paragraph::new(ext_text).style(Style::default().fg(Color::Gray));
        f.render_widget(extension, input_layout[1]);
    }

    fn execute_export(&mut self, app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult {
        let filename = self.textarea.lines().join("").trim().to_string();
        let mut path = self.current_dir.join(&filename);
        if path.extension().is_none() {
            let ext = match self.format {
                crate::event::ExportFormat::Asm => "asm",
                crate::event::ExportFormat::Html => "html",
            };
            path.set_extension(ext);
        }
        match self.format {
            crate::event::ExportFormat::Asm => {
                app_state.export_asm_path = Some(path.clone());
            }
            crate::event::ExportFormat::Html => {
                app_state.export_html_path = Some(path.clone());
            }
        }
        let res = match self.format {
            crate::event::ExportFormat::Asm => crate::exporter::export_asm(app_state, &path),
            crate::event::ExportFormat::Html => crate::exporter::export_html(app_state, &path),
        };
        if let Err(e) = res {
            ui_state.set_status_message(format!("Error exporting: {e}"));
            WidgetResult::Handled
        } else {
            match self.format {
                crate::event::ExportFormat::Asm => {
                    app_state.last_export_asm_filename = Some(filename.clone());
                }
                crate::event::ExportFormat::Html => {
                    app_state.last_export_html_filename = Some(filename.clone());
                }
            }
            let saved_filename = path.file_name().unwrap_or_default().to_string_lossy();
            ui_state.set_status_message(format!("Exported: {saved_filename}"));
            WidgetResult::Close
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let dialog =
            ExportAsDialog::new(None, crate::event::ExportFormat::Asm, current_dir.clone());
        assert_eq!(dialog.current_dir, current_dir);
        assert_eq!(dialog.focus, ExportAsFocus::Filename);
        assert_eq!(dialog.selected_index, 0);
        assert!(dialog.sub_dialog.is_none());
        assert_eq!(dialog.history.len(), 0);
    }

    #[test]
    fn test_next_previous() {
        let mut dialog = ExportAsDialog::new(
            None,
            crate::event::ExportFormat::Asm,
            std::env::current_dir().unwrap_or_default(),
        );
        dialog.files = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];
        dialog.selected_index = 0;

        dialog.next();
        assert_eq!(dialog.selected_index, 1);
        dialog.next();
        assert_eq!(dialog.selected_index, 2);
        dialog.next();
        assert_eq!(dialog.selected_index, 0);

        dialog.previous();
        assert_eq!(dialog.selected_index, 2);
        dialog.previous();
        assert_eq!(dialog.selected_index, 1);
    }

    #[test]
    fn test_page_up_down() {
        let mut dialog = ExportAsDialog::new(
            None,
            crate::event::ExportFormat::Asm,
            std::env::current_dir().unwrap_or_default(),
        );
        dialog.files = (0..25)
            .map(|i| PathBuf::from(format!("file_{}", i)))
            .collect();
        dialog.selected_index = 0;

        dialog.page_down();
        assert_eq!(dialog.selected_index, 10);
        dialog.page_down();
        assert_eq!(dialog.selected_index, 20);
        dialog.page_down();
        assert_eq!(dialog.selected_index, 24);

        dialog.page_up();
        assert_eq!(dialog.selected_index, 14);
        dialog.page_up();
        assert_eq!(dialog.selected_index, 4);
        dialog.page_up();
        assert_eq!(dialog.selected_index, 0);
    }
}
