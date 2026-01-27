use crate::state::AppState;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Exit,
    Open,
    Save,
    SaveAs,
    ExportProject,
    ExportProjectAs,
    Undo,
    Redo,
    Code,
    Byte,
    Word,
    Address,
    Text,
    Screencode,
    Analyze,
    DocumentSettings,
    JumpToAddress,
    JumpToLine,
    JumpToOperand,

    SetLoHi,
    SetHiLo,
    SetExternalFile,
    SideComment,
    LineComment,
    ToggleHexDump,
    ToggleSpritesView,
    About,
    ChangeOrigin,
    KeyboardShortcuts,
    Undefined,
    SystemSettings,
    NextImmediateFormat,
    PreviousImmediateFormat,
    Search,
    FindNext,
    FindPrevious,
    HexdumpViewModeNext,
    HexdumpViewModePrev,
    ToggleSpriteMulticolor,
    ToggleCharsetView,
    ToggleCharsetMulticolor,
    ToggleBitmapView,
    ToggleBitmapMulticolor,
    ToggleBlocksView,
    ToggleCollapsedBlock,
    ToggleSplitter,
    FindReferences,
    NavigateToAddress(u16),
    SetBytesBlockByOffset { start: usize, end: usize },
    SetLabel,
}

impl MenuAction {
    pub fn requires_document(&self) -> bool {
        !matches!(
            self,
            MenuAction::Exit
                | MenuAction::Open
                | MenuAction::About
                | MenuAction::KeyboardShortcuts
                | MenuAction::SystemSettings
                | MenuAction::Search
        )
    }
}

pub struct Menu;

