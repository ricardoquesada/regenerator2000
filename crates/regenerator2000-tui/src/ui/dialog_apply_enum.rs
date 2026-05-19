use crate::ui::widget::{Widget, WidgetResult, create_dialog_block};
use crate::ui_state::{AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState},
};
use ratatui_textarea::TextArea;
use regenerator2000_core::state::{Addr, AppState, EnumDefinition};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq)]
enum EnumListItem {
    NoneOption,
    Separator,
    EnumEntry {
        name: String,
        matching_variant: Option<String>,
        is_matching: bool,
    },
}

pub struct ApplyEnumDialog {
    search_textarea: TextArea<'static>,
    address: Addr,
    value: u16,
    list_state: ListState,
    filtered_items: Vec<EnumListItem>,
}

impl ApplyEnumDialog {
    #[must_use]
    pub fn new(address: Addr, value: u16, app_state: &AppState) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0)); // Start focused on <None> or first item

        let mut search_textarea = TextArea::default();
        search_textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default()),
        );
        search_textarea.set_cursor_line_style(Style::default());
        search_textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut dialog = Self {
            search_textarea,
            address,
            value,
            list_state,
            filtered_items: Vec::new(),
        };
        dialog.recalculate_filtered_items(app_state);
        dialog
    }

    fn recalculate_filtered_items(&mut self, app_state: &AppState) {
        let search_query = self.search_textarea.lines().join("").trim().to_lowercase();

        // Combine enums using 3-tier precedence (local > global > builtin)
        let mut all_enums: BTreeMap<String, &EnumDefinition> = BTreeMap::new();
        for (name, def) in &app_state.builtin_enums {
            all_enums.insert(name.clone(), def);
        }
        for (name, def) in &app_state.user_global_enums {
            all_enums.insert(name.clone(), def);
        }
        for (name, def) in &app_state.enums {
            all_enums.insert(name.clone(), def);
        }

        let mut matching = Vec::new();
        let mut other = Vec::new();

        for (name, enum_def) in all_enums {
            // Check if filter matches name case-insensitively
            if !search_query.is_empty() && !name.to_lowercase().contains(&search_query) {
                continue;
            }

            if let Some(variant) = enum_def.variants.get(&self.value) {
                matching.push(EnumListItem::EnumEntry {
                    name: name.clone(),
                    matching_variant: Some(variant.clone()),
                    is_matching: true,
                });
            } else {
                other.push(EnumListItem::EnumEntry {
                    name: name.clone(),
                    matching_variant: None,
                    is_matching: false,
                });
            }
        }

        // Build visual list items
        let mut items = Vec::new();
        items.push(EnumListItem::NoneOption);

        if !matching.is_empty() {
            items.extend(matching);
        }

        if !other.is_empty() {
            if items.len() > 1 {
                items.push(EnumListItem::Separator);
            }
            items.extend(other);
        }

        self.filtered_items = items;

        // Adjust selection if it falls out of bounds
        let len = self.filtered_items.len();
        if let Some(selected) = self.list_state.selected() {
            if selected >= len {
                self.list_state
                    .select(if len > 0 { Some(len - 1) } else { None });
            }
        } else if len > 0 {
            self.list_state.select(Some(0));
        }
    }
}

