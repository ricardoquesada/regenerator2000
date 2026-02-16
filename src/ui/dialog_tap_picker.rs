// TAP File Picker Dialog
// Shows list of programs from a TAP tape image and allows user to select one

use crate::parser::tap::TapEntry;
use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use std::path::PathBuf;

struct TapFilePickerEntry {
    entry: TapEntry,
    entropy: Option<f32>,
}

pub struct TapFilePickerDialog {
    files: Vec<TapFilePickerEntry>,
    selected_index: usize,
    tape_data: Vec<u8>,
    tape_path: PathBuf,
}

impl TapFilePickerDialog {
    pub fn new(entries: Vec<TapEntry>, tape_data: Vec<u8>, tape_path: PathBuf) -> Self {
        let picker_entries = entries
            .into_iter()
            .map(|entry| {
                let mut entropy = None;

                // Calculate entropy for the program data
                if let Ok((_start, data)) =
                    crate::parser::tap::extract_tap_program(&tape_data, &entry)
                {
                    if !data.is_empty() {
                        entropy = Some(crate::utils::calculate_entropy(&data));
                    } else {
                        entropy = Some(0.0);
                    }
                }

                TapFilePickerEntry { entry, entropy }
            })
            .collect();

        Self {
            files: picker_entries,
            selected_index: 0,
            tape_data,
            tape_path,
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

impl Widget for TapFilePickerDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let tape_name = self
            .tape_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let title = format!(
            " Select Program from {} (Enter: Load, Esc: Cancel) ",
            tape_name
        );
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(80, 60, 80, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let items: Vec<ListItem> = self
            .files
            .iter()
            .enumerate()
            .map(|(idx, picker_entry)| {
                let entry = &picker_entry.entry;

                // TAP files don't have embedded filenames, so we synthesize one
                let filename = format!("Program {}", idx + 1);
                let filename = if filename.len() > 37 {
                    format!("{}...", &filename[..34])
                } else {
                    filename
                };

                let addr_str = format!("${:04X}-${:04X}", entry.start_addr, entry.end_addr);

                let entropy_str = if let Some(e) = picker_entry.entropy {
                    format!("{:>5.2}", e)
                } else {
                    "     ".to_string()
                };

                let size = (entry.end_addr.saturating_sub(entry.start_addr) as usize) + 1;
                let size_str = format!("{:>5}", size);

                let content = format!(
                    "{:<37} PRG {} bytes  {:<11}  {}",
                    filename, size_str, addr_str, entropy_str
                );

                ListItem::new(Line::from(Span::styled(content, Style::default())))
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

                // Load the selected program
                let selected_entry = &self.files[self.selected_index].entry;

                match crate::parser::tap::extract_tap_program(&self.tape_data, selected_entry) {
                    Ok((load_address, program_data)) => {
                        app_state.origin = load_address;
                        match app_state.load_binary(load_address, program_data) {
                            Ok(loaded_data) => {
                                crate::ui::dialog_warning::WarningDialog::show_if_needed(
                                    loaded_data.entropy_warning,
                                    ui_state,
                                );

                                app_state.file_path = Some(self.tape_path.clone());
                                WidgetResult::Close
                            }
                            Err(e) => {
                                ui_state.set_status_message(format!("Error loading file: {}", e));
                                WidgetResult::Handled
                            }
                        }
                    }
                    Err(e) => {
                        ui_state.set_status_message(format!("Error loading program: {}", e));
                        WidgetResult::Handled
                    }
                }
            }
            _ => WidgetResult::Handled,
        }
    }
}
