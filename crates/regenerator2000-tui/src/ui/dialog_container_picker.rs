use crate::parser::d64::{D64FileEntry, DiskType, FileType};
use crate::parser::t64::T64Entry;
use crate::state::types::System;
use crate::state::{Addr, AppState};
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use std::path::PathBuf;

/// Container image format displayed by [`ContainerPickerDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    /// D64 / D71 / D81 floppy disk image.
    D64,
    /// T64 tape archive image.
    T64,
}

enum ContainerEntrySource {
    D64(D64FileEntry),
    T64(T64Entry),
}

struct ContainerPickerEntry {
    source: ContainerEntrySource,
    start_addr: Option<u16>,
    end_addr: Option<u16>,
    entropy: Option<f32>,
    t64_size_bytes: usize,
}

impl ContainerPickerEntry {
    fn from_d64(entry: D64FileEntry, container_data: &[u8]) -> Self {
        let mut start_addr = None;
        let mut end_addr = None;
        let mut entropy = None;

        if entry.file_type == FileType::PRG
            && let Ok(prg_bytes) = crate::parser::d64::extract_file(container_data, &entry)
            && let Ok(prg_data) = crate::parser::prg::parse_prg(&prg_bytes)
        {
            start_addr = Some(prg_data.origin);
            if prg_data.raw_data.is_empty() {
                end_addr = Some(prg_data.origin);
                entropy = Some(0.0);
            } else {
                end_addr = Some(
                    prg_data
                        .origin
                        .wrapping_add(prg_data.raw_data.len().saturating_sub(1) as u16),
                );
                entropy = Some(crate::utils::calculate_entropy(&prg_data.raw_data));
            }
        }

        Self {
            source: ContainerEntrySource::D64(entry),
            start_addr,
            end_addr,
            entropy,
            t64_size_bytes: 0,
        }
    }

    fn from_t64(entry: T64Entry, container_data: &[u8]) -> Self {
        let mut entropy = None;
        let mut size = 0;

        if let Ok(prg_bytes) = crate::parser::t64::extract_file(container_data, &entry)
            && let Ok(prg_data) = crate::parser::prg::parse_prg(&prg_bytes)
        {
            entropy = if prg_data.raw_data.is_empty() {
                Some(0.0)
            } else {
                Some(crate::utils::calculate_entropy(&prg_data.raw_data))
            };
            size = prg_data.raw_data.len();
        }

        let start_addr = Some(entry.start_address);
        let end_addr = Some(entry.end_address);

        Self {
            source: ContainerEntrySource::T64(entry),
            start_addr,
            end_addr,
            entropy,
            t64_size_bytes: size,
        }
    }

    fn truncate_filename(name: &str) -> String {
        if name.chars().count() > 37 {
            let prefix: String = name.chars().take(34).collect();
            format!("{prefix}...")
        } else {
            name.to_string()
        }
    }

    fn format_row(&self) -> (String, bool) {
        let addr_str = if let (Some(start), Some(end)) = (self.start_addr, self.end_addr) {
            format!("${start:04X}-${end:04X}")
        } else {
            String::new()
        };

        let entropy_str = if let Some(e) = self.entropy {
            format!("{e:>5.2}")
        } else {
            "     ".to_string()
        };

        match &self.source {
            ContainerEntrySource::D64(entry) => {
                let filename = Self::truncate_filename(&entry.filename);
                let content = format!(
                    "{:<37} {:>3} {:>4} blks  {:<11}  {}",
                    filename,
                    entry.file_type.as_str(),
                    entry.size_sectors,
                    addr_str,
                    entropy_str
                );
                (content, entry.file_type == FileType::PRG)
            }
            ContainerEntrySource::T64(entry) => {
                let filename = Self::truncate_filename(&entry.filename);
                let type_str = if entry.file_type == 1 { "PRG" } else { "???" };
                let size_str = format!("{:>5}", self.t64_size_bytes);
                let content = format!(
                    "{filename:<37} {type_str:>3} {size_str} bytes  {addr_str:<11}  {entropy_str}"
                );
                (content, entry.file_type == 1)
            }
        }
    }
}

