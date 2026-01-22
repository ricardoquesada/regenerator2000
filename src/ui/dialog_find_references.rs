use crate::state::AppState;
use crate::ui::menu::MenuAction;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{List, ListItem, ListState},
};

pub struct FindReferencesDialog {
    pub target_address: u16,
    pub references: Vec<u16>,
    pub selected_index: usize,
    pub list_items: Vec<String>,
}

impl FindReferencesDialog {
    pub fn new(app_state: &AppState, target_address: u16) -> Self {
        let mut refs = app_state
            .cross_refs
            .get(&target_address)
            .cloned()
            .unwrap_or_default();
        // Sort references for consistent display
        refs.sort_unstable();
        refs.dedup();

        let mut list_items = Vec::new();

        if refs.is_empty() {
            list_items.push("No references found".to_string());
        } else {
            for ref_addr in &refs {
                let mut text = format!("${:04X}", ref_addr);

                // Find line to get instruction details
                if let Some(idx) = app_state.get_line_index_for_address(*ref_addr) {
                    if let Some(line) = app_state.disassembly.get(idx) {
                        text.push_str(&format!("  {} {}", line.mnemonic, line.operand));
                    }
                } else if let Some(idx) = app_state.get_line_index_containing_address(*ref_addr) {
                    // Inside a block (e.g. data block)
                    if let Some(line) = app_state.disassembly.get(idx) {
                        text.push_str(&format!(
                            "  {} ${:04X} (Inside Block)",
                            line.mnemonic, line.address
                        ));
                    }
                }
                list_items.push(text);
            }
        }

        Self {
            target_address,
            references: refs,
            selected_index: 0,
            list_items,
        }
    }

    pub fn next(&mut self) {
        if !self.references.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.references.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.references.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.references.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }
}

impl Widget for FindReferencesDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = format!(" References to ${:04X} ", self.target_address);
        let block = crate::ui::widget::create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect(60, 50, area);
        f.render_widget(ratatui::widgets::Clear, area);

        let items: Vec<ListItem> = self
            .list_items
            .iter()
            .map(|t| ListItem::new(t.clone()))
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
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if !self.references.is_empty() {
                    let addr = self.references[self.selected_index];
                    // Close the dialog and navigate
                    ui_state.active_dialog = None;
                    WidgetResult::Action(MenuAction::NavigateToAddress(addr))
                } else {
                    WidgetResult::Close
                }
            }
            _ => WidgetResult::Handled,
        }
    }
}
