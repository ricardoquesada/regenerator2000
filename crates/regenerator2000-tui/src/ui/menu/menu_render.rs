use super::menu_model::MenuState;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
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

        if let Some(byte_idx) = category.name.char_indices().find_map(|(idx, ch)| {
            ch.eq_ignore_ascii_case(&category.accelerator)
                .then_some(idx)
        }) {
            let accel_len = category.name[byte_idx..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            let before = &category.name[..byte_idx];
            let accel = &category.name[byte_idx..byte_idx + accel_len];
            let after = &category.name[byte_idx + accel_len..];

            spans.push(Span::styled(" ", style));
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), style));
            }
            spans.push(Span::styled(
                accel.to_string(),
                style.add_modifier(Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(format!("{after} "), style));
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
    let category = &menu_state.categories[menu_state.selected_category];
    let (max_name_len, max_shortcut_len) = category.column_widths();
    let bar_area = Rect::new(top_area.x, top_area.y, top_area.width, 1);
    let area = menu_state.popup_rect(bar_area, f.area());

    f.render_widget(Clear, area);

    let items: Vec<ListItem> = category
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if item.is_separator {
                let separator_len = (area.width as usize).saturating_sub(2);
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
                if Some(i) == menu_state.selected_item {
                    style = Style::default()
                        .bg(theme.menu_selected_bg)
                        .fg(theme.menu_disabled_fg)
                        .add_modifier(Modifier::DIM);
                }
            }

            let shortcut = item.shortcut.clone().unwrap_or_default();
            let name = &item.name;
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

    let inner_height = (area.height as usize).saturating_sub(2);
    let offset = menu_state.popup_scroll_offset(inner_height);
    let mut state = ListState::default()
        .with_offset(offset)
        .with_selected(menu_state.selected_item);
    f.render_stateful_widget(list, area, &mut state);
}
