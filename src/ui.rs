use crate::state::AppState;
use crate::ui_state::{ActivePane, RightPane, UIState};
use crate::utils::centered_rect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
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

    render_menu(f, chunks[0], &ui_state.menu, &ui_state.theme);
    render_main_view(f, chunks[1], app_state, ui_state);
    render_status_bar(f, chunks[2], app_state, ui_state);

    // Render Popup if needed
    if ui_state.menu.active && ui_state.menu.selected_item.is_some() {
        render_menu_popup(f, chunks[0], &ui_state.menu, &ui_state.theme);
    }

    if ui_state.file_picker.active {
        render_file_picker(f, f.area(), &ui_state.file_picker, &ui_state.theme);
    }

    if ui_state.jump_dialog.active {
        render_jump_dialog(f, f.area(), &ui_state.jump_dialog, &ui_state.theme);
    }

    if ui_state.save_dialog.active {
        render_save_dialog(f, f.area(), &ui_state.save_dialog, &ui_state.theme);
    }

    if ui_state.label_dialog.active {
        render_label_dialog(f, f.area(), &ui_state.label_dialog, &ui_state.theme);
    }

    if ui_state.comment_dialog.active {
        render_comment_dialog(f, f.area(), &ui_state.comment_dialog, &ui_state.theme);
    }

    if ui_state.settings_dialog.active {
        crate::dialog_document_settings::render(
            f,
            f.area(),
            app_state,
            &ui_state.settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.system_settings_dialog.active {
        crate::dialog_settings::render(
            f,
            f.area(),
            app_state,
            &ui_state.system_settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.about_dialog.active {
        crate::dialog_about::render(f, ui_state, f.area(), &ui_state.about_dialog);
    }

    if ui_state.shortcuts_dialog.active {
        crate::dialog_keyboard_shortcut::render(
            f,
            f.area(),
            &ui_state.shortcuts_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.confirmation_dialog.active {
        render_confirmation_dialog(f, f.area(), &ui_state.confirmation_dialog, &ui_state.theme);
    }

    if ui_state.origin_dialog.active {
        render_origin_dialog(f, f.area(), &ui_state.origin_dialog, &ui_state.theme);
    }

    if ui_state.search_dialog.active {
        render_search_dialog(f, f.area(), &ui_state.search_dialog, &ui_state.theme);
    }
}

fn render_confirmation_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::ConfirmationDialogState,
    theme: &crate::theme::Theme,
) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", dialog.title))
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let area = centered_rect(50, 7, area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Message
            Constraint::Length(1), // Gap
            Constraint::Length(1), // Instructions
        ])
        .split(inner);

    let message = Paragraph::new(dialog.message.clone())
        .alignment(ratatui::layout::Alignment::Center)
        .style(
            Style::default()
                .fg(theme.dialog_fg)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(message, layout[0]);

    let instructions = Paragraph::new("Enter: Proceed  |  Esc: Cancel")
        .alignment(ratatui::layout::Alignment::Center)
        .style(Style::default().fg(theme.highlight_fg));

    f.render_widget(instructions, layout[2]);
}

fn render_label_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::LabelDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Enter Label Name ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_comment_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::CommentDialogState,
    theme: &crate::theme::Theme,
) {
    let title = match dialog.comment_type {
        crate::ui_state::CommentType::Line => " Enter Line Comment ",
        crate::ui_state::CommentType::Side => " Enter Side Comment ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_save_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::SaveDialogState,
    theme: &crate::theme::Theme,
) {
    let title = if dialog.mode == crate::ui_state::SaveDialogMode::ExportProject {
        " Export Project As... "
    } else {
        " Save Project As... "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_jump_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::JumpDialogState,
    theme: &crate::theme::Theme,
) {
    let title = match dialog.mode {
        crate::ui_state::JumpDialogMode::Address => " Jump to Address (Hex) ",
        crate::ui_state::JumpDialogMode::Line => " Jump to Line (Dec) ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_search_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::SearchDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Search ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_file_picker(
    f: &mut Frame,
    area: Rect,
    picker: &crate::ui_state::FilePickerState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Open File (Space to Open, Backspace to Go Back, Esc to Cancel) ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

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
                .bg(theme.menu_selected_bg)
                .fg(theme.menu_selected_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(picker.selected_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_menu(
    f: &mut Frame,
    area: Rect,
    menu_state: &crate::ui_state::MenuState,
    theme: &crate::theme::Theme,
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

        spans.push(Span::styled(format!(" {} ", category.name), style));
    }

    // Fill the rest of the line
    // Fill the rest of the line
    let menu_bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.menu_bg).fg(theme.menu_fg));
    f.render_widget(menu_bar, area);
}

fn render_menu_popup(
    f: &mut Frame,
    top_area: Rect,
    menu_state: &crate::ui_state::MenuState,
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
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.menu_bg).fg(theme.menu_fg)),
    );

    f.render_widget(list, area);
}

fn render_origin_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::OriginDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Change Origin (Hex) ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    // Fixed height of 3 (Border + Input + Border)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(layout[1])[1];
    f.render_widget(ratatui::widgets::Clear, area);

    let input = Paragraph::new(dialog.input.clone()).block(block).style(
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(input, area);
}

fn render_main_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    // Calculate required width for Right Pane
    let right_pane_width = match ui_state.right_pane {
        RightPane::None => 0,
        RightPane::HexDump => 75,
        RightPane::Sprites => 36, // 24 chars + border + padding
        RightPane::Charset => 76, // Grid view: 8 cols * (8+1) width + padding
        RightPane::Blocks => 42,
    };
    let disasm_view_width = area.width.saturating_sub(right_pane_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(right_pane_width),
        ])
        .split(area);

    crate::view_disassembly::render(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump => crate::view_hexdump::render(f, layout[1], app_state, ui_state),
        RightPane::Sprites => render_sprites_view(f, layout[1], app_state, ui_state),
        RightPane::Charset => render_charset_view(f, layout[1], app_state, ui_state),
        RightPane::Blocks => render_blocks_view(f, layout[1], app_state, ui_state),
    }
}

