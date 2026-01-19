use crate::state::AppState;
use crate::theme::Theme;
use crate::ui_state::UIState;
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};

pub struct DocumentSettingsDialog {
    pub active: bool,
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

impl DocumentSettingsDialog {
    pub fn new() -> Self {
        Self {
            active: false,
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

    pub fn open(&mut self) {
        self.active = true;
        self.selected_index = 0;
        self.is_selecting_platform = false;
        self.is_selecting_assembler = false;
        self.is_editing_xref_count = false;
        self.xref_count_input.clear();
        self.is_editing_arrow_columns = false;
        self.arrow_columns_input.clear();
        self.is_editing_text_char_limit = false;
        self.text_char_limit_input.clear();
        self.is_editing_addresses_per_line = false;
        self.addresses_per_line_input.clear();
        self.is_editing_bytes_per_line = false;
        self.bytes_per_line_input.clear();
    }

    pub fn close(&mut self) {
        self.active = false;
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

pub fn render(
    f: &mut Frame,
    area: Rect,
    app_state: &AppState,
    dialog: &DocumentSettingsDialog,
    theme: &Theme,
) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Document Settings ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let area = crate::utils::centered_rect(60, 60, area);
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
            dialog.selected_index == 0,
            false,
        ),
        checkbox(
            "Preserve long bytes (@w, +2, .abs, etc)",
            settings.preserve_long_bytes,
            dialog.selected_index == 1,
            false,
        ),
        checkbox(
            "BRK single byte",
            settings.brk_single_byte,
            dialog.selected_index == 2,
            false,
        ),
        checkbox(
            "Patch BRK",
            settings.patch_brk,
            dialog.selected_index == 3,
            patch_brk_disabled,
        ),
        checkbox(
            "Use Illegal Opcodes",
            settings.use_illegal_opcodes,
            dialog.selected_index == 4,
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
    let xref_selected = dialog.selected_index == 5;
    let xref_value_str = if dialog.is_editing_xref_count {
        dialog.xref_count_input.clone()
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
    let arrow_selected = dialog.selected_index == 6;
    let arrow_value_str = if dialog.is_editing_arrow_columns {
        dialog.arrow_columns_input.clone()
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
    let text_limit_selected = dialog.selected_index == 7;
    let text_limit_value_str = if dialog.is_editing_text_char_limit {
        dialog.text_char_limit_input.clone()
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
    let addr_limit_selected = dialog.selected_index == 8;
    let addr_limit_value_str = if dialog.is_editing_addresses_per_line {
        dialog.addresses_per_line_input.clone()
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
    let bytes_limit_selected = dialog.selected_index == 9;
    let bytes_limit_value_str = if dialog.is_editing_bytes_per_line {
        dialog.bytes_per_line_input.clone()
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
    let assembler_selected = dialog.selected_index == 10;
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
    let platform_selected = dialog.selected_index == 11;

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
    if dialog.is_selecting_platform {
        let popup_area = crate::utils::centered_rect(40, 50, area);
        f.render_widget(Clear, popup_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Platform ");

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
    if dialog.is_selecting_assembler {
        let popup_area = crate::utils::centered_rect(40, 30, area); // Smaller height for fewer items
        f.render_widget(Clear, popup_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Assembler ");

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

pub fn handle_input(
    key: crossterm::event::KeyEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) {
    let dialog = &mut ui_state.settings_dialog;

    match key.code {
        KeyCode::Esc => {
            if dialog.is_selecting_platform {
                dialog.is_selecting_platform = false;
            } else if dialog.is_selecting_assembler {
                dialog.is_selecting_assembler = false;
            } else if dialog.is_editing_xref_count {
                dialog.is_editing_xref_count = false;
                // Reset input to current value
                dialog.xref_count_input.clear();
            } else if dialog.is_editing_arrow_columns {
                dialog.is_editing_arrow_columns = false;
                dialog.arrow_columns_input.clear();
            } else if dialog.is_editing_text_char_limit {
                dialog.is_editing_text_char_limit = false;
                dialog.text_char_limit_input.clear();
            } else if dialog.is_editing_addresses_per_line {
                dialog.is_editing_addresses_per_line = false;
                dialog.addresses_per_line_input.clear();
            } else if dialog.is_editing_bytes_per_line {
                dialog.is_editing_bytes_per_line = false;
                dialog.bytes_per_line_input.clear();
            } else {
                dialog.close();
                ui_state.set_status_message("Ready");
                app_state.load_system_assets();
                app_state.perform_analysis();
                app_state.disassemble(); // Disassemble on close to apply all settings
            }
        }
        KeyCode::Up => {
            if dialog.is_selecting_platform {
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
            } else if dialog.is_selecting_assembler {
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
            } else if !dialog.is_editing_xref_count
                && !dialog.is_editing_arrow_columns
                && !dialog.is_editing_text_char_limit
                && !dialog.is_editing_addresses_per_line
                && !dialog.is_editing_bytes_per_line
            {
                dialog.previous();
            }
        }
        KeyCode::Left => {
            if !dialog.is_editing_xref_count
                && !dialog.is_editing_arrow_columns
                && !dialog.is_editing_text_char_limit
                && !dialog.is_editing_addresses_per_line
                && !dialog.is_editing_bytes_per_line
            {
                match dialog.selected_index {
                    6 => {
                        app_state.settings.max_xref_count =
                            app_state.settings.max_xref_count.saturating_sub(1);
                    }
                    7 => {
                        app_state.settings.max_arrow_columns =
                            app_state.settings.max_arrow_columns.saturating_sub(1);
                    }
                    8 => {
                        app_state.settings.text_char_limit =
                            app_state.settings.text_char_limit.saturating_sub(1);
                    }
                    9 => {
                        if app_state.settings.addresses_per_line > 1 {
                            app_state.settings.addresses_per_line -= 1;
                        }
                    }
                    10 => {
                        if app_state.settings.bytes_per_line > 1 {
                            app_state.settings.bytes_per_line -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Right => {
            if !dialog.is_editing_xref_count
                && !dialog.is_editing_arrow_columns
                && !dialog.is_editing_text_char_limit
                && !dialog.is_editing_addresses_per_line
                && !dialog.is_editing_bytes_per_line
            {
                match dialog.selected_index {
                    6 => {
                        app_state.settings.max_xref_count =
                            app_state.settings.max_xref_count.saturating_add(1);
                    }
                    7 => {
                        app_state.settings.max_arrow_columns =
                            app_state.settings.max_arrow_columns.saturating_add(1);
                    }
                    8 => {
                        app_state.settings.text_char_limit =
                            app_state.settings.text_char_limit.saturating_add(1);
                    }
                    9 => {
                        if app_state.settings.addresses_per_line < 8 {
                            app_state.settings.addresses_per_line += 1;
                        }
                    }
                    10 => {
                        if app_state.settings.bytes_per_line < 40 {
                            app_state.settings.bytes_per_line += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Down => {
            if dialog.is_selecting_platform {
                // Cycle platforms forwards
                let platforms = crate::state::Platform::all();
                let current_idx = platforms
                    .iter()
                    .position(|p| *p == app_state.settings.platform)
                    .unwrap_or(0);
                let new_idx = (current_idx + 1) % platforms.len();
                app_state.settings.platform = platforms[new_idx];
            } else if dialog.is_selecting_assembler {
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
            } else if !dialog.is_editing_xref_count
                && !dialog.is_editing_arrow_columns
                && !dialog.is_editing_text_char_limit
                && !dialog.is_editing_addresses_per_line
                && !dialog.is_editing_bytes_per_line
            {
                dialog.next();
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if dialog.is_selecting_platform {
                dialog.is_selecting_platform = false;
            } else if dialog.is_selecting_assembler {
                dialog.is_selecting_assembler = false;
            } else if dialog.is_editing_xref_count {
                // Commit value
                if let Ok(val) = dialog.xref_count_input.parse::<usize>() {
                    app_state.settings.max_xref_count = val;
                    dialog.is_editing_xref_count = false;
                }
            } else if dialog.is_editing_arrow_columns {
                // Commit value
                if let Ok(val) = dialog.arrow_columns_input.parse::<usize>() {
                    app_state.settings.max_arrow_columns = val;
                    dialog.is_editing_arrow_columns = false;
                }
            } else if dialog.is_editing_text_char_limit {
                // Commit value
                if let Ok(val) = dialog.text_char_limit_input.parse::<usize>() {
                    app_state.settings.text_char_limit = val;
                    dialog.is_editing_text_char_limit = false;
                }
            } else if dialog.is_editing_addresses_per_line {
                // Commit value
                if let Ok(val) = dialog.addresses_per_line_input.parse::<usize>() {
                    if (1..=8).contains(&val) {
                        app_state.settings.addresses_per_line = val;
                        dialog.is_editing_addresses_per_line = false;
                    } else {
                        // Invalid range, maybe reset or keep editing?
                        // Let's clamped it for UX or just keep editing?
                        // Keeping editing is safer.
                        dialog.addresses_per_line_input = "Invalid (1-8)".to_string();
                    }
                }
            } else if dialog.is_editing_bytes_per_line {
                // Commit value
                if let Ok(val) = dialog.bytes_per_line_input.parse::<usize>() {
                    if (1..=40).contains(&val) {
                        app_state.settings.bytes_per_line = val;
                        dialog.is_editing_bytes_per_line = false;
                    } else {
                        dialog.bytes_per_line_input = "Invalid (1-40)".to_string();
                    }
                }
            } else {
                // Toggle checkbox or enter mode
                match dialog.selected_index {
                    0 => app_state.settings.all_labels = !app_state.settings.all_labels,
                    1 => {
                        app_state.settings.preserve_long_bytes =
                            !app_state.settings.preserve_long_bytes;
                    }
                    2 => {
                        app_state.settings.brk_single_byte = !app_state.settings.brk_single_byte;
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
                                || app_state.settings.assembler == crate::state::Assembler::Ca65;
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
                        dialog.is_editing_xref_count = true;
                        dialog.xref_count_input = app_state.settings.max_xref_count.to_string();
                    }
                    6 => {
                        dialog.is_editing_arrow_columns = true;
                        dialog.arrow_columns_input =
                            app_state.settings.max_arrow_columns.to_string();
                    }
                    7 => {
                        dialog.is_editing_text_char_limit = true;
                        dialog.text_char_limit_input =
                            app_state.settings.text_char_limit.to_string();
                    }
                    8 => {
                        dialog.is_editing_addresses_per_line = true;
                        dialog.addresses_per_line_input =
                            app_state.settings.addresses_per_line.to_string();
                    }
                    9 => {
                        dialog.is_editing_bytes_per_line = true;
                        dialog.bytes_per_line_input = app_state.settings.bytes_per_line.to_string();
                    }
                    10 => {
                        dialog.is_selecting_assembler = true;
                    }
                    11 => {
                        dialog.is_selecting_platform = true;
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Backspace => {
            if dialog.is_editing_xref_count {
                dialog.xref_count_input.pop();
            } else if dialog.is_editing_arrow_columns {
                dialog.arrow_columns_input.pop();
            } else if dialog.is_editing_text_char_limit {
                dialog.text_char_limit_input.pop();
            } else if dialog.is_editing_addresses_per_line {
                dialog.addresses_per_line_input.pop();
            } else if dialog.is_editing_bytes_per_line {
                dialog.bytes_per_line_input.pop();
            }
        }
        KeyCode::Char(c) => {
            if dialog.is_editing_xref_count {
                if c.is_ascii_digit() {
                    dialog.xref_count_input.push(c);
                }
            } else if dialog.is_editing_arrow_columns {
                if c.is_ascii_digit() {
                    dialog.arrow_columns_input.push(c);
                }
            } else if dialog.is_editing_text_char_limit {
                if c.is_ascii_digit() {
                    dialog.text_char_limit_input.push(c);
                }
            } else if dialog.is_editing_addresses_per_line {
                if c.is_ascii_digit() {
                    dialog.addresses_per_line_input.push(c);
                }
            } else if dialog.is_editing_bytes_per_line && c.is_ascii_digit() {
                dialog.bytes_per_line_input.push(c);
            }
        }
        _ => {}
    }
}
