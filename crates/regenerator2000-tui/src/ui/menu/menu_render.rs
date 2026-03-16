use super::menu_model::MenuState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

pub fn render_menu(
    f: &mut Frame,
    area: Rect,
    menu_state: &MenuState,
    theme: &crate::theme::Theme,
    new_version: Option<&str>,
) {
    let mut spans = Vec::new();

    for (i, category) in menu_state.categories.iter().enumerate() {
        let style = if menu_state.active && i == menu_state.selected_category {
            Style::default()
                .bg(theme.menu_selected_bg)
                .fg(theme.menu_selected_fg)
        } else {
            Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
        };

        if category.name == "File" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("F", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("ile ", style));
        } else if category.name == "Edit" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("Edi", style));
            spans.push(Span::styled("t", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled(" ", style));
        } else if category.name == "Help" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("H", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("elp ", style));
        } else if category.name == "Jump" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("J", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("ump ", style));
        } else if category.name == "View" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("V", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("iew ", style));
        } else if category.name == "Search" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("Sea", style));
            spans.push(Span::styled("r", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("ch ", style));
        } else if category.name == "Debugger" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("Deb", style));
            spans.push(Span::styled("u", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("gger ", style));
        } else {
            spans.push(Span::styled(format!(" {} ", category.name), style));
        }
    }

    // Fill the rest of the line
    let menu_bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.menu_bg).fg(theme.menu_fg));
    f.render_widget(menu_bar, area);

    // Render version update badge on the right side of the menu bar
    if let Some(version) = new_version {
        let badge_text = format!(" New version {version} available ");
        let badge_width = badge_text.len() as u16;
        if area.width > badge_width {
            let badge_area = Rect::new(area.x + area.width - badge_width, area.y, badge_width, 1);
            f.render_widget(
                Paragraph::new(badge_text).style(
                    Style::default()
                        .bg(theme.highlight_fg)
                        .fg(theme.menu_bg)
                        .add_modifier(Modifier::BOLD),
                ),
                badge_area,
            );
        }
    }
}

pub fn render_menu_popup(
    f: &mut Frame,
    top_area: Rect,
    menu_state: &MenuState,
    theme: &crate::theme::Theme,
) {
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
            max_shortcut_len.max(item.shortcut.as_ref().map_or(0, std::string::String::len));
    }

    // Width = name + spacing + shortcut + borders/padding
    let content_width = max_name_len + 2 + max_shortcut_len; // 2 spaces gap
    let width = (content_width as u16 + 2).max(20); // +2 for list item padding/borders, min 20

    let height = category.items.len() as u16 + 2;

    let area = Rect::new(top_area.x + x_offset, top_area.y + 1, width, height);
    let area = area.intersection(f.area());

    f.render_widget(Clear, area);

    let items: Vec<ListItem> = category
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if item.is_separator {
                let separator_len = (width as usize).saturating_sub(2);
                let separator = "─".repeat(separator_len);
                return ListItem::new(separator).style(Style::default().fg(theme.menu_fg));
            }

            let mut style = if Some(i) == menu_state.selected_item {
                Style::default()
                    .bg(theme.menu_selected_bg)
                    .fg(theme.menu_selected_fg)
            } else {
                Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
            };

            if item.disabled {
                style = style.fg(theme.menu_disabled_fg).add_modifier(Modifier::DIM);
                // If disabled but selected, maybe keep cyan bg but dim text?
                if Some(i) == menu_state.selected_item {
                    style = Style::default()
                        .bg(theme.menu_selected_bg)
                        .fg(theme.menu_disabled_fg)
                        .add_modifier(Modifier::DIM);
                }
            }

            let shortcut = item.shortcut.clone().unwrap_or_default();
            let name = &item.name;
            // Dynamic formatting
            let content = format!("{name:<max_name_len$}  {shortcut:>max_shortcut_len$}");
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.menu_bg).fg(theme.menu_fg)),
    );

    f.render_widget(list, area);
}
