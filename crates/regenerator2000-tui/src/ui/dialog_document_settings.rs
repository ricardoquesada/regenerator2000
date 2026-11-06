use crate::assets::SystemConfig;
use crate::state::{AppState, Assembler, DocumentSettings, System};
use crate::theme::Theme;
use crate::ui::widget::{Widget, WidgetResult};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph};

/// Numeric document settings that support inline text editing and `Left`/`Right` stepping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericSetting {
    MaxXrefCount,
    MaxArrowColumns,
    TextCharLimit,
    AddressesPerLine,
    BytesPerLine,
    FillRunThreshold,
}

impl NumericSetting {
    /// Display label shown in the row and in the inline edit popup title.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MaxXrefCount => "Max X-Refs",
            Self::MaxArrowColumns => "Arrow Columns",
            Self::TextCharLimit => "Text Line Limit",
            Self::AddressesPerLine => "Words/Addrs per line",
            Self::BytesPerLine => "Bytes per line",
            Self::FillRunThreshold => "Fill run threshold",
        }
    }

    /// Inclusive `(min, max)` bounds allowed for this setting.
    #[must_use]
    pub const fn bounds(self) -> (usize, usize) {
        match self {
            Self::MaxXrefCount => (0, 40),
            Self::MaxArrowColumns => (1, 10),
            Self::TextCharLimit => (1, 80),
            Self::AddressesPerLine => (1, 8),
            Self::BytesPerLine => (1, 40),
            Self::FillRunThreshold => (0, 64),
        }
    }

    /// Reads the current value of this setting from [`DocumentSettings`].
    #[must_use]
    pub const fn get(self, settings: &DocumentSettings) -> usize {
        match self {
            Self::MaxXrefCount => settings.max_xref_count,
            Self::MaxArrowColumns => settings.max_arrow_columns,
            Self::TextCharLimit => settings.text_char_limit,
            Self::AddressesPerLine => settings.addresses_per_line,
            Self::BytesPerLine => settings.bytes_per_line,
            Self::FillRunThreshold => settings.fill_run_threshold,
        }
    }

    const fn field_mut(self, settings: &mut DocumentSettings) -> &mut usize {
        match self {
            Self::MaxXrefCount => &mut settings.max_xref_count,
            Self::MaxArrowColumns => &mut settings.max_arrow_columns,
            Self::TextCharLimit => &mut settings.text_char_limit,
            Self::AddressesPerLine => &mut settings.addresses_per_line,
            Self::BytesPerLine => &mut settings.bytes_per_line,
            Self::FillRunThreshold => &mut settings.fill_run_threshold,
        }
    }

    /// Writes `val` to the corresponding field in [`DocumentSettings`].
    pub fn set(self, settings: &mut DocumentSettings, val: usize) {
        *self.field_mut(settings) = val;
    }

    /// Formats the current value for display when not actively being edited.
    #[must_use]
    pub fn display_value(self, settings: &DocumentSettings) -> String {
        let val = self.get(settings);
        if matches!(self, Self::FillRunThreshold) && val == 0 {
            "off".to_string()
        } else {
            val.to_string()
        }
    }

    /// Adjusts the setting by `delta` within [`Self::bounds`], returning `true` if the value changed.
    pub fn step(self, settings: &mut DocumentSettings, delta: isize) -> bool {
        let (min, max) = self.bounds();
        let field = self.field_mut(settings);
        let current = *field;
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.cast_unsigned())
        }
        .clamp(min, max);

        if next == current {
            return false;
        }
        *field = next;
        true
    }
}

/// Boolean checkbox settings in [`DocumentSettingsDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolSetting {
    AllLabels,
    PreserveLongBytes,
    BrkSingleByte,
    PatchBrk,
    UseIllegalOpcodes,
    AutoAnalyze,
    ExcludeWellKnownLabels,
    ShowSystemComments,
}

