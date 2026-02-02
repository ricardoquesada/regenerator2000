// D64 File Picker Dialog
// Shows list of PRG files from a D64 disk image and allows user to select one

use crate::parser::d64::{D64FileEntry, FileType};
use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use std::path::PathBuf;

pub struct D64FilePickerDialog {
    files: Vec<D64FileEntry>,
    selected_index: usize,
    disk_data: Vec<u8>,
    disk_path: PathBuf,
}

impl D64FilePickerDialog {
    pub fn new(files: Vec<D64FileEntry>, disk_data: Vec<u8>, disk_path: PathBuf) -> Self {
        Self {
            files,
            selected_index: 0,
            disk_data,
            disk_path,
        }
    }
}

impl Widget for D64FilePickerDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let disk_name = self
            .disk_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let title = format!(" Select PRG from {} (Enter: Load, Esc: Cancel) ", disk_name);
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect(60, 60, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let items: Vec<ListItem> = self
            .files
            .iter()
            .map(|entry| {
                let filename = if entry.filename.len() > 40 {
                    format!("{}...", &entry.filename[..37])
                } else {
                    entry.filename.clone()
                };

                let content = format!(
                    "{:<40} {:>3} {:>5} blocks",
                    filename,
                    entry.file_type.as_str(),
                    entry.size_sectors
                );

                let is_prg = entry.file_type == FileType::PRG;

                // Dim non-PRG files when not selected (selection style handles the rest)
                let style = if !is_prg {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
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
        key: crossterm::event::KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                // Cancel and return to file browser
                WidgetResult::Close
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < self.files.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                // Load the selected file
                let selected_entry = &self.files[self.selected_index];

                if selected_entry.file_type != FileType::PRG {
                    ui_state.set_status_message(format!(
                        "Cannot load {} file: only PRG files supported",
                        selected_entry.file_type.as_str()
                    ));
                    return WidgetResult::Handled;
                }

                match crate::parser::d64::extract_file(&self.disk_data, selected_entry) {
                    Ok((load_address, program_data)) => {
                        // Set origin and raw data
                        app_state.origin = load_address;
                        // Let's redo.
                        match app_state.load_binary(load_address, program_data) {
                            Ok(loaded_data) => {
                                // Apply loaded UI state if needed (like cursor pos), though load_binary defaults them.
                                crate::ui::dialog_warning::WarningDialog::show_if_needed(
                                    loaded_data.entropy_warning,
                                    ui_state,
                                );

                                app_state.file_path = Some(self.disk_path.clone());
                                WidgetResult::Close
                            }
                            Err(e) => {
                                ui_state.set_status_message(format!("Error loading file: {}", e));
                                WidgetResult::Handled
                            }
                        }
                    }
                    Err(e) => {
                        ui_state.set_status_message(format!("Error loading file: {}", e));
                        WidgetResult::Handled
                    }
                }
            }
            _ => WidgetResult::Handled,
        }
    }
}
