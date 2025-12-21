use crate::state::AppState;
use crate::ui_state::UIState;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

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
    let items: Vec<ListItem> = app_state
        .disassembly
        .iter()
        .enumerate()
        .map(|(i, line)| {
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

            let content = if line.bytes.is_empty() {
                // Label Line
                let mut spans = vec![
                    Span::styled(
                        format!("{:04X}  ", line.address),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("{:<32}", line.mnemonic),
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];

                if !line.comment.is_empty() {
                    spans.push(Span::styled(
                        format!("; {}", line.comment),
                        Style::default()
                            .fg(Color::Gray)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }

                Line::from(spans)
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("{:04X}  ", line.address),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!("{: <12}", hex_bytes(&line.bytes)),
                        Style::default().fg(Color::DarkGray),
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
                ])
            };

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

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Disassembly "),
        )
        .highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black)); // This is if we use state select

    // We need to manage the ListState in AppState or here.
    // If we use `cursor_index` as the selected item.
    ui_state
        .disassembly_state
        .select(Some(ui_state.cursor_index));

    f.render_stateful_widget(list, area, &mut ui_state.disassembly_state);
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
