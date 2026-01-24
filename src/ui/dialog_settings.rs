use crate::state::AppState;
use crate::ui_state::UIState;
use crate::utils::centered_rect;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph};

use crate::ui::widget::{Widget, WidgetResult};

#[derive(Default)]
pub struct SettingsDialog {
    pub selected_index: usize,
    pub is_selecting_theme: bool,
}

impl SettingsDialog {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Widget for SettingsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Settings ", theme);

        let area = centered_rect(50, 40, area); // Increased height for popup space
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let items = vec![
            format!(
                "{} Open the latest file on startup",
                if app_state.system_config.open_last_project {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Auto-analyze on load",
                if app_state.system_config.auto_analyze {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Sync Hex Dump View",
                if app_state.system_config.sync_hex_dump {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Sync Charset View",
                if app_state.system_config.sync_charset_view {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Sync Sprites View",
                if app_state.system_config.sync_sprites_view {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Sync Bitmap View",
                if app_state.system_config.sync_bitmap_view {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Sync Blocks View",
                if app_state.system_config.sync_blocks_view {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!("Theme: < {} >", app_state.system_config.theme),
        ];

        for (i, item) in items.into_iter().enumerate() {
            let style = if self.selected_index == i {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            };

            f.render_widget(
                Paragraph::new(item).style(style),
                Rect::new(inner.x + 2, inner.y + 1 + i as u16, inner.width - 4, 1),
            );
        }

        // Theme Selection Popup
        if self.is_selecting_theme {
            let popup_area = centered_rect(40, 30, area);
            f.render_widget(Clear, popup_area);
            let block = crate::ui::widget::create_dialog_block(" Select Theme ", theme);

            let themes = crate::theme::Theme::all_names();
            let list_items: Vec<ListItem> = themes
                .iter()
                .map(|t| {
                    let is_selected = *t == app_state.system_config.theme;
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.menu_selected_bg)
                            .fg(theme.menu_selected_fg)
                    } else {
                        Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
                    };
                    ListItem::new(t.to_string()).style(style)
                })
                .collect();

            let selected_idx = themes
                .iter()
                .position(|t| *t == app_state.system_config.theme)
                .unwrap_or(0);

            let mut list_state = ListState::default();
            list_state.select(Some(selected_idx));

            let list = List::new(list_items)
                .block(block)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            f.render_stateful_widget(list, popup_area, &mut list_state);
        }
    }

    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                if self.is_selecting_theme {
                    self.is_selecting_theme = false;
                    WidgetResult::Handled
                } else {
                    ui_state.set_status_message("Ready");
                    WidgetResult::Close
                }
            }
            KeyCode::Up => {
                if self.is_selecting_theme {
                    // Cycle themes
                    let themes = crate::theme::Theme::all_names();
                    let current = app_state.system_config.theme.as_str();
                    let idx = themes.iter().position(|t| *t == current).unwrap_or(0);
                    let new_idx = if idx == 0 { themes.len() - 1 } else { idx - 1 };
                    let new_theme = themes[new_idx].to_string();
                    app_state.system_config.theme = new_theme.clone();
                    ui_state.theme = crate::theme::Theme::from_name(&new_theme);
                } else {
                    self.selected_index = self.selected_index.saturating_sub(1);
                }
                WidgetResult::Handled
            }
            KeyCode::Down => {
                if self.is_selecting_theme {
                    // Cycle themes
                    let themes = crate::theme::Theme::all_names();
                    let current = app_state.system_config.theme.as_str();
                    let idx = themes.iter().position(|t| *t == current).unwrap_or(0);
                    let new_idx = (idx + 1) % themes.len();
                    let new_theme = themes[new_idx].to_string();
                    app_state.system_config.theme = new_theme.clone();
                    ui_state.theme = crate::theme::Theme::from_name(&new_theme);
                } else {
                    // Limit to 7 (8 items)
                    if self.selected_index < 7 {
                        self.selected_index += 1;
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.is_selecting_theme {
                    self.is_selecting_theme = false;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 0 {
                    app_state.system_config.open_last_project =
                        !app_state.system_config.open_last_project;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 1 {
                    app_state.system_config.auto_analyze = !app_state.system_config.auto_analyze;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 2 {
                    app_state.system_config.sync_hex_dump = !app_state.system_config.sync_hex_dump;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 3 {
                    app_state.system_config.sync_charset_view =
                        !app_state.system_config.sync_charset_view;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 4 {
                    app_state.system_config.sync_sprites_view =
                        !app_state.system_config.sync_sprites_view;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 5 {
                    app_state.system_config.sync_bitmap_view =
                        !app_state.system_config.sync_bitmap_view;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 6 {
                    app_state.system_config.sync_blocks_view =
                        !app_state.system_config.sync_blocks_view;
                    let _ = app_state.system_config.save();
                } else if self.selected_index == 7 {
                    self.is_selecting_theme = true;
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}
