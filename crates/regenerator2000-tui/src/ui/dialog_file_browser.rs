use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::ui::widget::{Widget, WidgetResult};
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

/// Focus target inside [`FileBrowserDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserFocus {
    Filename,
    DirectoryList,
}

/// Operation mode for [`FileBrowserDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserMode {
    OpenProjectOrFile,
    ImportViceLabels,
    SaveProject,
    Export(crate::event::ExportFormat),
    ExportLabels,
}

impl FileBrowserMode {
    /// Returns `true` when the dialog includes a filename input box and overwrite confirmation.
    #[must_use]
    pub fn is_save_or_export(self) -> bool {
        matches!(
            self,
            Self::SaveProject | Self::Export(_) | Self::ExportLabels
        )
    }

    /// Returns the required target file extension (without leading dot) for save/export modes.
    #[must_use]
    pub fn target_extension(self) -> Option<&'static str> {
        match self {
            Self::SaveProject => Some("regen2000proj"),
            Self::Export(crate::event::ExportFormat::Asm) => Some("asm"),
            Self::Export(crate::event::ExportFormat::Html) => Some("html"),
            Self::ExportLabels => Some("lbl"),
            Self::OpenProjectOrFile | Self::ImportViceLabels => None,
        }
    }

    /// Returns the list of file extensions shown in the directory browser.
    #[must_use]
    fn filter_extensions(self) -> Vec<String> {
        match self {
            Self::OpenProjectOrFile => [
                "bin",
                "prg",
                "crt",
                "vsf",
                "t64",
                "d64",
                "d71",
                "d81",
                "dis65",
                "regen2000proj",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            Self::ImportViceLabels | Self::ExportLabels => vec!["lbl".to_string()],
            Self::SaveProject => vec!["regen2000proj".to_string()],
            Self::Export(crate::event::ExportFormat::Asm) => vec!["asm".to_string()],
            Self::Export(crate::event::ExportFormat::Html) => vec!["html".to_string()],
        }
    }

    /// Returns the modal title bar text for this mode.
    #[must_use]
    fn dialog_title(self) -> &'static str {
        match self {
            Self::OpenProjectOrFile => {
                " Open File (Enter to Open, Backspace to Go Back, Esc to Cancel) "
            }
            Self::ImportViceLabels => {
                " Import VICE Labels (Enter to Import, Backspace to Go Back, Esc to Cancel) "
            }
            Self::SaveProject => " Save Project As (Tab to Switch Focus, Esc to Cancel) ",
            Self::Export(crate::event::ExportFormat::Asm) => {
                " Export to .asm as... (Tab to Switch Focus) "
            }
            Self::Export(crate::event::ExportFormat::Html) => {
                " Export to .html as... (Tab to Switch Focus) "
            }
            Self::ExportLabels => " Export Labels As... (Tab to Switch Focus) ",
        }
    }

    /// Returns the [`AppAction`] wrapped by `ConfirmationDialog` when overwriting an existing file.
    #[must_use]
    fn confirm_action(self) -> Option<AppAction> {
        match self {
            Self::SaveProject => Some(AppAction::SaveAs),
            Self::Export(crate::event::ExportFormat::Asm) => Some(AppAction::ExportAsmAs),
            Self::Export(crate::event::ExportFormat::Html) => Some(AppAction::ExportHtmlAs),
            Self::ExportLabels => Some(AppAction::ExportViceLabels),
            Self::OpenProjectOrFile | Self::ImportViceLabels => None,
        }
    }

    /// Returns `true` if `action` is the confirmation callback for this save/export mode.
    #[must_use]
    fn matches_confirmed_action(self, action: &AppAction) -> bool {
        let AppAction::Confirmed(inner) = action else {
            return false;
        };
        matches!(
            (self, inner.as_ref()),
            (Self::SaveProject, AppAction::SaveAs)
                | (
                    Self::Export(_),
                    AppAction::ExportAsmAs | AppAction::ExportHtmlAs
                )
                | (Self::ExportLabels, AppAction::ExportViceLabels)
        )
    }
}