impl BoolSetting {
    #[must_use]
    const fn label(self) -> &'static str {
        match self {
            Self::AllLabels => "Display External Labels at top",
            Self::PreserveLongBytes => "Preserve long bytes (@w, +2, .abs, etc)",
            Self::BrkSingleByte => "BRK single byte",
            Self::PatchBrk => "Patch BRK",
            Self::UseIllegalOpcodes => "Use Illegal Opcodes",
            Self::AutoAnalyze => "Auto-generate Labels & Cross-refs",
            Self::ExcludeWellKnownLabels => "Exclude well-known addresses from auto-labels",
            Self::ShowSystemComments => "Show system comments",
        }
    }

    #[must_use]
    const fn get(self, settings: &DocumentSettings) -> bool {
        match self {
            Self::AllLabels => settings.all_labels,
            Self::PreserveLongBytes => settings.preserve_long_bytes,
            Self::BrkSingleByte => settings.brk_single_byte,
            Self::PatchBrk => settings.patch_brk,
            Self::UseIllegalOpcodes => settings.use_illegal_opcodes,
            Self::AutoAnalyze => settings.auto_analyze,
            Self::ExcludeWellKnownLabels => settings.exclude_well_known_labels,
            Self::ShowSystemComments => settings.show_system_comments,
        }
    }

    const fn field_mut(self, settings: &mut DocumentSettings) -> &mut bool {
        match self {
            Self::AllLabels => &mut settings.all_labels,
            Self::PreserveLongBytes => &mut settings.preserve_long_bytes,
            Self::BrkSingleByte => &mut settings.brk_single_byte,
            Self::PatchBrk => &mut settings.patch_brk,
            Self::UseIllegalOpcodes => &mut settings.use_illegal_opcodes,
            Self::AutoAnalyze => &mut settings.auto_analyze,
            Self::ExcludeWellKnownLabels => &mut settings.exclude_well_known_labels,
            Self::ShowSystemComments => &mut settings.show_system_comments,
        }
    }

    #[must_use]
    fn is_disabled(self, settings: &DocumentSettings) -> bool {
        matches!(self, Self::PatchBrk)
            && (settings.brk_single_byte
                || matches!(settings.assembler, Assembler::Kick | Assembler::Ca65))
    }

    fn toggle(self, app_state: &mut AppState, ui_state: &mut UIState) {
        if self.is_disabled(&app_state.settings) {
            return;
        }
        let field = self.field_mut(&mut app_state.settings);
        *field = !*field;

        match self {
            Self::AllLabels | Self::PreserveLongBytes => {
                DocumentSettingsDialog::sync_disassembly(app_state, ui_state, false, false);
            }
            Self::BrkSingleByte => {
                if app_state.settings.brk_single_byte {
                    app_state.settings.patch_brk = false;
                } else if matches!(
                    app_state.settings.assembler,
                    Assembler::Kick | Assembler::Ca65
                ) {
                    app_state.settings.patch_brk = true;
                }
                DocumentSettingsDialog::sync_disassembly(app_state, ui_state, false, true);
            }
            Self::PatchBrk | Self::UseIllegalOpcodes => {
                DocumentSettingsDialog::sync_disassembly(app_state, ui_state, false, true);
            }
            Self::AutoAnalyze => {
                if app_state.settings.auto_analyze {
                    DocumentSettingsDialog::sync_disassembly(app_state, ui_state, false, true);
                } else {
                    DocumentSettingsDialog::clear_auto_analysis(app_state, ui_state);
                }
            }
            Self::ExcludeWellKnownLabels => {
                DocumentSettingsDialog::sync_disassembly(app_state, ui_state, true, true);
            }
            Self::ShowSystemComments => {
                DocumentSettingsDialog::sync_disassembly(app_state, ui_state, true, false);
            }
        }
    }
}

/// Table-driven descriptor for a single interactive row in [`DocumentSettingsDialog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowDescriptor {
    Checkbox(BoolSetting),
    Description,
    Numeric(NumericSetting),
    Assembler,
    System,
    SystemFeature(usize),
}

impl RowDescriptor {
    /// Number of vertical terminal rows consumed by this descriptor (including any section header).
    #[must_use]
    const fn visual_height(self) -> u16 {
        match self {
            Self::Description | Self::SystemFeature(0) => 2,
            Self::Checkbox(_)
            | Self::Numeric(_)
            | Self::Assembler
            | Self::System
            | Self::SystemFeature(_) => 1,
        }
    }
}

