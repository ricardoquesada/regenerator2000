use crate::state::AppState;
use crate::ui_state::{ActivePane, UIState};
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
        ("Ctrl+Q", "Quit"),
        ("Ctrl+N", "New Project"),
        ("Ctrl+O", "Open File"),
        ("Ctrl+S", "Save Project"),
        ("Ctrl+Shift+S", "Save Project As..."),
        ("Ctrl+E", "Export .asm"),
        ("Ctrl+Shift+E", "Export .asm As..."),
        ("Ctrl+P", "Document Settings"),
        ("", ""),
        ("Navigation", ""),
        ("Up/Down/Left/Right", "Move Cursor"),
        ("PageUp/PageDown", "Page Up/Down"),
        ("Home/End", "Start/End of File"),
        ("G", "Jump to Address"),
        ("Ctrl+Shift+G", "Jump to Line"),
        ("Enter", "Jump to Operand (if valid)"),
        ("Backspace", "Navigate Back"),
        ("Tab", "Switch Pane (Disasm/Hex Dump)"),
        ("", ""),
        ("Editing", ""),
        ("IsVisualMode (Shift+V)", "Toggle Visual Selection Mode"),
        ("Shift+Arrows", "Select Text"),
        ("C", "Code"),
        ("B", "Byte"),
        ("W", "Word"),
        ("A", "Address"),
        ("T", "Text"),
        ("S", "Screencode"),
        ("Shift+U", "Undefined"),
        ("< (Shift+,)", "Lo/Hi Address"),
        ("> (Shift+.)", "Hi/Lo Address"),
        (";", "Side Comment"),
        ("Shift+;", "Line Comment"),
        ("L", "Label"),
        ("", ""),
        ("View", ""),
        ("Ctrl+2", "Toggle Hex Dump"),
        ("Ctrl+L", "Shifted PETSCII"),
        ("Ctrl+Shift+L", "Unshifted PETSCII"),
        ("", ""),
        ("History", ""),
        ("U/Ctrl+Z", "Undo"),
        ("Ctrl+R/Shift+U", "Redo"),
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
            let text = "Regenerator 2000\n(c) Ricardo Quesada 2026\nriq / L.I.A\nInspired by Regenerator, by Tom-Cat / Nostalgia";
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
    ];

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(items.len() as u16 + 1), // Checkboxes + padding
            Constraint::Length(2),                      // Platform
            Constraint::Length(2), // Assembler (increased to 2 to match platform spacing style/consistency if needed, or keeping previous logic) -- Previous was Min(1). Let's stick to consistent spacing.
            Constraint::Length(2), // Max X-Refs
            Constraint::Length(2), // Arrow Columns
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
    let platform_selected = dialog.selected_index == 4;

    let platform_text = format!("Platform: < {} >", settings.platform);
    let platform_widget = Paragraph::new(platform_text).style(if platform_selected {
        if dialog.is_selecting_platform {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD) // Active
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        }
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

    let assembler_selected = dialog.selected_index == 5;
    let assembler_text = format!("Assembler: < {} >", settings.assembler);

    let assembler_widget = Paragraph::new(assembler_text).style(if assembler_selected {
        if dialog.is_selecting_assembler {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(theme.dialog_fg)
    });

    // Assembler uses layout[2]
    f.render_widget(
        assembler_widget,
        Rect::new(layout[2].x + 2, layout[2].y, layout[2].width - 4, 1),
    );

    // X-Refs uses layout[3]
    let xref_selected = dialog.selected_index == 6;
    let xref_value_str = if dialog.is_editing_xref_count {
        dialog.xref_count_input.clone()
    } else {
        settings.max_xref_count.to_string()
    };
    let xref_text = format!("Max X-Refs: < {} >", xref_value_str);

    // Arrow Columns
    let arrow_selected = dialog.selected_index == 7;
    let arrow_value_str = if dialog.is_editing_arrow_columns {
        dialog.arrow_columns_input.clone()
    } else {
        settings.max_arrow_columns.to_string()
    };
    let arrow_text = format!("Arrow Columns: < {} >", arrow_value_str);
    let xref_widget = Paragraph::new(xref_text).style(if xref_selected {
        if dialog.is_editing_xref_count {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(theme.dialog_fg)
    });

    f.render_widget(
        xref_widget,
        Rect::new(layout[3].x + 2, layout[3].y, layout[3].width - 4, 1),
    );

    let arrow_widget = Paragraph::new(arrow_text).style(if arrow_selected {
        if dialog.is_editing_arrow_columns {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        }
    } else {
        Style::default().fg(theme.dialog_fg)
    });

    f.render_widget(
        arrow_widget,
        Rect::new(layout[4].x + 2, layout[4].y, layout[4].width - 4, 1),
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
            if i == 1 && dialog.is_selecting_theme {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD) // Active selection
            } else {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            }
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
    // Calculate required width for Hex Dump
    // Address (4) + Space (2) + Hex (49) + Separator (2) + ASCII (16) + Borders (2) = 75
    let hex_dump_width = if ui_state.show_hex_dump { 75 } else { 0 };
    let disasm_view_width = area.width.saturating_sub(hex_dump_width);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(disasm_view_width),
            Constraint::Length(hex_dump_width),
        ])
        .split(area);

    render_disassembly(f, layout[0], app_state, ui_state);

    if ui_state.show_hex_dump {
        render_hex_view(f, layout[1], app_state, ui_state);
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
        .title(" Hex Dump ")
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let inner_area = block.inner(area);

    let visible_height = inner_area.height as usize;
    // Each row is 16 bytes
    let bytes_per_row = 16;
    let total_rows = app_state.raw_data.len().div_ceil(bytes_per_row);

    let context_lines = visible_height / 2;
    let offset = ui_state.hex_cursor_index.saturating_sub(context_lines);

    let items: Vec<ListItem> = (0..visible_height)
        .map(|i| {
            let row_index = offset + i;
            if row_index >= total_rows {
                return ListItem::new("");
            }

            let address = app_state.origin as usize + (row_index * bytes_per_row);
            let start_offset = row_index * bytes_per_row;
            let end_offset = (start_offset + bytes_per_row).min(app_state.raw_data.len());
            let row_data = &app_state.raw_data[start_offset..end_offset];

            let mut hex_part = String::with_capacity(3 * 16);
            let mut ascii_part = String::with_capacity(16);

            for (j, &b) in row_data.iter().enumerate() {
                hex_part.push_str(&format!("{:02X} ", b));
                if j == 7 {
                    hex_part.push(' '); // Extra space after 8 bytes
                }
                let is_shifted = ui_state.petscii_mode == crate::ui_state::PetsciiMode::Shifted;
                ascii_part.push(crate::utils::petscii_to_unicode(b, is_shifted));
            }

            // Padding if row is incomplete
            if row_data.len() < bytes_per_row {
                let missing = bytes_per_row - row_data.len();
                for j in 0..missing {
                    hex_part.push_str("   ");
                    if (row_data.len() + j) == 7 {
                        hex_part.push(' ');
                    }
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
                    format!("{:04X}  ", address),
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

    let mut relevant_arrows: Vec<(usize, usize)> = Vec::new(); // (low, high) index

    for (src_idx, line) in app_state.disassembly.iter().enumerate() {
        if let Some(target_addr) = line.target_address {
            // Find dst_idx
            // Since disassembly can be large, linear scan for dst_idx for EVERY jump is O(Jumps * Lines).
            // Can we do better?
            // Use binary search if possible? app_state.disassembly is usually sorted by address.
            if let Ok(dst_idx) = app_state
                .disassembly
                .binary_search_by_key(&target_addr, |l| l.address)
            {
                // binary_search finds *one* match. We want the code line ideally, or label line, but visually correct.
                // Let's refine dst_idx to find the first line with that address (if multiple, e.g. label + byte).
                let mut refined_dst = dst_idx;
                while refined_dst > 0
                    && app_state.disassembly[refined_dst - 1].address == target_addr
                {
                    refined_dst -= 1;
                }
                // Now refined_dst is the first line with that address.
                // Wait, usually the label comes first.
                // 1000: Label
                // 1000: Code
                // Arrow pointing to 1000 should point to Label line (index refined_dst).

                let (low, high) = if src_idx < refined_dst {
                    (src_idx, refined_dst)
                } else {
                    (refined_dst, src_idx)
                };

                // Check intersection with view
                // Interval [low, high] overlaps [offset, end_view] if low <= end_view && high >= offset
                if low <= end_view && high >= offset {
                    relevant_arrows.push((src_idx, refined_dst));
                }
            }
        }
    }

    // Step 1.5: Filter pass-through arrows and limit columns
    let mut filtered_arrows = Vec::new();
    let mut pass_through_arrow: Option<(usize, usize)> = None;

    let view_start = offset;
    let view_end = offset + visible_height;

    for (src, dst) in relevant_arrows {
        let (low, high) = if src < dst { (src, dst) } else { (dst, src) };
        if low < view_start && high >= view_end {
            if pass_through_arrow.is_none() {
                pass_through_arrow = Some((src, dst));
            }
        } else {
            filtered_arrows.push((src, dst));
        }
    }

    if let Some(pt) = pass_through_arrow {
        filtered_arrows.push(pt);
    }

    let relevant_arrows = filtered_arrows;

    // Step 2: Assign columns to arrows
    let mut active_arrows: Vec<ArrowInfo> = Vec::new();

    let mut sorted_arrows = relevant_arrows;
    sorted_arrows.sort_by_key(|(src, dst)| (*src as isize - *dst as isize).abs());

    let max_allowed_cols = app_state.settings.max_arrow_columns;

    for (src, dst) in sorted_arrows {
        let (low, high) = if src < dst { (src, dst) } else { (dst, src) };

        // Try to assign the rightmost column (closest to address)
        let mut col = (max_allowed_cols as isize) - 1;
        let mut best_col = None;

        while col >= 0 {
            let has_conflict = active_arrows
                .iter()
                .any(|a| a.col == col as usize && !(a.end < low || a.start > high));

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
            let (low, high) = if arrow.start < arrow.end {
                (arrow.start, arrow.end)
            } else {
                (arrow.end, arrow.start)
            };

            if current_line >= low && current_line <= high {
                let c_idx = arrow.col * 2;

                // Vertical line
                if current_line > low && current_line < high && chars[c_idx] == ' ' {
                    chars[c_idx] = '│';
                }

                // Endpoints
                if current_line == arrow.start {
                    if arrow.start < arrow.end {
                        chars[c_idx] = '┌';
                        chars[c_idx + 1] = '─';
                    } else {
                        chars[c_idx] = '└';
                        chars[c_idx + 1] = '─';
                    }
                } else if current_line == arrow.end {
                    if arrow.start < arrow.end {
                        // Jump Down
                        chars[c_idx] = '└';
                        chars[c_idx + 1] = '─';
                        // Arrow head
                        if arrow.col == 0 {
                            chars[c_idx + 1] = '►'; // Just pointing right
                        }
                    } else {
                        // Jump Up
                        chars[c_idx] = '┌';
                        chars[c_idx + 1] = '─';
                    }
                }
            }
        }

        // Post-process for horizontal lines and crossings
        for arrow in &active_arrows {
            if current_line == arrow.start || current_line == arrow.end {
                let c_idx = arrow.col * 2;
                for c in chars.iter_mut().skip(c_idx + 1) {
                    if *c == ' ' {
                        *c = '─';
                    } else if *c == '│' {
                        *c = '┼';
                    }
                }
                if current_line == arrow.end {
                    let last = chars.len() - 1;
                    chars[last] = '►';
                }
            }
        }

        chars.iter().collect()
    };

    // Helper to render arrow string for the line comment associated with line 'i'
    // This represents the space "just above" line 'i'.
    let get_comment_arrow_str = |current_line: usize| -> String {
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

            let passes_through = (current_line > low && current_line < high)
                || (current_line == arrow.start && arrow.end < arrow.start)
                || (current_line == arrow.end && arrow.start < arrow.end);

            if passes_through {
                chars[c_idx] = '│';
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

            let style = if i == ui_state.cursor_index {
                Style::default().bg(ui_state.theme.selection_bg)
            } else if is_selected {
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

            // Generate arrow string
            let arrow_padding = get_arrow_str(i);

            if let Some(line_comment) = &line.line_comment {
                let comment_arrow_padding = get_comment_arrow_str(i);
                item_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:5} ", current_line_num),
                        Style::default().fg(ui_state.theme.bytes),
                    ),
                    Span::styled(
                        format!("{:width$} ", comment_arrow_padding, width = arrow_width),
                        Style::default().fg(ui_state.theme.arrow),
                    ),
                    Span::styled(
                        format!("; {}", line_comment),
                        Style::default().fg(ui_state.theme.comment),
                    ),
                ]));
                current_line_num += 1;
            }

            let content = Line::from(vec![
                Span::styled(
                    format!("{:5} ", current_line_num),
                    Style::default().fg(ui_state.theme.bytes),
                ),
                Span::styled(
                    format!("{:<width$} ", arrow_padding, width = arrow_width),
                    Style::default().fg(ui_state.theme.arrow),
                ),
                Span::styled(
                    format!("{:04X}  ", line.address),
                    Style::default().fg(ui_state.theme.address),
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
                    Style::default().fg(ui_state.theme.bytes),
                ),
                Span::styled(
                    format!("{: <16}", label_text),
                    Style::default()
                        .fg(ui_state.theme.label_def)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{: <4} ", line.mnemonic),
                    Style::default()
                        .fg(ui_state.theme.mnemonic)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{: <15}", line.operand),
                    Style::default().fg(ui_state.theme.operand),
                ),
                Span::styled(
                    format!("; {}", line.comment),
                    Style::default().fg(ui_state.theme.comment),
                ),
            ]);
            item_lines.push(content);
            current_line_num += 1;

            ListItem::new(item_lines).style(style)
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
    let status_msg = Paragraph::new(Span::styled(
        format!(" {}", ui_state.status_message),
        Style::default().add_modifier(Modifier::BOLD),
    ))
    .style(
        Style::default()
            .bg(ui_state.theme.status_bar_bg)
            .fg(ui_state.theme.status_bar_fg),
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