/// Unified file-browser dialog for opening binaries/projects, importing VICE labels,
/// saving projects, and exporting assembly, HTML, or VICE label files.
pub struct FileBrowserDialog {
    pub mode: FileBrowserMode,
    pub textarea: TextArea<'static>,
    pub current_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected_index: usize,
    pub filter_extensions: Vec<String>,
    pub history: Vec<PathBuf>,
    pub focus: FileBrowserFocus,
    pub sub_dialog: Option<Box<dyn Widget>>,
}

impl FileBrowserDialog {
    #[must_use]
    fn new_with_mode(
        mode: FileBrowserMode,
        initial_filename: Option<String>,
        current_dir: PathBuf,
    ) -> Self {
        let mut textarea = TextArea::default();
        if let Some(filename) = initial_filename {
            textarea.insert_str(filename);
            textarea.move_cursor(CursorMove::End);
        }
        let focus = if mode.is_save_or_export() {
            FileBrowserFocus::Filename
        } else {
            FileBrowserFocus::DirectoryList
        };
        let mut dialog = Self {
            mode,
            textarea,
            current_dir,
            files: Vec::new(),
            selected_index: 0,
            filter_extensions: mode.filter_extensions(),
            history: Vec::new(),
            focus,
            sub_dialog: None,
        };
        dialog.refresh_files();
        dialog
    }

    /// Creates a file browser dialog for opening a binary, container, or project file.
    #[must_use]
    pub fn new_open(current_dir: PathBuf) -> Self {
        Self::new_with_mode(FileBrowserMode::OpenProjectOrFile, None, current_dir)
    }

    /// Creates a file browser dialog for importing VICE `.lbl` labels.
    #[must_use]
    pub fn new_import_vice_labels(current_dir: PathBuf, last_path: Option<PathBuf>) -> Self {
        let start_dir = if let Some(path) = &last_path {
            path.parent()
                .map_or(current_dir, std::path::Path::to_path_buf)
        } else {
            current_dir
        };
        let mut dialog = Self::new_with_mode(FileBrowserMode::ImportViceLabels, None, start_dir);
        if let Some(path) = last_path
            && let Some(filename) = path.file_name()
            && let Some(pos) = dialog
                .files
                .iter()
                .position(|f| f.file_name() == Some(filename))
        {
            dialog.selected_index = pos;
        }
        dialog
    }

    /// Creates a file browser dialog for saving a `.regen2000proj` project file.
    #[must_use]
    pub fn new_save_project(initial_filename: Option<String>, current_dir: PathBuf) -> Self {
        Self::new_with_mode(FileBrowserMode::SaveProject, initial_filename, current_dir)
    }

    /// Creates a file browser dialog for exporting `.asm` or `.html` output.
    #[must_use]
    pub fn new_export(
        initial_filename: Option<String>,
        format: crate::event::ExportFormat,
        current_dir: PathBuf,
    ) -> Self {
        Self::new_with_mode(
            FileBrowserMode::Export(format),
            initial_filename,
            current_dir,
        )
    }

    /// Creates a file browser dialog for exporting VICE `.lbl` labels.
    #[must_use]
    pub fn new_export_labels(initial_filename: Option<String>, current_dir: PathBuf) -> Self {
        Self::new_with_mode(FileBrowserMode::ExportLabels, initial_filename, current_dir)
    }

    /// Reloads the filtered directory entries for `self.current_dir`.
    pub fn refresh_files(&mut self) {
        self.files = crate::utils::list_files(&self.current_dir, &self.filter_extensions);

        if self.current_dir.parent().is_some() {
            self.files.insert(0, self.current_dir.join(".."));
        }

        if self.selected_index >= self.files.len() {
            self.selected_index = 0;
        }
    }

