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

        let dialog_width = 80;
        let dialog_height = 24;
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
                        // Open the project
                        match app_state.load_file(path.clone()) {
                            Err(e) => {
                                ui_state.set_status_message(format!("Error loading file: {}", e));
                                app_state.system_config.remove_recent_project(&path);
                                let _ = app_state.system_config.save();
                            }
                            Ok(loaded_data) => {
                                let filename =
                                    path.file_name().unwrap_or_default().to_string_lossy();
                                ui_state.set_status_message(format!("Loaded: {}", filename));
                                ui_state.restore_session(&loaded_data, app_state);
                                crate::ui::dialog_warning::WarningDialog::show_if_needed(
                                    loaded_data.entropy_warning,
                                    ui_state,
                                );
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
