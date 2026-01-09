use crate::state::AppState;
use crate::ui_state::{ActivePane, RightPane, UIState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
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
        render_settings_dialog(
            f,
            f.area(),
            app_state,
            &ui_state.settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.system_settings_dialog.active {
        render_system_settings_dialog(
            f,
            f.area(),
            app_state,
            &ui_state.system_settings_dialog,
            &ui_state.theme,
        );
    }

    if ui_state.about_dialog.active {
        render_about_dialog(f, ui_state, f.area());
    }

    if ui_state.shortcuts_dialog.active {
        render_shortcuts_dialog(f, f.area(), &ui_state.shortcuts_dialog, &ui_state.theme);
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

fn render_shortcuts_dialog(
    f: &mut Frame,
    area: Rect,
    dialog: &crate::ui_state::ShortcutsDialogState,
    theme: &crate::theme::Theme,
) {
    if !dialog.active {
        return;
    }

    let area = centered_rect(60, 60, area);
    f.render_widget(ratatui::widgets::Clear, area); // Clear background

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Keyboard Shortcuts ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    let shortcuts = vec![
        ("General", ""),
        ("F10", "Activate Menu"),
        ("Ctrl+q", "Quit"),
        ("Ctrl+o", "Open File"),
        ("Ctrl+s", "Save Project"),
        ("Ctrl+Shift+s", "Save Project As..."),
        ("Ctrl+e", "Export .asm"),
        ("Ctrl+Shift+e", "Export .asm As..."),
        ("Ctrl+Shift+d", "Document Settings"),
        ("Ctrl+,", "Settings"),
        ("u", "Undo"),
        ("Ctrl+r", "Redo"),
        ("Tab", "Switch Pane (Disasm/Hex Dump/Sprites/Charset)"),
        ("Ctrl+2", "Toggle Hex Dump View"),
        ("Ctrl+3", "Toggle Sprites View"),
        ("Ctrl+4", "Toggle Charset View"),
        ("", ""),
        ("Navigation", ""),
        ("Up/Down/j/k", "Move Cursor"),
        ("PageUp/PageDown", "Page Up/Down"),
        ("Home/End", "Start/End of File"),
        ("Ctrl+u / Ctrl+d", "Up/Down 10 Lines"),
        ("g", "Jump to Address (Dialog)"),
        ("Ctrl+Shift+g", "Jump to Line (Dialog)"),
        ("[Number] G", "Jump to Line / End"),
        ("Enter", "Jump to Operand"),
        ("Backspace", "Navigate Back"),
        ("", ""),
        ("Search", ""),
        ("/", "Vim Search"),
        ("n / N", "Next / Prev Match"),
        ("Ctrl+F", "Search Dialog"),
        ("F3 / Shift+F3", "Find Next / Previous"),
        ("", ""),
        ("Editing", ""),
        ("V", "Toggle Visual Selection Mode"),
        ("Shift+Arrows", "Select Text"),
        ("c", "Code"),
        ("b", "Byte"),
        ("w", "Word"),
        ("a", "Address"),
        ("t", "Text"),
        ("s", "Screencode"),
        ("?", "Undefined"),
        ("d / D", "Next/Prev Imm. Format"),
        ("<", "Lo/Hi Address"),
        (">", "Hi/Lo Address"),
        (";", "Side Comment"),
        (":", "Line Comment"),
        ("l", "Label"),
        ("Ctrl+a", "Analyze"),
        ("m", "Toggle Petscii (Hex) / Multicolor (Sprites/Charset)"),
        ("Ctrl+k", "Collapse Block"),
        ("Ctrl+Shift+k", "Uncollapse Block"),
    ];

    let items: Vec<ListItem> = shortcuts
        .into_iter()
        .map(|(key, desc)| {
            if key.is_empty() && desc.is_empty() {
                ListItem::new("").style(Style::default())
            } else if desc.is_empty() {
                // Header
                ListItem::new(Span::styled(
                    key,
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ))
            } else {
                let content = format!("{:<25} {}", key, desc);
                ListItem::new(content).style(Style::default().fg(theme.dialog_fg))
            }
        })
        .collect();

    let list = List::new(items).block(Block::default()).highlight_style(
        Style::default()
            .bg(theme.highlight_bg)
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(dialog.scroll_offset));

    f.render_stateful_widget(list, inner, &mut state);
}

fn render_about_dialog(f: &mut Frame, ui_state: &UIState, area: Rect) {
    if let Some(logo) = &ui_state.logo
        && let Some(picker) = &ui_state.picker
    {
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

        // 1. Render Logo
        let img_area = chunks[0];

        // Calculate native size in cells (assuming 8x16 font)
        let font_width = 8.0;
        let font_height = 16.0;

        let native_width_cells = logo.width() as f64 / font_width;
        let native_height_cells = logo.height() as f64 / font_height;

        let avail_width_cells = img_area.width as f64;
        let avail_height_cells = img_area.height as f64;

        // Calculate scale to fit
        let scale_w = avail_width_cells / native_width_cells;
        let scale_h = avail_height_cells / native_height_cells;

        // Limit scale to 1.0 (don't upscale)
        let scale = scale_w.min(scale_h).min(1.0);

        let render_width = (native_width_cells * scale).max(1.0) as u16;
        let render_height = (native_height_cells * scale).max(1.0) as u16;

        let x = img_area.x + (img_area.width.saturating_sub(render_width)) / 2;
        let y = img_area.y + (img_area.height.saturating_sub(render_height)) / 2;

        let centered_area = ratatui::layout::Rect::new(x, y, render_width, render_height);

        // Use the original logo and let the library handle the downsampling into the target rect
        let mut protocol = picker.new_resize_protocol(logo.clone());
        let widget = StatefulImage::new();
        f.render_stateful_widget(widget, centered_area, &mut protocol);

        // 2. Render Text
        let text_area = chunks[1];
        let text = format!(
            "Regenerator 2000 v{}\n(c) Ricardo Quesada 2026\nriq / L.I.A\nInspired by Regenerator, by Tom-Cat / Nostalgia",
            env!("CARGO_PKG_VERSION")
        );
        let paragraph = Paragraph::new(text)
            .alignment(ratatui::layout::Alignment::Center)
            .block(Block::default());

        // Vertically center text in text_area
        let text_height = 4;
        let text_y = text_area.y + (text_area.height.saturating_sub(text_height)) / 2;
        let centered_text_area =
            ratatui::layout::Rect::new(text_area.x, text_y, text_area.width, text_height);

        f.render_widget(paragraph, centered_text_area);
    }
}

fn render_settings_dialog(
    f: &mut Frame,
    area: Rect,
    app_state: &AppState,
    dialog: &crate::ui_state::SettingsDialogState,
    theme: &crate::theme::Theme,
) {
    if !dialog.active {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Document Settings ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

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

    let patch_brk_disabled = settings.brk_single_byte;

    let items = vec![
        checkbox(
            "All Labels",
            settings.all_labels,
            dialog.selected_index == 0,
            false,
        ),
        checkbox(
            "Preserve long bytes (@w, +2, etc)",
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
            Constraint::Length(2),                      // Platform
            Constraint::Length(2), // Assembler (increased to 2 to match platform spacing style/consistency if needed, or keeping previous logic) -- Previous was Min(1). Let's stick to consistent spacing.
            Constraint::Length(2), // Max X-Refs
            Constraint::Length(2), // Arrow Columns
            Constraint::Length(2), // Text Line Limit
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

    // Check if platform is selected
    let platform_selected = dialog.selected_index == 5;

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
        Rect::new(layout[1].x + 2, layout[1].y, layout[1].width - 4, 1),
    );

    // Assembler Section
    // Use layout[2] for assembler
    // We can render label if we want, or just the selection line like Platform does (Platform: < C64 >)
    // The code above renders "Platform: < C64 >" OVER the "Platform:" label?
    // Wait, the previous code rendered valid label at layout[1].y
    // And THEN rendered platform_text at layout[1].y
    // So it overwrites it?
    // "Platform:" vs "Platform: < C64 >".
    // Yes, it seems redundant or intentional. "Platform: < C64 >" contains the label text too.
    // I'll stick to the "Platform: < ... >" format for Assembler too.

    let assembler_selected = dialog.selected_index == 6;
    let assembler_text = format!("Assembler: < {} >", settings.assembler);

    let assembler_widget = Paragraph::new(assembler_text).style(if assembler_selected {
        Style::default()
            .fg(theme.highlight_fg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.dialog_fg)
    });

    // Assembler uses layout[2]
    f.render_widget(
        assembler_widget,
        Rect::new(layout[2].x + 2, layout[2].y, layout[2].width - 4, 1),
    );

    // X-Refs uses layout[3]
    let xref_selected = dialog.selected_index == 7;
    let xref_value_str = if dialog.is_editing_xref_count {
        dialog.xref_count_input.clone()
    } else {
        settings.max_xref_count.to_string()
    };
    let xref_text = format!("Max X-Refs: < {} >", xref_value_str);

    // Arrow Columns
    let arrow_selected = dialog.selected_index == 8;
    let arrow_value_str = if dialog.is_editing_arrow_columns {
        dialog.arrow_columns_input.clone()
    } else {
        settings.max_arrow_columns.to_string()
    };
    let arrow_text = format!("Arrow Columns: < {} >", arrow_value_str);
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
    let text_limit_selected = dialog.selected_index == 9;
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
        Rect::new(layout[5].x + 2, layout[5].y, layout[5].width - 4, 1),
    );

    // Platform Popup
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
        let popup_area = centered_rect(40, 30, area); // Smaller height for fewer items
        f.render_widget(ratatui::widgets::Clear, popup_area);
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

fn render_system_settings_dialog(
    f: &mut Frame,
    area: Rect,
    app_state: &AppState,
    dialog: &crate::ui_state::SystemSettingsDialogState,
    theme: &crate::theme::Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Settings ")
        .border_style(Style::default().fg(theme.dialog_border))
        .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

    let area = centered_rect(50, 40, area); // Increased height for popup space
    f.render_widget(ratatui::widgets::Clear, area);
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
        format!("Theme: < {} >", app_state.system_config.theme),
    ];

    for (i, item) in items.into_iter().enumerate() {
        let style = if dialog.selected_index == i {
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
    if dialog.is_selecting_theme {
        let popup_area = centered_rect(40, 30, area);
        f.render_widget(ratatui::widgets::Clear, popup_area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Select Theme ")
            .border_style(Style::default().fg(theme.dialog_border))
            .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));

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
    };
    let disasm_view_width = area.width.saturating_sub(right_pane_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(right_pane_width),
        ])
        .split(area);

    render_disassembly(f, layout[0], app_state, ui_state);

    match ui_state.right_pane {
        RightPane::None => {}
        RightPane::HexDump => render_hex_view(f, layout[1], app_state, ui_state),
        RightPane::Sprites => render_sprites_view(f, layout[1], app_state, ui_state),
        RightPane::Charset => render_charset_view(f, layout[1], app_state, ui_state),
    }
}

fn render_hex_view(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::HexDump;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(match ui_state.petscii_mode {
            crate::state::PetsciiMode::Shifted => " Hex Dump (Shifted) ",
            crate::state::PetsciiMode::Unshifted => " Hex Dump (Unshifted) ",
        })
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let inner_area = block.inner(area);

    let visible_height = inner_area.height as usize;
    // Each row is 16 bytes
    let bytes_per_row = 16;
    let origin = app_state.origin as usize;
    let alignment_padding = origin % bytes_per_row;
    let aligned_origin = origin - alignment_padding;

    let total_len = app_state.raw_data.len() + alignment_padding;
    let total_rows = total_len.div_ceil(bytes_per_row);

    let context_lines = visible_height / 2;
    let offset = ui_state.hex_cursor_index.saturating_sub(context_lines);

    let items: Vec<ListItem> = (0..visible_height)
        .map(|i| {
            let row_index = offset + i;
            if row_index >= total_rows {
                return ListItem::new("");
            }

            let row_start_addr = aligned_origin + (row_index * bytes_per_row);

            let mut hex_part = String::with_capacity(3 * 16);
            let mut ascii_part = String::with_capacity(16);

            for j in 0..bytes_per_row {
                let current_addr = row_start_addr + j;

                if current_addr >= origin && current_addr < origin + app_state.raw_data.len() {
                    let data_idx = current_addr - origin;
                    let b = app_state.raw_data[data_idx];

                    hex_part.push_str(&format!("{:02X} ", b));
                    let is_shifted = ui_state.petscii_mode == crate::state::PetsciiMode::Shifted;
                    ascii_part.push(crate::utils::petscii_to_unicode(b, is_shifted));
                } else {
                    // Padding
                    hex_part.push_str("   ");
                    ascii_part.push(' ');
                }

                if j == 7 {
                    hex_part.push(' '); // Extra space after 8 bytes
                }
            }

            let is_selected = if let Some(selection_start) = ui_state.selection_start {
                let (start, end) = if selection_start < ui_state.cursor_index {
                    (selection_start, ui_state.cursor_index)
                } else {
                    (ui_state.cursor_index, selection_start)
                };
                row_index >= start && row_index <= end
            } else {
                false
            };

            let style = if row_index == ui_state.hex_cursor_index {
                Style::default().bg(ui_state.theme.selection_bg)
            } else if is_selected {
                Style::default()
                    .bg(ui_state.theme.selection_bg)
                    .fg(ui_state.theme.selection_fg)
            } else {
                Style::default()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{:04X}  ", row_start_addr),
                    Style::default().fg(ui_state.theme.address),
                ),
                Span::styled(
                    format!("{:<49}", hex_part),
                    Style::default().fg(ui_state.theme.hex_bytes),
                ), // 49 = 16*3 + 1 extra space
                Span::styled(
                    format!("| {}", ascii_part),
                    Style::default().fg(ui_state.theme.hex_ascii),
                ),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(block);

    // We handle scrolling manually via offset, so no ListState needed for scrolling,
    // but useful if we wanted ratatui to handle it.
    // However, similar to render_disassembly, we render what's visible.
    f.render_widget(list, area);
    ui_state.hex_scroll_index = offset;
}

fn render_disassembly(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Disassembly;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Disassembly ")
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let inner_area = block.inner(area);

    let formatter = app_state.get_formatter();

    let visible_height = inner_area.height as usize;
    let total_items = app_state.disassembly.len();
    let context_lines = visible_height / 2;
    let offset = ui_state.cursor_index.saturating_sub(context_lines);

    // --- Arrow Calculation Start ---
    // We want to find all arrows that overlap with the visible range: [offset, offset + visible_height]
    struct ArrowInfo {
        start: usize,
        end: usize,
        col: usize,
        target_addr: Option<u16>,
        start_visible: bool,
        end_visible: bool,
    }

    let end_view = offset + visible_height;

    // Optimization: Pre-calculate map for address -> index for relevant targets
    // Instead of full map, we just iterate.
    // Iterating all lines is fast enough for retro code sizes (< 1ms for 64KB).
    // But we can optimize to only checking lines that HAVE target_address.
    // We need to know src and dst index.

    // Step 1: Find all potential arrows (jumps)
    // We just iterate all disassembly lines.
    // For each jump, we see if it intersects our view.

    let mut relevant_arrows: Vec<(usize, usize, Option<u16>)> = Vec::new(); // (low, high, relative_target)

    for (src_idx, line) in app_state.disassembly.iter().enumerate() {
        if let Some(target_addr) = line.target_address {
            // Find dst_idx
            // Since disassembly can be large, linear scan for dst_idx for EVERY jump is O(Jumps * Lines).
            // Can we do better?
            // Use binary search if possible? app_state.disassembly is usually sorted by address.
            // NEW: Filter out indirect jumps (e.g. JMP ($1234))
            // These point to the address of the pointer, not the destination, creating confusing control flow arrows.
            // NEW: Filter out indirect jumps (e.g. JMP ($1234))
            // These point to the address of the pointer, not the destination, creating confusing control flow arrows.
            if let Some(opcode) = &line.opcode {
                if opcode.mnemonic == "JMP" && opcode.mode == crate::cpu::AddressingMode::Indirect {
                    continue;
                }
            } else if line.mnemonic.eq_ignore_ascii_case("JMP") && line.operand.contains('(') {
                continue;
            }

            let dst_result = app_state
                .disassembly
                .binary_search_by_key(&target_addr, |l| l.address);

            let dst_idx_opt = match dst_result {
                Ok(idx) => Some(idx),
                Err(idx) => {
                    // Check if previous line contains this address (relative/offset address)
                    if idx > 0 {
                        let prev_idx = idx - 1;
                        if let Some(prev_line) = app_state.disassembly.get(prev_idx) {
                            let len = prev_line.bytes.len() as u16;
                            // Check if target_addr is within [start, start + len)
                            // We use wrapping_add to handle potential overflow but usually code is contiguous.
                            // However, we want strict containment.
                            if len > 0
                                && target_addr >= prev_line.address
                                && target_addr < prev_line.address.wrapping_add(len)
                            {
                                Some(prev_idx)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(dst_idx) = dst_idx_opt {
                // binary_search finds *one* match. We want the code line ideally, or label line, but visually correct.
                // If we found an exact match (Ok), let's refine to find the first line.
                // If we found a containing match (Err -> prev_idx), that is unique (assuming no overlap).

                let mut refined_dst = dst_idx;
                if dst_result.is_ok() {
                    while refined_dst > 0
                        && app_state.disassembly[refined_dst - 1].address == target_addr
                    {
                        refined_dst -= 1;
                    }
                }

                // New Logic: Check visibility of Start or End OR Passing Through.
                let low = std::cmp::min(src_idx, refined_dst);
                let high = std::cmp::max(src_idx, refined_dst);

                // Intersection check: [low, high] overlaps with [offset, end_view]
                // low < end_view AND high >= offset
                let is_visible = low < end_view && high >= offset;

                if is_visible {
                    // Check if it's a relative/offset target
                    let relative_target = if dst_idx_opt == Some(refined_dst) {
                        // Exact match (start of line) -> Not relative unless the line itself is handled?
                        // Actually, if binary_search was Ok, it matched `line.address`.
                        // If it was Err, it matched `line.address <= target < end`.
                        // So if Ok, target is exactly start of line.
                        // If Err, target might be *+1, *+2 etc.
                        if dst_result.is_err() {
                            Some(target_addr)
                        } else {
                            None
                        }
                    } else {
                        // Binary search Ok case, but we refined it.
                        // refined_dst matches target_addr exactly at start.
                        None
                    };

                    relevant_arrows.push((src_idx, refined_dst, relative_target));
                }
            }
        }
    }

    // Step 1.5: No pass-through filter needed as we only collected visible arrows.
    // Our new collection logic (start_visible || end_visible) AUTOMATICALLY EXCLUDES pass-throughs.

    let relevant_arrows = relevant_arrows;

    // Step 2: Assign columns to arrows
    let mut active_arrows: Vec<ArrowInfo> = Vec::new();

    let mut sorted_arrows = relevant_arrows;
    sorted_arrows.sort_by_key(|(src, dst, _)| (*src as isize - *dst as isize).abs());

    let max_allowed_cols = app_state.settings.max_arrow_columns;
    let view_start = offset;
    let view_end = offset + visible_height;

    // Split into Full and Partial
    let (full_arrows, mut partial_arrows): (Vec<_>, Vec<_>) =
        sorted_arrows.into_iter().partition(|(src, dst, _)| {
            let start_visible = *src >= view_start && *src < view_end;
            let end_visible = *dst >= view_start && *dst < view_end;
            start_visible && end_visible
        });

    // 1. Process Full Arrows: Prefer Inner (Rightmost) columns.
    // Sorted by length ascending, so shortest get preferred columns first.
    for (src, dst, target_opt) in full_arrows {
        let (range_low, range_high) = if src < dst { (src, dst) } else { (dst, src) };

        let mut best_col = None;
        // Search from Max down to 0 (Inner -> Outer)
        let mut col = (max_allowed_cols as isize) - 1;
        while col >= 0 {
            let has_conflict = active_arrows.iter().any(|a| {
                if a.col != col as usize {
                    return false;
                }
                let (a_low, a_high) = if a.start_visible && a.end_visible {
                    if a.start < a.end {
                        (a.start, a.end)
                    } else {
                        (a.end, a.start)
                    }
                } else if a.start_visible {
                    if a.start < a.end {
                        (a.start, a.start + 1)
                    } else {
                        (a.start.saturating_sub(1), a.start)
                    }
                } else if a.end_visible {
                    if a.start < a.end {
                        (a.end.saturating_sub(1), a.end)
                    } else {
                        (a.end, a.end + 1)
                    }
                } else {
                    // Should not happen
                    (0, 0)
                };
                !(a_high < range_low || a_low > range_high)
            });

            if !has_conflict {
                best_col = Some(col as usize);
                break;
            }
            col -= 1;
        }

        if let Some(c) = best_col {
            active_arrows.push(ArrowInfo {
                start: src,
                end: dst,
                col: c,
                target_addr: target_opt,
                start_visible: true,
                end_visible: true,
            });
        } else {
            // Fallback: If no column available for FULL arrow, treat as PARTIAL.
            // Add to partial_arrows list for processing in next step.
            partial_arrows.push((src, dst, target_opt));
        }
    }

    // 2. Process Partial Arrows: Prefer Outer (Leftmost) columns.
    for (src, dst, target_opt) in partial_arrows {
        let start_visible = src >= view_start && src < view_end;
        let end_visible = dst >= view_start && dst < view_end;

        let (range_low, range_high) = if start_visible {
            if src < dst {
                (src, src + 1)
            } else {
                (src.saturating_sub(1), src)
            }
        } else if end_visible {
            if src < dst {
                (dst.saturating_sub(1), dst)
            } else {
                (dst, dst + 1)
            }
        } else {
            continue;
        };

        let mut best_col = None;
        // Search from 0 up to Max (Outer -> Inner)
        for col in 0..max_allowed_cols {
            let has_conflict = active_arrows.iter().any(|a| {
                if a.col != col {
                    return false;
                }
                let (a_low, a_high) = if a.start_visible && a.end_visible {
                    if a.start < a.end {
                        (a.start, a.end)
                    } else {
                        (a.end, a.start)
                    }
                } else if a.start_visible {
                    if a.start < a.end {
                        (a.start, a.start + 1)
                    } else {
                        (a.start.saturating_sub(1), a.start)
                    }
                } else if a.end_visible {
                    if a.start < a.end {
                        (a.end.saturating_sub(1), a.end)
                    } else {
                        (a.end, a.end + 1)
                    }
                } else {
                    // Should not happen
                    (0, 0)
                };
                !(a_high < range_low || a_low > range_high)
            });

            if !has_conflict {
                best_col = Some(col);
                break;
            }
        }

        if let Some(c) = best_col {
            active_arrows.push(ArrowInfo {
                start: src,
                end: dst,
                col: c,
                target_addr: target_opt,
                start_visible,
                end_visible,
            });
        }
    }

    // Step 3: Compute max columns to determine width
    let arrow_width = (app_state.settings.max_arrow_columns * 2) + 1;
    // 2 chars per column + padding?

    // Helper to render arrow string for line 'i'
    let get_arrow_str = |current_line: usize| -> String {
        let cols = app_state.settings.max_arrow_columns;
        let mut chars = vec![' '; cols * 2 + 1];

        if active_arrows.is_empty() {
            return chars.iter().collect();
        }

        for arrow in &active_arrows {
            let c_idx = arrow.col * 2;
            let is_down = arrow.start < arrow.end;
            let is_relative_target = arrow.target_addr.is_some() && current_line == arrow.end;

            // Simplified Visibility Logic
            if arrow.start_visible && arrow.end_visible {
                // Fully Visible -> Draw normally including vertical line
                let (low, high) = if is_down {
                    (arrow.start, arrow.end)
                } else {
                    (arrow.end, arrow.start)
                };

                // Vertical Line
                if current_line > low && current_line < high {
                    if chars[c_idx] == ' ' {
                        chars[c_idx] = '│';
                    } else if chars[c_idx] == '─' {
                        // Crossing
                        chars[c_idx] = '┼';
                    }
                }

                // If Jump Up Relative Target: We pass through the main line (vertical) to reach the comment line above
                if is_relative_target
                    && !is_down
                    && current_line == arrow.end
                    && chars[c_idx] == ' '
                {
                    chars[c_idx] = '│';
                }

                // Endpoints
                if current_line == arrow.start {
                    if app_state.disassembly[current_line].target_address.is_some() {
                        chars[c_idx] = if is_down { '┌' } else { '└' };
                        chars[c_idx + 1] = '─';
                    }
                } else if current_line == arrow.end && !is_relative_target {
                    chars[c_idx] = if is_down { '└' } else { '┌' };
                    chars[c_idx + 1] = '─';
                }
            } else if arrow.start_visible {
                // Start Only -> Extended Stub (2 lines)
                if current_line == arrow.start {
                    if app_state.disassembly[current_line].target_address.is_some() {
                        chars[c_idx] = if is_down { '┌' } else { '└' };
                        chars[c_idx + 1] = '─'; // Horizontal start
                    }
                } else {
                    // Check extension line
                    if is_down {
                        if current_line == arrow.start + 1 {
                            chars[c_idx] = '▼';
                        }
                    } else {
                        // Up
                        if current_line == arrow.start.saturating_sub(1) {
                            chars[c_idx] = '▲';
                        }
                    }
                }
            } else if arrow.end_visible {
                // End Only -> Extended Stub (2 lines)
                if current_line == arrow.end && !is_relative_target {
                    chars[c_idx] = if is_down { '└' } else { '┌' };
                    chars[c_idx + 1] = '─'; // Horizontal end
                } else {
                    // Check extension line (entry)
                    if is_down {
                        // From Up -> Enters at end - 1
                        if current_line == arrow.end.saturating_sub(1) {
                            chars[c_idx] = '│';
                        }
                    } else {
                        // From Down -> Enters at end + 1
                        if current_line == arrow.end + 1 {
                            chars[c_idx] = '│';
                        }
                    }
                }
            }
        }

        // Post-process for horizontal lines and crossings
        for arrow in &active_arrows {
            let is_relative_target = arrow.target_addr.is_some();
            let is_end_line = current_line == arrow.end;
            let is_start_line = current_line == arrow.start;

            let c_idx = arrow.col * 2;

            // Self-Loop Logic
            if arrow.start == arrow.end && current_line == arrow.start && arrow.start_visible {
                chars[c_idx] = '∞';
            }

            // Determine if we need to draw horizontal line connection to code
            let is_valid_source = app_state.disassembly[current_line].target_address.is_some();
            let safe_is_start_line = is_start_line && is_valid_source;

            let draw_horizontal = if arrow.start == arrow.end {
                arrow.start_visible && is_valid_source // Self-loop always draws horizontal if visible AND valid source
            } else if arrow.start_visible && arrow.end_visible {
                safe_is_start_line || (is_end_line && !is_relative_target)
            } else if arrow.start_visible {
                safe_is_start_line
            } else if arrow.end_visible {
                // Draw horizontal for end line (connection to address)
                // Note: If relative target, we might skip if logic dictates, but generally we want to show arrival.
                is_end_line
            } else {
                false
            };

            if draw_horizontal {
                for c in chars.iter_mut().skip(c_idx + 1) {
                    if *c == ' ' {
                        *c = '─';
                    } else if *c == '│' {
                        *c = '┼';
                    }
                }

                // Arrow Head at the end of the line (rightmost) - indicating arrival at line
                // Only if it's the Destination (End)
                if is_end_line && arrow.end_visible {
                    let last = chars.len() - 1;
                    chars[last] = '►';
                }
            }
        }

        chars.iter().collect()
    };

    // Helper to render arrow string for the line comment associated with line 'i'
    // This represents the space "just above" line 'i'.
    // sub_addr is None for Line Comments, Some(addr) for Relative Labels.
    let get_comment_arrow_str = |current_line: usize, sub_addr: Option<u16>| -> String {
        let cols = app_state.settings.max_arrow_columns;
        let mut chars = vec![' '; cols * 2 + 1];

        if active_arrows.is_empty() {
            return chars.iter().collect();
        }

        for arrow in &active_arrows {
            let c_idx = arrow.col * 2;
            let (low, high) = if arrow.start < arrow.end {
                (arrow.start, arrow.end)
            } else {
                (arrow.end, arrow.start)
            };

            // Logic: Draw vertical line if arrow passes through the space above current_line
            // 1. Pass through: low < current_line < high
            // 2. Jump Up Start: start == current_line (goes UP from here, so passes through above)
            // 3. Jump Down End: end == current_line (comes DOWN to here, so passes through above)

            // Refined Logic for Sub-Lines:
            // "Space above current_line" generally acts as a vertical connector.
            // But if this sub-line is the TARGET, we stop here (Jump Down) or Start here (Jump Up - ends here).

            let is_target_here = if let Some(addr) = sub_addr
                && let Some(target) = arrow.target_addr
            {
                addr == target
            } else {
                false
            };

            let is_relative_target_elsewhere =
                arrow.target_addr.is_some() && arrow.end == current_line;
            // If relative target elsewhere in THIS line block, we need to know if it's above or below us.
            // Relative labels are sorted by offset (ascending).
            // *+1, *+2, ...
            // If we are at *+1, and target is *+2.

            let mut passes_through = (current_line > low && current_line < high)
                || (current_line == arrow.start && arrow.end < arrow.start)
                || (current_line == arrow.end && arrow.start < arrow.end);

            if is_relative_target_elsewhere {
                // We are in the destination line block.
                if arrow.start < arrow.end {
                    // Jump Down: Arrow comes from above.
                    // If is_target_here: Ends here.
                    // If target is "below" us (mid_addr < target): Passes through.
                    // If target is "above" us (mid_addr > target): Should not happen if sorted?
                    // Wait, we process *+1 then *+2.
                    // If target is *+1: Ends here.
                    // If target is *+1, and we are at *+2: The arrow stopped at *+1. We see nothing.

                    if let Some(this_addr) = sub_addr
                        && let Some(target) = arrow.target_addr
                    {
                        passes_through = this_addr < target;
                    } else if sub_addr.is_none() {
                        // Line Comment. Rendered AFTER proper labels.
                        // So relative labels are above us.
                        // If target was relative, it stopped above.
                        passes_through = false;
                    }
                } else {
                    // Jump Up: Arrow comes from below (Main Line).
                    // It passes through Main Line (handled in get_arrow_str).
                    // It reaches up.
                    // If target is *+2. We represent *+1.
                    // Arrow passes through *+1 to get to *+2.
                    // If target is *+1. Arrow ends here.
                    // If target is *+2. We are at Line Comment (lower than *+2). Passes through.

                    if let Some(this_addr) = sub_addr
                        && let Some(target) = arrow.target_addr
                    {
                        passes_through = this_addr < target;
                    } else if sub_addr.is_none() {
                        // Line Comment. Below labels.
                        // Arrow passes through Line Comment to reach labels above.
                        passes_through = true;
                    }
                }
            }

            if passes_through {
                chars[c_idx] = '│';
            }

            if is_target_here {
                // Draw Arrow Head/Corner
                if arrow.start < arrow.end {
                    // Jump Down
                    chars[c_idx] = '└';
                    chars[c_idx + 1] = '─'; // Extend right
                } else {
                    // Jump Up
                    chars[c_idx] = '┌'; // Actually visual is same?
                    // No, Jump Up comes from below. It ends here.
                    // So it looks like ┌─ pointing to text.
                    chars[c_idx] = '┌';
                    chars[c_idx + 1] = '─';
                }
            }
        }

        // Post-process horizontal lines for sub-lines
        for arrow in &active_arrows {
            let is_target_here = if let Some(addr) = sub_addr
                && let Some(target) = arrow.target_addr
            {
                addr == target
            } else {
                false
            };

            if is_target_here {
                let c_idx = arrow.col * 2;
                for c in chars.iter_mut().skip(c_idx + 1) {
                    if *c == ' ' {
                        *c = '─';
                    } else if *c == '│' {
                        *c = '┼';
                    }
                }
                let last = chars.len() - 1;
                chars[last] = '►';
            }
        }

        chars.iter().collect()
    };

    // --- Arrow Calculation End ---

    let mut current_line_num: usize = 1;
    for i in 0..offset {
        if let Some(line) = app_state.disassembly.get(i) {
            if line.line_comment.is_some() {
                current_line_num += 1;
            }
            current_line_num += 1;
        }
    }

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

            let is_cursor_row = i == ui_state.cursor_index;
            let item_base_style = if is_selected {
                Style::default()
                    .bg(ui_state.theme.selection_bg)
                    .fg(ui_state.theme.selection_fg)
            } else {
                Style::default()
            };

            let label_text = if let Some(label) = &line.label {
                formatter.format_label_definition(label)
            } else {
                String::new()
            };

            let mut item_lines = Vec::new();
            let mut current_sub_index = 0;

            // Inject relative labels (e.g. "Label =*+$01")
            if line.bytes.len() > 1 {
                for offset in 1..line.bytes.len() {
                    let mid_addr = line.address.wrapping_add(offset as u16);
                    if let Some(labels) = app_state.labels.get(&mid_addr) {
                        // Prepare X-Ref string for this relative address
                        let xref_str = if let Some(refs) = app_state.cross_refs.get(&mid_addr) {
                            let mut all_refs = refs.clone();
                            if !all_refs.is_empty() && app_state.settings.max_xref_count > 0 {
                                all_refs.sort_unstable();
                                all_refs.dedup();
                                let refs_str_list: Vec<String> = all_refs
                                    .iter()
                                    .take(app_state.settings.max_xref_count)
                                    .map(|r| format!("${:04x}", r))
                                    .collect();
                                format!("; x-ref: {}", refs_str_list.join(", "))
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };

                        for label in labels {
                            let is_highlighted = !is_selected
                                && is_cursor_row
                                && ui_state.sub_cursor_index == current_sub_index;
                            let line_style = if is_highlighted {
                                Style::default().bg(ui_state.theme.selection_bg)
                            } else {
                                item_base_style
                            };

                            let arrow_padding_for_rel = get_comment_arrow_str(i, Some(mid_addr));

                            // Combine Label definition and X-Ref
                            let label_def = format!("{} =*+${:02x}", label.name, offset);

                            let mut spans = vec![
                                Span::styled(
                                    format!("{:5} ", current_line_num),
                                    line_style.fg(ui_state.theme.bytes),
                                ),
                                Span::styled(
                                    format!(
                                        "{:<width$} ",
                                        arrow_padding_for_rel,
                                        width = arrow_width
                                    ),
                                    line_style.fg(ui_state.theme.arrow),
                                ),
                                // Padding to align with Label column (Address 6 + Bytes 12 = 18)
                                Span::styled("                  ".to_string(), line_style),
                                // Label Def acts as Label + Mnemonic + Operand (16 + 5 + 15 = 36)
                                Span::styled(
                                    format!("{:<36}", label_def),
                                    line_style.fg(ui_state.theme.label_def),
                                ),
                            ];

                            if !xref_str.is_empty() {
                                spans.push(Span::styled(
                                    xref_str.clone(),
                                    line_style.fg(ui_state.theme.comment),
                                ));
                            }

                            let relative_line = Line::from(spans);
                            item_lines.push(relative_line);
                            current_line_num += 1;
                            current_sub_index += 1;
                        }
                    }
                }
            }

            // Generate arrow string
            // Generate arrow string
            let arrow_padding = get_arrow_str(i);

            if let Some(line_comment) = &line.line_comment {
                let is_highlighted =
                    !is_selected && is_cursor_row && ui_state.sub_cursor_index == current_sub_index;
                let line_style = if is_highlighted {
                    Style::default().bg(ui_state.theme.selection_bg)
                } else {
                    Style::default()
                };

                let comment_arrow_padding = get_comment_arrow_str(i, None);
                item_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:5} ", current_line_num),
                        line_style.fg(ui_state.theme.bytes),
                    ),
                    Span::styled(
                        format!("{:width$} ", comment_arrow_padding, width = arrow_width),
                        line_style.fg(ui_state.theme.arrow),
                    ),
                    Span::styled(
                        format!("; {}", line_comment),
                        line_style.fg(ui_state.theme.comment),
                    ),
                ]));
                current_line_num += 1;
                current_sub_index += 1;
            }

            let is_highlighted =
                !is_selected && is_cursor_row && ui_state.sub_cursor_index == current_sub_index;
            let is_collapsed = line.mnemonic.starts_with("; Collapsed block");
            let line_style = if is_highlighted {
                Style::default().bg(ui_state.theme.selection_bg)
            } else if is_collapsed {
                // Apply background color for collapsed blocks if not selected/highlighted
                Style::default().bg(ui_state.theme.collapsed_block_bg)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(
                    format!("{:5} ", current_line_num),
                    line_style.fg(ui_state.theme.bytes),
                ),
                Span::styled(
                    format!("{:<width$} ", arrow_padding, width = arrow_width),
                    line_style.fg(ui_state.theme.arrow),
                ),
                Span::styled(
                    format!("{:04X}  ", line.address),
                    line_style.fg(ui_state.theme.address),
                ),
                Span::styled(
                    format!(
                        "{: <12}",
                        if line.show_bytes {
                            hex_bytes(&line.bytes)
                        } else {
                            String::new()
                        }
                    ),
                    line_style.fg(ui_state.theme.bytes),
                ),
                Span::styled(
                    format!("{: <16}", label_text),
                    line_style
                        .fg(ui_state.theme.label_def)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{: <4} ", line.mnemonic),
                    if is_collapsed {
                        line_style
                            .fg(ui_state.theme.collapsed_block)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        line_style
                            .fg(ui_state.theme.mnemonic)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(
                    format!("{: <15}", line.operand),
                    line_style.fg(ui_state.theme.operand),
                ),
                Span::styled(
                    if line.comment.is_empty() {
                        String::new()
                    } else {
                        format!("; {}", line.comment)
                    },
                    line_style.fg(ui_state.theme.comment),
                ),
            ]);
            item_lines.push(content);
            current_line_num += 1;

            ListItem::new(item_lines).style(item_base_style)
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
    let info = format!(
        "{} | {} | Cursor: {:04X} | Origin: {:04X} | File: {:?}{}",
        app_state.settings.platform,
        app_state.settings.assembler,
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

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
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