/// Static table of the 15 fixed setting rows preceding any platform-specific [`SystemConfig`] rows.
const FIXED_ROWS: &[RowDescriptor] = &[
    RowDescriptor::Checkbox(BoolSetting::AllLabels),
    RowDescriptor::Checkbox(BoolSetting::PreserveLongBytes),
    RowDescriptor::Checkbox(BoolSetting::BrkSingleByte),
    RowDescriptor::Checkbox(BoolSetting::PatchBrk),
    RowDescriptor::Checkbox(BoolSetting::UseIllegalOpcodes),
    RowDescriptor::Checkbox(BoolSetting::AutoAnalyze),
    RowDescriptor::Description,
    RowDescriptor::Numeric(NumericSetting::MaxXrefCount),
    RowDescriptor::Numeric(NumericSetting::MaxArrowColumns),
    RowDescriptor::Numeric(NumericSetting::TextCharLimit),
    RowDescriptor::Numeric(NumericSetting::AddressesPerLine),
    RowDescriptor::Numeric(NumericSetting::BytesPerLine),
    RowDescriptor::Numeric(NumericSetting::FillRunThreshold),
    RowDescriptor::Assembler,
    RowDescriptor::System,
];

const SYSTEM_ROW_INDEX: usize = FIXED_ROWS.len() - 1;