/// Unified dialog for selecting and loading a PRG entry from a D64/D71/D81 disk
/// image or a T64 tape archive.
pub struct ContainerPickerDialog {
    kind: ContainerKind,
    files: Vec<ContainerPickerEntry>,
    selected_index: usize,
    container_data: Vec<u8>,
    container_path: PathBuf,
}

impl ContainerPickerDialog {
    /// Creates a picker dialog for a D64/D71/D81 disk image.
    #[must_use]
    pub fn new_d64(
        files: Vec<D64FileEntry>,
        container_data: Vec<u8>,
        container_path: PathBuf,
    ) -> Self {
        let entries = files
            .into_iter()
            .map(|entry| ContainerPickerEntry::from_d64(entry, &container_data))
            .collect();

        Self {
            kind: ContainerKind::D64,
            files: entries,
            selected_index: 0,
            container_data,
            container_path,
        }
    }

    /// Creates a picker dialog for a T64 tape archive.
    #[must_use]
    pub fn new_t64(files: Vec<T64Entry>, container_data: Vec<u8>, container_path: PathBuf) -> Self {
        let entries = files
            .into_iter()
            .map(|entry| ContainerPickerEntry::from_t64(entry, &container_data))
            .collect();

        Self {
            kind: ContainerKind::T64,
            files: entries,
            selected_index: 0,
            container_data,
            container_path,
        }
    }

    fn page_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    fn page_down(&mut self) {
        if !self.files.is_empty() {
            self.selected_index = (self.selected_index + 10).min(self.files.len() - 1);
        }
    }

    fn load_selected(&self, app_state: &mut AppState, ui_state: &mut UIState) -> WidgetResult {
        let Some(picker_entry) = self.files.get(self.selected_index) else {
            return WidgetResult::Handled;
        };

        let (extract_result, default_system) = match &picker_entry.source {
            ContainerEntrySource::D64(entry) => {
                if entry.file_type != FileType::PRG {
                    ui_state.set_status_message(format!(
                        "Cannot load {} file: only PRG files supported",
                        entry.file_type.as_str()
                    ));
                    return WidgetResult::Handled;
                }
                let system = match entry.disk_type {
                    DiskType::D64 => System::new(System::C64),
                    DiskType::D71_70 | DiskType::D71_80 | DiskType::D81 => {
                        System::new(System::C128)
                    }
                };
                (
                    crate::parser::d64::extract_file(&self.container_data, entry),
                    system,
                )
            }
            ContainerEntrySource::T64(entry) => {
                if entry.file_type != 1 {
                    ui_state.set_status_message(format!(
                        "Cannot load T64 file type {}: only Type 1 (PRG) supported",
                        entry.file_type
                    ));
                    return WidgetResult::Handled;
                }
                (
                    crate::parser::t64::extract_file(&self.container_data, entry),
                    System::new(System::C64),
                )
            }
        };

        match extract_result {
            Ok(prg_bytes) => match crate::parser::prg::parse_prg(&prg_bytes) {
                Ok(prg_data) => {
                    match app_state.load_binary(Addr(prg_data.origin), prg_data.raw_data) {
                        Ok(mut loaded_data) => {
                            loaded_data.suggested_system =
                                prg_data.suggested_system.or(Some(default_system));
                            loaded_data.suggested_entry_point =
                                prg_data.suggested_entry_point.map(Addr);
                            let abs_path = std::fs::canonicalize(&self.container_path)
                                .unwrap_or_else(|_| self.container_path.clone());
                            app_state.system_config.add_recent_project(abs_path);
                            let _ = app_state.system_config.save();
                            app_state.file_path = Some(self.container_path.clone());
                            ui_state.restore_session(&loaded_data, app_state);
                            WidgetResult::Close
                        }
                        Err(e) => {
                            ui_state.set_status_message(format!("Error loading file: {e}"));
                            WidgetResult::Handled
                        }
                    }
                }
                Err(e) => {
                    ui_state.set_status_message(format!("Error parsing PRG: {e}"));
                    WidgetResult::Handled
                }
            },
            Err(e) => {
                ui_state.set_status_message(format!("Error loading file: {e}"));
                WidgetResult::Handled
            }
        }
    }
}

