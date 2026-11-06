use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, List, ListItem},
};

pub struct OpenRecentDialog;

impl Widget for OpenRecentDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Open Recent ", theme);

        let area = crate::utils::centered_rect_adaptive(80, 60, 80, 24, area);

        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let recents = &app_state.system_config.recent_projects;

        if recents.is_empty() {
            let p = ratatui::widgets::Paragraph::new(
                "No recent projects.\nProjects are automatically saved here when opened.",
            )
            .style(Style::default().fg(theme.menu_disabled_fg))
            .alignment(ratatui::layout::Alignment::Center);

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
            let items: Vec<ListItem> = recents
                .iter()
                .map(|path| {
                    ListItem::new(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    )
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

            f.render_stateful_widget(list, inner, &mut ui_state.recent_list_state);
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        let recents = &app_state.system_config.recent_projects;

        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Down => {
                if !recents.is_empty() {
                    let i = match ui_state.recent_list_state.selected() {
                        Some(i) => {
                            if i >= recents.len() - 1 {
                                0
                            } else {
                                i + 1
                            }
                        }
                        None => 0,
                    };
                    ui_state.recent_list_state.select(Some(i));
                }
                WidgetResult::Handled
            }
            KeyCode::Up => {
                if !recents.is_empty() {
                    let i = match ui_state.recent_list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                recents.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    ui_state.recent_list_state.select(Some(i));
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if let Some(i) = ui_state.recent_list_state.selected() {
                    if let Some(path) = recents.get(i).cloned() {
                        match app_state.load_file(path.clone()) {
                            Err(e) => {
                                ui_state.set_status_message(format!("Error loading file: {e}"));
                                app_state.system_config.remove_recent_project(&path);
                                let _ = app_state.system_config.save();
                            }
                            Ok(loaded_data) => {
                                let filename =
                                    path.file_name().unwrap_or_default().to_string_lossy();
                                ui_state.set_status_message(format!("Loaded: {filename}"));
                                ui_state.restore_session(&loaded_data, app_state);
                            }
                        }
                        WidgetResult::Close
                    } else {
                        WidgetResult::Handled
                    }
                } else {
                    WidgetResult::Handled
                }
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    #[test]
    fn test_open_recent_dialog_render_navigation_and_missing_file_prune() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = OpenRecentDialog;

        // 1. Empty recents renders placeholder message.
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();
        let empty_rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(empty_rendered.contains("No recent projects."));

        // 2. Populated recents wrap on Up/Down and prune missing files on Enter.
        let p1 = PathBuf::from("/tmp/nonexistent_r2000_1.regen2000proj");
        let p2 = PathBuf::from("/tmp/nonexistent_r2000_2.regen2000proj");
        app_state.system_config.recent_projects = vec![p1.clone(), p2.clone()];
        ui_state.recent_list_state.select(Some(0));

        dialog.handle_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.recent_list_state.selected(), Some(1));

        dialog.handle_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(ui_state.recent_list_state.selected(), Some(0));

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
        assert_eq!(app_state.system_config.recent_projects, vec![p2]);
    }
}
