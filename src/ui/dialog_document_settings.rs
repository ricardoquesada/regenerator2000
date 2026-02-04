use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph};

pub struct DocumentSettingsDialog {
    pub selected_index: usize,
    pub is_selecting_platform: bool,
    pub is_selecting_assembler: bool,
    pub is_editing_xref_count: bool,
    pub xref_count_input: String,
    pub is_editing_arrow_columns: bool,
    pub arrow_columns_input: String,
    pub is_editing_text_char_limit: bool,
    pub text_char_limit_input: String,
    pub is_editing_addresses_per_line: bool,
    pub addresses_per_line_input: String,
    pub is_editing_bytes_per_line: bool,
    pub bytes_per_line_input: String,
}

impl Default for DocumentSettingsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentSettingsDialog {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            is_selecting_platform: false,
            is_selecting_assembler: false,
            is_editing_xref_count: false,
            xref_count_input: String::new(),
            is_editing_arrow_columns: false,
            arrow_columns_input: String::new(),
            is_editing_text_char_limit: false,
            text_char_limit_input: String::new(),
            is_editing_addresses_per_line: false,
            addresses_per_line_input: String::new(),
            is_editing_bytes_per_line: false,
            bytes_per_line_input: String::new(),
        }
    }

    pub fn next(&mut self) {
        let max_items = 12;
        self.selected_index = (self.selected_index + 1) % max_items;
    }

    pub fn previous(&mut self) {
        let max_items = 12;
        if self.selected_index == 0 {
            self.selected_index = max_items - 1;
        } else {
            self.selected_index -= 1;
        }
    }
}