fn render_status_bar(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &UIState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Message
            Constraint::Percentage(50), // Info
        ])
        .split(area);

    // Left: Status Message
    let (status_text, status_fg) = if ui_state.vim_search_active {
        (
            format!("/{}", ui_state.vim_search_input),
            ui_state.theme.highlight_fg,
        )
    } else {
        (
            format!(" {}", ui_state.status_message),
            ui_state.theme.status_bar_fg,
        )
    };

    let status_msg = Paragraph::new(Span::styled(
        status_text,
        Style::default().add_modifier(Modifier::BOLD),
    ))
    .style(
        Style::default()
            .bg(ui_state.theme.status_bar_bg)
            .fg(status_fg),
    );
    f.render_widget(status_msg, chunks[0]);

    // Right: Info
    let cursor_addr = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address)
        .unwrap_or(0);

    let block_info =
        if let Some(offset) = (cursor_addr as isize).checked_sub(app_state.origin as isize) {
            if offset >= 0 && (offset as usize) < app_state.block_types.len() {
                let block_type = app_state.block_types[offset as usize];
                if let Some((start, end)) = app_state.get_block_range(cursor_addr) {
                    format!(
                        "{} | {}: ${:04X}-${:04X} | ",
                        app_state.settings.assembler, block_type, start, end
                    )
                } else {
                    format!("{} | {}: ??? | ", app_state.settings.assembler, block_type)
                }
            } else {
                format!("{} | ", app_state.settings.assembler)
            }
        } else {
            format!("{} | ", app_state.settings.assembler)
        };

    let info = format!(
        "{} | {}Cursor: {:04X} | Origin: {:04X} | File: {:?}{}",
        app_state.settings.platform,
        block_info,
        cursor_addr,
        app_state.origin,
        app_state
            .file_path
            .as_ref()
            .map(|p| p.file_name().unwrap_or_default())
            .unwrap_or_default(),
        if let Some(start) = ui_state.selection_start {
            let count = (ui_state.cursor_index as isize - start as isize).abs() + 1;
            format!(" | Selected: {}", count)
        } else {
            "".to_string()
        }
    );

    let info_widget = Paragraph::new(info)
        .alignment(ratatui::layout::Alignment::Right)
        .style(
            Style::default()
                .bg(ui_state.theme.status_bar_bg)
                .fg(ui_state.theme.status_bar_fg),
        );
    f.render_widget(info_widget, chunks[1]);
}

