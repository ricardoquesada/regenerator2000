use crate::state::AppState;
// Theme import removed
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

// Re-export SearchFilters so existing `use crate::ui::dialog_search::SearchFilters` paths
// continue to work.
pub use crate::state::search::SearchFilters;

// Import search engine functions from the centralized module.
use crate::state::search;

pub struct SearchDialog {
    pub input: String,
    pub editing_filters: bool,
    pub selected_filter: usize,
    pub filters: SearchFilters,
}

impl SearchDialog {
    #[must_use]
    pub fn new(initial_query: String, filters: SearchFilters) -> Self {
        Self {
            input: initial_query,
            editing_filters: false,
            selected_filter: 0,
            filters,
        }
    }
}

use crossterm::event::KeyModifiers;

const FILTER_COUNT: usize = 5;

// Each entry: (label_text, shortcut_char, shortcut_position_in_label)
const FILTER_INFO: [(&str, char, usize); FILTER_COUNT] = [
    ("Labels", 'l', 0),
    ("Comments", 'c', 0),
    ("Instructions", 'i', 0),
    ("Hex bytes", 'h', 0),
    ("Text (PETSCII, Screencode)", 't', 0),
];

impl Widget for SearchDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;

        // Create a proper centered modal dialog
        // Height: 2 (border) + 3 (input w/ border) + 1 (filters label) + 5 (filters) + 1 (help) = 12
        let dialog_area = crate::utils::centered_rect_adaptive(50, 40, 50, 12, area);
        ui_state.active_dialog_area = dialog_area;

        f.render_widget(ratatui::widgets::Clear, dialog_area);

        let block = crate::ui::widget::create_dialog_block(" Search ", theme);
        f.render_widget(block.clone(), dialog_area);

        let inner = block.inner(dialog_area);

        let filter_rows = FILTER_COUNT as u16;
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),           // search input (with border)
                Constraint::Length(1),           // filters label / separator
                Constraint::Length(filter_rows), // filter checkboxes
                Constraint::Length(1),           // help text
            ])
            .split(inner);

        let input_area = layout[0];
        let label_area = layout[1];
        let filter_area = Rect::new(
            inner.x + 2,
            layout[2].y,
            inner.width.saturating_sub(4),
            filter_rows,
        );
        let help_area = layout[3];

        // Search input with a bordered sub-block and background
        let is_input_focused = !self.editing_filters;
        let input_border_style = if is_input_focused {
            Style::default().fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.dialog_border)
        };
        let input_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(input_border_style)
            .style(Style::default().bg(theme.highlight_bg));

        let input_style = if is_input_focused {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg).bg(theme.highlight_bg)
        };
        let input = Paragraph::new(self.input.clone())
            .block(input_block)
            .style(input_style);
        f.render_widget(input, input_area);

        // Show blinking cursor at end of input when focused
        if is_input_focused {
            f.set_cursor_position((input_area.x + 1 + self.input.len() as u16, input_area.y + 1));
        }

        // Filters section label with separator
        let label_style = Style::default()
            .fg(theme.dialog_fg)
            .add_modifier(Modifier::DIM);
        let separator_width = inner.width.saturating_sub(11) as usize; // " Filters " + padding
        let label_line = Line::from(vec![
            Span::styled(" Filters ", label_style),
            Span::styled(
                "─".repeat(separator_width),
                Style::default()
                    .fg(theme.dialog_border)
                    .add_modifier(Modifier::DIM),
            ),
        ]);
        f.render_widget(Paragraph::new(label_line), label_area);

        // Render filter checkboxes vertically
        let filter_values = self.filters.as_array();
        for (i, (label, shortcut_char, shortcut_pos)) in FILTER_INFO.iter().enumerate() {
            let check = if filter_values[i] { "[X]" } else { "[ ]" };
            let is_selected = self.editing_filters && self.selected_filter == i;

            // Build spans with the shortcut letter underlined
            let base_style = if is_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dialog_fg)
            };
            let shortcut_style = base_style.add_modifier(Modifier::UNDERLINED);

            // Split the label around the shortcut character
            let before = &label[..*shortcut_pos];
            let sc = &shortcut_char.to_uppercase().to_string();
            let after = &label[shortcut_pos + shortcut_char.len_utf8()..];

            let line = Line::from(vec![
                Span::styled(format!("{check} "), base_style),
                Span::styled(before.to_string(), base_style),
                Span::styled(sc.clone(), shortcut_style),
                Span::styled(after.to_string(), base_style),
            ]);
            f.render_widget(
                Paragraph::new(line),
                Rect::new(
                    filter_area.x,
                    filter_area.y + i as u16,
                    filter_area.width,
                    1,
                ),
            );
        }

        let help = Paragraph::new(
            " Tab: filters │ Space: toggle │ Alt+Key: toggle │ Alt+A/N: all/none │ Enter: search",
        )
        .style(Style::default().fg(theme.comment));
        f.render_widget(help, help_area);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Alt+key shortcuts work in both input and filter mode
        if key.modifiers.contains(KeyModifiers::ALT)
            && let KeyCode::Char(c) = key.code
        {
            match c {
                'a' => {
                    self.filters.set_all();
                    return WidgetResult::Handled;
                }
                'n' => {
                    self.filters.set_none();
                    return WidgetResult::Handled;
                }
                _ => {
                    for (i, (_, shortcut_char, _)) in FILTER_INFO.iter().enumerate() {
                        if c == *shortcut_char {
                            self.filters.toggle(i);
                            return WidgetResult::Handled;
                        }
                    }
                }
            }
        }

        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                ui_state.last_search_query = self.input.clone();
                ui_state.search_filters = self.filters.clone();
                perform_search(app_state, ui_state, true);
                WidgetResult::Close
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.editing_filters = !self.editing_filters;
                WidgetResult::Handled
            }
            _ if self.editing_filters => {
                match key.code {
                    KeyCode::Up | KeyCode::Left => {
                        if self.selected_filter == 0 {
                            self.selected_filter = FILTER_COUNT - 1;
                        } else {
                            self.selected_filter -= 1;
                        }
                    }
                    KeyCode::Down | KeyCode::Right => {
                        self.selected_filter = (self.selected_filter + 1) % FILTER_COUNT;
                    }
                    KeyCode::Char(' ') => {
                        self.filters.toggle(self.selected_filter);
                    }
                    _ => {}
                }
                WidgetResult::Handled
            }
            KeyCode::Backspace => {
                self.input.pop();
                WidgetResult::Handled
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}