pub struct DocumentSettingsDialog {
    pub selected_index: usize,
    pub is_selecting_system: bool,
    pub is_selecting_assembler: bool,
    pub editing_number: Option<(NumericSetting, String)>,
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
            editing_number: None,
            is_editing_description: false,
            description_input: ratatui_textarea::TextArea::default(),
        }
    }

    fn active_rows(system_config: &SystemConfig) -> Vec<RowDescriptor> {
        let mut rows = Vec::with_capacity(FIXED_ROWS.len() + system_config.features.len() + 2);
        rows.extend_from_slice(FIXED_ROWS);
        for i in 0..system_config.features.len() {
            rows.push(RowDescriptor::SystemFeature(i));
        }
        if system_config.has_excludes {
            rows.push(RowDescriptor::Checkbox(BoolSetting::ExcludeWellKnownLabels));
        }
        if system_config.has_comments {
            rows.push(RowDescriptor::Checkbox(BoolSetting::ShowSystemComments));
        }
        rows
    }

    fn row_style(selected: bool, theme: &Theme) -> Style {
        if selected {
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg)
        }
    }

    fn checkbox_span(
        indent: usize,
        label: &str,
        checked: bool,
        selected: bool,
        disabled: bool,
        theme: &Theme,
    ) -> Span<'static> {
        let check_char = if checked { "[X]" } else { "[ ]" };
        let prefix = " ".repeat(indent);
        let style = if disabled {
            let mut s = Style::default()
                .fg(theme.menu_disabled_fg)
                .add_modifier(Modifier::ITALIC);
            if selected {
                s = s.add_modifier(Modifier::BOLD);
            }
            s
        } else {
            Self::row_style(selected, theme)
        };
        Span::styled(format!("{prefix}{check_char} {label}"), style)
    }

    fn render_line(f: &mut Frame, inner: Rect, y: u16, widget: Paragraph<'_>) {
        let width = inner.width.saturating_sub(4);
        if y < inner.bottom() && width > 0 {
            f.render_widget(widget, Rect::new(inner.x.saturating_add(2), y, width, 1));
        }
    }

    fn render_selection_popup(
        f: &mut Frame,
        popup_area: Rect,
        title: &str,
        options: &[String],
        selected_idx: usize,
        theme: &Theme,
    ) {
        f.render_widget(Clear, popup_area);
        let block = crate::ui::widget::create_dialog_block(title, theme);
        let list_items: Vec<ListItem> = options
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let style = if i == selected_idx {
                    Style::default()
                        .bg(theme.menu_selected_bg)
                        .fg(theme.menu_selected_fg)
                } else {
                    Style::default().bg(theme.menu_bg).fg(theme.menu_fg)
                };
                ListItem::new(label.clone()).style(style)
            })
            .collect();

        let mut list_state = ListState::default();
        list_state.select(Some(selected_idx));

        let list = List::new(list_items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        f.render_stateful_widget(list, popup_area, &mut list_state);
    }

    fn cycle_assembler(app_state: &mut AppState, ui_state: &mut UIState, delta: isize) {
        let assemblers = Assembler::all();
        if assemblers.is_empty() {
            return;
        }
        let current_idx = assemblers
            .iter()
            .position(|a| *a == app_state.settings.assembler)
            .unwrap_or(0);
        let new_idx = if delta < 0 {
            (current_idx + assemblers.len() - 1) % assemblers.len()
        } else {
            (current_idx + 1) % assemblers.len()
        };
        app_state.settings.assembler = assemblers[new_idx];
        if matches!(
            app_state.settings.assembler,
            Assembler::Kick | Assembler::Ca65
        ) && !app_state.settings.brk_single_byte
        {
            app_state.settings.patch_brk = true;
        }
        Self::sync_disassembly(app_state, ui_state, false, false);
    }

    fn cycle_system(&mut self, app_state: &mut AppState, ui_state: &mut UIState, delta: isize) {
        let systems = crate::assets::get_available_systems();
        if systems.is_empty() {
            return;
        }
        let current_idx = systems
            .iter()
            .position(|p| app_state.settings.system == *p)
            .unwrap_or(0);
        let new_idx = if delta < 0 {
            (current_idx + systems.len() - 1) % systems.len()
        } else {
            (current_idx + 1) % systems.len()
        };
        app_state.settings.system = System::from(systems[new_idx].clone());
        app_state.settings.enabled_features.clear();
        self.selected_index = SYSTEM_ROW_INDEX;
        Self::sync_disassembly(app_state, ui_state, true, true);
    }

    fn clear_auto_analysis(app_state: &mut AppState, ui_state: &mut UIState) {
        let restore_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map(|l| l.address);
        let old_sub_cursor = ui_state.sub_cursor_index;

        let mut new_labels = app_state.labels.clone();
        for labels_vec in new_labels.values_mut() {
            labels_vec.retain(|l| l.kind != crate::state::LabelKind::Auto);
        }
        new_labels.retain(|_, v| !v.is_empty());

        let cmd = crate::commands::Command::SetAnalysisData {
            labels: new_labels,
            cross_refs: std::collections::BTreeMap::new(),
            old_labels: app_state.labels.clone(),
            old_cross_refs: app_state.cross_refs.clone(),
        };
        cmd.apply(app_state);
        app_state.push_command(cmd);
        app_state.disassemble();
        Self::restore_cursor(app_state, ui_state, restore_addr, old_sub_cursor);
    }

    fn sync_disassembly(
        app_state: &mut AppState,
        ui_state: &mut UIState,
        reload_assets: bool,
        reanalyze: bool,
    ) {
        let restore_addr = app_state
            .disassembly
            .get(ui_state.cursor_index)
            .map(|l| l.address);
        let old_sub_cursor = ui_state.sub_cursor_index;

        if reload_assets {
            app_state.load_system_assets();
        }
        if reanalyze && app_state.settings.auto_analyze {
            let (cmd, _) = app_state.perform_analysis();
            app_state.push_command(cmd);
        } else {
            app_state.disassemble();
        }

        Self::restore_cursor(app_state, ui_state, restore_addr, old_sub_cursor);
    }

    fn restore_cursor(
        app_state: &AppState,
        ui_state: &mut UIState,
        restore_addr: Option<crate::state::Addr>,
        old_sub_cursor: usize,
    ) {
        if let Some(addr) = restore_addr
            && let Some(new_idx) = app_state.get_line_index_for_address(addr)
        {
            ui_state.cursor_index = new_idx;
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
    }
}

impl Widget for DocumentSettingsDialog {
    fn render(&self, f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Document Settings ", theme);

        let area = crate::utils::centered_rect_adaptive(60, 60, 60, 28, area);
        ui_state.active_dialog_area = area;
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);

