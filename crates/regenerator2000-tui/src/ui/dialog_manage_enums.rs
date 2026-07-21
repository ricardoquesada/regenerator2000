use crate::ui::widget::{Widget, WidgetResult, create_dialog_block};
use crate::ui_state::{AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use regenerator2000_core::state::{Addr, AppState};

use super::dialog_edit_enum::EditEnumDialog;

pub struct ManageEnumsDialog {
    active_tab: usize, // 0 = Project, 1 = Global, 2 = System
    project_enums: Vec<String>,
    global_enums: Vec<String>,
    system_enums: Vec<String>,
    list_state: ListState,
    sub_dialog: Option<Box<dyn Widget>>,
}

impl ManageEnumsDialog {
    #[must_use]
    pub fn new(app_state: &AppState) -> Self {
        let mut dialog = Self {
            active_tab: 0,
            project_enums: Vec::new(),
            global_enums: Vec::new(),
            system_enums: Vec::new(),
            list_state: ListState::default(),
            sub_dialog: None,
        };
        dialog.recalculate_lists(app_state);
        if !dialog.project_enums.is_empty() {
            dialog.list_state.select(Some(0));
        }
        dialog
    }

    fn recalculate_lists(&mut self, app_state: &AppState) {
        self.project_enums = app_state.enums.keys().cloned().collect();
        self.global_enums = app_state.user_global_enums.keys().cloned().collect();
        self.system_enums = app_state.builtin_enums.keys().cloned().collect();

        let current_len = self.active_list_len();
        if let Some(selected) = self.list_state.selected() {
            if selected >= current_len {
                self.list_state.select(if current_len > 0 {
                    Some(current_len - 1)
                } else {
                    None
                });
            }
        } else if current_len > 0 {
            self.list_state.select(Some(0));
        }
    }

    fn active_list_len(&self) -> usize {
        match self.active_tab {
            0 => self.project_enums.len(),
            1 => self.global_enums.len(),
            2 => self.system_enums.len(),
            _ => 0,
        }
    }

    fn get_selected_enum_name(&self) -> Option<&str> {
        let selected = self.list_state.selected()?;
        match self.active_tab {
            0 => self.project_enums.get(selected).map(String::as_str),
            1 => self.global_enums.get(selected).map(String::as_str),
            2 => self.system_enums.get(selected).map(String::as_str),
            _ => None,
        }
    }

    fn switch_tab(&mut self, tab_idx: usize) {
        self.active_tab = tab_idx;
        let len = self.active_list_len();
        if len > 0 {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }
}

impl Widget for ManageEnumsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        // 1. If a sub-dialog modal is active (like Add/Edit modal), delegate rendering to it
        if let Some(sub) = &self.sub_dialog {
            // We still render the background list dialog
            self.render_background(f, area, app_state, ui_state);
            sub.render(f, area, app_state, ui_state);
            return;
        }

        self.render_background(f, area, app_state, ui_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // 2. Delegate inputs to sub-dialog if active
        if let Some(sub) = &mut self.sub_dialog {
            let res = sub.handle_input(key, app_state, ui_state);
            match res {
                WidgetResult::Close => {
                    self.sub_dialog = None;
                    self.recalculate_lists(app_state);
                    return WidgetResult::Handled;
                }
                WidgetResult::Action(action) => {
                    self.sub_dialog = None;
                    return WidgetResult::Action(action);
                }
                WidgetResult::Handled => return WidgetResult::Handled,
                WidgetResult::Ignored => return WidgetResult::Ignored,
            }
        }

        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Char('1') => {
                self.switch_tab(0);
                WidgetResult::Handled
            }
            KeyCode::Char('2') => {
                self.switch_tab(1);
                WidgetResult::Handled
            }
            KeyCode::Char('3') => {
                self.switch_tab(2);
                WidgetResult::Handled
            }
            KeyCode::Tab => {
                self.switch_tab((self.active_tab + 1) % 3);
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::empty() => {
                let len = self.active_list_len();
                if len > 0 {
                    let selected = self.list_state.selected().unwrap_or(0);
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::empty() => {
                let len = self.active_list_len();
                if len > 0 {
                    let selected = self.list_state.selected().unwrap_or(0);
                    if selected + 1 < len {
                        self.list_state.select(Some(selected + 1));
                    }
                }
                WidgetResult::Handled
            }
            // Add/Create Enum
            KeyCode::Char('a') if self.active_tab < 2 => {
                let is_global = self.active_tab == 1;
                self.sub_dialog = Some(Box::new(EditEnumDialog::new(None, is_global)));
                WidgetResult::Handled
            }
            // Edit Enum
            KeyCode::Char('e') | KeyCode::Enter if self.active_tab < 2 => {
                if let Some(name) = self.get_selected_enum_name() {
                    let enum_def = if self.active_tab == 0 {
                        app_state.enums.get(name).cloned()
                    } else {
                        app_state.user_global_enums.get(name).cloned()
                    };
                    if let Some(def) = enum_def {
                        let is_global = self.active_tab == 1;
                        self.sub_dialog = Some(Box::new(EditEnumDialog::new(Some(def), is_global)));
                    }
                }
                WidgetResult::Handled
            }
            // Copy System/Global to Project
            KeyCode::Char('c') if self.active_tab > 0 => {
                if let Some(name) = self.get_selected_enum_name() {
                    let source_def = if self.active_tab == 1 {
                        app_state.user_global_enums.get(name).cloned()
                    } else {
                        app_state.builtin_enums.get(name).cloned()
                    };

                    if let Some(def) = source_def {
                        // Verify that it doesn't clash in Project pool
                        if app_state.enums.contains_key(&def.name) {
                            ui_state.set_status_message(format!(
                                "Error: Enum '{}' already exists in Project Enums.",
                                def.name
                            ));
                        } else {
                            // Dispatch Action to save project enum definition
                            return WidgetResult::Action(AppAction::ApplyEnumDefinition {
                                name: def.name.clone(),
                                definition: Some(def),
                                rename_from: None,
                            });
                        }
                    }
                }
                WidgetResult::Handled
            }
            // Delete Enum
            KeyCode::Char('d') | KeyCode::Delete if self.active_tab < 2 => {
                if let Some(name) = self.get_selected_enum_name() {
                    let name_str = name.to_string();
                    let is_global = self.active_tab == 1;

                    // 3-level usage detection check
                    let usages: Vec<Addr> = app_state
                        .annotations
                        .iter()
                        .filter(|(_, entry)| entry.enum_usage.as_deref() == Some(name_str.as_str()))
                        .map(|(addr, _)| addr)
                        .collect();

                    let mut warning_msg = String::new();
                    if !usages.is_empty() {
                        warning_msg = format!(
                            "Warning: Enum is applied at {} addresses. Reverts to raw hex. ",
                            usages.len()
                        );
                    }

                    let confirm_action = if is_global {
                        AppAction::ApplyGlobalEnumDefinition {
                            name: name_str.clone(),
                            definition: None,
                            rename_from: None,
                        }
                    } else {
                        AppAction::ApplyEnumDefinition {
                            name: name_str.clone(),
                            definition: None,
                            rename_from: None,
                        }
                    };

                    let title = if is_global {
                        "Delete Global Enum"
                    } else {
                        "Delete Project Enum"
                    };
                    let message = format!(
                        "{}Are you sure you want to delete enum '{}'?",
                        warning_msg, name_str
                    );

                    // Push standard confirmation dialog
                    self.sub_dialog = Some(Box::new(
                        crate::ui::dialog_confirmation::ConfirmationDialog::new(
                            title,
                            message,
                            confirm_action,
                        ),
                    ));
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}

impl ManageEnumsDialog {
    fn render_background(
        &self,
        f: &mut Frame,
        area: Rect,
        app_state: &AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let block = create_dialog_block(" Manage Enums ", theme);

        // Center the dialog box nicely
        let area = crate::utils::centered_rect_adaptive(65, 60, 0, 20, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        // Layout: Vertical split: Tabs, main area, helper keys
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs header
                Constraint::Min(0),    // Main lists content
                Constraint::Length(1), // Helper keys
            ])
            .split(inner);

        let tabs_area = layout[0];
        let main_area = layout[1];
        let helpers_area = layout[2];

        // 1. Render Tabs
        let tab_titles = vec![
            format!(" [1] Project Enums ({}) ", self.project_enums.len()),
            format!(" [2] Global Enums ({}) ", self.global_enums.len()),
            format!(" [3] System Enums ({}) ", self.system_enums.len()),
        ];

        let mut spans = Vec::new();
        for (i, title) in tab_titles.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default()));
            }
            if i == self.active_tab {
                spans.push(Span::styled(
                    title,
                    Style::default()
                        .bg(theme.selection_bg)
                        .fg(theme.selection_fg)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(title, Style::default().fg(theme.dialog_fg)));
            }
        }

        let tabs_paragraph = Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border_inactive)),
        );
        f.render_widget(tabs_paragraph, tabs_area);

        // 2. Render Main Area: 2-column split (left: List, right: Detail Card)
        let main_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Left list
                Constraint::Percentage(60), // Right details
            ])
            .split(main_area);

        let list_area = main_split[0];
        let detail_area = main_split[1];

        // Render List
        let active_names = match self.active_tab {
            0 => &self.project_enums,
            1 => &self.global_enums,
            2 => &self.system_enums,
            _ => unreachable!(),
        };

        let selected_idx = self.list_state.selected().unwrap_or(0);
        let list_items: Vec<ListItem> = active_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let style = if i == selected_idx {
                    Style::default()
                        .bg(theme.selection_bg)
                        .fg(theme.selection_fg)
                } else {
                    Style::default().fg(theme.dialog_fg)
                };
                ListItem::new(format!("  {name} ")).style(style)
            })
            .collect();

        let list = List::new(list_items).block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(theme.border_inactive)),
        );

        let mut list_state_mut = self.list_state;
        f.render_stateful_widget(list, list_area, &mut list_state_mut);

        // Render Detail Card
        let selected_name = self.get_selected_enum_name();
        if let Some(name) = selected_name {
            let enum_def = match self.active_tab {
                0 => app_state.enums.get(name),
                1 => app_state.user_global_enums.get(name),
                2 => app_state.builtin_enums.get(name),
                _ => None,
            };

            if let Some(def) = enum_def {
                let scope_str = match self.active_tab {
                    0 => "Project-Specific",
                    1 => "Global User TOML",
                    2 => "System Built-In (Read-Only)",
                    _ => "",
                };

                // Usage stats
                let usage_count = app_state
                    .annotations
                    .iter()
                    .filter(|(_, entry)| entry.enum_usage.as_deref() == Some(name))
                    .count();

                let mut detail_lines = vec![
                    Line::from(vec![
                        Span::styled(" Name: ", Style::default().fg(theme.highlight_fg)),
                        Span::styled(
                            &def.name,
                            Style::default()
                                .fg(theme.dialog_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(" Scope: ", Style::default().fg(theme.highlight_fg)),
                        Span::styled(scope_str, Style::default().fg(theme.dialog_fg)),
                    ]),
                    Line::from(vec![
                        Span::styled(" Usages: ", Style::default().fg(theme.highlight_fg)),
                        Span::styled(
                            format!("{usage_count} instructions"),
                            Style::default().fg(theme.dialog_fg),
                        ),
                    ]),
                ];

                // Description (with wrapping)
                detail_lines.push(Line::from(Span::styled(
                    " Description: ",
                    Style::default().fg(theme.highlight_fg),
                )));
                if let Some(desc) = &def.description {
                    for line in desc.lines() {
                        detail_lines.push(Line::from(format!("   {line}")));
                    }
                } else {
                    detail_lines.push(Line::from(Span::styled(
                        "   <None>",
                        Style::default().fg(theme.border_inactive),
                    )));
                }

                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(Span::styled(
                    " Variants:",
                    Style::default().fg(theme.highlight_fg),
                )));

                for (k, v) in &def.variants {
                    detail_lines.push(Line::from(vec![
                        Span::styled(
                            format!("   ${k:02X} ({k})"),
                            Style::default().fg(theme.block_code_fg),
                        ),
                        Span::styled(" = ", Style::default().fg(theme.border_inactive)),
                        Span::styled(
                            v,
                            Style::default()
                                .fg(theme.block_petscii_text_fg)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }

                let details_paragraph = Paragraph::new(detail_lines);
                f.render_widget(details_paragraph, detail_area);
            }
        } else {
            let empty_msg = Paragraph::new(" No enums found in this tier. ")
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(theme.border_inactive));
            f.render_widget(empty_msg, detail_area);
        }

        // 3. Render Helpers
        let keys_str = match self.active_tab {
            0 | 1 => " [a] Add  [e/Enter] Edit  [d/Del] Delete  [c] Copy to Project  [Esc] Close ",
            2 => " [c] Copy to Project  [Esc] Close (System Enums are Read-Only) ",
            _ => "",
        };
        let helpers = Paragraph::new(keys_str)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(theme.highlight_fg));
        f.render_widget(helpers, helpers_area);
    }
}
