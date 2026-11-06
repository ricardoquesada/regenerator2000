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
pub enum ExportLabelsFocus {
    Filename,
    DirectoryList,
}

pub struct ExportLabelsDialog {
    pub textarea: TextArea<'static>,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub history: Vec<PathBuf>,
    pub focus: ExportLabelsFocus,
    pub sub_dialog: Option<Box<dyn Widget>>,
}

impl Default for ExportLabelsDialog {
    fn default() -> Self {
        Self::new(None, std::env::current_dir().unwrap_or_default())
    }
}

impl ExportLabelsDialog {
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
            focus: ExportLabelsFocus::Filename,
            sub_dialog: None,
        };
        dialog.refresh_files();
        dialog
    }

    pub fn refresh_files(&mut self) {
        let filter_extensions = vec!["lbl".to_string()];
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

    #[must_use]
    pub fn resolve_target_path(&self, filename: &str) -> PathBuf {
        let ext = "lbl";
        let has_ext = std::path::Path::new(filename)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext));
        if has_ext {
            self.current_dir.join(filename)
        } else {
            self.current_dir.join(format!("{filename}.{ext}"))
        }
    }
}

impl Widget for ExportLabelsDialog {
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
                    if let crate::state::actions::AppAction::Confirmed(inner) = &action
                        && matches!(**inner, crate::state::actions::AppAction::ExportViceLabels)
                    {
                        return self.execute_export(app_state, ui_state);
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
                ExportLabelsFocus::Filename => ExportLabelsFocus::DirectoryList,
                ExportLabelsFocus::DirectoryList => ExportLabelsFocus::Filename,
            };
            return WidgetResult::Handled;
        }

        match self.focus {
            ExportLabelsFocus::DirectoryList => match key.code {
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
                        ui_state.file_dialog_current_dir = self.current_dir.clone();
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
                            ui_state.file_dialog_current_dir = self.current_dir.clone();
                        } else if let Some(name) = selected_path.file_name() {
                            let name_str = name.to_string_lossy();
                            let stem = match name_str.rsplit_once('.') {
                                Some((prefix, ext))
                                    if !prefix.is_empty() && ext.eq_ignore_ascii_case("lbl") =>
                                {
                                    prefix
                                }
                                _ => &name_str,
                            };
                            self.textarea = TextArea::default();
                            self.textarea.insert_str(stem);
                            self.textarea.move_cursor(CursorMove::End);
                            self.focus = ExportLabelsFocus::Filename;
                        }
                        WidgetResult::Handled
                    }
                }
                _ => WidgetResult::Handled,
            },
            ExportLabelsFocus::Filename => match key.code {
                KeyCode::Enter => {
                    let filename = self.textarea.lines().join("").trim().to_string();
                    if filename.is_empty() {
                        WidgetResult::Handled
                    } else {
                        let path = self.resolve_target_path(&filename);

                        if path.exists() {
                            self.sub_dialog = Some(Box::new(
                                crate::ui::dialog_confirmation::ConfirmationDialog::new(
                                    "File Exists",
                                    format!("Are you sure you want to overwrite '{}'?", filename),
                                    crate::state::actions::AppAction::ExportViceLabels,
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

impl ExportLabelsDialog {
    fn render_background(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(
            " Export Labels As... (Tab to Switch Focus) ",
            theme,
        );

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
            .border_style(if self.focus == ExportLabelsFocus::DirectoryList {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let list = List::new(items)
            .block(list_block)
            .highlight_style(if self.focus == ExportLabelsFocus::DirectoryList {
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .highlight_symbol(if self.focus == ExportLabelsFocus::DirectoryList {
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
            .border_style(if self.focus == ExportLabelsFocus::Filename {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let input_inner = input_block.inner(dialog_layout[2]);
        f.render_widget(input_block, dialog_layout[2]);

        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(5)])
            .split(input_inner);

        let mut textarea = self.textarea.clone();
        let style = Style::default()
            .fg(if self.focus == ExportLabelsFocus::Filename {
                theme.menu_selected_fg
            } else {
                Color::Reset
            })
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);

        if self.focus == ExportLabelsFocus::Filename {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_layout[0]);

        let extension = Paragraph::new(".lbl").style(Style::default().fg(Color::Gray));
        f.render_widget(extension, input_layout[1]);
    }

    fn execute_export(&mut self, app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult {
        let filename = self.textarea.lines().join("").trim().to_string();
        let path = self.resolve_target_path(&filename);
        match app_state.export_vice_labels(path) {
            Ok(msg) => {
                app_state.last_export_labels_filename = Some(filename);
                ui_state.set_status_message(msg);
                WidgetResult::Close
            }
            Err(e) => {
                ui_state.set_status_message(format!("Error exporting labels: {e}"));
                WidgetResult::Handled
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let dialog = ExportLabelsDialog::new(None, current_dir.clone());
        assert_eq!(dialog.current_dir, current_dir);
        assert_eq!(dialog.focus, ExportLabelsFocus::Filename);
        assert_eq!(dialog.selected_index, 0);
        assert!(dialog.sub_dialog.is_none());
        assert_eq!(dialog.history.len(), 0);
    }

    #[test]
    fn test_next_previous() {
        let mut dialog = ExportLabelsDialog::new(None, std::env::current_dir().unwrap_or_default());
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
        let mut dialog = ExportLabelsDialog::new(None, std::env::current_dir().unwrap_or_default());
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

    #[test]
    fn test_resolve_target_path_preserves_dotted_stem() {
        let dir = PathBuf::from("/tmp");
        let dialog = ExportLabelsDialog::new(None, dir.clone());
        assert_eq!(
            dialog.resolve_target_path("c64_mule.dali"),
            dir.join("c64_mule.dali.lbl")
        );
        assert_eq!(
            dialog.resolve_target_path("c64_mule.dali.lbl"),
            dir.join("c64_mule.dali.lbl")
        );
    }

    #[test]
    fn test_select_existing_labels_file_strips_only_lbl_extension() {
        let dir = PathBuf::from("/tmp");
        let mut dialog = ExportLabelsDialog::new(None, dir.clone());
        dialog.files = vec![dir.join("c64_mule.dali.lbl")];
        dialog.selected_index = 0;
        dialog.focus = ExportLabelsFocus::DirectoryList;

        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );

        assert_eq!(dialog.textarea.lines().join(""), "c64_mule.dali");
        assert_eq!(dialog.focus, ExportLabelsFocus::Filename);
    }
}
