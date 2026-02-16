// T64 File Picker Dialog
// Shows list of files from a T64 tape image and allows user to select one

use crate::parser::t64::T64Entry;
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

struct T64FilePickerEntry {
    entry: T64Entry,
    entropy: Option<f32>,
    size: usize,
}

pub struct T64FilePickerDialog {
    files: Vec<T64FilePickerEntry>,
    selected_index: usize,
    disk_data: Vec<u8>,
    disk_path: PathBuf,
}

impl T64FilePickerDialog {
    pub fn new(files: Vec<T64Entry>, disk_data: Vec<u8>, disk_path: PathBuf) -> Self {
        let entries = files
            .into_iter()
            .map(|entry| {
                let mut entropy = None;
                let mut size = 0;

                // Extract to calculate entropy and size
                // T64 file type 1 is normal file. Others might be special.
                if let Ok((_start, data)) = crate::parser::t64::extract_file(&disk_data, &entry) {
                    if !data.is_empty() {
                        entropy = Some(crate::utils::calculate_entropy(&data));
                    } else {
                        entropy = Some(0.0);
                    }
                    size = data.len();
                }

                T64FilePickerEntry {
                    entry,
                    entropy,
                    size,
                }
            })
            .collect();

        Self {
            files: entries,
            selected_index: 0,
            disk_data,
            disk_path,
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
}

impl Widget for T64FilePickerDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let disk_name = self
            .disk_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let title = format!(
            " Select File from {} (Enter: Load, Esc: Cancel) ",
            disk_name
        );
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(80, 60, 80, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let items: Vec<ListItem> = self
            .files
            .iter()
            .map(|picker_entry| {
                let entry = &picker_entry.entry;
                let filename = if entry.filename.len() > 37 {
                    format!("{}...", &entry.filename[..34])
                } else {
                    entry.filename.clone()
                };

                let addr_str = format!("${:04X}-${:04X}", entry.start_address, entry.end_address);

                let entropy_str = if let Some(e) = picker_entry.entropy {
                    format!("{:>5.2}", e)
                } else {
                    "     ".to_string()
                };

                let size_str = format!("{:>5}", picker_entry.size);

                // For T64, almost everything is a PRG type 1, but let's check
                let type_str = if entry.file_type == 1 { "PRG" } else { "???" };

                let content = format!(
                    "{:<37} {:>3} {} bytes  {:<11}  {}",
                    filename, type_str, size_str, addr_str, entropy_str
                );

                let is_supported = entry.file_type == 1;

                let style = if !is_supported {
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
        use crossterm::event::{KeyCode, KeyModifiers};

        match key.code {
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.files.is_empty() {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.files.len() - 1;
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') => {
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
            KeyCode::Enter => {
                if self.files.is_empty() {
                    return WidgetResult::Handled;
                }

                // Load the selected file
                let selected_entry = &self.files[self.selected_index].entry;

                // Type 1 is normal file "PRG"
                if selected_entry.file_type != 1 {
                    ui_state.set_status_message(format!(
                        "Cannot load T64 file type {}: only Type 1 (PRG) supported",
                        selected_entry.file_type
                    ));
                    return WidgetResult::Handled;
                }

                match crate::parser::t64::extract_file(&self.disk_data, selected_entry) {
                    Ok((load_address, program_data)) => {
                        app_state.origin = load_address;
                        match app_state.load_binary(load_address, program_data) {
                            Ok(loaded_data) => {
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