impl Widget for Menu {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        render_menu(f, area, &ui_state.menu, &ui_state.theme);
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return WidgetResult::Ignored;
        }

        let menu_state = &mut ui_state.menu;
        let area = ui_state.menu_area;
        let col = mouse.column;
        let row = mouse.row;

        // 1. Check Menu Bar
        if row == area.y && col >= area.x && col < area.x + area.width {
            let mut current_x = area.x;
            for (i, category) in menu_state.categories.iter().enumerate() {
                let width = (category.name.len() + 2) as u16; // " name "
                if col >= current_x && col < current_x + width {
                    menu_state.selected_category = i;
                    menu_state.active = true;
                    menu_state.selected_item = None;
                    return WidgetResult::Handled;
                }
                current_x += width;
            }
        }

        // 2. Check Popup if active
        if menu_state.active {
            // Replicate popup geometry calculation
            let mut x_offset = 0;
            for i in 0..menu_state.selected_category {
                x_offset += menu_state.categories[i].name.len() as u16 + 2;
            }

            let category = &menu_state.categories[menu_state.selected_category];
            let mut max_name_len = 0;
            let mut max_shortcut_len = 0;
            for item in &category.items {
                max_name_len = max_name_len.max(item.name.len());
                max_shortcut_len =
                    max_shortcut_len.max(item.shortcut.as_ref().map(|s| s.len()).unwrap_or(0));
            }
            let content_width = max_name_len + 2 + max_shortcut_len;
            let width = (content_width as u16 + 2).max(20);
            let height = category.items.len() as u16 + 2;

            let popup_x = area.x + x_offset;
            let popup_y = area.y + 1;

            // Check if click is inside popup
            if col >= popup_x && col < popup_x + width && row >= popup_y && row < popup_y + height {
                // Clicked inside popup
                let rel_y = row - popup_y;
                if rel_y > 0 && rel_y < height - 1 {
                    // Inside borders
                    let item_idx = (rel_y - 1) as usize;
                    if item_idx < category.items.len() {
                        let item = &category.items[item_idx];
                        if item.is_separator {
                            return WidgetResult::Handled;
                        }
                        if item.disabled {
                            return WidgetResult::Handled;
                        }
                        // Execute action
                        if let Some(action) = &item.action {
                            menu_state.active = false;
                            menu_state.selected_item = None;
                            return WidgetResult::Action(action.clone());
                        }
                    }
                }
                return WidgetResult::Handled;
            }
        }

        WidgetResult::Ignored
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.menu.active = false;
                ui_state.menu.selected_item = None;
                ui_state.set_status_message("Ready");
                WidgetResult::Handled
            }
            KeyCode::Right => {
                ui_state.menu.next_category();
                WidgetResult::Handled
            }
            KeyCode::Left => {
                ui_state.menu.previous_category();
                WidgetResult::Handled
            }
            KeyCode::Char('f') if key.modifiers == KeyModifiers::ALT => {
                ui_state.menu.selected_category = 0;
                ui_state.menu.select_first_enabled_item();
                WidgetResult::Handled
            }
            KeyCode::Char('h') if key.modifiers == KeyModifiers::ALT => {
                if let Some(pos) = ui_state
                    .menu
                    .categories
                    .iter()
                    .position(|c| c.name == "Help")
                {
                    ui_state.menu.selected_category = pos;
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            KeyCode::Down => {
                ui_state.menu.next_item();
                WidgetResult::Handled
            }
            KeyCode::Up => {
                ui_state.menu.previous_item();
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                if let Some(item_idx) = ui_state.menu.selected_item {
                    let category_idx = ui_state.menu.selected_category;
                    let item = &ui_state.menu.categories[category_idx].items[item_idx];

                    if !item.disabled {
                        let action = item.action.clone();
                        if let Some(action) = action {
                            // Close menu after valid action
                            ui_state.menu.active = false;
                            ui_state.menu.selected_item = None;
                            return WidgetResult::Action(action);
                        }
                    } else {
                        // Optional: Feedback that it's disabled
                        ui_state.set_status_message("Item is disabled");
                    }
                } else {
                    // Enter on category -> open first item?
                    ui_state.menu.select_first_enabled_item();
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Ignored,
        }
    }
}

#[derive(Default)]
pub struct MenuState {
    pub active: bool,
    pub categories: Vec<MenuCategory>,
    pub selected_category: usize,
    pub selected_item: Option<usize>,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            active: false,
            categories: vec![
                MenuCategory {
                    name: "File".to_string(),
                    items: vec![
                        MenuItem::new("Open", Some("Ctrl+O"), Some(MenuAction::Open)),
                        MenuItem::new("Save", Some("Ctrl+S"), Some(MenuAction::Save)),
                        MenuItem::new("Save As...", Some("Alt+S"), Some(MenuAction::SaveAs)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Export Project",
                            Some("Ctrl+E"),
                            Some(MenuAction::ExportProject),
                        ),
                        MenuItem::new(
                            "Export Project As...",
                            Some("Alt+E"),
                            Some(MenuAction::ExportProjectAs),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Settings", Some("Alt+O"), Some(MenuAction::SystemSettings)),
                        MenuItem::separator(),
                        MenuItem::new("Exit", Some("Ctrl+Q"), Some(MenuAction::Exit)),
                    ],
                },
                MenuCategory {
                    name: "Edit".to_string(),
                    items: vec![
                        MenuItem::new("Undo", Some("U"), Some(MenuAction::Undo)),
                        MenuItem::new("Redo", Some("Ctrl+R"), Some(MenuAction::Redo)),
                        MenuItem::separator(),
                        MenuItem::new("Code", Some("C"), Some(MenuAction::Code)),
                        MenuItem::new("Byte", Some("B"), Some(MenuAction::Byte)),
                        MenuItem::new("Word", Some("W"), Some(MenuAction::Word)),
                        MenuItem::new("Address", Some("A"), Some(MenuAction::Address)),
                        MenuItem::new("Lo/Hi Address", Some("<"), Some(MenuAction::SetLoHi)),
                        MenuItem::new("Hi/Lo Address", Some(">"), Some(MenuAction::SetHiLo)),
                        MenuItem::new(
                            "External File",
                            Some("e"),
                            Some(MenuAction::SetExternalFile),
                        ),
                        MenuItem::new("PETSCII Text", Some("T"), Some(MenuAction::Text)),
                        MenuItem::new("Screencode Text", Some("S"), Some(MenuAction::Screencode)),
                        MenuItem::new("Undefined", Some("?"), Some(MenuAction::Undefined)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Next Imm. Mode Format",
                            Some("d"),
                            Some(MenuAction::NextImmediateFormat),
                        ),
                        MenuItem::new(
                            "Prev Imm. Mode Format",
                            Some("Shift+D"),
                            Some(MenuAction::PreviousImmediateFormat),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Splitter",
                            Some("|"),
                            Some(MenuAction::ToggleSplitter),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Side Comment", Some(";"), Some(MenuAction::SideComment)),
                        MenuItem::new(
                            "Line Comment",
                            Some("Shift+;"),
                            Some(MenuAction::LineComment),
                        ),
                        MenuItem::new("Set Label", Some("l"), Some(MenuAction::SetLabel)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Collapsed Block",
                            Some("Ctrl+K"),
                            Some(MenuAction::ToggleCollapsedBlock),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("Change Origin", None, Some(MenuAction::ChangeOrigin)),
                        MenuItem::separator(),
                        MenuItem::new("Analyze", Some("Ctrl+A"), Some(MenuAction::Analyze)),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Document Settings",
                            Some("Alt+D"),
                            Some(MenuAction::DocumentSettings),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Jump".to_string(),
                    items: vec![
                        MenuItem::new(
                            "Jump to address",
                            Some("G"),
                            Some(MenuAction::JumpToAddress),
                        ),
                        MenuItem::new("Jump to line", Some("Alt+G"), Some(MenuAction::JumpToLine)),
                        MenuItem::new(
                            "Jump to operand",
                            Some("Enter"),
                            Some(MenuAction::JumpToOperand),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Search".to_string(),
                    items: vec![
                        MenuItem::new("Search...", Some("Ctrl+F"), Some(MenuAction::Search)),
                        MenuItem::new("Find Next", Some("F3"), Some(MenuAction::FindNext)),
                        MenuItem::new(
                            "Find Previous",
                            Some("Shift+F3"),
                            Some(MenuAction::FindPrevious),
                        ),
                        MenuItem::new(
                            "Find References",
                            Some("Ctrl+x"),
                            Some(MenuAction::FindReferences),
                        ),
                    ],
                },
                MenuCategory {
                    name: "View".to_string(),
                    items: vec![
                        MenuItem::new(
                            "Next Hex Dump Mode",
                            Some("m"),
                            Some(MenuAction::HexdumpViewModeNext),
                        ),
                        MenuItem::new(
                            "Prev Hex Dump Mode",
                            Some("Shift+M"),
                            Some(MenuAction::HexdumpViewModePrev),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Sprites",
                            Some("m"),
                            Some(MenuAction::ToggleSpriteMulticolor),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Bitmap",
                            Some("m"),
                            Some(MenuAction::ToggleBitmapMulticolor),
                        ),
                        MenuItem::new(
                            "Toggle Multicolor Charset",
                            Some("m"),
                            Some(MenuAction::ToggleCharsetMulticolor),
                        ),
                        MenuItem::separator(),
                        MenuItem::new(
                            "Toggle Hex Dump",
                            Some("Alt+2"),
                            Some(MenuAction::ToggleHexDump),
                        ),
                        MenuItem::new(
                            "Toggle Sprites View",
                            Some("Alt+3"),
                            Some(MenuAction::ToggleSpritesView),
                        ),
                        MenuItem::new(
                            "Toggle Charset View",
                            Some("Alt+4"),
                            Some(MenuAction::ToggleCharsetView),
                        ),
                        MenuItem::new(
                            "Toggle Bitmap View",
                            Some("Alt+5"),
                            Some(MenuAction::ToggleBitmapView),
                        ),
                        MenuItem::new(
                            "Toggle Blocks View",
                            Some("Alt+6"),
                            Some(MenuAction::ToggleBlocksView),
                        ),
                    ],
                },
                MenuCategory {
                    name: "Help".to_string(),
                    items: vec![
                        MenuItem::new(
                            "Keyboard Shortcuts",
                            None,
                            Some(MenuAction::KeyboardShortcuts),
                        ),
                        MenuItem::separator(),
                        MenuItem::new("About", None, Some(MenuAction::About)),
                    ],
                },
            ],
            selected_category: 0,
            selected_item: None,
        }
    }

    pub fn next_category(&mut self) {
        self.selected_category = (self.selected_category + 1) % self.categories.len();
        // If we are active, select the first non-separator item
        if self.active {
            self.select_first_enabled_item();
        }
    }

    pub fn previous_category(&mut self) {
        if self.selected_category == 0 {
            self.selected_category = self.categories.len() - 1;
        } else {
            self.selected_category -= 1;
        }
        if self.active {
            self.select_first_enabled_item();
        }
    }

    pub fn next_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if count == 0 {
            return;
        }
        let current = self.selected_item.unwrap_or(0);
        let mut next = (current + 1) % count;

        // Skip separators and disabled items
        // We iterate at most `count` times to avoid infinite loop
        for _ in 0..count {
            let item = &self.categories[self.selected_category].items[next];
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(next);
                return;
            }
            next = (next + 1) % count;
        }
    }

    pub fn previous_item(&mut self) {
        let count = self.categories[self.selected_category].items.len();
        if count == 0 {
            return;
        }
        let current = self.selected_item.unwrap_or(0);

        let mut prev = if current == 0 { count - 1 } else { current - 1 };

        // We iterate at most `count` times to avoid infinite loop
        for _ in 0..count {
            let item = &self.categories[self.selected_category].items[prev];
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(prev);
                return;
            }
            prev = if prev == 0 { count - 1 } else { prev - 1 };
        }
    }

    pub fn select_first_enabled_item(&mut self) {
        let items = &self.categories[self.selected_category].items;
        for (i, item) in items.iter().enumerate() {
            if !item.is_separator && !item.disabled {
                self.selected_item = Some(i);
                return;
            }
        }
        self.selected_item = None;
    }
    pub fn update_availability(
        &mut self,
        app_state: &crate::state::AppState,
        cursor_index: usize,
        last_search_empty: bool,
        active_pane: ActivePane,
    ) {
        let has_document = !app_state.raw_data.is_empty();
        for category in &mut self.categories {
            for item in &mut category.items {
                if let Some(action) = &item.action {
                    if action.requires_document() && !has_document {
                        item.disabled = true;
                    } else {
                        // Context-specific checks
                        match action {
                            MenuAction::FindNext | MenuAction::FindPrevious => {
                                item.disabled = last_search_empty;
                            }
                            MenuAction::NextImmediateFormat
                            | MenuAction::PreviousImmediateFormat => {
                                let mut is_immediate = false;
                                if has_document
                                    && let Some(line) = app_state.disassembly.get(cursor_index)
                                    && let Some(opcode) = &line.opcode
                                    && opcode.mode == crate::cpu::AddressingMode::Immediate
                                {
                                    is_immediate = true;
                                }
                                item.disabled = !is_immediate;
                            }
                            MenuAction::HexdumpViewModeNext | MenuAction::HexdumpViewModePrev => {
                                item.disabled = active_pane != ActivePane::HexDump;
                            }
                            MenuAction::ToggleSpriteMulticolor => {
                                item.disabled = active_pane != ActivePane::Sprites;
                            }
                            MenuAction::ToggleCharsetMulticolor => {
                                item.disabled = active_pane != ActivePane::Charset;
                            }
                            MenuAction::SetLabel | MenuAction::FindReferences => {
                                item.disabled = active_pane != ActivePane::Disassembly;
                            }
                            _ => item.disabled = false,
                        }
                    }
                }
            }
        }
    }
}

pub struct MenuCategory {
    pub name: String,
    pub items: Vec<MenuItem>,
}

#[derive(Clone)]
pub struct MenuItem {
    pub name: String,
    pub shortcut: Option<String>,
    pub is_separator: bool,
    pub action: Option<MenuAction>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(name: &str, shortcut: Option<&str>, action: Option<MenuAction>) -> Self {
        Self {
            name: name.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            is_separator: false,
            action,
            disabled: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            name: String::new(),
            shortcut: None,
            is_separator: true,
            action: None,
            disabled: false,
        }
    }
}

pub fn render_menu(f: &mut Frame, area: Rect, menu_state: &MenuState, theme: &crate::theme::Theme) {
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
        } else if category.name == "Help" {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("H", style.add_modifier(Modifier::UNDERLINED)));
            spans.push(Span::styled("elp ", style));
        } else {
            spans.push(Span::styled(format!(" {} ", category.name), style));
        }
    }

    // Fill the rest of the line
    let menu_bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(theme.menu_bg).fg(theme.menu_fg));
    f.render_widget(menu_bar, area);
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
            max_shortcut_len.max(item.shortcut.as_ref().map(|s| s.len()).unwrap_or(0));
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

