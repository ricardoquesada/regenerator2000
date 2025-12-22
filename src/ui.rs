use crate::state::AppState;
use crate::ui_state::UIState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use ratatui_image::StatefulImage;

pub fn ui(f: &mut Frame, app_state: &AppState, ui_state: &mut UIState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Menu
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    render_menu(f, chunks[0], &ui_state.menu);
    render_main_view(f, chunks[1], app_state, ui_state);
    render_status_bar(f, chunks[2], app_state, ui_state);

    // Render Popup if needed
    if ui_state.menu.active && ui_state.menu.selected_item.is_some() {
        render_menu_popup(f, chunks[0], &ui_state.menu);
    }

    if ui_state.file_picker.active {
        render_file_picker(f, f.area(), &ui_state.file_picker);
    }

    if ui_state.jump_dialog.active {
        render_jump_dialog(f, f.area(), &ui_state.jump_dialog);
    }

    if ui_state.save_dialog.active {
        render_save_dialog(f, f.area(), &ui_state.save_dialog);
    }

    if ui_state.label_dialog.active {
        render_label_dialog(f, f.area(), &ui_state.label_dialog);
    }

    if ui_state.settings_dialog.active {
        render_settings_dialog(f, f.area(), app_state, &ui_state.settings_dialog);
    }

    if ui_state.about_dialog.active {
        render_about_dialog(f, ui_state, f.area());
    }
}

fn render_about_dialog(f: &mut Frame, ui_state: &UIState, area: Rect) {
    if let Some(logo) = &ui_state.logo {
        if let Some(picker) = &ui_state.picker {
            // Center popup
            let percent_x = 60;
            let percent_y = 60;
            let popup_width = area.width * percent_x / 100;
            let popup_height = area.height * percent_y / 100;
            let x = (area.width - popup_width) / 2;
            let y = (area.height - popup_height) / 2;

            let popup_area = ratatui::layout::Rect::new(x, y, popup_width, popup_height);

            f.render_widget(ratatui::widgets::Clear, popup_area);

            let block = Block::default().title(" About ").borders(Borders::ALL);
            let inner = block.inner(popup_area);
            f.render_widget(block, popup_area);

            // Split inner area: Top (Image), Bottom (Text)
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Percentage(70),
                    ratatui::layout::Constraint::Percentage(30),
                ])
                .split(inner);

            // 1. Render Logo (Scaled & Centered in chunks[0])
            let img_area = chunks[0];
            let img_width = logo.width() as u16;
            let img_height = logo.height() as u16;

            let term_height_scale = 2;
            let img_width_f = img_width as f64;
            let img_height_f = (img_height / term_height_scale) as f64;

            let avail_width_f = img_area.width as f64;
            let avail_height_f = img_area.height as f64;

            let width_scale = avail_width_f / img_width_f;
            let height_scale = avail_height_f / img_height_f;

            let scale = width_scale.min(height_scale).min(1.0);

            let render_width = (img_width_f * scale) as u16;
            let render_height = (img_height_f * scale) as u16;

            let x = img_area.x + (img_area.width.saturating_sub(render_width)) / 2;
            let y = img_area.y + (img_area.height.saturating_sub(render_height)) / 2;

            let centered_area = ratatui::layout::Rect::new(x, y, render_width, render_height);

            let mut protocol = picker.new_resize_protocol(logo.clone());
            let widget = StatefulImage::new();
            f.render_stateful_widget(widget, centered_area, &mut protocol);

            // 2. Render Text
            let text_area = chunks[1];
            let text = "Regenerator2000\n(c) Ricardo Quesada 2026";
            let paragraph = Paragraph::new(text)
                .alignment(ratatui::layout::Alignment::Center)
                .block(Block::default());

            // Vertically center text in text_area
            let text_height = 2;
            let text_y = text_area.y + (text_area.height.saturating_sub(text_height)) / 2;
            let centered_text_area =
                ratatui::layout::Rect::new(text_area.x, text_y, text_area.width, text_height);

            f.render_widget(paragraph, centered_text_area);
        }
    }
}

