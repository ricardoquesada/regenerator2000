// D64 File Picker Dialog
// Shows list of PRG files from a D64 disk image and allows user to select one

use crate::parser::d64::{D64FileEntry, FileType};
use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, _ui_state: &mut UIState) {
        let area = crate::utils::centered_rect(60, 60, area);

        // Split area into title, file list, and help
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(10),   // File list
                Constraint::Length(3), // Help
            ])
            .split(area);

        // Render title
        let disk_name = self
            .disk_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");
        let title = Paragraph::new(format!("Select PRG file from: {}", disk_name))
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Render file list
        let items: Vec<ListItem> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, entry)| {
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
                let is_selected = i == self.selected_index;

                let style = match (is_selected, is_prg) {
                    (true, true) => Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                    (true, false) => Style::default().fg(Color::DarkGray).bg(Color::Gray),
                    (false, true) => Style::default().fg(Color::White),
                    (false, false) => Style::default().fg(Color::DarkGray),
                };

                ListItem::new(Line::from(Span::styled(content, style)))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} files ", self.files.len())),
        );

        f.render_widget(list, chunks[1]);

        // Render help
        let help = Paragraph::new("↑/↓: Navigate  Enter: Load  Esc: Cancel")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(help, chunks[2]);
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
