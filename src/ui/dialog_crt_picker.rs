// CRT Bank Picker Dialog
// Shows list of CHIP banks from a CRT cartridge image and allows user to select one

use crate::parser::crt::{CrtChip, CrtHeader};
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

struct CrtPickerEntry {
    chip: CrtChip,
    entropy: f32,
}

pub struct CrtBankPickerDialog {
    chips: Vec<CrtPickerEntry>,
    selected_index: usize,
    file_path: PathBuf,
    crt_header: CrtHeader,
}

impl CrtBankPickerDialog {
    pub fn new(crt_header: CrtHeader, file_path: PathBuf) -> Self {
        let entries = crt_header
            .chips
            .iter()
            .map(|chip| {
                let entropy = crate::utils::calculate_entropy(&chip.data);
                CrtPickerEntry {
                    chip: chip.clone(),
                    entropy,
                }
            })
            .collect();

        Self {
            chips: entries,
            selected_index: 0,
            file_path,
            crt_header,
        }
    }

    fn page_up(&mut self) {
        // Move back by 10 items
        self.selected_index = self.selected_index.saturating_sub(10);
    }

    fn page_down(&mut self) {
        if !self.chips.is_empty() {
            // Advance by 10 items, but don't go past the last item
            self.selected_index = (self.selected_index + 10).min(self.chips.len() - 1);
        }
    }
}

impl Widget for CrtBankPickerDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let file_name = self
            .file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let hardware_name = self.crt_header.get_hardware_name();
        let title = format!(
            " Select Bank from {} [{}] (Enter: Load, Esc: Cancel) ",
            file_name, hardware_name
        );
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(80, 60, 80, 14, area);
        ui_state.active_dialog_area = area;
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let items: Vec<ListItem> = self
            .chips
            .iter()
            .map(|picker_entry| {
                let chip = &picker_entry.chip;
                let start = chip.load_address;
                let end = start.wrapping_add(chip.data.len() as u16).wrapping_sub(1);

                let addr_str = format!("${:04X}-${:04X}", start, end);
                let entropy_str = format!("{:>5.2}", picker_entry.entropy);
                let size_str = format!("${:04X}", chip.data.len());
                let bank_str = format!("Bank {:02}", chip.bank);
                let type_str = format!("Type {:02}", chip.chip_type);

                let content = format!(
                    "{:<10} {:<10} {:<11} Size: {:<6} {}",
                    bank_str, type_str, addr_str, size_str, entropy_str
                );

                let style = Style::default();

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
            KeyCode::Esc => {
                // Cancel and return to file browser
                WidgetResult::Close
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.chips.is_empty() {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.chips.len() - 1;
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.chips.is_empty() {
                    if self.selected_index < self.chips.len() - 1 {
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
                // Load the selected chip
                if self.selected_index < self.chips.len() {
                    let selected_entry = &self.chips[self.selected_index];
                    let chip = &selected_entry.chip;

                    // Set origin and raw data
                    app_state.origin = chip.load_address;

                    match app_state.load_binary(chip.load_address, chip.data.clone()) {
                        Ok(loaded_data) => {
                            crate::ui::dialog_warning::WarningDialog::show_if_needed(
                                loaded_data.entropy_warning,
                                ui_state,
                            );

                            app_state.file_path = Some(self.file_path.clone());
                            WidgetResult::Close
                        }
                        Err(e) => {
                            ui_state.set_status_message(format!("Error loading bank: {}", e));
                            WidgetResult::Handled
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