impl Widget for DocumentSettingsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Document Settings ", theme);

        let area = crate::utils::centered_rect(60, 60, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let settings = &app_state.settings;

        // Helper for checkboxes
        let checkbox = |label: &str, checked: bool, selected: bool, disabled: bool| {
            let check_char = if checked { "[X]" } else { "[ ]" };
            let style = if disabled {
                if selected {
                    Style::default()
                        .fg(theme.menu_disabled_fg)
                        .add_modifier(Modifier::BOLD | Modifier::ITALIC) // Selected but disabled
                } else {
                    Style::default()
                        .fg(theme.menu_disabled_fg)
                        .add_modifier(Modifier::ITALIC) // Disabled and Italic
                }
            } else if selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            };
            Span::styled(format!("{} {}", check_char, label), style)
        };

        let patch_brk_disabled = settings.brk_single_byte
            || settings.assembler == crate::state::Assembler::Kick
            || settings.assembler == crate::state::Assembler::Ca65;

        let items = vec![
            checkbox(
                "All Labels",
                settings.all_labels,
                self.selected_index == 0,
                false,
            ),
            checkbox(
                "Preserve long bytes (@w, +2, .abs, etc)",
                settings.preserve_long_bytes,
                self.selected_index == 1,
                false,
            ),
            checkbox(
                "BRK single byte",
                settings.brk_single_byte,
                self.selected_index == 2,
                false,
            ),
            checkbox(
                "Patch BRK",
                settings.patch_brk,
                self.selected_index == 3,
                patch_brk_disabled,
            ),
            checkbox(
                "Use Illegal Opcodes",
                settings.use_illegal_opcodes,
                self.selected_index == 4,
                false,
            ),
        ];

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(items.len() as u16 + 1), // Checkboxes + padding
                Constraint::Length(2),                      // Max X-Refs
                Constraint::Length(2),                      // Arrow Columns
                Constraint::Length(2),                      // Text Line Limit
                Constraint::Length(2),                      // Addresses Per Line
                Constraint::Length(2),                      // Bytes Per Line
                Constraint::Length(2),                      // Assembler
                Constraint::Length(2),                      // Platform
            ])
            .split(inner);

        for (i, item) in items.into_iter().enumerate() {
            f.render_widget(
                Paragraph::new(item),
                Rect::new(
                    layout[0].x + 2,
                    layout[0].y + 1 + i as u16,
                    layout[0].width - 4,
                    1,
                ),
            );
        }

        // X-Refs uses layout[1]
        let xref_selected = self.selected_index == 5;
        let xref_value_str = if self.is_editing_xref_count {
            self.xref_count_input.clone()
        } else {
            settings.max_xref_count.to_string()
        };
        let xref_text = format!("Max X-Refs: < {} >", xref_value_str);

        let xref_widget = Paragraph::new(xref_text).style(if xref_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            xref_widget,
            Rect::new(layout[1].x + 2, layout[1].y, layout[1].width - 4, 1),
        );

        // Arrow Columns
        let arrow_selected = self.selected_index == 6;
        let arrow_value_str = if self.is_editing_arrow_columns {
            self.arrow_columns_input.clone()
        } else {
            settings.max_arrow_columns.to_string()
        };
        let arrow_text = format!("Arrow Columns: < {} >", arrow_value_str);

        let arrow_widget = Paragraph::new(arrow_text).style(if arrow_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            arrow_widget,
            Rect::new(layout[2].x + 2, layout[2].y, layout[2].width - 4, 1),
        );

        // Text Line Limit
        let text_limit_selected = self.selected_index == 7;
        let text_limit_value_str = if self.is_editing_text_char_limit {
            self.text_char_limit_input.clone()
        } else {
            settings.text_char_limit.to_string()
        };
        let text_limit_text = format!("Text Line Limit: < {} >", text_limit_value_str);

        let text_limit_widget = Paragraph::new(text_limit_text).style(if text_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            text_limit_widget,
            Rect::new(layout[3].x + 2, layout[3].y, layout[3].width - 4, 1),
        );

        // Addresses Per Line
        let addr_limit_selected = self.selected_index == 8;
        let addr_limit_value_str = if self.is_editing_addresses_per_line {
            self.addresses_per_line_input.clone()
        } else {
            settings.addresses_per_line.to_string()
        };
        let addr_limit_text = format!("Words/Addrs per line: < {} >", addr_limit_value_str);

        let addr_limit_widget = Paragraph::new(addr_limit_text).style(if addr_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            addr_limit_widget,
            Rect::new(layout[4].x + 2, layout[4].y, layout[4].width - 4, 1),
        );

        // Bytes Per Line
        let bytes_limit_selected = self.selected_index == 9;
        let bytes_limit_value_str = if self.is_editing_bytes_per_line {
            self.bytes_per_line_input.clone()
        } else {
            settings.bytes_per_line.to_string()
        };
        let bytes_limit_text = format!("Bytes per line: < {} >", bytes_limit_value_str);

        let bytes_limit_widget = Paragraph::new(bytes_limit_text).style(if bytes_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            bytes_limit_widget,
            Rect::new(layout[5].x + 2, layout[5].y, layout[5].width - 4, 1),
        );

        // Assembler Section
        let assembler_selected = self.selected_index == 10;
        let assembler_text = format!("Assembler: < {} >", settings.assembler);

        let assembler_widget = Paragraph::new(assembler_text).style(if assembler_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        // Assembler uses layout[6]
        f.render_widget(
            assembler_widget,
            Rect::new(layout[6].x + 2, layout[6].y, layout[6].width - 4, 1),
        );

        // Platform Section (Moved to end)
        let platform_label = Span::raw("Platform:");
        f.render_widget(
            Paragraph::new(platform_label),
            Rect::new(layout[7].x + 2, layout[7].y, layout[7].width - 4, 1),
        );

        let platforms = crate::state::Platform::all();

        // Check if platform is selected
        let platform_selected = self.selected_index == 11;

        let platform_text = format!("Platform: < {} >", settings.platform);
        let platform_widget = Paragraph::new(platform_text).style(if platform_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            platform_widget,
            Rect::new(layout[7].x + 2, layout[7].y, layout[7].width - 4, 1),
        );

        // Platform Popup
        if self.is_selecting_platform {
            let popup_area = crate::utils::centered_rect(40, 50, area);
            f.render_widget(Clear, popup_area);
            let block = crate::ui::widget::create_dialog_block(" Select Platform ", theme);

            let list_items: Vec<ListItem> = platforms
                .iter()
                .map(|p| {
                    let is_selected = *p == settings.platform;
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.menu_selected_bg)
                            .fg(theme.menu_selected_fg)
                    } else {
                        Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
                    };
                    ListItem::new(p.to_string()).style(style)
                })
                .collect();

            let selected_idx = platforms
                .iter()
                .position(|p| *p == settings.platform)
                .unwrap_or(0);

            let mut list_state = ListState::default();
            list_state.select(Some(selected_idx));

            let list = List::new(list_items)
                .block(block)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD));
            f.render_stateful_widget(list, popup_area, &mut list_state);
        }

        // Assembler Popup
        if self.is_selecting_assembler {
            let popup_area = crate::utils::centered_rect(40, 30, area); // Smaller height for fewer items
            f.render_widget(Clear, popup_area);
            let block = crate::ui::widget::create_dialog_block(" Select Assembler ", theme);

            let assemblers = crate::state::Assembler::all();
            let list_items: Vec<ListItem> = assemblers
                .iter()
                .map(|a| {
                    let is_selected = *a == settings.assembler;
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.menu_selected_bg)
                            .fg(theme.menu_selected_fg)
                    } else {
                        Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
                    };
                    ListItem::new(a.to_string()).style(style)
                })
                .collect();

            let selected_idx = assemblers
                .iter()
                .position(|a| *a == settings.assembler)
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
                if self.is_selecting_platform {
                    self.is_selecting_platform = false;
                } else if self.is_selecting_assembler {
                    self.is_selecting_assembler = false;
                } else if self.is_editing_xref_count {
                    self.is_editing_xref_count = false;
                    // Reset input to current value
                    self.xref_count_input.clear();
                } else if self.is_editing_arrow_columns {
                    self.is_editing_arrow_columns = false;
                    self.arrow_columns_input.clear();
                } else if self.is_editing_text_char_limit {
                    self.is_editing_text_char_limit = false;
                    self.text_char_limit_input.clear();
                } else if self.is_editing_addresses_per_line {
                    self.is_editing_addresses_per_line = false;
                    self.addresses_per_line_input.clear();
                } else if self.is_editing_bytes_per_line {
                    self.is_editing_bytes_per_line = false;
                    self.bytes_per_line_input.clear();
                } else {
                    ui_state.set_status_message("Ready");
                    app_state.load_system_assets();
                    app_state.perform_analysis();
                    app_state.disassemble(); // Disassemble on close to apply all settings
                    return WidgetResult::Close;
                }
            }
            KeyCode::Up => {
                if self.is_selecting_platform {
                    // Cycle platforms backwards
                    let platforms = crate::state::Platform::all();
                    let current_idx = platforms
                        .iter()
                        .position(|p| *p == app_state.settings.platform)
                        .unwrap_or(0);
                    let new_idx = if current_idx == 0 {
                        platforms.len() - 1
                    } else {
                        current_idx - 1
                    };
                    app_state.settings.platform = platforms[new_idx];
                } else if self.is_selecting_assembler {
                    // Cycle assemblers backwards
                    let assemblers = crate::state::Assembler::all();
                    let current_idx = assemblers
                        .iter()
                        .position(|a| *a == app_state.settings.assembler)
                        .unwrap_or(0);
                    let new_idx = if current_idx == 0 {
                        assemblers.len() - 1
                    } else {
                        current_idx - 1
                    };
                    app_state.settings.assembler = assemblers[new_idx];
                    if (app_state.settings.assembler == crate::state::Assembler::Kick
                        || app_state.settings.assembler == crate::state::Assembler::Ca65)
                        && !app_state.settings.brk_single_byte
                    {
                        app_state.settings.patch_brk = true;
                    }
                } else if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                {
                    self.previous();
                }
            }
            KeyCode::Left => {
                if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                {
                    match self.selected_index {
                        5 => {
                            app_state.settings.max_xref_count =
                                app_state.settings.max_xref_count.saturating_sub(1);
                        }
                        6 => {
                            app_state.settings.max_arrow_columns =
                                app_state.settings.max_arrow_columns.saturating_sub(1);
                        }
                        7 => {
                            app_state.settings.text_char_limit =
                                app_state.settings.text_char_limit.saturating_sub(1);
                        }
                        8 => {
                            if app_state.settings.addresses_per_line > 1 {
                                app_state.settings.addresses_per_line -= 1;
                            }
                        }
                        9 => {
                            if app_state.settings.bytes_per_line > 1 {
                                app_state.settings.bytes_per_line -= 1;
                            }
                        }
                        10 => {
                            let assemblers = crate::state::Assembler::all();
                            let current_idx = assemblers
                                .iter()
                                .position(|a| *a == app_state.settings.assembler)
                                .unwrap_or(0);
                            let new_idx = if current_idx == 0 {
                                assemblers.len() - 1
                            } else {
                                current_idx - 1
                            };
                            app_state.settings.assembler = assemblers[new_idx];
                            if (app_state.settings.assembler == crate::state::Assembler::Kick
                                || app_state.settings.assembler == crate::state::Assembler::Ca65)
                                && !app_state.settings.brk_single_byte
                            {
                                app_state.settings.patch_brk = true;
                            }
                        }
                        11 => {
                            let platforms = crate::state::Platform::all();
                            let current_idx = platforms
                                .iter()
                                .position(|p| *p == app_state.settings.platform)
                                .unwrap_or(0);
                            let new_idx = if current_idx == 0 {
                                platforms.len() - 1
                            } else {
                                current_idx - 1
                            };
                            app_state.settings.platform = platforms[new_idx];
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Right => {
                if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                {
                    match self.selected_index {
                        5 => {
                            app_state.settings.max_xref_count =
                                app_state.settings.max_xref_count.saturating_add(1);
                        }
                        6 => {
                            app_state.settings.max_arrow_columns =
                                app_state.settings.max_arrow_columns.saturating_add(1);
                        }
                        7 => {
                            app_state.settings.text_char_limit =
                                app_state.settings.text_char_limit.saturating_add(1);
                        }
                        8 => {
                            if app_state.settings.addresses_per_line < 8 {
                                app_state.settings.addresses_per_line += 1;
                            }
                        }
                        9 => {
                            if app_state.settings.bytes_per_line < 40 {
                                app_state.settings.bytes_per_line += 1;
                            }
                        }
                        10 => {
                            let assemblers = crate::state::Assembler::all();
                            let current_idx = assemblers
                                .iter()
                                .position(|a| *a == app_state.settings.assembler)
                                .unwrap_or(0);
                            let new_idx = (current_idx + 1) % assemblers.len();
                            app_state.settings.assembler = assemblers[new_idx];
                            if (app_state.settings.assembler == crate::state::Assembler::Kick
                                || app_state.settings.assembler == crate::state::Assembler::Ca65)
                                && !app_state.settings.brk_single_byte
                            {
                                app_state.settings.patch_brk = true;
                            }
                        }
                        11 => {
                            let platforms = crate::state::Platform::all();
                            let current_idx = platforms
                                .iter()
                                .position(|p| *p == app_state.settings.platform)
                                .unwrap_or(0);
                            let new_idx = (current_idx + 1) % platforms.len();
                            app_state.settings.platform = platforms[new_idx];
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Down => {
                if self.is_selecting_platform {
                    // Cycle platforms forwards
                    let platforms = crate::state::Platform::all();
                    let current_idx = platforms
                        .iter()
                        .position(|p| *p == app_state.settings.platform)
                        .unwrap_or(0);
                    let new_idx = (current_idx + 1) % platforms.len();
                    app_state.settings.platform = platforms[new_idx];
                } else if self.is_selecting_assembler {
                    // Cycle assemblers forwards
                    let assemblers = crate::state::Assembler::all();
                    let current_idx = assemblers
                        .iter()
                        .position(|a| *a == app_state.settings.assembler)
                        .unwrap_or(0);
                    let new_idx = (current_idx + 1) % assemblers.len();
                    app_state.settings.assembler = assemblers[new_idx];
                    if (app_state.settings.assembler == crate::state::Assembler::Kick
                        || app_state.settings.assembler == crate::state::Assembler::Ca65)
                        && !app_state.settings.brk_single_byte
                    {
                        app_state.settings.patch_brk = true;
                    }
                } else if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                {
                    self.next();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.is_selecting_platform {
                    self.is_selecting_platform = false;
                } else if self.is_selecting_assembler {
                    self.is_selecting_assembler = false;
                } else if self.is_editing_xref_count {
                    // Commit value
                    if let Ok(val) = self.xref_count_input.parse::<usize>() {
                        app_state.settings.max_xref_count = val;
                        self.is_editing_xref_count = false;
                    }
                } else if self.is_editing_arrow_columns {
                    // Commit value
                    if let Ok(val) = self.arrow_columns_input.parse::<usize>() {
                        app_state.settings.max_arrow_columns = val;
                        self.is_editing_arrow_columns = false;
                    }
                } else if self.is_editing_text_char_limit {
                    // Commit value
                    if let Ok(val) = self.text_char_limit_input.parse::<usize>() {
                        app_state.settings.text_char_limit = val;
                        self.is_editing_text_char_limit = false;
                    }
                } else if self.is_editing_addresses_per_line {
                    // Commit value
                    if let Ok(val) = self.addresses_per_line_input.parse::<usize>() {
                        if (1..=8).contains(&val) {
                            app_state.settings.addresses_per_line = val;
                            self.is_editing_addresses_per_line = false;
                        } else {
                            // Invalid range, maybe reset or keep editing?
                            // Let's clamped it for UX or just keep editing?
                            // Keeping editing is safer.
                            self.addresses_per_line_input = "Invalid (1-8)".to_string();
                        }
                    }
                } else if self.is_editing_bytes_per_line {
                    // Commit value
                    if let Ok(val) = self.bytes_per_line_input.parse::<usize>() {
                        if (1..=40).contains(&val) {
                            app_state.settings.bytes_per_line = val;
                            self.is_editing_bytes_per_line = false;
                        } else {
                            self.bytes_per_line_input = "Invalid (1-40)".to_string();
                        }
                    }
                } else {
                    // Toggle checkbox or enter mode
                    match self.selected_index {
                        0 => app_state.settings.all_labels = !app_state.settings.all_labels,
                        1 => {
                            app_state.settings.preserve_long_bytes =
                                !app_state.settings.preserve_long_bytes;
                        }
                        2 => {
                            app_state.settings.brk_single_byte =
                                !app_state.settings.brk_single_byte;
                            if app_state.settings.brk_single_byte {
                                app_state.settings.patch_brk = false;
                            } else if app_state.settings.assembler == crate::state::Assembler::Kick
                                || app_state.settings.assembler == crate::state::Assembler::Ca65
                            {
                                app_state.settings.patch_brk = true;
                            }
                        }
                        3 => {
                            if !app_state.settings.brk_single_byte {
                                let is_enforced = app_state.settings.assembler
                                    == crate::state::Assembler::Kick
                                    || app_state.settings.assembler
                                        == crate::state::Assembler::Ca65;
                                if !is_enforced {
                                    app_state.settings.patch_brk = !app_state.settings.patch_brk;
                                }
                            }
                        }
                        4 => {
                            app_state.settings.use_illegal_opcodes =
                                !app_state.settings.use_illegal_opcodes;
                        }
                        5 => {
                            self.is_editing_xref_count = true;
                            self.xref_count_input = app_state.settings.max_xref_count.to_string();
                        }
                        6 => {
                            self.is_editing_arrow_columns = true;
                            self.arrow_columns_input =
                                app_state.settings.max_arrow_columns.to_string();
                        }
                        7 => {
                            self.is_editing_text_char_limit = true;
                            self.text_char_limit_input =
                                app_state.settings.text_char_limit.to_string();
                        }
                        8 => {
                            self.is_editing_addresses_per_line = true;
                            self.addresses_per_line_input =
                                app_state.settings.addresses_per_line.to_string();
                        }
                        9 => {
                            self.is_editing_bytes_per_line = true;
                            self.bytes_per_line_input =
                                app_state.settings.bytes_per_line.to_string();
                        }
                        10 => {
                            self.is_selecting_assembler = true;
                        }
                        11 => {
                            self.is_selecting_platform = true;
                        }
                        _ => {}
                    }
                }
            }
            KeyCode::Backspace => {
                if self.is_editing_xref_count {
                    self.xref_count_input.pop();
                } else if self.is_editing_arrow_columns {
                    self.arrow_columns_input.pop();
                } else if self.is_editing_text_char_limit {
                    self.text_char_limit_input.pop();
                } else if self.is_editing_addresses_per_line {
                    self.addresses_per_line_input.pop();
                } else if self.is_editing_bytes_per_line {
                    self.bytes_per_line_input.pop();
                }
            }
            KeyCode::Char(c) => {
                if self.is_editing_xref_count {
                    if c.is_ascii_digit() {
                        self.xref_count_input.push(c);
                    }
                } else if self.is_editing_arrow_columns {
                    if c.is_ascii_digit() {
                        self.arrow_columns_input.push(c);
                    }
                } else if self.is_editing_text_char_limit {
                    if c.is_ascii_digit() {
                        self.text_char_limit_input.push(c);
                    }
                } else if self.is_editing_addresses_per_line {
                    if c.is_ascii_digit() {
                        self.addresses_per_line_input.push(c);
                    }
                } else if self.is_editing_bytes_per_line && c.is_ascii_digit() {
                    self.bytes_per_line_input.push(c);
                }
            }
            _ => {}
        }
        WidgetResult::Handled
    }
}