pub fn perform_search(app_state: &mut AppState, ui_state: &mut UIState, forward: bool) {
    let query = ui_state.last_search_query.clone();
    if query.is_empty() {
        ui_state.set_status_message("No search query");
        return;
    }

    let query_lower = query.to_lowercase();
    let disassembly_len = app_state.disassembly.len();
    if disassembly_len == 0 {
        return;
    }

    let start_idx = ui_state.cursor_index;
    let mut found_idx = None;
    let mut found_sub_idx = 0;

    let hex_pattern = if ui_state.search_filters.hex_bytes {
        search::parse_hex_pattern(&query)
    } else {
        None
    };
    let filters = &ui_state.search_filters;

    // Check current line first for subsequent matches
    if let Some(line) = app_state.disassembly.get(start_idx) {
        let matches = search::get_line_matches(
            line,
            app_state,
            &query_lower,
            hex_pattern.as_deref(),
            filters,
        );

        let candidate = if forward {
            matches
                .into_iter()
                .find(|&sub| sub > ui_state.sub_cursor_index)
        } else {
            matches
                .into_iter()
                .rev()
                .find(|&sub| sub < ui_state.sub_cursor_index)
        };

        if let Some(sub) = candidate {
            ui_state.core.navigation_history.push((
                ActivePane::Disassembly,
                crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index),
            ));
            ui_state.sub_cursor_index = sub;
            ui_state.set_status_message(format!("Found '{query}'"));
            return;
        }
    }

    // Iterate other lines
    for i in 1..disassembly_len {
        let idx = if forward {
            (start_idx + i) % disassembly_len
        } else {
            // backward wrap
            if i <= start_idx {
                start_idx - i
            } else {
                disassembly_len - (i - start_idx)
            }
        };

        if let Some(line) = app_state.disassembly.get(idx) {
            let matches = search::get_line_matches(
                line,
                app_state,
                &query_lower,
                hex_pattern.as_deref(),
                filters,
            );
            if !matches.is_empty() {
                found_idx = Some(idx);
                found_sub_idx = if forward {
                    matches[0]
                } else {
                    matches[matches.len() - 1]
                };
                break;
            }

            // Check collapsed content
            let pc = line.address.offset_from(app_state.origin);
            if app_state
                .collapsed_blocks
                .iter()
                .find(|(s, _)| *s == pc)
                .copied()
                .is_some_and(|(start, end)| {
                    search::search_collapsed_content(
                        app_state,
                        start,
                        end,
                        &query_lower,
                        hex_pattern.as_deref(),
                        filters,
                    )
                })
            {
                found_idx = Some(idx);
                found_sub_idx = 0;
                break;
            }
        }
    }

    if let Some(idx) = found_idx {
        ui_state.core.navigation_history.push((
            ActivePane::Disassembly,
            crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index),
        ));
        ui_state.cursor_index = idx;
        ui_state.sub_cursor_index = found_sub_idx;
        ui_state.set_status_message(format!("Found '{query}'"));
    } else {
        ui_state.set_status_message(format!("'{query}' not found"));
    }
}