    /// Moves selection to the next entry, wrapping around to index `0`.
    pub fn next(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.files.len();
        }
    }

    /// Moves selection to the previous entry, wrapping around to the last entry.
    pub fn previous(&mut self) {
        if !self.files.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.files.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Moves selection up by 10 entries.
    pub fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    /// Moves selection down by 10 entries, clamped to the last entry.
    pub fn page_down(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 10).min(self.files.len() - 1);
        }
    }

    /// Resolves `filename` inside `self.current_dir`, appending the mode's target extension
    /// unless `filename` already ends with that extension (preserving dotted stems such as `c64_mule.dali`).
    #[must_use]
    pub fn resolve_target_path(&self, filename: &str) -> PathBuf {
        let Some(ext) = self.mode.target_extension() else {
            return self.current_dir.join(filename);
        };
        let has_ext = std::path::Path::new(filename)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext));
        if has_ext {
            self.current_dir.join(filename)
        } else {
            self.current_dir.join(format!("{filename}.{ext}"))
        }
    }

    fn format_directory_items(&self) -> Vec<ListItem<'static>> {
        let parent_path = self.current_dir.join("..");
        self.files
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
            .collect()
    }

    fn render_background(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(self.mode.dialog_title(), theme);

        if !self.mode.is_save_or_export() {
            let area = crate::utils::centered_rect_adaptive(60, 40, 50, 14, area);
            ui_state.active_dialog_area = area;
            f.render_widget(ratatui::widgets::Clear, area);

            let items = self.format_directory_items();
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
            return;
        }

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

        let items = self.format_directory_items();
        let list_block = ratatui::widgets::Block::default()
            .title(format!(
                " Directory: {} ",
                self.current_dir.to_string_lossy()
            ))
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(if self.focus == FileBrowserFocus::DirectoryList {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let list = List::new(items)
            .block(list_block)
            .highlight_style(if self.focus == FileBrowserFocus::DirectoryList {
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .highlight_symbol(if self.focus == FileBrowserFocus::DirectoryList {
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
            .border_style(if self.focus == FileBrowserFocus::Filename {
                Style::default().fg(theme.menu_selected_bg)
            } else {
                Style::default().fg(Color::DarkGray)
            });

        let input_inner = input_block.inner(dialog_layout[2]);
        f.render_widget(input_block, dialog_layout[2]);

        let ext = self.mode.target_extension().unwrap_or("");
        let ext_text = format!(".{ext}");
        let suffix_width = (ext_text.len() + 1) as u16;

        let input_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Fill(1), Constraint::Length(suffix_width)])
            .split(input_inner);

        let mut textarea = self.textarea.clone();
        let style = Style::default()
            .fg(if self.focus == FileBrowserFocus::Filename {
                theme.menu_selected_fg
            } else {
                Color::Reset
            })
            .add_modifier(Modifier::BOLD);
        textarea.set_style(style);

        if self.focus == FileBrowserFocus::Filename {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_layout[0]);

        let extension = Paragraph::new(ext_text).style(Style::default().fg(Color::Gray));
        f.render_widget(extension, input_layout[1]);
    }

    fn execute_save_or_export(
        &mut self,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let filename = self.textarea.lines().join("").trim().to_string();
        let path = self.resolve_target_path(&filename);

        match self.mode {
            FileBrowserMode::SaveProject => {
                let saved_filename = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                ui_state.sync_tui_to_core();
                let ctx = regenerator2000_core::navigation::create_save_context(
                    app_state,
                    &ui_state.core,
                );
                let old_project_path = app_state.project_path.replace(path.clone());

                match app_state.save_project(ctx, true) {
                    Ok(()) => {
                        app_state.project_path = Some(path);
                        app_state.last_save_as_filename = Some(filename);
                        ui_state.set_status_message(format!("Project saved: {saved_filename}"));
                        WidgetResult::Close
                    }
                    Err(e) => {
                        app_state.project_path = old_project_path;
                        ui_state.set_status_message(format!("Error saving: {e}"));
                        WidgetResult::Handled
                    }
                }
            }
            FileBrowserMode::Export(format) => {
                let res = match format {
                    crate::event::ExportFormat::Asm => {
                        crate::exporter::export_asm(app_state, &path)
                    }
                    crate::event::ExportFormat::Html => {
                        crate::exporter::export_html(app_state, &path)
                    }
                };
                if let Err(e) = res {
                    ui_state.set_status_message(format!("Error exporting: {e}"));
                    WidgetResult::Handled
                } else {
                    match format {
                        crate::event::ExportFormat::Asm => {
                            app_state.export_asm_path = Some(path.clone());
                            app_state.last_export_asm_filename = Some(filename);
                        }
                        crate::event::ExportFormat::Html => {
                            app_state.export_html_path = Some(path.clone());
                            app_state.last_export_html_filename = Some(filename);
                        }
                    }
                    let saved_filename = path.file_name().unwrap_or_default().to_string_lossy();
                    ui_state.set_status_message(format!("Exported: {saved_filename}"));
                    WidgetResult::Close
                }
            }
            FileBrowserMode::ExportLabels => match app_state.export_vice_labels(path) {
                Ok(msg) => {
                    app_state.last_export_labels_filename = Some(filename);
                    ui_state.set_status_message(msg);
                    WidgetResult::Close
                }
                Err(e) => {
                    ui_state.set_status_message(format!("Error exporting labels: {e}"));
                    WidgetResult::Handled
                }
            },
            FileBrowserMode::OpenProjectOrFile | FileBrowserMode::ImportViceLabels => {
                WidgetResult::Handled
            }
        }
    }

    fn execute_open_file(
        &self,
        selected_path: PathBuf,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if self.mode == FileBrowserMode::ImportViceLabels {
            return if let Err(e) = app_state.import_vice_labels(selected_path.clone()) {
                ui_state.set_status_message(format!("Error importing labels: {e}"));
                WidgetResult::Handled
            } else {
                let filename = selected_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                app_state.last_import_labels_path = Some(selected_path);
                ui_state.set_status_message(format!("Imported: {filename}"));
                WidgetResult::Close
            };
        }

        if selected_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("crt"))
        {
            match std::fs::read(&selected_path) {
                Ok(crt_data) => match crate::parser::crt::parse_crt_chips(&crt_data) {
                    Ok(crt_header) => {
                        if crt_header.chips.len() > 1 {
                            ui_state.push_dialog(Box::new(
                                crate::ui::dialog_crt_picker::CrtBankPickerDialog::new(
                                    crt_header,
                                    selected_path,
                                ),
                            ));
                            return WidgetResult::Close;
                        }
                    }
                    Err(e) => {
                        ui_state.set_status_message(format!("Error parsing CRT: {e}"));
                        return WidgetResult::Handled;
                    }
                },
                Err(e) => {
                    ui_state.set_status_message(format!("Error reading file: {e}"));
                    return WidgetResult::Handled;
                }
            }
        }

        if selected_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("t64"))
        {
            match std::fs::read(&selected_path) {
                Ok(tape_data) => match crate::parser::t64::parse_t64_directory(&tape_data) {
                    Ok(files) => {
                        if files.is_empty() {
                            ui_state.set_status_message("No files in T64 image");
                            return WidgetResult::Handled;
                        }

                        ui_state.push_dialog(Box::new(
                            crate::ui::dialog_container_picker::ContainerPickerDialog::new_t64(
                                files,
                                tape_data,
                                selected_path,
                            ),
                        ));
                        return WidgetResult::Close;
                    }
                    Err(e) => {
                        ui_state.set_status_message(format!("Error parsing T64: {e}"));
                        return WidgetResult::Handled;
                    }
                },
                Err(e) => {
                    ui_state.set_status_message(format!("Error reading file: {e}"));
                    return WidgetResult::Handled;
                }
            }
        }

        if selected_path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                e.eq_ignore_ascii_case("d64")
                    || e.eq_ignore_ascii_case("d71")
                    || e.eq_ignore_ascii_case("d81")
            })
        {
            match std::fs::read(&selected_path) {
                Ok(disk_data) => match crate::parser::d64::parse_d64_directory(&disk_data) {
                    Ok(files) => {
                        let prg_files: Vec<_> = files
                            .into_iter()
                            .filter(|f| f.file_type == crate::parser::d64::FileType::PRG)
                            .collect();

                        if prg_files.is_empty() {
                            ui_state.set_status_message("No PRG files in disk image");
                            return WidgetResult::Handled;
                        }

                        ui_state.push_dialog(Box::new(
                            crate::ui::dialog_container_picker::ContainerPickerDialog::new_d64(
                                prg_files,
                                disk_data,
                                selected_path,
                            ),
                        ));
                        return WidgetResult::Close;
                    }
                    Err(e) => {
                        ui_state.set_status_message(format!("Error parsing D64/D71: {e}"));
                        return WidgetResult::Handled;
                    }
                },
                Err(e) => {
                    ui_state.set_status_message(format!("Error reading file: {e}"));
                    return WidgetResult::Handled;
                }
            }
        }

        match app_state.load_file(selected_path.clone()) {
            Err(e) => {
                ui_state.set_status_message(format!("Error loading file: {e}"));
                WidgetResult::Handled
            }
            Ok(loaded_data) => {
                let filename = selected_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                ui_state.set_status_message(format!("Loaded: {filename}"));
                ui_state.restore_session(&loaded_data, app_state);
                WidgetResult::Close
            }
        }
    }
}

