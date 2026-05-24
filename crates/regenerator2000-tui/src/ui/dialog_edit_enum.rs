use crate::ui::widget::{Widget, WidgetResult, create_dialog_block};
use crate::ui_state::{AppAction, UIState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use ratatui_textarea::TextArea;
use regenerator2000_core::state::{Addr, AppState, EnumDefinition};

pub struct EditEnumDialog {
    original_def: Option<EnumDefinition>,
    is_global: bool,
    name_textarea: TextArea<'static>,
    desc_textarea: TextArea<'static>,
    variants: Vec<(String, String)>, // (Key string, Value string)
    list_state: ListState,
    active_focus: usize, // 0 = Name, 1 = Description, 2 = Variants List
    sub_dialog: Option<AddVariantDialog>,
}

impl EditEnumDialog {
    #[must_use]
    pub fn new(original_def: Option<EnumDefinition>, is_global: bool) -> Self {
        let mut name_textarea = TextArea::default();
        let mut desc_textarea = TextArea::default();
        let mut variants = Vec::new();

        name_textarea.set_cursor_line_style(Style::default());
        desc_textarea.set_cursor_line_style(Style::default());

        if let Some(def) = &original_def {
            name_textarea.insert_str(&def.name);
            if let Some(d) = &def.description {
                desc_textarea.insert_str(d);
            }
            for (k, v) in &def.variants {
                variants.push((format!("0x{k:02X}"), v.clone()));
            }
        }

        let mut list_state = ListState::default();
        if !variants.is_empty() {
            list_state.select(Some(0));
        }

        Self {
            original_def,
            is_global,
            name_textarea,
            desc_textarea,
            variants,
            list_state,
            active_focus: 0,
            sub_dialog: None,
        }
    }

    fn get_name(&self) -> String {
        self.name_textarea.lines().join("").trim().to_string()
    }

    fn get_description(&self) -> Option<String> {
        let desc = self.desc_textarea.lines().join("\n").trim().to_string();
        if desc.is_empty() { None } else { Some(desc) }
    }
}

impl Widget for EditEnumDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        if let Some(sub) = &self.sub_dialog {
            self.render_background(f, area, app_state, ui_state);
            sub.render(f, area, app_state, ui_state);
            return;
        }
        self.render_background(f, area, app_state, ui_state);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Delegate inputs to sub-dialog if active
        if let Some(sub) = &mut self.sub_dialog {
            let res = sub.handle_input(key, app_state, ui_state);
            match res {
                WidgetResult::Close => {
                    self.sub_dialog = None;
                    return WidgetResult::Handled;
                }
                WidgetResult::Action(AppAction::Confirmed(box_action)) => {
                    // Sub-dialog returns variant data inside Confirmed action wrapper
                    // We process it here!
                    if let AppAction::ApplyComment {
                        text: value,
                        address: Addr(key_num),
                        ..
                    } = *box_action
                    {
                        // Key is key_num, value is value string
                        let key_str = format!("0x{key_num:02X}");
                        // Add or update variant
                        if let Some(sel) = self.list_state.selected()
                            && self.sub_dialog.as_ref().is_some_and(|s| s.is_edit)
                        {
                            self.variants[sel] = (key_str, value);
                        } else {
                            self.variants.push((key_str, value));
                            self.list_state.select(Some(self.variants.len() - 1));
                        }
                    }
                    self.sub_dialog = None;
                    return WidgetResult::Handled;
                }
                _ => return WidgetResult::Handled,
            }
        }

        match key.code {
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Tab => {
                self.active_focus = (self.active_focus + 1) % 3;
                WidgetResult::Handled
            }
            KeyCode::BackTab => {
                self.active_focus = if self.active_focus == 0 {
                    2
                } else {
                    self.active_focus - 1
                };
                WidgetResult::Handled
            }
            KeyCode::Up | KeyCode::Char('k')
                if self.active_focus == 2 && key.modifiers == KeyModifiers::empty() =>
            {
                if !self.variants.is_empty() {
                    let selected = self.list_state.selected().unwrap_or(0);
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                }
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.active_focus == 2 && key.modifiers == KeyModifiers::empty() =>
            {
                let len = self.variants.len();
                if len > 0 {
                    let selected = self.list_state.selected().unwrap_or(0);
                    if selected + 1 < len {
                        self.list_state.select(Some(selected + 1));
                    }
                }
                WidgetResult::Handled
            }
            // Add Variant row
            KeyCode::Char('a') if self.active_focus == 2 => {
                self.sub_dialog = Some(AddVariantDialog::new(None, None));
                WidgetResult::Handled
            }
            // Edit Variant row
            KeyCode::Char('e') | KeyCode::Enter if self.active_focus == 2 => {
                if let Some(sel) = self.list_state.selected()
                    && sel < self.variants.len()
                {
                    let (k, v) = &self.variants[sel];
                    self.sub_dialog = Some(AddVariantDialog::new(Some(k), Some(v)));
                }
                WidgetResult::Handled
            }
            // Delete Variant row
            KeyCode::Char('d') | KeyCode::Delete if self.active_focus == 2 => {
                if let Some(sel) = self.list_state.selected()
                    && sel < self.variants.len()
                {
                    self.variants.remove(sel);
                    let len = self.variants.len();
                    if len > 0 {
                        self.list_state.select(Some(sel.min(len - 1)));
                    } else {
                        self.list_state.select(None);
                    }
                }
                WidgetResult::Handled
            }
            // Save everything
            KeyCode::Char('s') if key.modifiers == KeyModifiers::CONTROL => {
                self.save_enum(app_state, ui_state)
            }
            _ => {
                // Forward text inputs
                if self.active_focus == 0 {
                    self.name_textarea.input(key);
                } else if self.active_focus == 1 {
                    self.desc_textarea.input(key);
                }
                WidgetResult::Handled
            }
        }
    }
}