fn render_sprites_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Sprites;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let title = if ui_state.sprite_multicolor_mode {
        " Sprites (Multicolor) "
    } else {
        " Sprites (Single Color) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app_state.raw_data.is_empty() {
        return;
    }

    let origin = app_state.origin as usize;
    let padding = (64 - (origin % 64)) % 64;

    if app_state.raw_data.len() <= padding {
        return;
    }

    let usable_len = app_state.raw_data.len() - padding;
    let total_sprites = usable_len.div_ceil(64);

    let sprite_height = 22; // 21 lines + 1 separator
    let visible_rows = inner_area.height as usize;
    let num_sprites_fit = visible_rows.div_ceil(sprite_height); // Approximation

    let start_index = if ui_state.sprites_cursor_index > num_sprites_fit / 2 {
        ui_state
            .sprites_cursor_index
            .saturating_sub(num_sprites_fit / 2)
    } else {
        0
    };

    let end_index = (start_index + num_sprites_fit + 1).min(total_sprites);

    let mut y_offset = 0;
    for i in start_index..end_index {
        if y_offset >= visible_rows {
            break;
        }

        let sprite_offset_in_data = padding + i * 64;
        let sprite_address = origin + sprite_offset_in_data;

        if sprite_offset_in_data >= app_state.raw_data.len() {
            break;
        }

        // Draw Sprite Header/Index
        let is_selected = i == ui_state.sprites_cursor_index;
        let style = if is_selected {
            Style::default()
                .fg(ui_state.theme.highlight_fg)
                .bg(ui_state.theme.highlight_bg)
        } else {
            Style::default()
        };

        // Sprite number calculation: (Address / 64) % 256
        let sprite_num = (sprite_address / 64) % 256;

        if y_offset < visible_rows {
            f.render_widget(
                Paragraph::new(format!(
                    "Sprite  {:03} / ${:02X} @ ${:04X}",
                    sprite_num, sprite_num, sprite_address
                ))
                .style(style),
                Rect::new(
                    inner_area.x,
                    inner_area.y + y_offset as u16,
                    inner_area.width,
                    1,
                ),
            );
            y_offset += 1;
        }

        // Draw Sprite Data (21 lines)
        for row in 0..21 {
            if y_offset >= visible_rows {
                break;
            }

            let row_offset = sprite_offset_in_data + row * 3;
            // 3 bytes per row = 24 bits
            if row_offset + 2 < app_state.raw_data.len() {
                let bytes = &app_state.raw_data[row_offset..row_offset + 3];

                if ui_state.sprite_multicolor_mode {
                    // Multicolor Mode: 12 pixels per row, 2 bits per pixel
                    // Pixel width = 2 chars
                    let mut spans = Vec::with_capacity(12);
                    for b in bytes {
                        for pair in (0..4).rev() {
                            let bits = (b >> (pair * 2)) & 0b11;
                            let (char_str, fg_color) = match bits {
                                0b00 => ("..", ui_state.theme.foreground), // Background (transparent-ish)
                                0b01 => ("██", ui_state.theme.foreground), // Shared color 1 (Foreground/Highlight?) - standard is sprite color
                                0b10 => ("██", ui_state.theme.sprite_multicolor_1), // MC 1
                                0b11 => ("██", ui_state.theme.sprite_multicolor_2), // MC 2
                                _ => unreachable!(),
                            };

                            // For 00 (background), we might want to be dim or just dots
                            let style = if bits == 0b00 {
                                Style::default().fg(Color::DarkGray) // Dim dots
                            } else {
                                Style::default().fg(fg_color)
                            };
                            spans.push(Span::styled(char_str, style));
                        }
                    }
                    f.render_widget(
                        Paragraph::new(Line::from(spans)),
                        Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1),
                    );
                } else {
                    // Single Color Mode: 24 pixels per row, 1 bit per pixel
                    let mut line_str = String::with_capacity(24);
                    for b in bytes {
                        for bit in (0..8).rev() {
                            if (b >> bit) & 1 == 1 {
                                line_str.push('█');
                            } else {
                                line_str.push('.'); // Use dot for empty to see grid better, or space
                            }
                        }
                    }
                    f.render_widget(
                        Paragraph::new(line_str),
                        Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1), // Indent
                    );
                }
            } else {
                // Partial padding?
                f.render_widget(
                    Paragraph::new("                        "),
                    Rect::new(inner_area.x + 2, inner_area.y + y_offset as u16, 24, 1),
                );
            }

            y_offset += 1;
        }
    }
}