pub fn handle_menu_action(app_state: &mut AppState, ui_state: &mut UIState, action: MenuAction) {
    if action.requires_document() && app_state.raw_data.is_empty() {
        ui_state.set_status_message("No open document");
        return;
    }

    // Context-specific checks for actions that didn't fit in update_availability
    // or need enforcement even via shortcuts
    if action == MenuAction::FindReferences && ui_state.active_pane != ActivePane::Disassembly {
        ui_state.set_status_message("Action only available in Disassembly View");
        return;
    }

    // Check for changes on destructive actions
    let is_destructive = matches!(action, MenuAction::Exit | MenuAction::Open);

    if is_destructive && app_state.is_dirty() {
        ui_state.active_dialog = Some(Box::new(
            crate::ui::dialog_confirmation::ConfirmationDialog::new(
                "Unsaved Changes",
                "You have unsaved changes. Proceed?",
                action,
            ),
        ));
        return;
    }

    execute_menu_action(app_state, ui_state, action);
}

pub fn execute_menu_action(app_state: &mut AppState, ui_state: &mut UIState, action: MenuAction) {
    ui_state.set_status_message(format!("Action: {:?}", action));

    match action {
        MenuAction::Exit => ui_state.should_quit = true,

        MenuAction::Open => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_open::OpenDialog::new(
                ui_state.file_dialog_current_dir.clone(),
            )));
            ui_state.set_status_message("Select a file to open");
        }
        MenuAction::Save => {
            if app_state.project_path.is_some() {
                let context = create_save_context(app_state, ui_state);
                if let Err(e) = app_state.save_project(context, true) {
                    ui_state.set_status_message(format!("Error saving: {}", e));
                } else {
                    ui_state.set_status_message("Project saved");
                }
            } else {
                ui_state.active_dialog =
                    Some(Box::new(crate::ui::dialog_save_as::SaveAsDialog::new()));
                ui_state.set_status_message("Enter Project filename");
            }
        }
        MenuAction::SaveAs => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_save_as::SaveAsDialog::new()));
            ui_state.set_status_message("Enter Project filename");
        }
        MenuAction::ExportProject => {
            if let Some(path) = &app_state.export_path {
                if let Err(e) = crate::exporter::export_asm(app_state, path) {
                    ui_state.set_status_message(format!("Error exporting: {}", e));
                } else {
                    ui_state.set_status_message("Project Exported");
                }
            } else {
                ui_state.active_dialog =
                    Some(Box::new(crate::ui::dialog_export_as::ExportAsDialog::new()));
                ui_state.set_status_message("Enter .asm filename");
            }
        }
        MenuAction::ExportProjectAs => {
            ui_state.active_dialog =
                Some(Box::new(crate::ui::dialog_export_as::ExportAsDialog::new()));
            ui_state.set_status_message("Enter .asm filename");
        }
        MenuAction::DocumentSettings => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_document_settings::DocumentSettingsDialog::new(),
            ));
            ui_state.set_status_message("Document Settings");
        }
        MenuAction::Analyze => {
            // Capture current address
            let current_addr = app_state
                .disassembly
                .get(ui_state.cursor_index)
                .map(|l| l.address);

            ui_state.set_status_message(app_state.perform_analysis());

            // Restore cursor
            if let Some(addr) = current_addr {
                if let Some(idx) = app_state.get_line_index_containing_address(addr) {
                    ui_state.cursor_index = idx;
                } else if let Some(idx) = app_state.get_line_index_for_address(addr) {
                    // Fallback
                    ui_state.cursor_index = idx;
                } else {
                    // Fallback to origin if address lost
                    if let Some(idx) = app_state.get_line_index_for_address(app_state.origin) {
                        ui_state.cursor_index = idx;
                    }
                }
            } else {
                // If we didn't have a valid cursor (empty?), go to origin
                if let Some(idx) = app_state.get_line_index_for_address(app_state.origin) {
                    ui_state.cursor_index = idx;
                }
            }
        }
        MenuAction::SetLabel => {
            crate::ui::view_disassembly::action_set_label(app_state, ui_state);
        }
        MenuAction::Undo => {
            ui_state.set_status_message(app_state.undo_last_command());
        }
        MenuAction::Redo => {
            ui_state.set_status_message(app_state.redo_last_command());
        }

        MenuAction::Code => apply_block_type(app_state, ui_state, crate::state::BlockType::Code),
        MenuAction::Byte => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::DataByte)
        }
        MenuAction::Word => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::DataWord)
        }
        MenuAction::SetExternalFile => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::ExternalFile)
        }
        MenuAction::Address => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::Address)
        }
        MenuAction::Text => apply_block_type(app_state, ui_state, crate::state::BlockType::Text),
        MenuAction::Screencode => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::Screencode)
        }
        MenuAction::Undefined => {
            apply_block_type(app_state, ui_state, crate::state::BlockType::Undefined)
        }
        MenuAction::JumpToAddress => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_jump_to_address::JumpToAddressDialog::new(),
            ));
            ui_state.set_status_message("Enter address (Hex)");
        }
        MenuAction::JumpToLine => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_jump_to_line::JumpToLineDialog::new(),
            ));
            ui_state.set_status_message("Enter Line Number (Dec)");
        }
        MenuAction::Search => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_search::SearchDialog::new(
                ui_state.last_search_query.clone(),
            )));
            ui_state.set_status_message("Search...");
        }
        MenuAction::FindNext => {
            crate::ui::dialog_search::perform_search(app_state, ui_state, true);
        }
        MenuAction::FindPrevious => {
            crate::ui::dialog_search::perform_search(app_state, ui_state, false);
        }
        MenuAction::FindReferences => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let addr = if line.address == 0 && line.bytes.is_empty() {
                    line.external_label_address.unwrap_or(0)
                } else {
                    line.address
                };
                ui_state.active_dialog = Some(Box::new(
                    crate::ui::dialog_find_references::FindReferencesDialog::new(app_state, addr),
                ));
                ui_state.set_status_message(format!("References to ${:04X}", addr));
            } else {
                ui_state.set_status_message("No address selected");
            }
        }
        MenuAction::NavigateToAddress(target_addr) => {
            match ui_state.active_pane {
                ActivePane::Disassembly => {
                    if let Some(idx) = app_state
                        .get_line_index_containing_address(target_addr)
                        .or_else(|| app_state.get_line_index_for_address(target_addr))
                    {
                        ui_state
                            .navigation_history
                            .push((ActivePane::Disassembly, ui_state.cursor_index));
                        ui_state.cursor_index = idx;

                        // Smart Jump: Select relevant sub-line if applicable
                        if let Some(line) = app_state.disassembly.get(idx) {
                            ui_state.sub_cursor_index =
                                crate::ui::view_disassembly::DisassemblyView::get_sub_index_for_address(
                                    line,
                                    app_state,
                                    target_addr,
                                );
                        } else {
                            ui_state.sub_cursor_index = 0;
                        }

                        ui_state.set_status_message(format!("Jumped to ${:04X}", target_addr));
                    } else if !app_state.disassembly.is_empty() {
                        // Fallback to closest or valid range?
                        // Existing logic was "Jumped to end" if not found?
                        // Let's stick to "not found" or strict check unless requested otherwise.
                        // But wait, the dialog logic had a fallback:
                        // } else if !app_state.disassembly.is_empty() { ... jump to end ... }
                        // We can keep that if desired, but "Address not found" is usually better.
                        // Let's stick to strict behavior for now, or maybe just log it.
                        ui_state
                            .set_status_message(format!("Address ${:04X} not found", target_addr));
                    }
                }
                ActivePane::HexDump => {
                    let origin = app_state.origin as usize;
                    let target = target_addr as usize;
                    let end_addr = origin + app_state.raw_data.len();

                    if target >= origin && target < end_addr {
                        let alignment_padding = origin % 16;
                        let aligned_origin = origin - alignment_padding;
                        let offset = target - aligned_origin;
                        let row = offset / 16;
                        ui_state.hex_cursor_index = row;
                        ui_state.set_status_message(format!("Jumped to ${:04X}", target_addr));
                    } else {
                        ui_state.set_status_message("Address out of range");
                    }
                }
                ActivePane::Sprites => {
                    let origin = app_state.origin as usize;
                    let target = target_addr as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    let aligned_start = origin + padding;
                    let end_addr = origin + app_state.raw_data.len();

                    if target >= aligned_start && target < end_addr {
                        let offset = target - aligned_start;
                        let sprite_idx = offset / 64;
                        ui_state.sprites_cursor_index = sprite_idx;
                        ui_state.set_status_message(format!(
                            "Jumped to sprite at ${:04X}",
                            target_addr
                        ));
                    } else {
                        ui_state.set_status_message("Address out of range or unaligned");
                    }
                }
                ActivePane::Charset => {
                    let origin = app_state.origin as usize;
                    let target = target_addr as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    let end_addr = origin + app_state.raw_data.len();

                    if target >= aligned_start_addr && target < end_addr {
                        let offset = target - aligned_start_addr;
                        let char_idx = offset / 8;
                        ui_state.charset_cursor_index = char_idx;
                        ui_state
                            .set_status_message(format!("Jumped to char at ${:04X}", target_addr));
                    } else {
                        ui_state.set_status_message("Address out of range");
                    }
                }
                ActivePane::Blocks => {
                    ui_state.set_status_message("Jump to address not supported in Blocks view");
                }
                ActivePane::Bitmap => {
                    ui_state.set_status_message("Jump to address not supported in Bitmap view");
                }
            }
        }

        MenuAction::JumpToOperand => {
            let target_addr = match ui_state.active_pane {
                ActivePane::Disassembly => {
                    if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                        // Try to extract address from operand.
                        // We utilize the opcode mode if available.
                        if let Some(opcode) = &line.opcode {
                            use crate::cpu::AddressingMode;
                            match opcode.mode {
                                AddressingMode::Absolute
                                | AddressingMode::AbsoluteX
                                | AddressingMode::AbsoluteY => {
                                    if line.bytes.len() >= 3 {
                                        Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Indirect => {
                                    // JMP ($1234) -> target is $1234
                                    if line.bytes.len() >= 3 {
                                        Some((line.bytes[2] as u16) << 8 | (line.bytes[1] as u16))
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::Relative => {
                                    // Branch
                                    if line.bytes.len() >= 2 {
                                        let offset = line.bytes[1] as i8;
                                        Some(
                                            line.address
                                                .wrapping_add(2)
                                                .wrapping_add(offset as u16),
                                        )
                                    } else {
                                        None
                                    }
                                }
                                AddressingMode::ZeroPage
                                | AddressingMode::ZeroPageX
                                | AddressingMode::ZeroPageY
                                | AddressingMode::IndirectX
                                | AddressingMode::IndirectY => {
                                    if line.bytes.len() >= 2 {
                                        Some(line.bytes[1] as u16)
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            line.external_label_address
                        }
                    } else {
                        None
                    }
                }
                ActivePane::HexDump => {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    Some((aligned_origin + ui_state.hex_cursor_index * 16) as u16)
                }
                ActivePane::Sprites => {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    Some((origin + padding + ui_state.sprites_cursor_index * 64) as u16)
                }
                ActivePane::Charset => {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    Some((aligned_start_addr + ui_state.charset_cursor_index * 8) as u16)
                }
                ActivePane::Blocks => {
                    // Jump to start of selected block
                    let blocks = app_state.get_blocks_view_items();
                    let idx = ui_state.blocks_list_state.selected().unwrap_or(0);
                    if idx < blocks.len() {
                        match blocks[idx] {
                            crate::state::BlockItem::Block { start, .. } => {
                                let offset = start;
                                Some(app_state.origin.wrapping_add(offset))
                            }
                            crate::state::BlockItem::Splitter(addr) => Some(addr),
                        }
                    } else {
                        None
                    }
                }
                ActivePane::Bitmap => {
                    let origin = app_state.origin as usize;
                    // Bitmaps must be aligned to 8192-byte boundaries
                    let first_aligned_addr = ((origin / 8192) * 8192)
                        + if origin.is_multiple_of(8192) { 0 } else { 8192 };
                    let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
                    Some(bitmap_addr as u16)
                }
            };

            if let Some(addr) = target_addr {
                // Perform Jump
                if let Some(idx) = app_state
                    .get_line_index_containing_address(addr)
                    .or_else(|| app_state.get_line_index_for_address(addr))
                {
                    ui_state
                        .navigation_history
                        .push((ActivePane::Disassembly, ui_state.cursor_index));
                    ui_state.cursor_index = idx;
                    ui_state.active_pane = ActivePane::Disassembly;
                    ui_state.sub_cursor_index = 0; // Reset sub-line selection
                    ui_state.set_status_message(format!("Jumped to ${:04X}", addr));
                } else {
                    ui_state.set_status_message(format!("Address ${:04X} not found", addr));
                }
            } else if ui_state.active_pane == ActivePane::Disassembly {
                ui_state.set_status_message("No target address");
            }
        }
        MenuAction::About => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_about::AboutDialog::new(
                ui_state,
            )));
            ui_state.set_status_message("About Regenerator 2000");
        }
        MenuAction::HexdumpViewModeNext => {
            let new_mode = match ui_state.hexdump_view_mode {
                crate::state::HexdumpViewMode::ScreencodeShifted => {
                    crate::state::HexdumpViewMode::ScreencodeUnshifted
                }
                crate::state::HexdumpViewMode::ScreencodeUnshifted => {
                    crate::state::HexdumpViewMode::PETSCIIShifted
                }
                crate::state::HexdumpViewMode::PETSCIIShifted => {
                    crate::state::HexdumpViewMode::PETSCIIUnshifted
                }
                crate::state::HexdumpViewMode::PETSCIIUnshifted => {
                    crate::state::HexdumpViewMode::ScreencodeShifted
                }
            };
            ui_state.hexdump_view_mode = new_mode;
            update_hexdump_status(ui_state, new_mode);
        }
        MenuAction::HexdumpViewModePrev => {
            let new_mode = match ui_state.hexdump_view_mode {
                crate::state::HexdumpViewMode::ScreencodeShifted => {
                    crate::state::HexdumpViewMode::PETSCIIUnshifted
                }
                crate::state::HexdumpViewMode::ScreencodeUnshifted => {
                    crate::state::HexdumpViewMode::ScreencodeShifted
                }
                crate::state::HexdumpViewMode::PETSCIIShifted => {
                    crate::state::HexdumpViewMode::ScreencodeUnshifted
                }
                crate::state::HexdumpViewMode::PETSCIIUnshifted => {
                    crate::state::HexdumpViewMode::PETSCIIShifted
                }
            };
            ui_state.hexdump_view_mode = new_mode;
            update_hexdump_status(ui_state, new_mode);
        }
        MenuAction::ToggleSplitter => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected()
                    && idx < blocks.len()
                    // If it's a splitter, toggle it (remove it).
                    && let crate::state::BlockItem::Splitter(addr) = blocks[idx]
                {
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(app_state);
                    app_state.push_command(command);
                    ui_state.set_status_message(format!("Removed splitter at ${:04X}", addr));
                }
            } else if ui_state.active_pane == ActivePane::Disassembly {
                let addr_to_toggle = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|line| line.address);

                if let Some(addr) = addr_to_toggle {
                    let command = crate::commands::Command::ToggleSplitter { address: addr };
                    command.apply(app_state);
                    app_state.push_command(command);
                    ui_state.set_status_message(format!("Toggled splitter at ${:04X}", addr));
                }
            }
        }
        MenuAction::ToggleSpriteMulticolor => {
            ui_state.sprite_multicolor_mode = !ui_state.sprite_multicolor_mode;
            if ui_state.sprite_multicolor_mode {
                ui_state.set_status_message("Sprites: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Sprites: Single Color Mode");
            }
        }
        MenuAction::ToggleCharsetMulticolor => {
            ui_state.charset_multicolor_mode = !ui_state.charset_multicolor_mode;
            if ui_state.charset_multicolor_mode {
                ui_state.set_status_message("Charset: Multicolor Mode ON");
            } else {
                ui_state.set_status_message("Charset: Single Color Mode");
            }
        }
        MenuAction::SetLoHi => apply_block_type(app_state, ui_state, crate::state::BlockType::LoHi),
        MenuAction::SetHiLo => apply_block_type(app_state, ui_state, crate::state::BlockType::HiLo),
        MenuAction::SideComment => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let address = line.address;
                let current_comment = app_state
                    .user_side_comments
                    .get(&address)
                    .map(|s| s.as_str());
                ui_state.active_dialog =
                    Some(Box::new(crate::ui::dialog_comment::CommentDialog::new(
                        current_comment,
                        crate::ui::dialog_comment::CommentType::Side,
                    )));
                ui_state.set_status_message(format!("Edit Side Comment at ${:04X}", address));
            }
        }
        MenuAction::LineComment => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let address = line.address;
                let current_comment = app_state
                    .user_line_comments
                    .get(&address)
                    .map(|s| s.as_str());
                ui_state.active_dialog =
                    Some(Box::new(crate::ui::dialog_comment::CommentDialog::new(
                        current_comment,
                        crate::ui::dialog_comment::CommentType::Line,
                    )));
                ui_state.set_status_message(format!("Edit Line Comment at ${:04X}", address));
            }
        }
        MenuAction::ToggleHexDump => {
            if ui_state.right_pane == crate::ui_state::RightPane::HexDump {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Hex Dump View Hidden");
                if ui_state.active_pane == ActivePane::HexDump {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::HexDump;
                ui_state.active_pane = ActivePane::HexDump;
                ui_state.set_status_message("Hex Dump View Shown");
            }
        }
        MenuAction::ToggleSpritesView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Sprites {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Sprites View Hidden");
                if ui_state.active_pane == ActivePane::Sprites {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Sprites;
                ui_state.active_pane = ActivePane::Sprites;
                ui_state.set_status_message("Sprites View Shown");
            }
        }
        MenuAction::ToggleCharsetView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Charset {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Charset View Hidden");
                if ui_state.active_pane == ActivePane::Charset {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Charset;
                ui_state.active_pane = ActivePane::Charset;
                ui_state.set_status_message("Charset View Shown");
            }
        }
        MenuAction::ToggleBitmapView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Bitmap {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Bitmap View Hidden");
                if ui_state.active_pane == ActivePane::Bitmap {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Bitmap;
                ui_state.active_pane = ActivePane::Bitmap;
                ui_state.set_status_message("Bitmap View Shown");
            }
        }
        MenuAction::ToggleBitmapMulticolor => {
            ui_state.bitmap_multicolor_mode = !ui_state.bitmap_multicolor_mode;
            ui_state.set_status_message(if ui_state.bitmap_multicolor_mode {
                "Multicolor mode enabled"
            } else {
                "Single color mode enabled"
            });
        }
        MenuAction::ToggleBlocksView => {
            if ui_state.right_pane == crate::ui_state::RightPane::Blocks {
                ui_state.right_pane = crate::ui_state::RightPane::None;
                ui_state.set_status_message("Blocks View Hidden");
                if ui_state.active_pane == ActivePane::Blocks {
                    ui_state.active_pane = ActivePane::Disassembly;
                }
            } else {
                ui_state.right_pane = crate::ui_state::RightPane::Blocks;
                ui_state.active_pane = ActivePane::Blocks;
                ui_state.set_status_message("Blocks View Shown");
            }
        }
        MenuAction::KeyboardShortcuts => {
            ui_state.active_dialog = Some(Box::new(
                crate::ui::dialog_keyboard_shortcut::ShortcutsDialog::new(),
            ));
            ui_state.set_status_message("Keyboard Shortcuts");
        }
        MenuAction::ChangeOrigin => {
            ui_state.active_dialog = Some(Box::new(crate::ui::dialog_origin::OriginDialog::new(
                app_state.origin,
            )));
            ui_state.set_status_message("Enter new origin (Hex)");
        }
        MenuAction::SystemSettings => {
            ui_state.active_dialog =
                Some(Box::new(crate::ui::dialog_settings::SettingsDialog::new()));
            ui_state.set_status_message("Settings");
        }
        MenuAction::NextImmediateFormat => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let has_immediate = if let Some(opcode) = &line.opcode {
                    opcode.mode == crate::cpu::AddressingMode::Immediate
                } else {
                    false
                };

                if has_immediate {
                    let val = line.bytes.get(1).copied().unwrap_or(0);
                    let current_fmt = app_state
                        .immediate_value_formats
                        .get(&line.address)
                        .copied()
                        .unwrap_or(crate::state::ImmediateFormat::Hex);

                    let next_fmt = match current_fmt {
                        crate::state::ImmediateFormat::Hex => {
                            crate::state::ImmediateFormat::InvertedHex
                        }
                        crate::state::ImmediateFormat::InvertedHex => {
                            crate::state::ImmediateFormat::Decimal
                        }
                        crate::state::ImmediateFormat::Decimal => {
                            if val <= 128 {
                                crate::state::ImmediateFormat::Binary
                            } else {
                                crate::state::ImmediateFormat::NegativeDecimal
                            }
                        }
                        crate::state::ImmediateFormat::NegativeDecimal => {
                            crate::state::ImmediateFormat::Binary
                        }
                        crate::state::ImmediateFormat::Binary => {
                            crate::state::ImmediateFormat::InvertedBinary
                        }
                        crate::state::ImmediateFormat::InvertedBinary => {
                            crate::state::ImmediateFormat::Hex
                        }
                    };

                    let command = crate::commands::Command::SetImmediateFormat {
                        address: line.address,
                        new_format: Some(next_fmt),
                        old_format: Some(current_fmt),
                    };
                    command.apply(app_state);
                    app_state.undo_stack.push(command);
                    app_state.disassemble();
                }
            }
        }
        MenuAction::PreviousImmediateFormat => {
            if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                let has_immediate = if let Some(opcode) = &line.opcode {
                    opcode.mode == crate::cpu::AddressingMode::Immediate
                } else {
                    false
                };

                if has_immediate {
                    let val = line.bytes.get(1).copied().unwrap_or(0);
                    let current_fmt = app_state
                        .immediate_value_formats
                        .get(&line.address)
                        .copied()
                        .unwrap_or(crate::state::ImmediateFormat::Hex);

                    let next_fmt = match current_fmt {
                        crate::state::ImmediateFormat::Hex => {
                            crate::state::ImmediateFormat::InvertedBinary
                        }
                        crate::state::ImmediateFormat::InvertedBinary => {
                            crate::state::ImmediateFormat::Binary
                        }
                        crate::state::ImmediateFormat::Binary => {
                            if val <= 128 {
                                crate::state::ImmediateFormat::Decimal
                            } else {
                                crate::state::ImmediateFormat::NegativeDecimal
                            }
                        }
                        crate::state::ImmediateFormat::NegativeDecimal => {
                            crate::state::ImmediateFormat::Decimal
                        }
                        crate::state::ImmediateFormat::Decimal => {
                            crate::state::ImmediateFormat::InvertedHex
                        }
                        crate::state::ImmediateFormat::InvertedHex => {
                            crate::state::ImmediateFormat::Hex
                        }
                    };

                    let command = crate::commands::Command::SetImmediateFormat {
                        address: line.address,
                        new_format: Some(next_fmt),
                        old_format: Some(current_fmt),
                    };
                    command.apply(app_state);
                    app_state.undo_stack.push(command);
                    app_state.disassemble();
                }
            }
        }
        MenuAction::SetBytesBlockByOffset { start, end } => {
            // Set block type to DataByte for a specific byte offset range
            let block_type = crate::state::BlockType::DataByte;
            let max_len = app_state.block_types.len();
            if start < max_len {
                let valid_end = end.min(max_len.saturating_sub(1));
                let range = start..(valid_end + 1);

                let old_types = app_state.block_types[range.clone()].to_vec();

                let command = crate::commands::Command::SetBlockType {
                    range: range.clone(),
                    new_type: block_type,
                    old_types,
                };

                command.apply(app_state);
                app_state.push_command(command);
                app_state.disassemble();

                let start_addr = app_state.origin.wrapping_add(start as u16);
                let end_addr = app_state.origin.wrapping_add(valid_end as u16);
                ui_state.set_status_message(format!(
                    "Set bytes block ${:04X}-${:04X} ({} bytes)",
                    start_addr,
                    end_addr,
                    valid_end - start + 1
                ));
            } else {
                ui_state.set_status_message("Error: offset out of range");
            }
        }
        MenuAction::ToggleCollapsedBlock => {
            if ui_state.active_pane == ActivePane::Blocks {
                let blocks = app_state.get_blocks_view_items();
                if let Some(idx) = ui_state.blocks_list_state.selected() {
                    if let Some(crate::state::BlockItem::Block { start, end, .. }) = blocks.get(idx)
                    {
                        let start_offset = *start as usize;
                        let end_offset = *end as usize;

                        let current_cursor_addr = app_state
                            .disassembly
                            .get(ui_state.cursor_index)
                            .map(|line| line.address);

                        // Check if already collapsed
                        if let Some(&range) = app_state
                            .collapsed_blocks
                            .iter()
                            .find(|(s, e)| *s == start_offset && *e == end_offset)
                        {
                            // Uncollapse
                            let command = crate::commands::Command::UncollapseBlock { range };
                            command.apply(app_state);
                            app_state.undo_stack.push(command);
                            app_state.disassemble();
                            ui_state.set_status_message("Block Uncollapsed");
                        } else {
                            // Collapse
                            let command = crate::commands::Command::CollapseBlock {
                                range: (start_offset, end_offset),
                            };
                            command.apply(app_state);
                            app_state.undo_stack.push(command);
                            app_state.disassemble();
                            ui_state.set_status_message("Block Collapsed");
                        }

                        // Restore cursor to the same address if possible
                        if let Some(addr) = current_cursor_addr {
                            if let Some(new_idx) = app_state.get_line_index_containing_address(addr)
                            {
                                ui_state.cursor_index = new_idx;
                            } else {
                                // Fallback
                            }
                        }
                    } else {
                        ui_state.set_status_message("Selected item is not a block");
                    }
                }
            } else {
                let cursor_addr = app_state
                    .disassembly
                    .get(ui_state.cursor_index)
                    .map(|line| line.address)
                    .unwrap_or(0);

                // First check if we are ON a collapsed block placeholder (Uncollapse case)
                if let Some(line) = app_state.disassembly.get(ui_state.cursor_index) {
                    let offset = (line.address as usize).wrapping_sub(app_state.origin as usize);
                    if let Some(&range) = app_state
                        .collapsed_blocks
                        .iter()
                        .find(|(s, _)| *s == offset)
                    {
                        let command = crate::commands::Command::UncollapseBlock { range };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);
                        app_state.disassemble();
                        ui_state.set_status_message("Block Uncollapsed");
                        return;
                    }
                }

                // If not uncollapsing, try to Collapse
                if let Some((start_addr, end_addr)) = app_state.get_block_range(cursor_addr) {
                    let start_offset =
                        (start_addr as usize).wrapping_sub(app_state.origin as usize);
                    let end_offset = (end_addr as usize).wrapping_sub(app_state.origin as usize);

                    // Check if already collapsed
                    if let Some(&range) = app_state
                        .collapsed_blocks
                        .iter()
                        .find(|(s, e)| *s == start_offset && *e == end_offset)
                    {
                        let command = crate::commands::Command::UncollapseBlock { range };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);
                        app_state.disassemble();
                        ui_state.set_status_message("Block Uncollapsed");
                    } else {
                        // Collapse
                        let command = crate::commands::Command::CollapseBlock {
                            range: (start_offset, end_offset),
                        };
                        command.apply(app_state);
                        app_state.undo_stack.push(command);

                        ui_state.selection_start = None; // clear selection if any
                        ui_state.is_visual_mode = false;
                        app_state.disassemble();
                        ui_state.set_status_message("Block Collapsed");

                        // Move cursor to start of collapsed block
                        if let Some(idx) = app_state.get_line_index_containing_address(start_addr) {
                            ui_state.cursor_index = idx;
                        }
                    }
                } else {
                    ui_state.set_status_message("No block found at cursor");
                }
            }
        }
    }
}

