use crate::state::{AppState, LabelKind};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
    #[must_use]
    pub fn new(app_state: &AppState) -> Self {
        let mut symbols = Vec::new();
        for (addr, labels) in &app_state.labels {
            for label in labels {
                // Focus on User and System labels, maybe skip Auto if too noisy?
                // User requirement said "global and user-defined".
                // I'll include User and System.
                if label.kind != LabelKind::Auto {
                    symbols.push((addr.0, label.name.clone()));
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

        // Use adaptive centered rect for the main container
        let area = crate::utils::centered_rect_adaptive(60, 60, 60, 12, area);
        ui_state.active_dialog_area = area;

        f.render_widget(ratatui::widgets::Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        // Split into: Search input | Results list | Help text
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input (with border)
                Constraint::Min(1),    // Results list
                Constraint::Length(1), // Help / status line
            ])
            .split(inner);

        // --- Search input with bordered sub-block ---
        let input_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(theme.highlight_fg))
            .style(Style::default().bg(theme.highlight_bg));

        let input_widget = Paragraph::new(self.input.clone()).block(input_block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input_widget, layout[0]);

        // Blinking cursor at end of input
        f.set_cursor_position((layout[0].x + 1 + self.input.len() as u16, layout[0].y + 1));

        // --- Results list ---
        if self.filtered_symbols.is_empty() {
            let msg = if self.input.is_empty() {
                "No symbols defined"
            } else {
                "No matching symbols"
            };
            let empty = Paragraph::new(msg)
                .style(
                    Style::default()
                        .fg(theme.comment)
                        .add_modifier(Modifier::ITALIC),
                )
                .alignment(Alignment::Center);

            // Vertically center the message
            let v_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(1),
                    Constraint::Fill(1),
                ])
                .split(layout[1]);
            f.render_widget(empty, v_layout[1]);
        } else {
            let items: Vec<ListItem> = self
                .filtered_symbols
                .iter()
                .enumerate()
                .map(|(i, (addr, name))| {
                    let style = if i == self.selected_index {
                        Style::default()
                            .bg(theme.selection_bg)
                            .fg(theme.selection_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.foreground)
                    };
                    ListItem::new(format!("${addr:04X}  {name}")).style(style)
                })
                .collect();

            let list = List::new(items).highlight_symbol(">> ");
            f.render_widget(list, layout[1]);
        }

        // --- Help / status line ---
        let count_text = format!(
            " {} of {} symbols │ ↑↓: select │ Enter: go │ Esc: close",
            self.filtered_symbols.len(),
            self.all_symbols.len()
        );
        let help = Paragraph::new(count_text).style(Style::default().fg(theme.comment));
        f.render_widget(help, layout[2]);
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
                        crate::state::actions::AppAction::NavigateToAddress(crate::state::Addr(
                            *addr,
                        )),
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
            crate::state::Addr(0x1000),
            vec![Label {
                name: "MainLoop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        labels.insert(
            crate::state::Addr(0x2000),
            vec![Label {
                name: "DataStart".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        labels.insert(
            crate::state::Addr(0xfffe),
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
