use crate::state::AppState;
use crate::utils::{petscii_to_unicode, screencode_to_petscii};
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

#[derive(Debug, Clone)]
pub struct SearchFilters {
    pub labels: bool,
    pub comments: bool,
    pub instructions: bool,
    pub hex_bytes: bool,
    pub text: bool,
}

impl Default for SearchFilters {
    fn default() -> Self {
        Self {
            labels: true,
            comments: true,
            instructions: true,
            hex_bytes: true,
            text: true,
        }
    }
}

impl SearchFilters {
    fn as_array(&self) -> [bool; 5] {
        [
            self.labels,
            self.comments,
            self.instructions,
            self.hex_bytes,
            self.text,
        ]
    }

    fn toggle(&mut self, index: usize) {
        match index {
            0 => self.labels = !self.labels,
            1 => self.comments = !self.comments,
            2 => self.instructions = !self.instructions,
            3 => self.hex_bytes = !self.hex_bytes,
            4 => self.text = !self.text,
            _ => {}
        }
    }
}

pub struct SearchDialog {
    pub input: String,
    pub editing_filters: bool,
    pub selected_filter: usize,
    pub filters: SearchFilters,
}

impl SearchDialog {
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
                Span::styled(format!("{} ", check), base_style),
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

        let help =
            Paragraph::new(" Tab: filters │ Space: toggle │ Alt+Key: toggle │ Enter: search")
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
            for (i, (_, shortcut_char, _)) in FILTER_INFO.iter().enumerate() {
                if c == *shortcut_char {
                    self.filters.toggle(i);
                    return WidgetResult::Handled;
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
    let query = &ui_state.last_search_query;
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
        parse_hex_pattern(query)
    } else {
        None
    };
    let filters = &ui_state.search_filters;

    // Check current line first for subsequent matches
    if let Some(line) = app_state.disassembly.get(start_idx) {
        let matches = get_line_matches(
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
            ui_state.navigation_history.push((
                ActivePane::Disassembly,
                crate::ui_state::NavigationTarget::Index(ui_state.cursor_index),
            ));
            ui_state.sub_cursor_index = sub;
            ui_state.set_status_message(format!("Found '{}'", query));
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
            let matches = get_line_matches(
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
            let pc = line.address.wrapping_sub(app_state.origin) as usize;
            if app_state
                .collapsed_blocks
                .iter()
                .find(|(s, _)| *s == pc)
                .copied()
                .is_some_and(|(start, end)| {
                    search_collapsed_content(
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
        ui_state.navigation_history.push((
            ActivePane::Disassembly,
            crate::ui_state::NavigationTarget::Index(ui_state.cursor_index),
        ));
        ui_state.cursor_index = idx;
        ui_state.sub_cursor_index = found_sub_idx;
        ui_state.set_status_message(format!("Found '{}'", query));
    } else {
        ui_state.set_status_message(format!("'{}' not found", query));
    }
}

fn get_line_matches(
    line: &crate::disassembler::DisassemblyLine,
    app_state: &AppState,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    filters: &SearchFilters,
) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut current_sub = 0;

    // 1. Relative Labels
    if line.bytes.len() > 1 {
        for offset in 1..line.bytes.len() {
            let mid_addr = line.address.wrapping_add(offset as u16);
            if let Some(labels) = app_state.labels.get(&mid_addr) {
                for _ in labels {
                    if filters.labels
                        && labels
                            .iter()
                            .any(|l| l.name.to_lowercase().contains(query_lower))
                    {
                        matches.push(current_sub);
                    }
                    current_sub += 1;
                }
            }
        }
    }

    // 2. Line Comment
    if let Some(lc) = &line.line_comment {
        for comment_line in lc.lines() {
            if filters.comments && comment_line.to_lowercase().contains(query_lower) {
                matches.push(current_sub);
            }
            current_sub += 1;
        }
    }

    // 3. Instruction Content
    let mut instruction_match = match_instruction_content(line, query_lower, filters);

    // 4. Hex pattern search and String pattern search
    if !instruction_match {
        for offset in 0..line.bytes.len() {
            let addr = line.address.wrapping_add(offset as u16);

            // Hex pattern search
            if let Some(pattern) = hex_pattern
                && check_hex_pattern(addr, pattern, app_state)
            {
                instruction_match = true;
                break;
            }

            // String pattern search (PETSCII / Screencode)
            if check_string_pattern(addr, query_lower, app_state, filters) {
                instruction_match = true;
                break;
            }
        }
    }

    if instruction_match {
        matches.push(current_sub);
    }

    matches
}

fn match_instruction_content(
    line: &crate::disassembler::DisassemblyLine,
    query_lower: &str,
    filters: &SearchFilters,
) -> bool {
    if filters.hex_bytes {
        let bytes_hex = line
            .bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        if bytes_hex
            .match_indices(query_lower)
            .any(|(idx, _)| idx % 2 == 0)
        {
            return true;
        }

        let bytes_hex_spaces = line
            .bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        if bytes_hex_spaces.contains(query_lower) {
            return true;
        }
    }

    if filters.instructions && line.mnemonic.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.instructions && line.operand.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.comments && line.comment.to_lowercase().contains(query_lower) {
        return true;
    }

    if filters.labels
        && let Some(lbl) = &line.label
        && lbl.to_lowercase().contains(query_lower)
    {
        return true;
    }

    false
}

fn search_collapsed_content(
    app_state: &AppState,
    start: usize,
    end: usize,
    query_lower: &str,
    hex_pattern: Option<&[Option<u8>]>,
    filters: &SearchFilters,
) -> bool {
    if start >= app_state.raw_data.len() || end >= app_state.raw_data.len() {
        return false;
    }

    let origin = app_state.origin.wrapping_add(start as u16);
    let data_slice = &app_state.raw_data[start..=end];

    if start >= app_state.block_types.len() || end >= app_state.block_types.len() {
        return false;
    }
    let block_slice = &app_state.block_types[start..=end];

    let ctx = crate::disassembler::DisassemblyContext {
        data: data_slice,
        block_types: block_slice,
        labels: &app_state.labels,
        origin,
        settings: &app_state.settings,
        system_comments: &app_state.system_comments,
        user_side_comments: &app_state.user_side_comments,
        user_line_comments: &app_state.user_line_comments,
        immediate_value_formats: &app_state.immediate_value_formats,
        cross_refs: &app_state.cross_refs,
        analysis_hints: &app_state.analysis_hints,
        collapsed_blocks: &[],
        splitters: &app_state.splitters,
    };
    let expanded_lines = app_state.disassembler.disassemble_ctx(&ctx);

    for line in expanded_lines {
        if !get_line_matches(&line, app_state, query_lower, hex_pattern, filters).is_empty() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disassembler::DisassemblyLine;

    #[test]
    fn test_match_instruction_content_bytes_alignment() {
        let line = DisassemblyLine {
            address: 0x1000,
            bytes: vec![0x8D, 0x02, 0x08], // 8d0208
            mnemonic: "STA".to_string(),
            operand: "$0802".to_string(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // "d020" is in "8d0208" starting at index 1 -> Should FAIL
        assert!(!match_instruction_content(
            &line,
            "d020",
            &SearchFilters::default()
        ));

        // "8d02" is in "8d0208" starting at index 0 -> Should PASS
        assert!(match_instruction_content(
            &line,
            "8d02",
            &SearchFilters::default()
        ));
    }

    #[test]
    fn test_get_line_matches_sub_indices() {
        use crate::state::Label;
        let mut app_state = AppState::new();
        // Mid-address label at 0x1001
        app_state.labels.insert(
            0x1001,
            vec![Label {
                name: "mid_label".to_string(),
                label_type: crate::state::LabelType::AbsoluteAddress,
                kind: crate::state::LabelKind::User,
            }],
        );

        let line = DisassemblyLine {
            address: 0x1000,
            bytes: vec![0xA9, 0x00], // LDA #$00
            mnemonic: "LDA".to_string(),
            operand: "#$00".to_string(),
            comment: "side comment".to_string(),
            line_comment: Some("line 1\nline 2\nline 3".to_string()),
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // Sub-indices mapping:
        // 0: label at 0x1001
        // 1: line comment "line 1"
        // 2: line comment "line 2"
        // 3: line comment "line 3"
        // 4: Instruction (LDA #$00 ; side comment)

        // Test label match
        let filters = SearchFilters::default();
        let matches = get_line_matches(&line, &app_state, "mid_label", None, &filters);
        assert_eq!(matches, vec![0]);

        // Test line comment matches
        assert_eq!(
            get_line_matches(&line, &app_state, "line 1", None, &filters),
            vec![1]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 2", None, &filters),
            vec![2]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "line 3", None, &filters),
            vec![3]
        );

        // Test instruction matches
        assert_eq!(
            get_line_matches(&line, &app_state, "lda", None, &filters),
            vec![4]
        );
        assert_eq!(
            get_line_matches(&line, &app_state, "side", None, &filters),
            vec![4]
        );

        // Test multiple matches (e.g. "line" matches all comment lines)
        assert_eq!(
            get_line_matches(&line, &app_state, "line", None, &filters),
            vec![1, 2, 3]
        );
    }
}

fn parse_hex_pattern(query: &str) -> Option<Vec<Option<u8>>> {
    let mut pattern = Vec::new();

    // Safety check: ensure only contains hex chars, spaces, and '?'
    let allowed_chars = "0123456789abcdefABCDEF? ";
    if query.chars().any(|c| !allowed_chars.contains(c)) {
        return None;
    }

    // Remove spaces to handle both "A9 00" and "A900"
    let clean: String = query.chars().filter(|c| !c.is_whitespace()).collect();

    if clean.is_empty() {
        return None;
    }

    // Hex pattern must be pairs of characters (bytes)
    if !clean.len().is_multiple_of(2) {
        return None;
    }

    let chars: Vec<char> = clean.chars().collect();
    for chunk in chars.chunks(2) {
        let s: String = chunk.iter().collect();
        if s == "??" {
            pattern.push(None);
        } else {
            match u8::from_str_radix(&s, 16) {
                Ok(b) => pattern.push(Some(b)),
                Err(_) => return None,
            }
        }
    }
    Some(pattern)
}

fn check_hex_pattern(address: u16, pattern: &[Option<u8>], app_state: &AppState) -> bool {
    let raw_len = app_state.raw_data.len();
    if raw_len == 0 {
        return false;
    }

    let start_offset = (address.wrapping_sub(app_state.origin)) as usize;

    if start_offset >= raw_len {
        return false;
    }

    // Check if the pattern fits in the remaining data
    if start_offset + pattern.len() > raw_len {
        return false;
    }

    for (i, &byte_pat) in pattern.iter().enumerate() {
        let idx = start_offset + i;
        if let Some(target) = byte_pat
            && (idx >= raw_len || app_state.raw_data[idx] != target)
        {
            return false;
        }
    }

    true
}

/// Check if the query string matches the raw bytes at the given address.
/// Checks both PETSCII and Screencode encodings, case-insensitively.
fn check_string_pattern(
    address: u16,
    query: &str,
    app_state: &AppState,
    filters: &SearchFilters,
) -> bool {
    let raw_len = app_state.raw_data.len();
    if raw_len == 0 || query.is_empty() {
        return false;
    }

    let start_offset = (address.wrapping_sub(app_state.origin)) as usize;
    if start_offset >= raw_len {
        return false;
    }

    let query_chars: Vec<char> = query.chars().collect();
    let query_len = query_chars.len();

    // Check if the pattern fits in the remaining data
    if start_offset + query_len > raw_len {
        return false;
    }

    if !filters.text {
        return false;
    }

    // Check PETSCII encoding (both shifted and unshifted)
    let petscii_match = (0..=1).any(|shift| {
        let shifted = shift == 1;
        query_chars.iter().enumerate().all(|(i, &query_char)| {
            let idx = start_offset + i;
            let byte = app_state.raw_data[idx];
            let petscii_char = petscii_to_unicode(byte, shifted);
            petscii_char.eq_ignore_ascii_case(&query_char)
        })
    });

    if petscii_match {
        return true;
    }

    // Check Screencode encoding (convert to PETSCII first, then to Unicode)
    let screencode_match = (0..=1).any(|shift| {
        let shifted = shift == 1;
        query_chars.iter().enumerate().all(|(i, &query_char)| {
            let idx = start_offset + i;
            let screencode_byte = app_state.raw_data[idx];
            let petscii_byte = screencode_to_petscii(screencode_byte);
            let sc_char = petscii_to_unicode(petscii_byte, shifted);
            sc_char.eq_ignore_ascii_case(&query_char)
        })
    });

    if screencode_match {
        return true;
    }

    false
}

#[cfg(test)]
mod tests_hex {
    use super::*;

    #[test]
    fn test_parse_hex_pattern() {
        // Valid patterns
        assert_eq!(
            parse_hex_pattern("A9 00"),
            Some(vec![Some(0xA9), Some(0x00)])
        );
        assert_eq!(
            parse_hex_pattern("A9 00 ?? D0"),
            Some(vec![Some(0xA9), Some(0x00), None, Some(0xD0)])
        );
        assert_eq!(
            parse_hex_pattern("a900??d0"),
            Some(vec![Some(0xA9), Some(0x00), None, Some(0xD0)])
        );
        assert_eq!(parse_hex_pattern("??"), Some(vec![None]));

        // Invalid patterns
        assert_eq!(parse_hex_pattern("A"), None); // odd length
        assert_eq!(parse_hex_pattern("A9 0"), None); // odd length
        assert_eq!(parse_hex_pattern("G0"), None); // invalid char
        assert_eq!(parse_hex_pattern("LDA"), None); // invalid chars
        assert_eq!(parse_hex_pattern(""), None); // empty
        assert_eq!(parse_hex_pattern("A?"), None); // invalid wildcard (must be ??)
    }
}

#[cfg(test)]
mod tests_string {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn test_check_string_pattern() {
        let mut app_state = AppState::new();
        app_state.raw_data = vec![
            0x48, 0x45, 0x4C, 0x4C, 0x4F, // "HELLO" in PETSCII (unshifted)
            0x08, 0x05, 0x0C, 0x0C, 0x0F, // "hello" in Screencodes (Shifted/Lowercase)
        ];
        app_state.origin = 0x1000;

        // 1. PETSCII match (unshifted)
        let filters = SearchFilters::default();
        assert!(check_string_pattern(0x1000, "HELLO", &app_state, &filters));
        assert!(check_string_pattern(0x1000, "hello", &app_state, &filters)); // Case-insensitive
        assert!(check_string_pattern(0x1000, "HellO", &app_state, &filters));

        // 2. Screencode match
        // Note: screencode_to_petscii(0x08) -> 0x48 ('H' or 'h' shifted)
        assert!(check_string_pattern(0x1005, "hello", &app_state, &filters));
        assert!(check_string_pattern(0x1005, "HELLO", &app_state, &filters));

        // 3. No match
        assert!(!check_string_pattern(0x1000, "WORLD", &app_state, &filters));
        assert!(!check_string_pattern(0x1008, "HELLO", &app_state, &filters)); // Out of bounds/Too long
    }

    #[test]
    fn test_string_search_at_offset() {
        use crate::disassembler::DisassemblyLine;

        let mut app_state = AppState::new();
        // Screencode for "LANDING": 0C 01 0E 04 09 0E 07
        // Let's put a dummy byte at the start to force an offset
        app_state.raw_data = vec![0x00, 0x0C, 0x01, 0x0E, 0x04, 0x09, 0x0E, 0x07];
        app_state.origin = 0x1000;

        let line = DisassemblyLine {
            address: 0x1000,
            bytes: app_state.raw_data.clone(),
            mnemonic: String::new(),
            operand: String::new(),
            comment: String::new(),
            line_comment: None,
            label: None,
            opcode: None,
            show_bytes: true,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        };

        // This is what get_line_matches CURRENTLY does (simplified)
        let query = "landing";
        let filters = SearchFilters::default();
        let found_at_start = check_string_pattern(line.address, query, &app_state, &filters);
        assert!(
            !found_at_start,
            "Should not find at start due to leading 0x00"
        );

        // The bug is that we don't check offsets. This test will help me verify the fix.
        let mut found_at_any_offset = false;
        for offset in 0..line.bytes.len() {
            if check_string_pattern(line.address + offset as u16, query, &app_state, &filters) {
                found_at_any_offset = true;
                break;
            }
        }
        assert!(found_at_any_offset, "Should find 'landing' at offset 1");
    }
}
