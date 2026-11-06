use crate::state::AppState;
use crate::ui_state::UIState;
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
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next(&mut self) {
        let max_items = 10;
        self.selected_index = (self.selected_index + 1) % max_items;
    }

    pub fn previous(&mut self) {
        let max_items = 10;
        if self.selected_index == 0 {
            self.selected_index = max_items - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    fn cycle_theme(app_state: &mut AppState, ui_state: &mut UIState, delta: isize) {
        let themes = crate::theme::Theme::all_names();
        if themes.is_empty() {
            return;
        }
        let current = app_state.system_config.theme.as_str();
        let idx = themes.iter().position(|t| t == current).unwrap_or(0);
        let len = themes.len();
        let new_idx = if delta < 0 {
            if idx == 0 { len - 1 } else { idx - 1 }
        } else {
            (idx + 1) % len
        };
        let new_theme = themes[new_idx].clone();
        app_state.system_config.theme.clone_from(&new_theme);
        ui_state.theme = crate::theme::Theme::from_name(&new_theme);
        let _ = app_state.system_config.save();
    }
}

impl Widget for SettingsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Settings ", theme);

        let area = crate::utils::centered_rect_adaptive(50, 40, 40, 17, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let items = vec![
            // Group 1: Sync views (indices 0-4)
            format!(
                "{} Sync Blocks View",
                if app_state.system_config.sync_blocks_view {
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
                "{} Sync Sprites View",
                if app_state.system_config.sync_sprites_view {
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
                "{} Sync Bitmap View",
                if app_state.system_config.sync_bitmap_view {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            // Group 2: General toggles (indices 5-7)
            format!(
                "{} Open the latest file on startup",
                if app_state.system_config.open_last_project {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Default is Unexplored (for new files)",
                if app_state.system_config.default_is_unexplored {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            format!(
                "{} Check for updates",
                if app_state.system_config.check_for_updates {
                    "[X]"
                } else {
                    "[ ]"
                }
            ),
            // Group 3: Entropy threshold (index 8)
            format!(
                "Entropy Warning Threshold: < {:.1} >",
                app_state.system_config.entropy_threshold
            ),
            // Group 4: Theme (index 9)
            format!("Theme: < {} >", app_state.system_config.theme),
        ];

        // Extra y-offset for visual group separators:
        // Group 1 (0-4), gap, Group 2 (5-7), gap, Group 3 (8), gap, Group 4 (9)
        let group_offset = |i: usize| -> u16 {
            match i {
                0..=4 => 0, // Sync views
                5..=7 => 1, // General toggles (1 separator after sync views)
                8 => 2,     // Entropy (2 separators total)
                _ => 3,     // Theme (3 separators total)
            }
        };

        for (i, item) in items.into_iter().enumerate() {
            let style = if self.selected_index == i {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            };

            let y = inner.y + 1 + i as u16 + group_offset(i);
            let width = inner.width.saturating_sub(4);
            if y < inner.bottom() && width > 0 {
                f.render_widget(
                    Paragraph::new(item).style(style),
                    Rect::new(inner.x + 2, y, width, 1),
                );
            }
        }

        // Theme Selection Popup
        if self.is_selecting_theme {
            let popup_area = crate::utils::centered_rect_adaptive(40, 30, 30, 13, area);
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
                    Self::cycle_theme(app_state, ui_state, -1);
                } else {
                    self.previous();
                }
                WidgetResult::Handled
            }
            KeyCode::Down => {
                if self.is_selecting_theme {
                    Self::cycle_theme(app_state, ui_state, 1);
                } else {
                    self.next();
                }
                WidgetResult::Handled
            }
            KeyCode::Left => {
                if !self.is_selecting_theme && self.selected_index == 8 {
                    app_state.system_config.entropy_threshold =
                        (app_state.system_config.entropy_threshold - 0.1).max(0.0);
                    let _ = app_state.system_config.save();
                } else if !self.is_selecting_theme && self.selected_index == 9 {
                    Self::cycle_theme(app_state, ui_state, -1);
                }
                WidgetResult::Handled
            }
            KeyCode::Right => {
                if !self.is_selecting_theme && self.selected_index == 8 {
                    app_state.system_config.entropy_threshold =
                        (app_state.system_config.entropy_threshold + 0.1).min(8.0);
                    let _ = app_state.system_config.save();
                } else if !self.is_selecting_theme && self.selected_index == 9 {
                    Self::cycle_theme(app_state, ui_state, 1);
                }
                WidgetResult::Handled
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.is_selecting_theme {
                    self.is_selecting_theme = false;
                    let _ = app_state.system_config.save();
                    return WidgetResult::Handled;
                }
                if self.selected_index == 9 {
                    self.is_selecting_theme = true;
                    return WidgetResult::Handled;
                }
                let toggled = match self.selected_index {
                    0 => {
                        app_state.system_config.sync_blocks_view =
                            !app_state.system_config.sync_blocks_view;
                        true
                    }
                    1 => {
                        app_state.system_config.sync_hex_dump =
                            !app_state.system_config.sync_hex_dump;
                        true
                    }
                    2 => {
                        app_state.system_config.sync_sprites_view =
                            !app_state.system_config.sync_sprites_view;
                        true
                    }
                    3 => {
                        app_state.system_config.sync_charset_view =
                            !app_state.system_config.sync_charset_view;
                        true
                    }
                    4 => {
                        app_state.system_config.sync_bitmap_view =
                            !app_state.system_config.sync_bitmap_view;
                        true
                    }
                    5 => {
                        app_state.system_config.open_last_project =
                            !app_state.system_config.open_last_project;
                        true
                    }
                    6 => {
                        app_state.system_config.default_is_unexplored =
                            !app_state.system_config.default_is_unexplored;
                        true
                    }
                    7 => {
                        app_state.system_config.check_for_updates =
                            !app_state.system_config.check_for_updates;
                        true
                    }
                    _ => false,
                };
                if toggled {
                    let _ = app_state.system_config.save();
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
    use crate::theme::Theme;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_settings_dialog_toggles_entropy_and_theme_popup_esc() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = SettingsDialog::new();

        // Toggle index 0 (sync_blocks_view) with Space.
        let initial_sync = app_state.system_config.sync_blocks_view;
        dialog.handle_input(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(app_state.system_config.sync_blocks_view, !initial_sync);

        // Select index 8 (Entropy threshold) and step right/left within [0.0, 8.0].
        dialog.selected_index = 8;
        app_state.system_config.entropy_threshold = 7.95;
        dialog.handle_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert!((app_state.system_config.entropy_threshold - 8.0).abs() < f32::EPSILON);

        // Select index 9 (Theme), open popup, render, and verify Esc closes popup first.
        dialog.selected_index = 9;
        dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert!(dialog.is_selecting_theme);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                dialog.render(f, f.area(), &app_state, &mut ui_state);
            })
            .unwrap();

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert!(!dialog.is_selecting_theme);

        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
    }
}