        let inner = block.inner(area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let settings = &app_state.settings;
        let system_config = crate::assets::load_system_config(settings.system.as_str());
        let rows = Self::active_rows(&system_config);

        let required_rows: u16 = rows.iter().copied().map(RowDescriptor::visual_height).sum();
        let spacer_len = u16::from(inner.height > required_rows);

        let mut y = inner.y;
        for (idx, &row) in rows.iter().enumerate() {
            let selected = self.selected_index == idx;
            let style = Self::row_style(selected, theme);

            match row {
                RowDescriptor::Checkbox(setting) => {
                    let span = Self::checkbox_span(
                        0,
                        setting.label(),
                        setting.get(settings),
                        selected,
                        setting.is_disabled(settings),
                        theme,
                    );
                    Self::render_line(f, inner, y, Paragraph::new(span));
                    y = y.saturating_add(1);
                }
                RowDescriptor::Description => {
                    y = y.saturating_add(spacer_len);
                    let title = if self.is_editing_description {
                        "Description (Enter to save):"
                    } else {
                        "Description:"
                    };
                    Self::render_line(f, inner, y, Paragraph::new(title).style(style));
                    y = y.saturating_add(1);

                    let width = inner.width.saturating_sub(4);
                    if y < inner.bottom() && width > 0 {
                        let desc_rect = Rect::new(inner.x.saturating_add(2), y, width, 1);
                        if self.is_editing_description {
                            let mut textarea = self.description_input.clone();
                            textarea.set_style(
                                Style::default()
                                    .fg(theme.highlight_fg)
                                    .bg(theme.menu_selected_bg),
                            );
                            textarea.set_cursor_style(
                                Style::default().add_modifier(Modifier::REVERSED),
                            );
                            textarea.set_cursor_line_style(Style::default());
                            f.render_widget(&textarea, desc_rect);
                        } else {
                            let value = if settings.description.is_empty() {
                                "(empty)"
                            } else {
                                settings.description.as_str()
                            };
                            f.render_widget(Paragraph::new(value).style(style), desc_rect);
                        }
                    }
                    y = y.saturating_add(1);
                }
                RowDescriptor::Numeric(setting) => {
                    let value_str = match &self.editing_number {
                        Some((active, input)) if *active == setting => input.clone(),
                        _ => setting.display_value(settings),
                    };
                    let text = format!("{}: < {value_str} >", setting.label());
                    Self::render_line(f, inner, y, Paragraph::new(text).style(style));
                    y = y.saturating_add(1);
                }
                RowDescriptor::Assembler | RowDescriptor::System => {
                    let text = if matches!(row, RowDescriptor::Assembler) {
                        format!("Assembler: < {} >", settings.assembler)
                    } else {
                        format!("Target System: < {} >", settings.system)
                    };
                    Self::render_line(f, inner, y, Paragraph::new(text).style(style));
                    y = y.saturating_add(1);
                }
                RowDescriptor::SystemFeature(feat_idx) => {
                    if feat_idx == 0 {
                        Self::render_line(
                            f,
                            inner,
                            y,
                            Paragraph::new(Span::styled(
                                "System Labels:",
                                Style::default().add_modifier(Modifier::BOLD),
                            )),
                        );
                        y = y.saturating_add(1);
                    }
                    if let Some(feature) = system_config.features.get(feat_idx) {
                        let is_enabled = settings
                            .enabled_features
                            .get(&feature.id)
                            .copied()
                            .unwrap_or(feature.default);
                        let span = Self::checkbox_span(
                            2,
                            &feature.name,
                            is_enabled,
                            selected,
                            false,
                            theme,
                        );
                        Self::render_line(f, inner, y, Paragraph::new(span));
                        y = y.saturating_add(1);
                    }
                }
            }
        }

        if self.is_selecting_system {
            let systems = crate::assets::get_available_systems();
            let selected_idx = systems
                .iter()
                .position(|p| settings.system == *p)
                .unwrap_or(0);
            let popup_area = crate::utils::centered_rect_adaptive(40, 50, 50, 10, area);
            Self::render_selection_popup(
                f,
                popup_area,
                " Select System ",
                &systems,
                selected_idx,
                theme,
            );
        }

        if self.is_selecting_assembler {
            let assemblers = Assembler::all();
            let options: Vec<String> = assemblers.iter().map(ToString::to_string).collect();
            let selected_idx = assemblers
                .iter()
                .position(|a| *a == settings.assembler)
                .unwrap_or(0);
            let popup_area = crate::utils::centered_rect_adaptive(40, 30, 30, 8, area);
            Self::render_selection_popup(
                f,
                popup_area,
                " Select Assembler ",
                &options,
                selected_idx,
                theme,
            );
        }

        if let Some((setting, value_str)) = &self.editing_number {
            let popup_area = crate::utils::centered_rect_adaptive(30, 40, 0, 3, area);
            f.render_widget(Clear, popup_area);
            let title = format!(" Edit {} ", setting.label());
            let block = crate::ui::widget::create_dialog_block(&title, theme);

            let widget = Paragraph::new(value_str.as_str())
                .style(
                    Style::default()
                        .fg(theme.highlight_fg)
                        .add_modifier(Modifier::BOLD),
                )
                .block(block);
            f.render_widget(widget, popup_area);

            if popup_area.width > 2 && popup_area.height > 1 {
                let cursor_x = (popup_area.x + 1 + value_str.chars().count() as u16)
                    .min(popup_area.right().saturating_sub(2));
                f.set_cursor_position((cursor_x, popup_area.y + 1));
            }
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        if self.is_editing_description {
            match key.code {
                KeyCode::Esc => {
                    self.is_editing_description = false;
                    self.description_input = ratatui_textarea::TextArea::default();
                    ui_state.set_status_message("Ready");
                }
                KeyCode::Enter => {
                    app_state.settings.description =
                        self.description_input.lines().join("").trim().to_string();
                    self.is_editing_description = false;
                }
                _ => {
                    self.description_input.input(key);
                }
            }
            return WidgetResult::Handled;
        }

        if let Some((setting, ref mut input)) = self.editing_number {
            match key.code {
                KeyCode::Esc => {
                    self.editing_number = None;
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let (min, max) = setting.bounds();
                    if let Ok(val) = input.parse::<usize>()
                        && (min..=max).contains(&val)
                    {
                        setting.set(&mut app_state.settings, val);
                        self.editing_number = None;
                        Self::sync_disassembly(app_state, ui_state, false, false);
                    } else {
                        ui_state.set_status_message(format!("Enter a value from {min} to {max}"));
                        input.clear();
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if c.is_ascii_digit() && input.len() < 4 => {
                    input.push(c);
                }
                _ => {}
            }
            return WidgetResult::Handled;
        }

        if self.is_selecting_system {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.is_selecting_system = false;
                }
                KeyCode::Up => self.cycle_system(app_state, ui_state, -1),
                KeyCode::Down => self.cycle_system(app_state, ui_state, 1),
                _ => {}
            }
            return WidgetResult::Handled;
        }

        if self.is_selecting_assembler {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.is_selecting_assembler = false;
                }
                KeyCode::Up => Self::cycle_assembler(app_state, ui_state, -1),
                KeyCode::Down => Self::cycle_assembler(app_state, ui_state, 1),
                _ => {}
            }
            return WidgetResult::Handled;
        }

