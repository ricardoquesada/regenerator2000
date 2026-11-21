//! Modal dialog (`Alt+B` / `Ctrl+B`) for browsing, jumping to, and deleting bookmarks.

use crate::state::actions::AppAction;
use crate::state::{Addr, AppState};
use crate::ui::widget::{self, Widget, WidgetResult};
use crate::ui_state::UIState;
use crate::utils;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, List, ListItem, Paragraph};

/// Modal dialog listing all bookmarked addresses for quick navigation or removal.
pub struct BookmarksDialog;

impl BookmarksDialog {
    /// Creates a new [`BookmarksDialog`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for BookmarksDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for BookmarksDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = widget::create_dialog_block(" Bookmarks ", theme);

        let area = utils::centered_rect_adaptive(50, 40, 50, 20, area);

        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let bookmarks: Vec<(Addr, &String)> = app_state
            .annotations
            .iter()
            .filter_map(|(addr, entry)| entry.bookmark.as_ref().map(|name| (addr, name)))
            .collect();

        if bookmarks.is_empty() {
            let p = Paragraph::new("No bookmarks set.\nPress Ctrl+B to toggle bookmark.")
                .style(Style::default().fg(theme.menu_disabled_fg))
                .alignment(Alignment::Center);

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

                    let display_name = if name.is_empty() { "Bookmark" } else { name };

                    ListItem::new(format!("${addr:04X} - {display_name}{label}"))
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
        let bookmarks: Vec<Addr> = app_state
            .annotations
            .iter()
            .filter_map(|(addr, entry)| entry.bookmark.as_ref().map(|_| addr))
            .collect();

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
                        WidgetResult::Action(AppAction::NavigateToAddress(addr))
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
                    app_state.set_bookmark(addr, None);

                    // Adjust selection
                    let bookmarks_len = app_state
                        .annotations
                        .iter()
                        .filter(|(_, entry)| entry.bookmark.is_some())
                        .count();
                    if i >= bookmarks_len && bookmarks_len > 0 {
                        ui_state
                            .bookmarks_list_state
                            .select(Some(bookmarks_len - 1));
                    } else if bookmarks_len == 0 {
                        ui_state.bookmarks_list_state.select(None);
                    }

                    ui_state.set_status_message(format!("Bookmark at ${addr:04X} removed"));
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;

    #[test]
    fn test_bookmarks_dialog_delete_pushes_undoable_command() {
        let mut app_state = AppState::new();
        let addr = Addr(0x1000);
        app_state.set_bookmark(addr, Some("Entry".to_string()));

        let mut ui_state = UIState::new(Theme::default());
        ui_state.bookmarks_list_state.select(Some(0));
        let mut dialog = BookmarksDialog::new();

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Handled);
        assert!(
            app_state
                .annotations
                .get(addr)
                .and_then(|e| e.bookmark.as_ref())
                .is_none()
        );

        let undo_msg = app_state.undo_last_command();
        assert_eq!(undo_msg, "Undone");
        assert_eq!(
            app_state
                .annotations
                .get(addr)
                .and_then(|e| e.bookmark.as_deref()),
            Some("Entry")
        );
    }
}