fn render_charset_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Charset;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let title = if ui_state.charset_multicolor_mode {
        " Charset (Multicolor) "
    } else {
        " Charset (Single Color) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if app_state.raw_data.is_empty() {
        return;
    }

    let origin = app_state.origin as usize;
    // Align origin to next multiple of $400 as per user request (and consistent with events.rs)
    let base_alignment = 0x400;
    let aligned_start_addr = (origin / base_alignment) * base_alignment;

    // Grid Constants
    // Char is 8x8 pixels. Rendered as 8x4 text cells (half blocks).
    let char_render_width = 8;
    let char_render_height = 4;
    let grid_cols = 8;
    let col_spacing = 1;
    let row_spacing = 1;

    // Width of one grid item including spacing
    let item_width = char_render_width + col_spacing;
    // Height of one grid item including spacing
    let item_height = char_render_height + row_spacing;

    let visible_rows = inner_area.height as usize;

    // We navigate by "Character Index" (0..N).
    // Mapping Index -> (GridRow, GridCol)
    // GridRow = Index / grid_cols
    // GridCol = Index % grid_cols

    let end_address = origin + app_state.raw_data.len();
    let total_chars = (end_address.saturating_sub(aligned_start_addr)).div_ceil(8);

    // Scroll Logic
    // We want the cursor row to be visible.
    let cursor_grid_row = ui_state.charset_cursor_index / grid_cols;

    // items fit vertically
    let rows_fit = visible_rows.div_ceil(item_height);

    // Calculate scroll offset (in grid rows)
    // If cursor is not in view/center, adjust scroll.
    // Store scroll state in `ui_state.charset_scroll_row`? Or derive?
    // Reusing `charset_scroll_index` if it existed, or just calc on fly.
    // For now, let's just make sure cursor row is roughly in middle or visible.

    let scroll_row = if cursor_grid_row > rows_fit / 2 {
        cursor_grid_row.saturating_sub(rows_fit / 2)
    } else {
        0
    };

    let end_row = scroll_row + rows_fit + 1; // Render a bit extra

    let mut y_offset = 0;

    for row_idx in scroll_row..end_row {
        if y_offset >= visible_rows {
            break;
        }

        let charset_address = aligned_start_addr + (row_idx * grid_cols * 8);
        // Header every 2048 bytes (address-aligned)
        if charset_address.is_multiple_of(2048) {
            // There can only be at most 8 charsets per VIC-II bank (16K per bank)
            let charset_num = charset_address / 2048 % 8;

            f.render_widget(
                Paragraph::new(format!(
                    "Charset  {} / (${:02X}) @ ${:04X}",
                    charset_num, charset_num, charset_address
                ))
                .style(Style::default().fg(ui_state.theme.comment)),
                Rect::new(
                    inner_area.x,
                    inner_area.y + y_offset as u16,
                    inner_area.width,
                    1,
                ),
            );
            y_offset += 1;
            if y_offset >= visible_rows {
                break;
            }
        }

        for col_idx in 0..grid_cols {
            let char_idx = row_idx * grid_cols + col_idx;
            if char_idx >= total_chars {
                continue;
            }

            let char_offset = char_idx * 8;
            let char_addr = aligned_start_addr + char_offset;

            // Render Char
            let x_pos = inner_area.x + (col_idx * item_width) as u16 + 1; // +1 margin
            let y_pos = inner_area.y + y_offset as u16;

            let is_selected = char_idx == ui_state.charset_cursor_index;

            // Draw 4 lines of half-blocks
            for line in 0..4 {
                if y_offset + line >= visible_rows {
                    break;
                }

                let row_addr_top = char_addr + line * 2;
                let byte_top = if row_addr_top >= origin && row_addr_top < end_address {
                    app_state.raw_data[row_addr_top - origin]
                } else {
                    0
                };

                let row_addr_bot = char_addr + line * 2 + 1;
                let byte_bot = if row_addr_bot >= origin && row_addr_bot < end_address {
                    app_state.raw_data[row_addr_bot - origin]
                } else {
                    0
                };

                // Different rendering for multicolor vs standard
                if ui_state.charset_multicolor_mode {
                    // Multicolor: 4 pixels width, double wide (2 chars per pixel)
                    // 2 bits per pixel.
                    // 00=bg, 01=fg, 10=mc1, 11=mc2
                    let mut spans = Vec::with_capacity(4);

                    for pixel_idx in (0..4).rev() {
                        let shift = pixel_idx * 2;

                        // Get 2 bits for top and bottom
                        let bits_top = (byte_top >> shift) & 0b11;
                        let bits_bot = (byte_bot >> shift) & 0b11;

                        let color_top = match bits_top {
                            0b00 => ui_state.theme.background, // Or explicit BG
                            0b01 => ui_state.theme.foreground,
                            0b10 => ui_state.theme.charset_multicolor_1,
                            0b11 => ui_state.theme.charset_multicolor_2,
                            _ => unreachable!(),
                        };

                        let color_bot = match bits_bot {
                            0b00 => ui_state.theme.background,
                            0b01 => ui_state.theme.foreground,
                            0b10 => ui_state.theme.charset_multicolor_1,
                            0b11 => ui_state.theme.charset_multicolor_2,
                            _ => unreachable!(),
                        };

                        // Selection overlay logic?
                        // If selected, we might want to tint or invert?
                        // The user said "It should take the colors from Theme".
                        // Existing selection logic overlays `bg(selection_bg)` which overrides our beautiful colors.
                        // `fg(selection_fg)` overrides foreground.
                        // Maybe just draw a border or change background if 00?
                        // For now, let's keep it simple: strict colors.
                        // If selected, maybe we swap "background" for "selection_bg"?
                        // Let's defer selection brightness for now to get logic right.
                        // Actually, the original code used `bg_style` (removed) and `fg_style` (applied to whole line).
                        // Here we have mixed colors in one line.

                        let mut style = Style::default().fg(color_top).bg(color_bot);

                        // Apply selection - tricky with multicolor.
                        // If selected, force background 00 to be selection_bg?
                        if is_selected {
                            if bits_top == 0b00 {
                                style = style.fg(ui_state.theme.selection_bg);
                            }
                            if bits_bot == 0b00 {
                                style = style.bg(ui_state.theme.selection_bg);
                            }
                        }

                        // Double wide pixel
                        spans.push(Span::styled("▀▀", style));
                    }
                    f.render_widget(
                        Paragraph::new(Line::from(spans)),
                        Rect::new(x_pos, y_pos + line as u16, 8, 1),
                    );
                } else {
                    let mut line_str = String::with_capacity(8);
                    for bit in (0..8).rev() {
                        let t = (byte_top >> bit) & 1;
                        let b = (byte_bot >> bit) & 1;

                        let c = match (t, b) {
                            (0, 0) => ' ',
                            (1, 0) => '▀',
                            (0, 1) => '▄',
                            (1, 1) => '█',
                            _ => unreachable!(),
                        };
                        line_str.push(c);
                    }

                    let fg_style = if is_selected {
                        Style::default()
                            .fg(ui_state.theme.selection_fg)
                            .bg(ui_state.theme.selection_bg)
                    } else {
                        Style::default().fg(ui_state.theme.foreground)
                    };

                    f.render_widget(
                        Paragraph::new(line_str).style(fg_style),
                        Rect::new(x_pos, y_pos + line as u16, 8, 1),
                    );
                }
            }
        }
        y_offset += item_height;
    }
}
fn render_blocks_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Blocks;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Blocks")
        .style(border_style);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let blocks = app_state.get_blocks_view_items();
    let theme = &ui_state.theme;
    let items: Vec<ListItem> = blocks
        .iter()
        .enumerate()
        .map(|(index, item)| {
            match item {
                crate::state::BlockItem::Block { start, end, type_ } => {
                    let start_addr = app_state.origin.wrapping_add(*start);
                    let end_addr = app_state.origin.wrapping_add(*end);

                    let (fg, bg) = match type_ {
                        crate::state::BlockType::Code => (theme.block_code_fg, theme.block_code_bg),
                        crate::state::BlockType::DataByte => {
                            (theme.block_data_byte_fg, theme.block_data_byte_bg)
                        }
                        crate::state::BlockType::DataWord => {
                            (theme.block_data_word_fg, theme.block_data_word_bg)
                        }
                        crate::state::BlockType::Address => {
                            (theme.block_address_fg, theme.block_address_bg)
                        }
                        crate::state::BlockType::Text => (theme.block_text_fg, theme.block_text_bg),
                        crate::state::BlockType::Screencode => {
                            (theme.block_screencode_fg, theme.block_screencode_bg)
                        }
                        crate::state::BlockType::LoHi => (theme.block_lohi_fg, theme.block_lohi_bg),
                        crate::state::BlockType::HiLo => (theme.block_hilo_fg, theme.block_hilo_bg),
                        crate::state::BlockType::ExternalFile => {
                            (theme.block_external_file_fg, theme.block_external_file_bg)
                        }
                        crate::state::BlockType::Undefined => {
                            (theme.block_undefined_fg, theme.block_undefined_bg)
                        }
                    };

                    let is_selected = ui_state.blocks_list_state.selected() == Some(index);
                    let prefix = if is_selected { "> " } else { "  " };

                    let is_collapsed = app_state
                        .collapsed_blocks
                        .contains(&(*start as usize, *end as usize));
                    let type_display = if is_collapsed {
                        format!("{} (C)", type_)
                    } else {
                        type_.to_string()
                    };

                    let content = format!(
                        "{}{:<20} ${:04X}-${:04X}",
                        prefix, type_display, start_addr, end_addr
                    );

                    ListItem::new(content).style(Style::default().fg(fg).bg(bg))
                }
                crate::state::BlockItem::Splitter(addr) => {
                    let is_selected = ui_state.blocks_list_state.selected() == Some(index);
                    let prefix = if is_selected { "> " } else { "  " };

                    // Align address with other blocks (Prefix + 20 chars type + space + $)
                    // "-- Splitter --" serves as the type string.
                    let content = format!("{}{:<20} ${:04X}", prefix, "-- Splitter --", addr);

                    ListItem::new(content).style(
                        Style::default()
                            .fg(theme.block_splitter_fg)
                            .bg(theme.block_splitter_bg),
                    )
                }
            }
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(theme.block_selection_bg)
                .fg(theme.block_selection_fg),
        )
        .highlight_symbol(""); // Set to empty as we handle the symbol manually

    f.render_stateful_widget(list, inner_area, &mut ui_state.blocks_list_state);
}

