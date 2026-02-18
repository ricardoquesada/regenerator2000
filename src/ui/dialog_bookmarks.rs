use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult, create_dialog_block};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, List, ListItem},
};

pub struct BookmarksDialog;

impl Widget for BookmarksDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = create_dialog_block(" Bookmarks ", theme);

        // Center dialog
        let dialog_width = 50;
        let dialog_height = 20;
        let area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length((area.height.saturating_sub(dialog_height)) / 2),
                Constraint::Length(dialog_height),
                Constraint::Min(0),
            ])
            .split(area)[1];

        let area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length((area.width.saturating_sub(dialog_width)) / 2),
                Constraint::Length(dialog_width),
                Constraint::Min(0),
            ])
            .split(area)[1];

        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let bookmarks: Vec<_> = app_state.bookmarks.iter().collect();

        if bookmarks.is_empty() {
            let p = ratatui::widgets::Paragraph::new(
                "No bookmarks set.\nPress Ctrl+B to toggle bookmark.",
            )
            .style(Style::default().fg(theme.menu_disabled_fg))
            .alignment(ratatui::layout::Alignment::Center);

            // Vertical center the text
            let v_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Fill(1),
                    Constraint::Length(2),
                    Constraint::Fill(1),
                ])
                .split(inner);
            f.render_widget(p, v_layout[1]);
        } else {
            let items: Vec<ListItem> = bookmarks
                .iter()
                .map(|(addr, name)| {
                    let label = if let Some(labels) = app_state.labels.get(addr)
                        && let Some(first) = labels.first()
                    {
                        format!(" ({})", first.name)
                    } else {
                        String::new()
                    };

                    // Format: $ADDR - Name (Label)
                    // If name is pseudo-random (default uuid), maybe show something else?
                    // Implementation plan said "store bookmarks as map u16 -> String".
                    // The string is the name.
                    // If name is empty, we can just show address.
                    let display_name = if name.is_empty() { "Bookmark" } else { name };

                    ListItem::new(format!("${:04X} - {}{}", addr, display_name, label))
                })
                .collect();

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(theme.menu_selected_bg)
                        .fg(theme.menu_selected_fg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(list, inner, &mut ui_state.bookmarks_list_state);
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let bookmarks: Vec<u16> = app_state.bookmarks.keys().copied().collect();

        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Down => {
                if !bookmarks.is_empty() {
                    let i = match ui_state.bookmarks_list_state.selected() {
                        Some(i) => {
                            if i >= bookmarks.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    ui_state.bookmarks_list_state.select(Some(i));
                }
                WidgetResult::Handled
            }
            KeyCode::Up => {
                if !bookmarks.is_empty() {
                    let i = match ui_state.bookmarks_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                bookmarks.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    ui_state.bookmarks_list_state.select(Some(i));
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if let Some(i) = ui_state.bookmarks_list_state.selected() {
                    if let Some(&addr) = bookmarks.get(i) {
                        crate::ui::menu::perform_jump_to_address(app_state, ui_state, addr);
                        WidgetResult::Close
                    } else {
                        WidgetResult::Handled
                    }
                } else {
                    WidgetResult::Handled
                }
            }
            KeyCode::Delete | KeyCode::Backspace => {
                // Allow deleting bookmark from list
                if let Some(i) = ui_state.bookmarks_list_state.selected()
                    && let Some(&addr) = bookmarks.get(i)
                {
                    let command = crate::commands::Command::SetBookmark {
                        address: addr,
                        new_name: None,
                        old_name: app_state.bookmarks.get(&addr).cloned(),
                    };
                    command.apply(app_state);
                    app_state.push_command(command);

                    // Adjust selection
                    if i >= app_state.bookmarks.len() && !app_state.bookmarks.is_empty() {
                        ui_state
                            .bookmarks_list_state
                            .select(Some(app_state.bookmarks.len() - 1));
                    } else if app_state.bookmarks.is_empty() {
                        ui_state.bookmarks_list_state.select(None);
                    }

                    ui_state.set_status_message(format!("Bookmark at ${:04X} removed", addr));
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }
}
