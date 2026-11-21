//! Modal dialog (`Ctrl+P`) for searching and jumping to defined symbols.

use crate::state::actions::AppAction;
use crate::state::{Addr, AppState, LabelKind};
use crate::ui::widget::{self, Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::utils;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

/// Address and display name pair for a non-auto symbol listed in [`GoToSymbolDialog`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    /// Target 6502 address associated with the label.
    pub address: u16,
    /// User or system label name.
    pub name: String,
}

/// Modal quick-jump dialog filtering all non-auto labels in [`AppState`].
pub struct GoToSymbolDialog {
    /// Current incremental filter query typed by the user.
    pub input: String,
    /// Full sorted list of searchable symbols.
    pub all_symbols: Vec<SymbolEntry>,
    all_symbols_lower: Vec<String>,
    /// Subset of [`Self::all_symbols`] matching [`Self::input`].
    pub filtered_symbols: Vec<SymbolEntry>,
    /// Index of the currently highlighted entry in [`Self::filtered_symbols`].
    pub selected_index: usize,
}

impl GoToSymbolDialog {
    /// Creates a new [`GoToSymbolDialog`] populated with all non-auto labels in `app_state`.
    #[must_use]
    pub fn new(app_state: &AppState) -> Self {
        let mut entries: Vec<(u16, String, String)> = Vec::new();
        for (addr, labels) in &app_state.labels {
            for label in labels {
                if label.kind != LabelKind::Auto {
                    entries.push((addr.0, label.name.clone(), label.name.to_lowercase()));
                }
            }
        }
        entries.sort_by(|a, b| a.2.cmp(&b.2));

        let mut all_symbols = Vec::with_capacity(entries.len());
        let mut all_symbols_lower = Vec::with_capacity(entries.len());
        for (address, name, lower) in entries {
            all_symbols.push(SymbolEntry { address, name });
            all_symbols_lower.push(lower);
        }

        Self {
            input: String::new(),
            filtered_symbols: all_symbols.clone(),
            all_symbols,
            all_symbols_lower,
            selected_index: 0,
        }
    }

    fn update_filter(&mut self) {
        let query = self.input.to_lowercase();
        self.filtered_symbols = self
            .all_symbols
            .iter()
            .zip(&self.all_symbols_lower)
            .filter(|(_, lower_name)| lower_name.contains(&query))
            .map(|(sym, _)| sym.clone())
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
        let block = widget::create_dialog_block(" Go to Symbol ", theme);

        let area = utils::centered_rect_adaptive(60, 60, 60, 12, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search input (with border)
                Constraint::Min(1),    // Results list
                Constraint::Length(1), // Help / status line
            ])
            .split(inner);

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.highlight_fg))
            .style(Style::default().bg(theme.highlight_bg));

        let input_widget = Paragraph::new(self.input.clone()).block(input_block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input_widget, layout[0]);

        if layout[0].width > 2 && layout[0].height > 1 {
            let cursor_x = (layout[0].x + 1 + self.input.chars().count() as u16)
                .min(layout[0].right().saturating_sub(2));
            f.set_cursor_position((cursor_x, layout[0].y + 1));
        }

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
                .map(|(i, entry)| {
                    let style = if i == self.selected_index {
                        Style::default()
                            .bg(theme.selection_bg)
                            .fg(theme.selection_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.foreground)
                    };
                    ListItem::new(format!("${:04X}  {}", entry.address, entry.name)).style(style)
                })
                .collect();

            let list = List::new(items).highlight_symbol(">> ");
            let mut state = ListState::default();
            state.select(Some(self.selected_index));
            f.render_stateful_widget(list, layout[1], &mut state);
        }

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
        _ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Enter => {
                if let Some(entry) = self.filtered_symbols.get(self.selected_index) {
                    WidgetResult::Action(AppAction::NavigateToAddress(Addr(entry.address)))
                } else {
                    WidgetResult::Close
                }
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
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
    use crate::theme::Theme;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;
    use std::collections::BTreeMap;

    fn create_mock_app() -> AppState {
        let mut app = AppState::new();
        let mut labels = BTreeMap::new();
        labels.insert(
            Addr(0x1000),
            vec![Label {
                name: "MainLoop".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::Subroutine,
            }],
        );
        labels.insert(
            Addr(0x2000),
            vec![Label {
                name: "DataStart".to_string(),
                kind: LabelKind::User,
                label_type: LabelType::AbsoluteAddress,
            }],
        );
        labels.insert(
            Addr(0xfffe),
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
        assert_eq!(dialog.filtered_symbols[0].name, "MainLoop");

        dialog.input = "data".to_string();
        dialog.update_filter();
        assert_eq!(dialog.filtered_symbols.len(), 1);
        assert_eq!(dialog.filtered_symbols[0].name, "DataStart");

        dialog.input = "start".to_string();
        dialog.update_filter();
        assert_eq!(dialog.filtered_symbols.len(), 1);
        assert_eq!(dialog.filtered_symbols[0].name, "DataStart");
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
            &mut UIState::new(Theme::default()),
        );
        assert_eq!(dialog.selected_index, 0);

        // Key Up
        let key_up = KeyEvent::from(KeyCode::Up);
        dialog.handle_input(
            key_up,
            &mut AppState::new(),
            &mut UIState::new(Theme::default()),
        );
        assert_eq!(dialog.selected_index, 2);
    }

    #[test]
    fn test_stateful_scrolling_renders_selected_symbol() {
        let mut app = AppState::new();
        for i in 0..30u16 {
            app.labels.insert(
                Addr(0x1000 + i),
                vec![Label {
                    name: format!("Sym_{i:02}"),
                    kind: LabelKind::User,
                    label_type: LabelType::Subroutine,
                }],
            );
        }
        let mut dialog = GoToSymbolDialog::new(&app);
        dialog.selected_index = 29;

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("creating test terminal should succeed");
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app, &mut ui_state);
            })
            .expect("drawing GoToSymbolDialog should succeed");

        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(Cell::symbol)
            .collect();
        assert!(rendered.contains("Sym_29"));
    }

    #[test]
    fn test_ctrl_and_alt_char_ignored() {
        let app = create_mock_app();
        let mut dialog = GoToSymbolDialog::new(&app);
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());

        dialog.handle_input(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app_state,
            &mut ui_state,
        );
        dialog.handle_input(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT),
            &mut app_state,
            &mut ui_state,
        );
        assert!(dialog.input.is_empty());
    }
}