impl EditEnumDialog {
    fn save_enum(&self, app_state: &AppState, ui_state: &mut UIState) -> WidgetResult {
        let name = self.get_name();
        // Validate Name:
        // If creating or renaming, validate that name is unique and alphanumeric.
        let is_rename = self.original_def.as_ref().is_none_or(|d| d.name != name);
        if is_rename && let Err(err) = app_state.validate_new_enum_name(&name) {
            ui_state.set_status_message(format!("Error: {err}"));
            return WidgetResult::Handled;
        }

        // Validate variants
        let mut variants_map = std::collections::BTreeMap::new();
        for (k_str, v_str) in &self.variants {
            let k_trimmed = k_str.trim();
            let parsed_val = if let Some(hex) = k_trimmed
                .strip_prefix("0x")
                .or_else(|| k_trimmed.strip_prefix("0X"))
            {
                u16::from_str_radix(hex, 16)
            } else if let Some(hex) = k_trimmed.strip_prefix('$') {
                u16::from_str_radix(hex, 16)
            } else {
                k_trimmed.parse::<u16>()
            };

            match parsed_val {
                Ok(val) => {
                    variants_map.insert(val, v_str.clone());
                }
                Err(_) => {
                    ui_state.set_status_message(format!("Error: Invalid variant key '{k_str}'"));
                    return WidgetResult::Handled;
                }
            }
        }

        let new_def = EnumDefinition {
            name: name.clone(),
            description: self.get_description(),
            variants: variants_map,
            source_file: self
                .original_def
                .as_ref()
                .and_then(|d| d.source_file.clone()),
        };

        // Dispatch appropriate save action
        let rename_from = self.original_def.as_ref().map(|d| d.name.clone());
        if self.is_global {
            WidgetResult::Action(AppAction::ApplyGlobalEnumDefinition {
                name,
                definition: Some(new_def),
                rename_from,
            })
        } else {
            WidgetResult::Action(AppAction::ApplyEnumDefinition {
                name: name.clone(),
                definition: Some(new_def),
                rename_from,
            })
        }
    }