impl Widget for ContainerPickerDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let container_name = self
            .container_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let noun = match self.kind {
            ContainerKind::D64 => "PRG",
            ContainerKind::T64 => "File",
        };
        let title = format!(" Select {noun} from {container_name} (Enter: Load, Esc: Cancel) ");
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(80, 60, 80, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area);

        let items: Vec<ListItem> = self
            .files
            .iter()
            .map(|picker_entry| {
                let (content, is_supported) = picker_entry.format_row();
                let style = if is_supported {
                    Style::default()
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                ListItem::new(Line::from(Span::styled(content, style)))
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
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                if !self.files.is_empty() {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.files.len() - 1;
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                if !self.files.is_empty() {
                    if self.selected_index < self.files.len() - 1 {
                        self.selected_index += 1;
                    } else {
                        self.selected_index = 0;
                    }
                }
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
            KeyCode::Enter => self.load_selected(app_state, ui_state),
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_d64_picker_multibyte_filename_render_does_not_panic() {
        let exact_37 = "♠".repeat(37);
        assert_eq!(
            ContainerPickerEntry::truncate_filename(&exact_37),
            exact_37,
            "37-char multibyte PETSCII filename must not be truncated"
        );
        let over_37 = "♠".repeat(40);
        let truncated = ContainerPickerEntry::truncate_filename(&over_37);
        assert_eq!(truncated.chars().count(), 37);
        assert_eq!(truncated, format!("{}...", "♠".repeat(34)));

        let entry = D64FileEntry {
            filename: over_37,
            file_type: FileType::PRG,
            track: 1,
            sector: 0,
            size_sectors: 10,
            disk_type: DiskType::D64,
        };
        let dialog =
            ContainerPickerDialog::new_d64(vec![entry], Vec::new(), PathBuf::from("test.d64"));
        let app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains(&truncated));
    }

    #[test]
    fn test_d64_picker_empty_files_enter_is_handled() {
        let mut dialog =
            ContainerPickerDialog::new_d64(Vec::new(), Vec::new(), PathBuf::from("empty.d64"));
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let result = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(result, WidgetResult::Handled);
    }

    #[test]
    fn test_t64_picker_multibyte_filename_render_does_not_panic() {
        let entry = T64Entry {
            file_type: 1,
            start_address: 0x0801,
            end_address: 0x0900,
            offset: 64,
            filename: "█".repeat(40),
        };
        let dialog =
            ContainerPickerDialog::new_t64(vec![entry], Vec::new(), PathBuf::from("test.t64"));
        let app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(rendered.contains(&format!("{}...", "█".repeat(34))));
    }

    #[test]
    fn test_t64_picker_empty_files_enter_is_handled() {
        let mut dialog =
            ContainerPickerDialog::new_t64(Vec::new(), Vec::new(), PathBuf::from("empty.t64"));
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        let result = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(result, WidgetResult::Handled);
    }

    #[test]
    fn test_container_picker_navigation_and_unsupported_file_type_rejection() {
        let entries = vec![
            D64FileEntry {
                filename: "README".to_string(),
                file_type: FileType::SEQ,
                track: 1,
                sector: 0,
                size_sectors: 2,
                disk_type: DiskType::D64,
            },
            D64FileEntry {
                filename: "GAME".to_string(),
                file_type: FileType::PRG,
                track: 1,
                sector: 1,
                size_sectors: 5,
                disk_type: DiskType::D64,
            },
        ];
        let mut dialog =
            ContainerPickerDialog::new_d64(entries, Vec::new(), PathBuf::from("test.d64"));
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(crate::theme::Theme::default());

        // Enter on SEQ entry is rejected with a status message.
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert_eq!(
            ui_state.status_message.as_deref(),
            Some("Cannot load SEQ file: only PRG files supported")
        );

        // Up from 0 wraps to last item (1); Down from 1 wraps back to 0.
        dialog.handle_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.selected_index, 1);
        dialog.handle_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.selected_index, 0);

        // Esc closes the picker.
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
    }
}