impl Widget for FileBrowserDialog {
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
                    if self.mode.matches_confirmed_action(&action) {
                        return self.execute_save_or_export(app_state, ui_state);
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

        if key.code == KeyCode::Tab && self.mode.is_save_or_export() {
            self.focus = match self.focus {
                FileBrowserFocus::Filename => FileBrowserFocus::DirectoryList,
                FileBrowserFocus::DirectoryList => FileBrowserFocus::Filename,
            };
            return WidgetResult::Handled;
        }

        match self.focus {
            FileBrowserFocus::DirectoryList => match key.code {
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
                    let Some(selected_path) = self.files.get(self.selected_index).cloned() else {
                        return WidgetResult::Handled;
                    };
                    if selected_path.is_dir() {
                        self.history.push(self.current_dir.clone());
                        self.current_dir = selected_path;
                        self.refresh_files();
                        self.selected_index = 0;
                        ui_state.file_dialog_current_dir = self.current_dir.clone();
                        WidgetResult::Handled
                    } else if let Some(target_ext) = self.mode.target_extension() {
                        if let Some(name) = selected_path.file_name() {
                            let name_str = name.to_string_lossy();
                            let stem = match name_str.rsplit_once('.') {
                                Some((prefix, ext))
                                    if !prefix.is_empty()
                                        && ext.eq_ignore_ascii_case(target_ext) =>
                                {
                                    prefix
                                }
                                _ => &name_str,
                            };
                            self.textarea = TextArea::default();
                            self.textarea.insert_str(stem);
                            self.textarea.move_cursor(CursorMove::End);
                            self.focus = FileBrowserFocus::Filename;
                        }
                        WidgetResult::Handled
                    } else {
                        self.execute_open_file(selected_path, app_state, ui_state)
                    }
                }
                _ => WidgetResult::Handled,
            },
            FileBrowserFocus::Filename => match key.code {
                KeyCode::Enter => {
                    let filename = self.textarea.lines().join("").trim().to_string();
                    if filename.is_empty() {
                        WidgetResult::Handled
                    } else {
                        let path = self.resolve_target_path(&filename);
                        if path.exists()
                            && let Some(action) = self.mode.confirm_action()
                        {
                            self.sub_dialog = Some(Box::new(
                                crate::ui::dialog_confirmation::ConfirmationDialog::new(
                                    "File Exists",
                                    format!("Are you sure you want to overwrite '{filename}'?"),
                                    action,
                                ),
                            ));
                            WidgetResult::Handled
                        } else {
                            self.execute_save_or_export(app_state, ui_state)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization_modes() {
        let current_dir = std::env::current_dir().unwrap_or_default();
        let save = FileBrowserDialog::new_save_project(None, current_dir.clone());
        assert_eq!(save.current_dir, current_dir);
        assert_eq!(save.focus, FileBrowserFocus::Filename);
        assert_eq!(save.selected_index, 0);
        assert!(save.sub_dialog.is_none());

        let open = FileBrowserDialog::new_open(current_dir);
        assert_eq!(open.focus, FileBrowserFocus::DirectoryList);
    }

    #[test]
    fn test_next_previous_and_page_navigation() {
        let mut dialog =
            FileBrowserDialog::new_save_project(None, std::env::current_dir().unwrap_or_default());
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

        dialog.files = (0..25)
            .map(|i| PathBuf::from(format!("file_{i}")))
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
    }

    #[test]
    fn test_resolve_target_path_preserves_dotted_stem_all_modes() {
        let dir = PathBuf::from("/tmp");

        let save = FileBrowserDialog::new_save_project(None, dir.clone());
        assert_eq!(
            save.resolve_target_path("c64_mule.dali"),
            dir.join("c64_mule.dali.regen2000proj")
        );
        assert_eq!(
            save.resolve_target_path("c64_mule.dali.regen2000proj"),
            dir.join("c64_mule.dali.regen2000proj")
        );

        let asm = FileBrowserDialog::new_export(None, crate::event::ExportFormat::Asm, dir.clone());
        assert_eq!(
            asm.resolve_target_path("c64_mule.dali"),
            dir.join("c64_mule.dali.asm")
        );
        assert_eq!(
            asm.resolve_target_path("c64_mule.dali.asm"),
            dir.join("c64_mule.dali.asm")
        );

        let html =
            FileBrowserDialog::new_export(None, crate::event::ExportFormat::Html, dir.clone());
        assert_eq!(
            html.resolve_target_path("c64_mule.dali"),
            dir.join("c64_mule.dali.html")
        );

        let lbl = FileBrowserDialog::new_export_labels(None, dir.clone());
        assert_eq!(
            lbl.resolve_target_path("c64_mule.dali"),
            dir.join("c64_mule.dali.lbl")
        );
        assert_eq!(
            lbl.resolve_target_path("c64_mule.dali.lbl"),
            dir.join("c64_mule.dali.lbl")
        );
    }

    #[test]
    fn test_select_existing_file_strips_only_target_extension() {
        let dir = PathBuf::from("/tmp");
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        for (mut dialog, file_name) in [
            (
                FileBrowserDialog::new_save_project(None, dir.clone()),
                "c64_mule.dali.regen2000proj",
            ),
            (
                FileBrowserDialog::new_export(None, crate::event::ExportFormat::Asm, dir.clone()),
                "c64_mule.dali.asm",
            ),
            (
                FileBrowserDialog::new_export_labels(None, dir.clone()),
                "c64_mule.dali.lbl",
            ),
        ] {
            dialog.files = vec![dir.join(file_name)];
            dialog.selected_index = 0;
            dialog.focus = FileBrowserFocus::DirectoryList;
            dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            );
            assert_eq!(dialog.textarea.lines().join(""), "c64_mule.dali");
            assert_eq!(dialog.focus, FileBrowserFocus::Filename);
        }
    }
}