fn render_settings_dialog(
    f: &mut Frame,
    area: Rect,
    app_state: &AppState,
    dialog: &crate::ui_state::SettingsDialogState,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Document Settings ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let area = centered_rect(60, 60, area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    let settings = &app_state.settings;

    // Helper for checkboxes
    let checkbox = |label: &str, checked: bool, selected: bool, disabled: bool| {
        let check_char = if checked { "[X]" } else { "[ ]" };
        let style = if disabled {
            if selected {
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC) // Selected but disabled
            } else {
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC) // Disabled and Italic
            }
        } else if selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Span::styled(format!("{} {}", check_char, label), style)
    };

    let patch_brk_disabled = settings.brk_single_byte;

    let items = vec![
        checkbox(
            "All Labels",
            settings.all_labels,
            dialog.selected_index == 0,
            false,
        ),
        checkbox(
            "Use @w for long bytes",
            settings.use_w_prefix,
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
    ];

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(items.len() as u16 + 1), // Checkboxes + padding
            Constraint::Min(1),                         // Platform list
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

    // Platform Section
    let platform_label = Span::styled(
        "Platform:",
        Style::default().add_modifier(Modifier::UNDERLINED),
    );
    f.render_widget(
        Paragraph::new(platform_label),
        Rect::new(layout[1].x + 2, layout[1].y, layout[1].width - 4, 1),
    );

    let platforms = crate::state::Platform::all();

    // We need to show the list of platforms. Since it's long, we can scroll it?
    // Or just show all if it fits. 13 items. 60% of screen height should fit 13 items + 5 checkboes = 18 lines.

    // Check if platform is selected
    let platform_selected = dialog.selected_index == 4;

    let platform_text = format!("Platform: < {} >", settings.platform);
    let platform_widget = Paragraph::new(platform_text).style(if platform_selected {
        if dialog.is_selecting_platform {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD) // Active
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(Color::White)
    });

    f.render_widget(
        platform_widget,
        Rect::new(layout[1].x + 2, layout[1].y, layout[1].width - 4, 1),
    );

    // If selecting platform, show the list popup
    if dialog.is_selecting_platform {
        let popup_area = centered_rect(40, 50, area);
        f.render_widget(ratatui::widgets::Clear, popup_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Platform ");

        let list_items: Vec<ListItem> = platforms
            .iter()
            .map(|p| {
                let is_selected = *p == settings.platform;
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(p.to_string()).style(style)
            })
            .collect();

        // We need a ListState to scroll to current selection.
        // Since we don't have a persistent ListState for this in UIState (my bad),
        // I'll create a temporary one here. It won't remember scroll position between frames perfectly if the list is huge,
        // but for 13 items it might fit or basic scrolling defaults to 0.
        // Wait, to support scrolling I need to persist the state or correct index.
        // The `settings.platform` acts as the "selected index" equivalent.
        // I can find the index of `settings.platform` in `platforms`.

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
}

fn render_label_dialog(f: &mut Frame, area: Rect, dialog: &crate::ui_state::LabelDialogState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Enter Label Name ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let area = centered_rect(50, 20, area);
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_save_dialog(f: &mut Frame, area: Rect, dialog: &crate::ui_state::SaveDialogState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save Project As... ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let area = centered_rect(50, 20, area);
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_jump_dialog(f: &mut Frame, area: Rect, dialog: &crate::ui_state::JumpDialogState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Jump to Address (Hex) ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let area = centered_rect(40, 20, area);
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_file_picker(f: &mut Frame, area: Rect, picker: &crate::ui_state::FilePickerState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Open File (Space to Open, Backspace to Go Back, Esc to Cancel) ")
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let area = centered_rect(60, 50, area);
    f.render_widget(ratatui::widgets::Clear, area); // Clear background

    let items: Vec<ListItem> = picker
        .files
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let name = if path.is_dir() {
                format!("{}/", name)
            } else {
                name.to_string()
            };

            ListItem::new(name)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(picker.selected_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_menu(f: &mut Frame, area: Rect, menu_state: &crate::ui_state::MenuState) {
    let mut spans = Vec::new();

    for (i, category) in menu_state.categories.iter().enumerate() {
        let style = if menu_state.active && i == menu_state.selected_category {
            Style::default().bg(Color::White).fg(Color::Black)
        } else {
            Style::default().bg(Color::Blue).fg(Color::White)
        };

        spans.push(Span::styled(format!(" {} ", category.name), style));
    }

    // Fill the rest of the line
    let menu_bar =
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(menu_bar, area);
}

fn render_menu_popup(f: &mut Frame, top_area: Rect, menu_state: &crate::ui_state::MenuState) {
    // Calculate position based on selected category
    // This is a bit hacky without exact text width calculation, but we can estimate.
    let mut x_offset = 0;
    for i in 0..menu_state.selected_category {
        x_offset += menu_state.categories[i].name.len() as u16 + 2; // +2 for padding
    }

    let category = &menu_state.categories[menu_state.selected_category];

    // Calculate dynamic width
    let mut max_name_len = 0;
    let mut max_shortcut_len = 0;
    for item in &category.items {
        max_name_len = max_name_len.max(item.name.len());
        max_shortcut_len =
            max_shortcut_len.max(item.shortcut.as_ref().map(|s| s.len()).unwrap_or(0));
    }

    // Width = name + spacing + shortcut + borders/padding
    let content_width = max_name_len + 2 + max_shortcut_len; // 2 spaces gap
    let width = (content_width as u16 + 2).max(20); // +2 for list item padding/borders, min 20

    let height = category.items.len() as u16 + 2;

    let area = Rect::new(top_area.x + x_offset, top_area.y + 1, width, height);

    use ratatui::widgets::Clear;
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = category
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if item.is_separator {
                let separator_len = (width as usize).saturating_sub(2);
                let separator = "─".repeat(separator_len);
                return ListItem::new(separator).style(Style::default().fg(Color::White));
            }

            let style = if Some(i) == menu_state.selected_item {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default()
            };

            let shortcut = item.shortcut.clone().unwrap_or_default();
            let name = &item.name;
            // Dynamic formatting
            let content = format!(
                "{:<name_w$}  {:>short_w$}",
                name,
                shortcut,
                name_w = max_name_len,
                short_w = max_shortcut_len
            );
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

fn render_main_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Disassembly ");
    let inner_area = block.inner(area);

    let visible_height = inner_area.height as usize;
    let total_items = app_state.disassembly.len();
    let context_lines = visible_height / 2;
    let offset = ui_state.cursor_index.saturating_sub(context_lines);

    let items: Vec<ListItem> = app_state
        .disassembly
        .iter()
        .skip(offset)
        .take(visible_height)
        .enumerate()
        .map(|(local_i, line)| {
            let i = offset + local_i;
            let is_selected = if let Some(selection_start) = ui_state.selection_start {
                let (start, end) = if selection_start < ui_state.cursor_index {
                    (selection_start, ui_state.cursor_index)
                } else {
                    (ui_state.cursor_index, selection_start)
                };
                i >= start && i <= end
            } else {
                false
            };

            let style = if i == ui_state.cursor_index {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let label_text = if let Some(label) = &line.label {
                format!("{}:", label)
            } else {
                String::new()
            };

            let content = Line::from(vec![
                Span::styled(
                    format!("{:04X}  ", line.address),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{: <12}", hex_bytes(&line.bytes)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{: <16}", label_text),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{: <4} ", line.mnemonic),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{: <15}", line.operand),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("; {}", line.comment),
                    Style::default().fg(Color::Gray),
                ),
            ]);

            ListItem::new(content).style(style)
        })
        .collect();

    // Calculate scroll based on cursor to keep it in view
    // A simple basic list widget:
    // Ideally we use a ListState, but here we just render items.
    // Ratatui's List widget handles scrolling if we pass the state, but we are managing state manually for now via `state.disassembly` slice maybe?
    // Or we just pass the full list and set the state.

    // For large lists, we should only render what's visible or use ListState.
    // Let's use ListState and passing the items.

    let list = List::new(items).block(block);

    let mut state = ListState::default();
    if total_items > 0 {
        let local_cursor = ui_state.cursor_index.saturating_sub(offset);
        if local_cursor < visible_height {
            state.select(Some(local_cursor));
        }
    }
    f.render_stateful_widget(list, area, &mut state);
    ui_state.scroll_index = offset;
}

fn render_status_bar(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &UIState) {
    let status = format!(
        " Cursor: {:04X} | Origin: {:04X} | File: {:?}{}",
        app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map(|l| l.address)
            .unwrap_or(0),
        app_state.origin,
        app_state
            .file_path
            .as_ref()
            .map(|p| p.file_name().unwrap_or_default())
            .unwrap_or_default(),
        if let Some(start) = ui_state.selection_start {
            let count = (ui_state.cursor_index as isize - start as isize).abs() + 1;
            format!(" | Selected: {} lines", count)
        } else {
            "".to_string()
        }
    );
    let bar = Paragraph::new(status).style(Style::default().bg(Color::Blue).fg(Color::White));
    f.render_widget(bar, area);
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}