        let system_config = crate::assets::load_system_config(app_state.settings.system.as_str());
        let rows = Self::active_rows(&system_config);
        let total_items = rows.len();
        let selected_row = rows.get(self.selected_index).copied();

        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                Self::sync_disassembly(app_state, ui_state, false, false);
                return WidgetResult::Close;
            }
            KeyCode::Up => {
                if self.selected_index == 0 {
                    self.selected_index = total_items - 1;
                } else {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down => {
                self.selected_index = (self.selected_index + 1) % total_items;
            }
            KeyCode::Left => match selected_row {
                Some(RowDescriptor::Numeric(setting)) => {
                    if setting.step(&mut app_state.settings, -1) {
                        Self::sync_disassembly(app_state, ui_state, false, false);
                    }
                }
                Some(RowDescriptor::Assembler) => Self::cycle_assembler(app_state, ui_state, -1),
                Some(RowDescriptor::System) => self.cycle_system(app_state, ui_state, -1),
                _ => {}
            },
            KeyCode::Right => match selected_row {
                Some(RowDescriptor::Numeric(setting)) => {
                    if setting.step(&mut app_state.settings, 1) {
                        Self::sync_disassembly(app_state, ui_state, false, false);
                    }
                }
                Some(RowDescriptor::Assembler) => Self::cycle_assembler(app_state, ui_state, 1),
                Some(RowDescriptor::System) => self.cycle_system(app_state, ui_state, 1),
                _ => {}
            },
            KeyCode::Enter | KeyCode::Char(' ') => match selected_row {
                Some(RowDescriptor::Checkbox(setting)) => setting.toggle(app_state, ui_state),
                Some(RowDescriptor::Description) => {
                    self.is_editing_description = true;
                    let mut textarea = ratatui_textarea::TextArea::default();
                    textarea.insert_str(&app_state.settings.description);
                    textarea.move_cursor(ratatui_textarea::CursorMove::End);
                    self.description_input = textarea;
                }
                Some(RowDescriptor::Numeric(setting)) => {
                    let current = setting.get(&app_state.settings).to_string();
                    self.editing_number = Some((setting, current));
                }
                Some(RowDescriptor::Assembler) => {
                    self.is_selecting_assembler = true;
                }
                Some(RowDescriptor::System) => {
                    self.is_selecting_system = true;
                }
                Some(RowDescriptor::SystemFeature(feat_idx)) => {
                    if let Some(feature) = system_config.features.get(feat_idx) {
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
                        Self::sync_disassembly(app_state, ui_state, true, true);
                    }
                }
                None => {}
            },
            _ => {}
        }
        WidgetResult::Handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_document_settings_esc_cancels_all_submodes_before_closing() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = DocumentSettingsDialog::new();

        // 1. Esc while editing number cancels only numeric editing.
        dialog.editing_number = Some((NumericSetting::FillRunThreshold, "16".to_string()));
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.editing_number, None);

        // 2. Esc while editing description cancels description editing and keeps dialog open.
        dialog.is_editing_description = true;
        dialog.description_input.insert_str("unsaved draft");
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert!(!dialog.is_editing_description);
        assert!(dialog.description_input.lines().join("").is_empty());

        // 3. Esc while selecting system or assembler closes the popup without closing the dialog.
        dialog.is_selecting_system = true;
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert!(!dialog.is_selecting_system);

        dialog.is_selecting_assembler = true;
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Handled
        );
        assert!(!dialog.is_selecting_assembler);

        // 4. Esc in top-level state closes the dialog and sets status to "Ready".
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            ),
            WidgetResult::Close
        );
        assert_eq!(ui_state.status_message.as_deref(), Some("Ready"));
    }

    #[test]
    fn test_document_settings_invalid_numeric_input_recovers_cleanly() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = DocumentSettingsDialog::new();
        dialog.editing_number = Some((NumericSetting::MaxXrefCount, "99".to_string()));

        let res = dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(res, WidgetResult::Handled);
        assert_eq!(
            dialog.editing_number,
            Some((NumericSetting::MaxXrefCount, String::new()))
        );
        assert_eq!(
            ui_state.status_message.as_deref(),
            Some("Enter a value from 0 to 40")
        );
    }

    #[test]
    fn test_document_settings_numeric_row_edit_and_step_roundtrip() {
        let mut app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = DocumentSettingsDialog::new();

        // Select FillRunThreshold row via descriptor table and step down to 0 ("off").
        let fill_idx = FIXED_ROWS
            .iter()
            .position(|r| *r == RowDescriptor::Numeric(NumericSetting::FillRunThreshold))
            .unwrap();
        dialog.selected_index = fill_idx;
        app_state.settings.fill_run_threshold = 1;

        dialog.handle_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(app_state.settings.fill_run_threshold, 0);
        assert_eq!(
            NumericSetting::FillRunThreshold.display_value(&app_state.settings),
            "off"
        );

        // Press Enter to open inline editor, type "24", and commit with Enter.
        dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(
            dialog.editing_number,
            Some((NumericSetting::FillRunThreshold, "0".to_string()))
        );
        dialog.handle_input(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        for ch in ['2', '4'] {
            dialog.handle_input(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app_state,
                &mut ui_state,
            );
        }
        dialog.handle_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut app_state,
            &mut ui_state,
        );
        assert_eq!(dialog.editing_number, None);
        assert_eq!(app_state.settings.fill_run_threshold, 24);
    }

    #[test]
    fn test_document_settings_render_standard_small_and_popup_states() {
        let app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let mut dialog = DocumentSettingsDialog::new();

        for (w, h) in [(80, 28), (80, 24), (20, 10), (3, 3)] {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|f| {
                    let area = f.area();
                    dialog.render(f, area, &app_state, &mut ui_state);
                })
                .unwrap();
        }

        // Render active popups (system selector, assembler selector, numeric editor, description editor).
        dialog.is_selecting_system = true;
        dialog.is_selecting_assembler = true;
        dialog.editing_number = Some((NumericSetting::BytesPerLine, "16".to_string()));
        dialog.is_editing_description = true;
        let backend = TestBackend::new(80, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                dialog.render(f, area, &app_state, &mut ui_state);
            })
            .unwrap();
    }
}