    fn render_background(
        &self,
        f: &mut Frame,
        area: Rect,
        _app_state: &AppState,
        ui_state: &mut UIState,
    ) {
        let theme = &ui_state.theme;
        let mode_str = if self.original_def.is_some() {
            "Edit"
        } else {
            "Add"
        };
        let scope_str = if self.is_global { "Global" } else { "Project" };
        let title = format!(" {mode_str} {scope_str} Enum ");
        let block = create_dialog_block(&title, theme);

        let area = crate::utils::centered_rect_adaptive(60, 65, 0, 22, area);
        ui_state.active_dialog_area = area;

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        // Layout: name input (3 lines), description input (5 lines), variants list (remaining)
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Name input
                Constraint::Length(5), // Description input
                Constraint::Min(0),    // Variants list table
                Constraint::Length(1), // Helpers
            ])
            .split(inner);

        let name_area = layout[0];
        let desc_area = layout[1];
        let vars_area = layout[2];
        let helpers_area = layout[3];

        // Render Name Input
        let mut name_ta = self.name_textarea.clone();
        let name_border_fg = if self.active_focus == 0 {
            theme.highlight_fg
        } else {
            theme.border_inactive
        };
        name_ta.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(name_border_fg))
                .title(" Enum Name "),
        );
        name_ta.set_style(Style::default().fg(theme.dialog_fg));
        f.render_widget(&name_ta, name_area);

        // Render Description Input
        let mut desc_ta = self.desc_textarea.clone();
        let desc_border_fg = if self.active_focus == 1 {
            theme.highlight_fg
        } else {
            theme.border_inactive
        };
        desc_ta.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(desc_border_fg))
                .title(" Description "),
        );
        desc_ta.set_style(Style::default().fg(theme.dialog_fg));
        f.render_widget(&desc_ta, desc_area);

        // Render Variants list
        let vars_border_fg = if self.active_focus == 2 {
            theme.highlight_fg
        } else {
            theme.border_inactive
        };
        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(vars_border_fg))
            .title(" Variants ($XX = NAME) ");

        let list_inner = list_block.inner(vars_area);
        f.render_widget(list_block, vars_area);

        let selected_idx = self.list_state.selected().unwrap_or(0);
        let list_items: Vec<ListItem> = self
            .variants
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                let style = if i == selected_idx && self.active_focus == 2 {
                    Style::default()
                        .bg(theme.selection_bg)
                        .fg(theme.selection_fg)
                } else {
                    Style::default().fg(theme.dialog_fg)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("  {k} "), Style::default().fg(theme.block_code_fg)),
                    Span::styled(" = ", Style::default().fg(theme.border_inactive)),
                    Span::styled(v, Style::default().add_modifier(Modifier::BOLD)),
                ]))
                .style(style)
            })
            .collect();

        let list = List::new(list_items);
        let mut list_state_mut = self.list_state;
        f.render_stateful_widget(list, list_inner, &mut list_state_mut);

        // Render Helpers
        let helpers_str = match self.active_focus {
            0 | 1 => " [Tab] Focus Next  [Ctrl+S] Save  [Esc] Cancel ",
            2 => {
                " [a] Add Var  [e/Enter] Edit Var  [d/Del] Delete Var  [Tab] Focus Next  [Ctrl+S] Save "
            }
            _ => "",
        };
        let helpers = Paragraph::new(helpers_str)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(theme.highlight_fg));
        f.render_widget(helpers, helpers_area);
    }
}

// =============================================================================
// Nested Add/Edit Variant row Dialog
// =============================================================================
pub struct AddVariantDialog {
    is_edit: bool,
    key_ta: TextArea<'static>,
    val_ta: TextArea<'static>,
    active_focus: usize, // 0 = Key, 1 = Value
}

