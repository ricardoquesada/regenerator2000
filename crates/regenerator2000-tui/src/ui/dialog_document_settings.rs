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
    pub is_selecting_system: bool,
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
    pub is_editing_fill_threshold: bool,
    pub fill_threshold_input: String,
    pub is_editing_description: bool,
    pub description_input: ratatui_textarea::TextArea<'static>,
}

impl Default for DocumentSettingsDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentSettingsDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            is_selecting_system: false,
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
            is_editing_fill_threshold: false,
            fill_threshold_input: String::new(),
            is_editing_description: false,
            description_input: ratatui_textarea::TextArea::default(),
        }
    }

    pub fn next(&mut self) {
        let max_items = 16;
        self.selected_index = (self.selected_index + 1) % max_items;
    }

    pub fn previous(&mut self) {
        let max_items = 16;
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

        let area = crate::utils::centered_rect_adaptive(60, 60, 60, 20, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let settings = &app_state.settings;

        // Helper for checkboxes
        let checkbox =
            |indent: usize, label: &str, checked: bool, selected: bool, disabled: bool| {
                let check_char = if checked { "[X]" } else { "[ ]" };
                let prefix = " ".repeat(indent);
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
                Span::styled(format!("{prefix}{check_char} {label}"), style)
            };

        let patch_brk_disabled = settings.brk_single_byte
            || settings.assembler == crate::state::Assembler::Kick
            || settings.assembler == crate::state::Assembler::Ca65;

        // Base items always present
        let items = vec![
            checkbox(
                0,
                "Display External Labels at top",
                settings.all_labels,
                self.selected_index == 0,
                false,
            ),
            checkbox(
                0,
                "Preserve long bytes (@w, +2, .abs, etc)",
                settings.preserve_long_bytes,
                self.selected_index == 1,
                false,
            ),
            checkbox(
                0,
                "BRK single byte",
                settings.brk_single_byte,
                self.selected_index == 2,
                false,
            ),
            checkbox(
                0,
                "Patch BRK",
                settings.patch_brk,
                self.selected_index == 3,
                patch_brk_disabled,
            ),
            checkbox(
                0,
                "Use Illegal Opcodes",
                settings.use_illegal_opcodes,
                self.selected_index == 4,
                false,
            ),
            checkbox(
                0,
                "Auto-generate Labels & Cross-refs",
                settings.auto_analyze,
                self.selected_index == 5,
                false,
            ),
        ];

        // Dynamic System Config Options
        let system_config = crate::assets::load_system_config(&settings.system);

        // Indices calculation for rigid elements
        let fixed_opts_start = items.len();
        let idx_description = fixed_opts_start;
        let idx_xref = fixed_opts_start + 1;
        let idx_arrow = fixed_opts_start + 2;
        let idx_text_limit = fixed_opts_start + 3;
        let idx_addr_limit = fixed_opts_start + 4;
        let idx_bytes_limit = fixed_opts_start + 5;
        let idx_fill_threshold = fixed_opts_start + 6;
        let idx_assembler = fixed_opts_start + 7;
        let idx_system = fixed_opts_start + 8;

        let mut dynamic_items = Vec::new();
        let dynamic_start_idx = idx_system + 1;

        for (i, feature) in system_config.features.iter().enumerate() {
            let idx = dynamic_start_idx + i;
            let is_enabled = settings
                .enabled_features
                .get(&feature.id)
                .copied()
                .unwrap_or(feature.default);
            dynamic_items.push(checkbox(
                2,
                &feature.name,
                is_enabled,
                self.selected_index == idx,
                false,
            ));
        }

        // Exclude and Comments checkbox indices (after dynamic label items)
        let idx_exclude_comments = dynamic_start_idx + dynamic_items.len();
        let idx_system_comments = idx_exclude_comments + usize::from(system_config.has_excludes);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(items.len() as u16 + 1), // Base Checkboxes + padding
                Constraint::Length(1),                      // Spacer
                Constraint::Length(3),                      // Description
                Constraint::Length(2),                      // Max X-Refs
                Constraint::Length(2),                      // Arrow Columns
                Constraint::Length(2),                      // Text Line Limit
                Constraint::Length(2),                      // Addresses Per Line
                Constraint::Length(2),                      // Bytes Per Line
                Constraint::Length(2),                      // Fill Run Threshold
                Constraint::Length(2),                      // Assembler
                Constraint::Length(2),                      // System
                Constraint::Length(u16::from(!dynamic_items.is_empty())), // System Labels Header
                Constraint::Length(dynamic_items.len() as u16), // Dynamic items
                Constraint::Length(0),                      // System Comments Header (Removed)
                Constraint::Length(u16::from(system_config.has_excludes)), // Exclude comments checkbox
                Constraint::Length(u16::from(system_config.has_comments)), // System Comments checkbox
            ])
            .split(inner);

        // Render Base Items
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

        // Render Dynamic Items Header and List
        if !dynamic_items.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "System Labels:",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Rect::new(layout[11].x + 2, layout[11].y, layout[11].width - 4, 1),
            );

            for (i, item) in dynamic_items.into_iter().enumerate() {
                f.render_widget(
                    Paragraph::new(item),
                    Rect::new(
                        layout[12].x + 2,
                        layout[12].y + i as u16,
                        layout[12].width - 4,
                        1,
                    ),
                );
            }
        }

        // Render Exclude Labels checkbox
        if system_config.has_excludes {
            let exclude_checkbox = checkbox(
                0,
                "Exclude well-known addresses from symbolic analysis",
                settings.exclude_well_known_labels,
                self.selected_index == idx_exclude_comments,
                false,
            );
            f.render_widget(
                Paragraph::new(exclude_checkbox),
                Rect::new(layout[14].x + 2, layout[14].y, layout[14].width - 4, 1),
            );
        }

        // Render System Comments section
        if system_config.has_comments {
            let comments_checkbox = checkbox(
                0,
                "Show system comments",
                settings.show_system_comments,
                self.selected_index == idx_system_comments,
                false,
            );
            f.render_widget(
                Paragraph::new(comments_checkbox),
                Rect::new(layout[15].x + 2, layout[15].y, layout[15].width - 4, 1),
            );
        }

        // Description uses layout[2]
        let desc_selected = self.selected_index == idx_description;

        let desc_title = if self.is_editing_description {
            "Description (Enter to save):"
        } else {
            "Description:"
        };

        // Two lines for description: Title, then Box
        let desc_chunk = layout[2];
        let desc_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(desc_chunk);

        f.render_widget(
            Paragraph::new(desc_title).style(if desc_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            }),
            Rect::new(desc_chunk.x + 2, desc_chunks[0].y, desc_chunk.width - 4, 1),
        );

        if self.is_editing_description {
            let mut textarea = self.description_input.clone();
            let style = Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.menu_selected_bg);
            textarea.set_style(style);
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
            textarea.set_cursor_line_style(Style::default());
            f.render_widget(
                &textarea,
                Rect::new(desc_chunk.x + 2, desc_chunks[1].y, desc_chunk.width - 4, 1),
            );
        } else {
            let desc_value_str = if settings.description.is_empty() {
                "(empty)".to_string()
            } else {
                settings.description.clone()
            };

            let desc_style = if desc_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            };

            f.render_widget(
                Paragraph::new(desc_value_str).style(desc_style),
                Rect::new(desc_chunk.x + 2, desc_chunks[1].y, desc_chunk.width - 4, 1),
            );
        }

        // X-Refs uses layout[3] (layout[1] is spacer, layout[2] is description)
        let xref_selected = self.selected_index == idx_xref;
        let xref_value_str = if self.is_editing_xref_count {
            self.xref_count_input.clone()
        } else {
            settings.max_xref_count.to_string()
        };
        let xref_text = format!("Max X-Refs: < {xref_value_str} >");

        let xref_widget = Paragraph::new(xref_text).style(if xref_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            xref_widget,
            Rect::new(layout[3].x + 2, layout[3].y, layout[3].width - 4, 1),
        );

        // Arrow Columns
        let arrow_selected = self.selected_index == idx_arrow;
        let arrow_value_str = if self.is_editing_arrow_columns {
            self.arrow_columns_input.clone()
        } else {
            settings.max_arrow_columns.to_string()
        };
        let arrow_text = format!("Arrow Columns: < {arrow_value_str} >");

        let arrow_widget = Paragraph::new(arrow_text).style(if arrow_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            arrow_widget,
            Rect::new(layout[4].x + 2, layout[4].y, layout[4].width - 4, 1),
        );

        // Text Line Limit
        let text_limit_selected = self.selected_index == idx_text_limit;
        let text_limit_value_str = if self.is_editing_text_char_limit {
            self.text_char_limit_input.clone()
        } else {
            settings.text_char_limit.to_string()
        };
        let text_limit_text = format!("Text Line Limit: < {text_limit_value_str} >");

        let text_limit_widget = Paragraph::new(text_limit_text).style(if text_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            text_limit_widget,
            Rect::new(layout[5].x + 2, layout[5].y, layout[5].width - 4, 1),
        );

        // Addresses Per Line
        let addr_limit_selected = self.selected_index == idx_addr_limit;
        let addr_limit_value_str = if self.is_editing_addresses_per_line {
            self.addresses_per_line_input.clone()
        } else {
            settings.addresses_per_line.to_string()
        };
        let addr_limit_text = format!("Words/Addrs per line: < {addr_limit_value_str} >");

        let addr_limit_widget = Paragraph::new(addr_limit_text).style(if addr_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            addr_limit_widget,
            Rect::new(layout[6].x + 2, layout[6].y, layout[6].width - 4, 1),
        );

        // Bytes Per Line
        let bytes_limit_selected = self.selected_index == idx_bytes_limit;
        let bytes_limit_value_str = if self.is_editing_bytes_per_line {
            self.bytes_per_line_input.clone()
        } else {
            settings.bytes_per_line.to_string()
        };
        let bytes_limit_text = format!("Bytes per line: < {bytes_limit_value_str} >");

        let bytes_limit_widget = Paragraph::new(bytes_limit_text).style(if bytes_limit_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            bytes_limit_widget,
            Rect::new(layout[7].x + 2, layout[7].y, layout[7].width - 4, 1),
        );

        // Fill Run Threshold
        let fill_threshold_selected = self.selected_index == idx_fill_threshold;
        let fill_threshold_value_str = if self.is_editing_fill_threshold {
            self.fill_threshold_input.clone()
        } else {
            let t = settings.fill_run_threshold;
            if t == 0 {
                "off".to_string()
            } else {
                t.to_string()
            }
        };
        let fill_threshold_text = format!("Fill run threshold: < {fill_threshold_value_str} >");

        let fill_threshold_widget =
            Paragraph::new(fill_threshold_text).style(if fill_threshold_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            });

        f.render_widget(
            fill_threshold_widget,
            Rect::new(layout[8].x + 2, layout[8].y, layout[8].width - 4, 1),
        );

        // Assembler Section
        let assembler_selected = self.selected_index == idx_assembler;
        let assembler_text = format!("Assembler: < {} >", settings.assembler);

        let assembler_widget = Paragraph::new(assembler_text).style(if assembler_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        // Assembler uses layout[9]
        f.render_widget(
            assembler_widget,
            Rect::new(layout[9].x + 2, layout[9].y, layout[9].width - 4, 1),
        );

        // System Section (Moved to end)
        let system_label = Span::raw("System:");
        f.render_widget(
            Paragraph::new(system_label),
            Rect::new(layout[10].x + 2, layout[10].y, layout[10].width - 4, 1),
        );

        let systems = crate::assets::get_available_systems();

        // Check if system is selected
        let system_selected = self.selected_index == idx_system;

        let system_text = format!("System: < {} >", settings.system);
        let system_widget = Paragraph::new(system_text).style(if system_selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        });

        f.render_widget(
            system_widget,
            Rect::new(layout[10].x + 2, layout[10].y, layout[10].width - 4, 1),
        );

        // System Popup
        if self.is_selecting_system {
            let popup_area = crate::utils::centered_rect_adaptive(40, 50, 50, 10, area);
            f.render_widget(Clear, popup_area);
            let block = crate::ui::widget::create_dialog_block(" Select System ", theme);

            let list_items: Vec<ListItem> = systems
                .iter()
                .map(|p| {
                    let is_selected = settings.system == *p;
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.menu_selected_bg)
                            .fg(theme.menu_selected_fg)
                    } else {
                        Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
                    };
                    ListItem::new(p.clone()).style(style)
                })
                .collect();

            let selected_idx = systems
                .iter()
                .position(|p| settings.system == *p)
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
            let popup_area = crate::utils::centered_rect_adaptive(40, 30, 30, 8, area); // Smaller height for fewer items
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

        // Number Input Popup
        let (editing_any, field_name, value_str) = if self.is_editing_xref_count {
            (true, "Max X-Refs", &self.xref_count_input)
        } else if self.is_editing_arrow_columns {
            (true, "Arrow Columns", &self.arrow_columns_input)
        } else if self.is_editing_text_char_limit {
            (true, "Text Line Limit", &self.text_char_limit_input)
        } else if self.is_editing_addresses_per_line {
            (true, "Words/Addrs per line", &self.addresses_per_line_input)
        } else if self.is_editing_bytes_per_line {
            (true, "Bytes per line", &self.bytes_per_line_input)
        } else if self.is_editing_fill_threshold {
            (true, "Fill run threshold", &self.fill_threshold_input)
        } else {
            (false, "", &String::new())
        };

        if editing_any {
            let popup_area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
            f.render_widget(Clear, popup_area);
            let title = format!(" Edit {field_name} ");
            let block = crate::ui::widget::create_dialog_block(&title, theme);

            let widget = Paragraph::new(value_str.clone())
                .style(
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                )
                .block(block);
            f.render_widget(widget, popup_area);

            // Blinking cursor
            f.set_cursor_position((popup_area.x + 1 + value_str.len() as u16, popup_area.y + 1));
        }
    }

    fn handle_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Calculate dynamic max items for navigation
        let system_config = crate::assets::load_system_config(&app_state.settings.system);
        let dynamic_items_count = system_config.features.len();

        let base_items_count = 6; // AllLabels, PreserveLongBytes, BrkSingle, PatchBrk, IllegalOpcodes, AutoAnalyze

        let idx_description = base_items_count;
        let idx_xref = base_items_count + 1;
        let idx_arrow = base_items_count + 2;
        let idx_text_limit = base_items_count + 3;
        let idx_addr_limit = base_items_count + 4;
        let idx_bytes_limit = base_items_count + 5;
        let idx_fill_threshold = base_items_count + 6;
        let idx_assembler = base_items_count + 7;
        let idx_system = base_items_count + 8;
        let dynamic_start_idx = idx_system + 1;
        let idx_exclude_comments = dynamic_start_idx + dynamic_items_count;
        let idx_system_comments = idx_exclude_comments + usize::from(system_config.has_excludes);

        let total_items = if system_config.has_comments {
            idx_system_comments + 1
        } else if system_config.has_excludes {
            idx_exclude_comments + 1
        } else {
            dynamic_start_idx + dynamic_items_count
        };

        let next = |idx: &mut usize| {
            *idx = (*idx + 1) % total_items;
        };
        let prev = |idx: &mut usize| {
            if *idx == 0 {
                *idx = total_items - 1;
            } else {
                *idx -= 1;
            }
        };

        if self.is_editing_description {
            match key.code {
                KeyCode::Esc => {
                    self.is_editing_description = false;
                    self.description_input = ratatui_textarea::TextArea::default();
                    ui_state.set_status_message("Ready");
                    return WidgetResult::Handled;
                }
                KeyCode::Enter => {
                    app_state.settings.description =
                        self.description_input.lines().join("").trim().to_string();
                    self.is_editing_description = false;
                    return WidgetResult::Handled;
                }
                _ => {
                    self.description_input.input(key);
                    return WidgetResult::Handled;
                }
            }
        }

        match key.code {
            KeyCode::Esc => {
                if self.is_selecting_system {
                    self.is_selecting_system = false;
                } else if self.is_selecting_assembler {
                    self.is_selecting_assembler = false;
                } else if self.is_editing_xref_count {
                    self.is_editing_xref_count = false;
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

                    // Capture current address to preserve cursor position
                    let restore_addr = app_state
                        .disassembly
                        .get(ui_state.cursor_index)
                        .map(|l| l.address);
                    let old_sub_cursor = ui_state.sub_cursor_index;

                    app_state.load_system_assets();
                    if app_state.settings.auto_analyze {
                        app_state.perform_analysis();
                    }
                    app_state.disassemble();

                    // Restore cursor position
                    if let Some(addr) = restore_addr
                        && let Some(new_idx) = app_state.get_line_index_for_address(addr)
                    {
                        ui_state.cursor_index = new_idx;
                        // Attempt to preserve sub-cursor (e.g. comments/labels) if valid
                        if let Some(line) = app_state.disassembly.get(new_idx) {
                            let counts = crate::ui::view_disassembly::DisassemblyView::get_visual_line_counts(
                                    line, app_state,
                                );
                            if old_sub_cursor < counts.total() {
                                ui_state.sub_cursor_index = old_sub_cursor;
                            } else {
                                ui_state.sub_cursor_index = 0;
                            }
                        }
                    }

                    return WidgetResult::Close;
                }
            }
            KeyCode::Up => {
                if self.is_selecting_system {
                    let systems = crate::assets::get_available_systems();
                    if !systems.is_empty() {
                        let current_idx = systems
                            .iter()
                            .position(|p| app_state.settings.system == *p)
                            .unwrap_or(0);
                        let new_idx = if current_idx == 0 {
                            systems.len() - 1
                        } else {
                            current_idx - 1
                        };
                        app_state.settings.system =
                            crate::state::System::from(systems[new_idx].clone());
                        // Reset features when changing system
                        app_state.settings.enabled_features.clear();
                        self.selected_index = idx_system;
                    }
                } else if self.is_selecting_assembler {
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
                    && !self.is_editing_fill_threshold
                    && !self.is_editing_description
                {
                    prev(&mut self.selected_index);
                }
            }
            KeyCode::Left
                if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                    && !self.is_editing_fill_threshold
                    && !self.is_editing_description =>
            {
                if self.selected_index == idx_xref {
                    app_state.settings.max_xref_count =
                        app_state.settings.max_xref_count.saturating_sub(1);
                } else if self.selected_index == idx_arrow {
                    app_state.settings.max_arrow_columns =
                        app_state.settings.max_arrow_columns.saturating_sub(1);
                } else if self.selected_index == idx_text_limit {
                    app_state.settings.text_char_limit =
                        app_state.settings.text_char_limit.saturating_sub(1);
                } else if self.selected_index == idx_addr_limit {
                    if app_state.settings.addresses_per_line > 1 {
                        app_state.settings.addresses_per_line -= 1;
                    }
                } else if self.selected_index == idx_bytes_limit {
                    if app_state.settings.bytes_per_line > 1 {
                        app_state.settings.bytes_per_line -= 1;
                    }
                } else if self.selected_index == idx_fill_threshold {
                    app_state.settings.fill_run_threshold =
                        app_state.settings.fill_run_threshold.saturating_sub(1);
                } else if self.selected_index == idx_assembler {
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
                } else if self.selected_index == idx_system {
                    let systems = crate::assets::get_available_systems();
                    if !systems.is_empty() {
                        let current_idx = systems
                            .iter()
                            .position(|p| app_state.settings.system == *p)
                            .unwrap_or(0);
                        let new_idx = if current_idx == 0 {
                            systems.len() - 1
                        } else {
                            current_idx - 1
                        };
                        app_state.settings.system =
                            crate::state::System::from(systems[new_idx].clone());
                        // Reset features when changing system
                        app_state.settings.enabled_features.clear();
                        self.selected_index = idx_system;
                    }
                }
            }
            KeyCode::Right
                if !self.is_editing_xref_count
                    && !self.is_editing_arrow_columns
                    && !self.is_editing_text_char_limit
                    && !self.is_editing_addresses_per_line
                    && !self.is_editing_bytes_per_line
                    && !self.is_editing_fill_threshold
                    && !self.is_editing_description =>
            {
                if self.selected_index == idx_xref {
                    if app_state.settings.max_xref_count < 40 {
                        app_state.settings.max_xref_count += 1;
                    }
                } else if self.selected_index == idx_arrow {
                    if app_state.settings.max_arrow_columns < 10 {
                        app_state.settings.max_arrow_columns += 1;
                    }
                } else if self.selected_index == idx_text_limit {
                    if app_state.settings.text_char_limit < 80 {
                        app_state.settings.text_char_limit += 1;
                    }
                } else if self.selected_index == idx_addr_limit {
                    if app_state.settings.addresses_per_line < 8 {
                        app_state.settings.addresses_per_line += 1;
                    }
                } else if self.selected_index == idx_bytes_limit {
                    if app_state.settings.bytes_per_line < 40 {
                        app_state.settings.bytes_per_line += 1;
                    }
                } else if self.selected_index == idx_fill_threshold {
                    if app_state.settings.fill_run_threshold < 64 {
                        app_state.settings.fill_run_threshold += 1;
                    }
                } else if self.selected_index == idx_assembler {
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
                } else if self.selected_index == idx_system {
                    let systems = crate::assets::get_available_systems();
                    if !systems.is_empty() {
                        let current_idx = systems
                            .iter()
                            .position(|p| app_state.settings.system == *p)
                            .unwrap_or(0);
                        let new_idx = (current_idx + 1) % systems.len();
                        app_state.settings.system =
                            crate::state::System::from(systems[new_idx].clone());
                        // Reset features when changing system
                        app_state.settings.enabled_features.clear();
                        self.selected_index = idx_system;
                    }
                }
            }
            KeyCode::Down => {
                if self.is_selecting_system {
                    let systems = crate::assets::get_available_systems();
                    if !systems.is_empty() {
                        let current_idx = systems
                            .iter()
                            .position(|p| app_state.settings.system == *p)
                            .unwrap_or(0);
                        let new_idx = (current_idx + 1) % systems.len();
                        app_state.settings.system =
                            crate::state::System::from(systems[new_idx].clone());
                        // Reset features when changing system
                        app_state.settings.enabled_features.clear();
                        self.selected_index = idx_system;
                    }
                } else if self.is_selecting_assembler {
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
                    && !self.is_editing_fill_threshold
                    && !self.is_editing_description
                {
                    next(&mut self.selected_index);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.is_selecting_system {
                    self.is_selecting_system = false;
                } else if self.is_selecting_assembler {
                    self.is_selecting_assembler = false;
                } else if self.is_editing_xref_count {
                    if let Ok(val) = self.xref_count_input.parse::<usize>() {
                        if val <= 40 {
                            app_state.settings.max_xref_count = val;
                            self.is_editing_xref_count = false;
                        } else {
                            self.xref_count_input = "Invalid (0-40)".to_string();
                        }
                    }
                } else if self.is_editing_arrow_columns {
                    if let Ok(val) = self.arrow_columns_input.parse::<usize>() {
                        if (1..=10).contains(&val) {
                            app_state.settings.max_arrow_columns = val;
                            self.is_editing_arrow_columns = false;
                        } else {
                            self.arrow_columns_input = "Invalid (1-10)".to_string();
                        }
                    }
                } else if self.is_editing_text_char_limit {
                    if let Ok(val) = self.text_char_limit_input.parse::<usize>() {
                        if (1..=80).contains(&val) {
                            app_state.settings.text_char_limit = val;
                            self.is_editing_text_char_limit = false;
                        } else {
                            self.text_char_limit_input = "Invalid (1-80)".to_string();
                        }
                    }
                } else if self.is_editing_addresses_per_line {
                    if let Ok(val) = self.addresses_per_line_input.parse::<usize>() {
                        if (1..=8).contains(&val) {
                            app_state.settings.addresses_per_line = val;
                            self.is_editing_addresses_per_line = false;
                        } else {
                            self.addresses_per_line_input = "Invalid (1-8)".to_string();
                        }
                    }
                } else if self.is_editing_bytes_per_line {
                    if let Ok(val) = self.bytes_per_line_input.parse::<usize>() {
                        if (1..=40).contains(&val) {
                            app_state.settings.bytes_per_line = val;
                            self.is_editing_bytes_per_line = false;
                        } else {
                            self.bytes_per_line_input = "Invalid (1-40)".to_string();
                        }
                    }
                } else if self.is_editing_fill_threshold {
                    if let Ok(val) = self.fill_threshold_input.parse::<usize>() {
                        if val <= 64 {
                            app_state.settings.fill_run_threshold = val;
                            self.is_editing_fill_threshold = false;
                        } else {
                            self.fill_threshold_input = "Invalid (0-64)".to_string();
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
                        3 if !app_state.settings.brk_single_byte => {
                            let is_enforced = app_state.settings.assembler
                                == crate::state::Assembler::Kick
                                || app_state.settings.assembler == crate::state::Assembler::Ca65;
                            if !is_enforced {
                                app_state.settings.patch_brk = !app_state.settings.patch_brk;
                            }
                        }
                        4 => {
                            app_state.settings.use_illegal_opcodes =
                                !app_state.settings.use_illegal_opcodes;
                        }
                        5 => {
                            app_state.settings.auto_analyze = !app_state.settings.auto_analyze;
                            if app_state.settings.auto_analyze {
                                // Toggled ON: run analysis to regenerate labels & xrefs
                                app_state.perform_analysis();
                            } else {
                                // Toggled OFF: remove auto-generated labels, clear xrefs
                                for labels_vec in app_state.labels.values_mut() {
                                    labels_vec.retain(|l| l.kind != crate::state::LabelKind::Auto);
                                }
                                app_state.labels.retain(|_, v| !v.is_empty());
                                app_state.cross_refs.clear();
                                app_state.disassemble();
                            }
                        }
                        idx if idx == idx_system_comments && system_config.has_comments => {
                            app_state.settings.show_system_comments =
                                !app_state.settings.show_system_comments;
                            // Reload system assets and re-disassemble for immediate feedback
                            app_state.load_system_assets();
                            app_state.disassemble();
                        }
                        idx if idx == idx_exclude_comments && system_config.has_excludes => {
                            app_state.settings.exclude_well_known_labels =
                                !app_state.settings.exclude_well_known_labels;
                            // Reload system assets and re-disassemble for immediate feedback
                            app_state.load_system_assets();
                            app_state.disassemble();
                        }
                        idx if idx >= dynamic_start_idx => {
                            // Dynamic items (system labels)
                            let system_config =
                                crate::assets::load_system_config(&app_state.settings.system);
                            let config_idx = idx - dynamic_start_idx;
                            if let Some(feature) = system_config.features.get(config_idx) {
                                let current_val = app_state
                                    .settings
                                    .enabled_features
                                    .get(&feature.id)
                                    .copied()
                                    .unwrap_or(feature.default);
                                app_state
                                    .settings
                                    .enabled_features
                                    .insert(feature.id.clone(), !current_val);

                                // Reload system labels and re-disassemble for immediate feedback
                                app_state.load_system_assets();
                                app_state.disassemble();
                            }
                        }
                        idx if idx == idx_description => {
                            self.is_editing_description = true;
                            let mut textarea = ratatui_textarea::TextArea::default();
                            textarea.insert_str(&app_state.settings.description);
                            textarea.move_cursor(ratatui_textarea::CursorMove::End);
                            self.description_input = textarea;
                        }
                        idx if idx == idx_xref => {
                            self.is_editing_xref_count = true;
                            self.xref_count_input = app_state.settings.max_xref_count.to_string();
                        }
                        idx if idx == idx_arrow => {
                            self.is_editing_arrow_columns = true;
                            self.arrow_columns_input =
                                app_state.settings.max_arrow_columns.to_string();
                        }
                        idx if idx == idx_text_limit => {
                            self.is_editing_text_char_limit = true;
                            self.text_char_limit_input =
                                app_state.settings.text_char_limit.to_string();
                        }
                        idx if idx == idx_addr_limit => {
                            self.is_editing_addresses_per_line = true;
                            self.addresses_per_line_input =
                                app_state.settings.addresses_per_line.to_string();
                        }
                        idx if idx == idx_bytes_limit => {
                            self.is_editing_bytes_per_line = true;
                            self.bytes_per_line_input =
                                app_state.settings.bytes_per_line.to_string();
                        }
                        idx if idx == idx_fill_threshold => {
                            self.is_editing_fill_threshold = true;
                            self.fill_threshold_input =
                                app_state.settings.fill_run_threshold.to_string();
                        }
                        idx if idx == idx_assembler => {
                            self.is_selecting_assembler = true;
                        }
                        idx if idx == idx_system => {
                            self.is_selecting_system = true;
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
                } else if self.is_editing_fill_threshold {
                    self.fill_threshold_input.pop();
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
                } else if self.is_editing_fill_threshold && c.is_ascii_digit() {
                    self.fill_threshold_input.push(c);
                }
            }
            _ => {}
        }
        WidgetResult::Handled
    }
}
