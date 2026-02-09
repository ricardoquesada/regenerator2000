use crate::state::{AppState, LabelKind};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{List, ListItem, Paragraph},
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct GoToSymbolDialog {
    pub input: String,
    pub all_symbols: Vec<(u16, String)>,
    pub filtered_symbols: Vec<(u16, String)>,
    pub selected_index: usize,
}

impl GoToSymbolDialog {
    pub fn new(app_state: &AppState) -> Self {
        let mut symbols = Vec::new();
        for (addr, labels) in &app_state.labels {
            for label in labels {
                // Focus on User and System labels, maybe skip Auto if too noisy?
                // User requirement said "global and user-defined".
                // I'll include User and System.
                if label.kind != LabelKind::Auto {
                    symbols.push((*addr, label.name.clone()));
                }
            }
        }
        // Also include auto labels if they have a non-default name?
        // Actually, let's just include everything for now and see.
        symbols.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

        Self {
            input: String::new(),
            all_symbols: symbols.clone(),
            filtered_symbols: symbols,
            selected_index: 0,
        }
    }

    fn update_filter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered_symbols = self
            .all_symbols
            .iter()
            .filter(|(_, name)| name.to_lowercase().contains(&query))
            .cloned()
            .collect();

        if self.selected_index >= self.filtered_symbols.len() && !self.filtered_symbols.is_empty() {
            self.selected_index = self.filtered_symbols.len() - 1;
        } else if self.filtered_symbols.is_empty() {
            self.selected_index = 0;
        }
    }
}

impl Widget for GoToSymbolDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Go to Symbol ", theme);

        // Dialog layout:
        // Use adaptive centered rect for the main container
        let area = crate::utils::centered_rect_adaptive(60, 60, 60, 10, area);
        ui_state.active_dialog_area = area;

        f.render_widget(ratatui::widgets::Clear, area);

        // Split into Search (top) and Results (bottom)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search box
                Constraint::Min(1),    // Results
            ])
            .split(area);

        let input_widget = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input_widget, layout[0]);

        let items: Vec<ListItem> = self
            .filtered_symbols
            .iter()
            .enumerate()
            .map(|(i, (addr, name))| {
                let style = if i == self.selected_index {
                    Style::default()
                        .bg(theme.selection_bg)
                        .fg(theme.selection_fg)
                } else {
                    Style::default().fg(theme.foreground)
                };
                ListItem::new(format!("${:04X}  {}", addr, name)).style(style)
            })
            .collect();

        let list = List::new(items).block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(theme.dialog_border)),
        );
        f.render_widget(list, layout[1]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Enter => {
                if let Some((addr, _)) = self.filtered_symbols.get(self.selected_index) {
                    crate::ui::menu::execute_menu_action(
                        _app_state,
                        ui_state,
                        crate::ui_state::MenuAction::NavigateToAddress(*addr),
                    );
                }
                WidgetResult::Close
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.update_filter();
                WidgetResult::Handled
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.update_filter();
                WidgetResult::Handled
            }
            KeyCode::Down => {
                if !self.filtered_symbols.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.filtered_symbols.len();
                }
                WidgetResult::Handled
            }
            KeyCode::Up => {
                if !self.filtered_symbols.is_empty() {
                    if self.selected_index == 0 {
                        self.selected_index = self.filtered_symbols.len() - 1;
                    } else {
                        self.selected_index -= 1;
                    }
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, Label, LabelType};
    use std::collections::BTreeMap;

    fn create_mock_app() -> AppState {
        let mut app = AppState::new();
        let mut labels = BTreeMap::new();
        labels.insert(
            0x1000,
            vec![Label {
                name: "MainLoop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        labels.insert(
            0x2000,
            vec![Label {
                name: "DataStart".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        labels.insert(
            0xfffe,
            vec![Label {
                name: "IRQVector".to_string(),
                kind: LabelKind::System,
                label_type: LabelType::Predefined,
            }],
        );
        app.labels = labels;
        app
    }

    #[test]
    fn test_initialization() {
        let app = create_mock_app();
        let dialog = GoToSymbolDialog::new(&app);
        assert_eq!(dialog.all_symbols.len(), 3);
        assert_eq!(dialog.filtered_symbols.len(), 3);
    }

    #[test]
    fn test_filtering() {
        let app = create_mock_app();
        let mut dialog = GoToSymbolDialog::new(&app);

        dialog.input = "main".to_string();
        dialog.update_filter();
        assert_eq!(dialog.filtered_symbols.len(), 1);
        assert_eq!(dialog.filtered_symbols[0].1, "MainLoop");

        dialog.input = "data".to_string();
        dialog.update_filter();
        assert_eq!(dialog.filtered_symbols.len(), 1);
        assert_eq!(dialog.filtered_symbols[0].1, "DataStart");

        dialog.input = "start".to_string();
        dialog.update_filter();
        assert_eq!(dialog.filtered_symbols.len(), 1);
        assert_eq!(dialog.filtered_symbols[0].1, "DataStart");
    }

    #[test]
    fn test_selection_wrap() {
        let app = create_mock_app();
        let mut dialog = GoToSymbolDialog::new(&app);

        // 3 items: index 0, 1, 2
        dialog.selected_index = 2;

        // Key Down
        let key_down = KeyEvent::from(KeyCode::Down);
        dialog.handle_input(
            key_down,
            &mut AppState::new(),
            &mut UIState::new(crate::theme::Theme::default()),
        );
        assert_eq!(dialog.selected_index, 0);

        // Key Up
        let key_up = KeyEvent::from(KeyCode::Up);
        dialog.handle_input(
            key_up,
            &mut AppState::new(),
            &mut UIState::new(crate::theme::Theme::default()),
        );
        assert_eq!(dialog.selected_index, 2);
    }
}