impl AddVariantDialog {
    fn new(initial_key: Option<&str>, initial_val: Option<&str>) -> Self {
        let mut key_ta = TextArea::default();
        let mut val_ta = TextArea::default();
        key_ta.set_cursor_line_style(Style::default());
        val_ta.set_cursor_line_style(Style::default());

        let is_edit = initial_key.is_some();
        if let Some(k) = initial_key {
            key_ta.insert_str(k);
        }
        if let Some(v) = initial_val {
            val_ta.insert_str(v);
        }

        Self {
            is_edit,
            key_ta,
            val_ta,
            active_focus: if is_edit { 1 } else { 0 }, // Start focused on Val if editing
        }
    }
}

impl Widget for AddVariantDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let title = if self.is_edit {
            " Edit Variant "
        } else {
            " Add Variant "
        };
        let block = create_dialog_block(title, theme);

        // Center a smaller nested dialog box
        let area = crate::utils::centered_rect_adaptive(45, 30, 0, 10, area);

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Key input
                Constraint::Length(3), // Value input
                Constraint::Length(1), // Helper
            ])
            .split(inner);

        let mut key_textarea = self.key_ta.clone();
        let key_border_fg = if self.active_focus == 0 {
            theme.highlight_fg
        } else {
            theme.border_inactive
        };
        key_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(key_border_fg))
                .title(" Key (e.g., 0x01, $0A, 15) "),
        );
        key_textarea.set_style(Style::default().fg(theme.dialog_fg));
        f.render_widget(&key_textarea, layout[0]);

        let mut val_textarea = self.val_ta.clone();
        let val_border_fg = if self.active_focus == 1 {
            theme.highlight_fg
        } else {
            theme.border_inactive
        };
        val_textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(val_border_fg))
                .title(" Variant Name "),
        );
        val_textarea.set_style(Style::default().fg(theme.dialog_fg));
        f.render_widget(&val_textarea, layout[1]);

        let helpers = Paragraph::new(" [Tab] Next  [Enter] Apply  [Esc] Cancel ")
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(theme.highlight_fg));
        f.render_widget(helpers, layout[2]);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => WidgetResult::Close,
            KeyCode::Tab => {
                self.active_focus = (self.active_focus + 1) % 2;
                WidgetResult::Handled
            }
            KeyCode::BackTab => {
                self.active_focus = if self.active_focus == 0 { 1 } else { 0 };
                WidgetResult::Handled
            }
            KeyCode::Enter => {
                let key_str = self.key_ta.lines().join("").trim().to_string();
                let val_str = self.val_ta.lines().join("").trim().to_string();

                if key_str.is_empty() || val_str.is_empty() {
                    ui_state.set_status_message("Error: Both fields must be filled.");
                    return WidgetResult::Handled;
                }

                // Quick validation of variant key format
                let k_trimmed = key_str.trim();
                let parsed_val = if let Some(hex) = k_trimmed
                    .strip_prefix("0x")
                    .or_else(|| k_trimmed.strip_prefix("0X"))
                {
                    u16::from_str_radix(hex, 16)
                } else if let Some(hex) = k_trimmed.strip_prefix('$') {
                    u16::from_str_radix(hex, 16)
                } else {
                    k_trimmed.parse::<u16>()
                };

                match parsed_val {
                    Ok(val) => {
                        // We package it inside a dummy AppAction::ApplyComment to carry key and value string back.
                        // Key goes to `address` (represented as Addr), value goes to `text`.
                        WidgetResult::Action(AppAction::Confirmed(Box::new(
                            AppAction::ApplyComment {
                                address: Addr(val),
                                text: val_str,
                                kind: regenerator2000_core::state::types::CommentKind::Side, // placeholder
                            },
                        )))
                    }
                    Err(_) => {
                        ui_state.set_status_message(
                            "Error: Key must be a valid number (dec or hex 0x/$).",
                        );
                        WidgetResult::Handled
                    }
                }
            }
            _ => {
                if self.active_focus == 0 {
                    self.key_ta.input(key);
                } else {
                    self.val_ta.input(key);
                }
                WidgetResult::Handled
            }
        }
    }
}