fn apply_block_type(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    block_type: crate::state::BlockType,
) {
    let needs_even = matches!(
        block_type,
        crate::state::BlockType::LoHi | crate::state::BlockType::HiLo
    );

    if ui_state.active_pane == ActivePane::Blocks {
        let blocks = app_state.get_blocks_view_items();
        if let Some(idx) = ui_state.blocks_list_state.selected()
            && idx < blocks.len()
            && let crate::state::BlockItem::Block { start, end, .. } = blocks[idx]
        {
            let len = (end as usize) - (start as usize) + 1;
            if needs_even && !len.is_multiple_of(2) {
                ui_state.set_status_message(format!(
                    "Error: {} requires even number of bytes",
                    block_type
                ));
                return;
            }
            app_state.set_block_type_region(block_type, Some(start as usize), end as usize);
            ui_state.set_status_message(format!("Set block type to {}", block_type));
        }
    } else if let Some(start_index) = ui_state.selection_start {
        let start = start_index.min(ui_state.cursor_index);
        let end = start_index.max(ui_state.cursor_index);
        let len = end - start + 1;

        if needs_even && len % 2 != 0 {
            ui_state.set_status_message(format!(
                "Error: {} requires even number of bytes",
                block_type
            ));
            return;
        }

        let target_address = if let Some(line) = app_state.disassembly.get(end) {
            line.address
                .wrapping_add(line.bytes.len() as u16)
                .wrapping_sub(1)
        } else {
            0
        };

        app_state.set_block_type_region(block_type, Some(start), end);
        ui_state.selection_start = None;
        ui_state.is_visual_mode = false;

        if let Some(idx) = app_state.get_line_index_containing_address(target_address) {
            ui_state.cursor_index = idx;
        }

        ui_state.set_status_message(format!("Set block type to {}", block_type));
    } else {
        // Single line
        if needs_even {
            ui_state.set_status_message(format!(
                "Error: {} requires even number of bytes",
                block_type
            ));
            return;
        }
        app_state.set_block_type_region(
            block_type,
            ui_state.selection_start,
            ui_state.cursor_index,
        );
        ui_state.set_status_message(format!("Set block type to {}", block_type));
    }
}

