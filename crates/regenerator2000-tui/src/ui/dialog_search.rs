use crate::state::AppState;
// Theme import removed
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use ratatui_textarea::{CursorMove, TextArea};

use crate::ui::widget::{Widget, WidgetResult};

// Re-export SearchFilters so existing `use crate::ui::dialog_search::SearchFilters` paths
// continue to work.
pub use crate::state::search::SearchFilters;

// Import search engine functions from the centralized module.
use crate::state::search;

pub struct SearchDialog {
    pub textarea: TextArea<'static>,
    pub editing_filters: bool,
    pub selected_filter: usize,
    pub filters: SearchFilters,
    /// Holds a regex error message when [`SearchFilters::use_regex`] is `true`
    /// and the current query is not a valid regular expression.
    pub regex_error: Option<String>,
}

impl SearchDialog {
    #[must_use]
    pub fn new(initial_query: impl AsRef<str>, filters: SearchFilters) -> Self {
        let initial_query = initial_query.as_ref();
        let mut textarea = TextArea::default();
        if !initial_query.is_empty() {
            textarea.insert_str(initial_query);
            textarea.move_cursor(CursorMove::End);
        }
        Self {
            textarea,
            editing_filters: false,
            selected_filter: 0,
            filters,
            regex_error: None,
        }
    }

    /// Return the current search input text.
    #[must_use]
    fn input_text(&self) -> String {
        self.textarea.lines().join("")
    }

    /// Validate the current input as a regex and update [`Self::regex_error`].
    /// Clears the error when regex mode is off or the pattern is valid.
    fn validate_regex(&mut self) {
        let input = self.input_text();
        if self.filters.use_regex && !input.is_empty() {
            match search::compile_regex(&input) {
                Ok(_) => self.regex_error = None,
                Err(e) => self.regex_error = Some(e.to_string()),
            }
        } else {
            self.regex_error = None;
        }
    }
}

use crossterm::event::KeyModifiers;

const FILTER_COUNT: usize = 6;

// Each entry: (label_text, shortcut_char, shortcut_position_in_label)
const FILTER_INFO: [(&str, char, usize); FILTER_COUNT] = [
    ("Labels", 'l', 0),
    ("Comments", 'c', 0),
    ("Instructions", 'i', 0),
    ("Hex bytes", 'h', 0),
    ("Text (PETSCII, Screencode)", 't', 0),
    ("Regex mode", 'r', 0),
];

impl Widget for SearchDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;

        // Create a proper centered modal dialog
        // Height: 2 (border) + 3 (input w/ border) + 1 (filters label) + 6 (filters) + 1 (error hint) + 1 (help) = 14
        let dialog_area = crate::utils::centered_rect_adaptive(50, 40, 50, 14, area);
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
                Constraint::Length(1),           // regex error hint
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
        let error_area = layout[3];
        let help_area = layout[4];

        // Search input using TextArea with a bordered sub-block
        let is_input_focused = !self.editing_filters;
        let input_border_style = if is_input_focused {
            Style::default().fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.dialog_border)
        };

        let mut textarea = self.textarea.clone();
        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(input_border_style)
                .style(Style::default().bg(theme.highlight_bg)),
        );

        let input_style = if is_input_focused {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dialog_fg).bg(theme.highlight_bg)
        };
        textarea.set_style(input_style);

        if is_input_focused {
            textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        } else {
            textarea.set_cursor_style(Style::default().fg(theme.dialog_fg));
        }
        textarea.set_cursor_line_style(Style::default());

        f.render_widget(&textarea, input_area);

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

        // Regex error hint (shown when use_regex is on and pattern is invalid)
        if let Some(err) = &self.regex_error {
            let truncated = if err.len() > 60 {
                format!(" [!] {}…", &err[..57])
            } else {
                format!(" [!] {err}")
            };
            f.render_widget(
                Paragraph::new(truncated)
                    .style(Style::default().fg(ratatui::style::Color::LightRed)),
                error_area,
            );
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
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
                // Persist the query and filters into `UIState` and dispatch `AppAction::FindNext`
                // so `Core` (`navigation_handler::perform_search`) acts as the single source of
                // truth for search execution (including collapsed blocks) and closes this dialog
                // via `AppAction::FindNext.closes_dialog()`.
                ui_state.last_search_query = self.input_text();
                ui_state.search_filters = self.filters.clone();
                WidgetResult::Action(crate::state::actions::AppAction::FindNext)
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
                        // Re-validate when toggling regex mode (index 5).
                        if self.selected_filter == 5 {
                            self.validate_regex();
                        }
                    }
                    _ => {}
                }
                WidgetResult::Handled
            }
            // Delegate all other keys to the TextArea (handles cursor movement,
            // home/end, backspace, delete, character input, etc.)
            _ => {
                self.textarea.input(key);
                self.validate_regex();
                WidgetResult::Handled
            }
        }
    }
}