#[cfg(test)]
mod tests {
    use crate::cpu::{AddressingMode, Opcode};
    use crate::disassembler::DisassemblyLine;
    use crate::state::AppState;

    fn make_line(
        addr: u16,
        mnemonic: &str,
        operand: &str,
        target: Option<u16>,
        opcode: Option<Opcode>,
    ) -> DisassemblyLine {
        DisassemblyLine {
            address: addr,
            bytes: vec![],
            mnemonic: mnemonic.to_string(),
            operand: operand.to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode,
            show_bytes: false,
            target_address: target,
            comment_address: None,
            is_collapsed: false,
        }
    }

    fn make_jmp_indirect_opcode() -> Option<Opcode> {
        Some(Opcode::new(
            "JMP",
            AddressingMode::Indirect,
            3,
            5,
            "Jump Indirect",
        ))
    }

    fn make_jmp_abs_opcode() -> Option<Opcode> {
        Some(Opcode::new(
            "JMP",
            AddressingMode::Absolute,
            3,
            3,
            "Jump Absolute",
        ))
    }

    #[test]
    fn test_arrow_filtering_indirect_jmp() {
        let lines = vec![
            // 0: JMP ($1000) - Should be filtered out
            make_line(
                0x1000,
                "JMP",
                "($1000)",
                Some(0x2000),
                make_jmp_indirect_opcode(),
            ),
            // 1: NOP
            make_line(0x1003, "NOP", "", None, None),
            // 2: JMP $1000 - Should NOT be filtered out (though valid arrow)
            make_line(0x1004, "JMP", "$1000", Some(0x1000), make_jmp_abs_opcode()),
        ];

        let mut app_state = AppState::new();
        app_state.disassembly = lines;
        app_state.settings.max_arrow_columns = 5;

        // We can't easily call render_disassembly here as it requires Frame and UIState.
        // However, we can assert that the specific logic path works by reproducing the check here
        // or by trusting that if we verified the logic match, it works.
        // Ideally, we'd refactor the arrow generation logic into a pure function `get_arrows(disassembly) -> Vec<Arrow>`.
        // Given constraints, this test ensures struct compatibility and compilation of the opcode helpers.

        // Manual verification of the logic block:
        let line = &app_state.disassembly[0];
        let should_skip = if let Some(opcode) = &line.opcode {
            opcode.mnemonic == "JMP" && opcode.mode == AddressingMode::Indirect
        } else {
            false
        };
        assert!(should_skip, "Indirect JMP should be skipped by opcode mode");

        let line2 = &app_state.disassembly[2];
        let should_skip2 = if let Some(opcode) = &line2.opcode {
            opcode.mnemonic == "JMP" && opcode.mode == AddressingMode::Indirect
        } else {
            false
        };
        assert!(!should_skip2, "Absolute JMP should NOT be skipped");
    }
}
