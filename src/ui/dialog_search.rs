use crate::state::AppState;
// Theme import removed
use crate::ui_state::{ActivePane, UIState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::ui::widget::{Widget, WidgetResult};

pub struct SearchDialog {
    pub input: String,
}

impl SearchDialog {
    pub fn new(initial_query: String) -> Self {
        Self {
            input: initial_query,
        }
    }
}

impl Widget for SearchDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let block = crate::ui::widget::create_dialog_block(" Search ", theme);

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

        let input = Paragraph::new(self.input.clone()).block(block).style(
            Style::default()
                .fg(theme.highlight_fg)
                .add_modifier(Modifier::BOLD),
        );
        f.render_widget(input, area);
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            KeyCode::Enter => {
                ui_state.last_search_query = self.input.clone();
                perform_search(app_state, ui_state, true);
                WidgetResult::Close
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

    let hex_pattern = parse_hex_pattern(query);

    // Check current line first for subsequent matches
    if let Some(line) = app_state.disassembly.get(start_idx) {
        let matches = get_line_matches(line, app_state, &query_lower, hex_pattern.as_deref());

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
            ui_state
                .navigation_history
                .push((ActivePane::Disassembly, ui_state.cursor_index));
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
            let matches = get_line_matches(line, app_state, &query_lower, hex_pattern.as_deref());
            if !matches.is_empty() {
                found_idx = Some(idx);
                found_sub_idx = if forward {
                    *matches.first().unwrap()
                } else {
                    *matches.last().unwrap()
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
        ui_state
            .navigation_history
            .push((ActivePane::Disassembly, ui_state.cursor_index));
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
) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut current_sub = 0;

    // 1. Relative Labels
    if line.bytes.len() > 1 {
        for offset in 1..line.bytes.len() {
            let mid_addr = line.address.wrapping_add(offset as u16);
            if let Some(labels) = app_state.labels.get(&mid_addr) {
                for label in labels {
                    if label.name.to_lowercase().contains(query_lower) {
                        matches.push(current_sub);
                    }
                    current_sub += 1;
                }
            }
        }
    }

    // 2. Line Comment
    if let Some(lc) = &line.line_comment {
        if lc.to_lowercase().contains(query_lower) {
            matches.push(current_sub);
        }
        current_sub += 1;
    }

    // 3. Instruction Content
    let mut instruction_match = match_instruction_content(line, query_lower);

    if !instruction_match
        && let Some(pattern) = hex_pattern
        && check_hex_pattern(line.address, pattern, app_state)
    {
        instruction_match = true;
    }

    if instruction_match {
        matches.push(current_sub);
    }

    matches
}

fn match_instruction_content(
    line: &crate::disassembler::DisassemblyLine,
    query_lower: &str,
) -> bool {
    if format!("{:04x}", line.address).contains(query_lower) {
        return true;
    }

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

    if line.mnemonic.to_lowercase().contains(query_lower) {
        return true;
    }

    if line.operand.to_lowercase().contains(query_lower) {
        return true;
    }

    if line.comment.to_lowercase().contains(query_lower) {
        return true;
    }

    if let Some(lbl) = &line.label
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

    let expanded_lines = app_state.disassembler.disassemble(
        data_slice,
        block_slice,
        &app_state.labels,
        origin,
        &app_state.settings,
        &app_state.system_comments,
        &app_state.user_side_comments,
        &app_state.user_line_comments,
        &app_state.immediate_value_formats,
        &app_state.cross_refs,
        &[],
        &app_state.splitters,
    );

    for line in expanded_lines {
        if !get_line_matches(&line, app_state, query_lower, hex_pattern).is_empty() {
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
            comment_address: None,
            is_collapsed: false,
        };

        // "d020" is in "8d0208" starting at index 1 -> Should FAIL
        assert!(!match_instruction_content(&line, "d020"));

        // "8d02" is in "8d0208" starting at index 0 -> Should PASS
        assert!(match_instruction_content(&line, "8d02"));
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