fn create_save_context(
    app_state: &AppState,
    ui_state: &UIState,
) -> crate::state::ProjectSaveContext {
    let cursor_addr = app_state
        .disassembly
        .get(ui_state.cursor_index)
        .map(|l| l.address);

    let hex_addr = if !app_state.raw_data.is_empty() {
        let origin = app_state.origin as usize;
        let alignment_padding = origin % 16;
        let aligned_origin = origin - alignment_padding;
        let row_start_offset = ui_state.hex_cursor_index * 16;
        let addr = aligned_origin + row_start_offset;
        Some(addr as u16)
    } else {
        None
    };

    let sprites_addr = if !app_state.raw_data.is_empty() {
        let origin = app_state.origin as usize;
        let padding = (64 - (origin % 64)) % 64;
        let sprite_offset = ui_state.sprites_cursor_index * 64;
        let addr = origin + padding + sprite_offset;
        Some(addr as u16)
    } else {
        None
    };

    let charset_addr = if !app_state.raw_data.is_empty() {
        let origin = app_state.origin as usize;
        let base_alignment = 0x400;
        let aligned_start_addr = (origin / base_alignment) * base_alignment;
        let char_offset = ui_state.charset_cursor_index * 8;
        let addr = aligned_start_addr + char_offset;
        Some(addr as u16)
    } else {
        None
    };

    let bitmap_addr = if !app_state.raw_data.is_empty() {
        let origin = app_state.origin as usize;
        // Bitmaps must be aligned to 8192-byte boundaries
        let first_aligned_addr =
            ((origin / 8192) * 8192) + if origin.is_multiple_of(8192) { 0 } else { 8192 };
        let bitmap_addr = first_aligned_addr + (ui_state.bitmap_cursor_index * 8192);
        Some(bitmap_addr as u16)
    } else {
        None
    };

    let right_pane_str = format!("{:?}", ui_state.right_pane);

    crate::state::ProjectSaveContext {
        cursor_address: cursor_addr,
        hex_dump_cursor_address: hex_addr,
        sprites_cursor_address: sprites_addr,
        right_pane_visible: Some(right_pane_str),
        charset_cursor_address: charset_addr,
        bitmap_cursor_address: bitmap_addr,
        sprite_multicolor_mode: ui_state.sprite_multicolor_mode,
        charset_multicolor_mode: ui_state.charset_multicolor_mode,
        bitmap_multicolor_mode: ui_state.bitmap_multicolor_mode,
        hexdump_view_mode: ui_state.hexdump_view_mode,
        splitters: app_state.splitters.clone(),
        blocks_view_cursor: ui_state.blocks_list_state.selected(),
    }
}

fn update_hexdump_status(ui_state: &mut UIState, mode: crate::state::HexdumpViewMode) {
    let status = match mode {
        crate::state::HexdumpViewMode::PETSCIIUnshifted => "Unshifted (PETSCII)",
        crate::state::HexdumpViewMode::PETSCIIShifted => "Shifted (PETSCII)",
        crate::state::HexdumpViewMode::ScreencodeShifted => "Shifted (Screencode)",
        crate::state::HexdumpViewMode::ScreencodeUnshifted => "Unshifted (Screencode)",
    };
    ui_state.set_status_message(format!("Hex Dump: {}", status));
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_menu_popup_bounds_panic() {
        // Create a very small terminal (20x5)
        // The default "File" menu is longer than 5 lines
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut menu_state = MenuState::new();
        menu_state.selected_category = 0; // File menu
        menu_state.active = true;

        let theme = crate::theme::Theme::default();

        // This should NOT panic with the fix
        let res = terminal.draw(|f| {
            let area = f.area();
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(area);

            let top_area = chunks[0];
            render_menu_popup(f, top_area, &menu_state, &theme);
        });

        assert!(res.is_ok());
    }
}