impl Widget for ApplyEnumDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = format!(
            " Apply Enum at ${:04X} (val: ${:02X}/{}) ",
            self.address.0, self.value, self.value
        );
        let block = create_dialog_block(&title, theme);

        // Dialog bounds (Centered, adapt size: Width = 50, Height = 15)
        let area = crate::utils::centered_rect_adaptive(50, 60, 0, 16, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        use ratatui::layout::{Constraint, Direction, Layout};
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search Box
                Constraint::Min(0),    // List
            ])
            .split(inner);

        let search_area = layout[0];
        let list_area = layout[1];

        // 1. Render Search Input
        let mut textarea = self.search_textarea.clone();
        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(theme.highlight_fg))
                .title(" Search / Filter "),
        );
        textarea.set_style(Style::default().fg(theme.dialog_fg));
        f.render_widget(&textarea, search_area);

        // 2. Render List
        let selected_idx = self.list_state.selected().unwrap_or(0);
        let mut list_items = Vec::new();

        for (i, item) in self.filtered_items.iter().enumerate() {
            let is_selected = i == selected_idx;
            match item {
                EnumListItem::NoneOption => {
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.selection_bg)
                            .fg(theme.selection_fg)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.dialog_fg)
                    };
                    list_items.push(
                        ListItem::new(Line::from(" <None> (Clear Enum Usage) ")).style(style),
                    );
                }
                EnumListItem::Separator => {
                    list_items.push(ListItem::new(Line::from(Span::styled(
                        " ────────────────────────────────────────────────── ",
                        Style::default().fg(theme.border_inactive),
                    ))));
                }
                EnumListItem::EnumEntry {
                    name,
                    matching_variant,
                    is_matching,
                } => {
                    let name_span = if *is_matching {
                        Span::styled(
                            name.clone(),
                            Style::default()
                                .fg(theme.block_code_fg)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(name.clone(), Style::default().fg(theme.dialog_fg))
                    };

                    let mut spans = vec![Span::styled("   ", Style::default()), name_span];
                    if let Some(variant) = matching_variant {
                        spans.push(Span::styled(
                            format!(" ({variant})"),
                            Style::default().fg(theme.block_petscii_text_fg),
                        ));
                    }

                    let style = if is_selected {
                        Style::default()
                            .bg(theme.selection_bg)
                            .fg(theme.selection_fg)
                    } else {
                        Style::default()
                    };

                    list_items.push(ListItem::new(Line::from(spans)).style(style));
                }
            }
        }

        let list = List::new(list_items).block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(theme.dialog_border))
                .title(" Available Enums "),
        );

        let mut list_state_mut = self.list_state;
        f.render_stateful_widget(list, list_area, &mut list_state_mut);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::empty() => {
                if self.filtered_items.is_empty() {
                    return WidgetResult::Handled;
                }
                let mut selected = self.list_state.selected().unwrap_or(0);
                if selected > 0 {
                    selected -= 1;
                    // Skip Separator
                    if let Some(EnumListItem::Separator) = self.filtered_items.get(selected) {
                        if selected > 0 {
                            selected -= 1;
                        } else {
                            selected += 1;
                        }
                    }
                    self.list_state.select(Some(selected));
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::empty() => {
                let len = self.filtered_items.len();
                if len == 0 {
                    return WidgetResult::Handled;
                }
                let mut selected = self.list_state.selected().unwrap_or(0);
                if selected + 1 < len {
                    selected += 1;
                    // Skip Separator
                    if let Some(EnumListItem::Separator) = self.filtered_items.get(selected) {
                        if selected + 1 < len {
                            selected += 1;
                        } else {
                            selected -= 1;
                        }
                    }
                    self.list_state.select(Some(selected));
                }
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if self.filtered_items.is_empty() {
                    return WidgetResult::Handled;
                }
                let selected = self.list_state.selected().unwrap_or(0);
                if selected < self.filtered_items.len() {
                    match &self.filtered_items[selected] {
                        EnumListItem::NoneOption => {
                            WidgetResult::Action(AppAction::ApplyEnumUsage {
                                address: self.address,
                                enum_name: None,
                            })
                        }
                        EnumListItem::Separator => WidgetResult::Handled,
                        EnumListItem::EnumEntry { name, .. } => {
                            WidgetResult::Action(AppAction::ApplyEnumUsage {
                                address: self.address,
                                enum_name: Some(name.clone()),
                            })
                        }
                    }
                } else {
                    WidgetResult::Handled
                }
            }
            _ => {
                // Send input to dynamic filter textarea
                if self.search_textarea.input(key) {
                    self.recalculate_filtered_items(app_state);
                    // Reset list focus on filter change
                    self.list_state.select(Some(0));
                }
                WidgetResult::Handled
            }
        }
    }
}
