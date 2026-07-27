//! Visual layout calculations, line height breakdown, and disassembly line index mapping.
//!
//! This module provides functions and data structures for calculating how many visual terminal
//! rows an instruction line occupies (taking multi-line comments, mid-instruction labels, and long labels
//! into account) and translating between 1-indexed visual screen lines and instruction indices.

use regenerator2000_core::disassembler::LABEL_COLUMN_WIDTH;
use regenerator2000_core::state::AppState;

/// Breakdown of visual lines consumed by a single disassembled instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualLineCounts {
    /// Number of mid-instruction label lines.
    pub labels: usize,
    /// Number of multi-line comment lines.
    pub comments: usize,
    /// Number of instruction/label header lines.
    pub instruction: usize,
}

impl VisualLineCounts {
    /// Returns the total visual height in lines required for this instruction.
    ///
    /// # Returns
    ///
    /// The sum of label lines, comment lines, and instruction lines.
    #[must_use]
    pub fn total(&self) -> usize {
        self.labels + self.comments + self.instruction
    }
}

/// Computes visual line counts breakdown for a single instruction line.
///
/// # Arguments
///
/// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
/// * `app_state` - Reference to application state [`AppState`].
///
/// # Returns
///
/// A [`VisualLineCounts`] struct detailing visual line requirements.
#[must_use]
pub fn get_visual_line_counts(
    line: &regenerator2000_core::disassembler::DisassemblyLine,
    app_state: &AppState,
) -> VisualLineCounts {
    let mut labels = 0;
    if line.bytes.len() > 1 {
        for offset in 1..line.bytes.len() {
            let mid_addr = line.address.wrapping_add(offset as u16);
            if let Some(l) = app_state.labels.get(&mid_addr) {
                labels += l.len();
            }
        }
    }

    let mut comments = 0;
    if let Some(comment) = &line.line_comment {
        comments += comment.lines().count();
    }

    let label_own_line = if let Some(label) = &line.label
        && label.len() >= LABEL_COLUMN_WIDTH
    {
        1
    } else {
        0
    };

    VisualLineCounts {
        labels,
        comments,
        instruction: 1 + label_own_line,
    }
}

/// Returns total visual height for an instruction line.
///
/// # Arguments
///
/// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
/// * `app_state` - Reference to application state [`AppState`].
///
/// # Returns
///
/// `usize` total visual line count.
#[must_use]
pub fn get_visual_line_count_for_instruction(
    line: &regenerator2000_core::disassembler::DisassemblyLine,
    app_state: &AppState,
) -> usize {
    get_visual_line_counts(line, app_state).total()
}

/// Maps a 1-indexed visual line number to the corresponding instruction index in disassembly.
///
/// # Arguments
///
/// * `app_state` - Reference to application state [`AppState`].
/// * `target_line` - 1-indexed visual line number to resolve.
///
/// # Returns
///
/// `Some(usize)` instruction line index, or `None` if out of bounds.
#[must_use]
pub fn get_index_for_visual_line(app_state: &AppState, target_line: usize) -> Option<usize> {
    let mut current_visual_line = 1;
    for (index, line) in app_state.disassembly.iter().enumerate() {
        let lines_for_this_instruction = get_visual_line_count_for_instruction(line, app_state);

        if target_line >= current_visual_line
            && target_line < current_visual_line + lines_for_this_instruction
        {
            return Some(index);
        }

        current_visual_line += lines_for_this_instruction;
    }
    None
}

/// Resolves sub-cursor line offset within an instruction for a target address.
///
/// # Arguments
///
/// * `line` - Reference to target [`DisassemblyLine`](regenerator2000_core::disassembler::DisassemblyLine).
/// * `app_state` - Reference to application state [`AppState`].
/// * `target_addr` - Target 16-bit memory address value.
///
/// # Returns
///
/// Sub-cursor offset line index `usize`.
#[must_use]
pub fn get_sub_index_for_address(
    line: &regenerator2000_core::disassembler::DisassemblyLine,
    app_state: &AppState,
    target_addr: u16,
) -> usize {
    line.get_sub_index_for_address(app_state, target_addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regenerator2000_core::disassembler::DisassemblyLine;
    use regenerator2000_core::state::Addr;

    fn make_test_line(
        addr: u16,
        len: usize,
        label: Option<&str>,
        line_comment: Option<&str>,
    ) -> DisassemblyLine {
        DisassemblyLine {
            address: Addr(addr),
            bytes: vec![0xEA; len],
            mnemonic: "NOP".to_string(),
            operand: String::new(),
            comment: String::new(),
            line_comment: line_comment.map(String::from),
            label: label.map(String::from),
            opcode: None,
            show_bytes: false,
            target_address: None,
            external_label_address: None,
            is_collapsed: false,
        }
    }

    #[test]
    fn test_visual_line_count_single_instruction() {
        let app_state = AppState::new();
        let line = make_test_line(0x1000, 1, None, None);
        let counts = get_visual_line_counts(&line, &app_state);
        assert_eq!(counts.total(), 1);
        assert_eq!(counts.instruction, 1);
        assert_eq!(counts.comments, 0);
        assert_eq!(counts.labels, 0);
    }

    #[test]
    fn test_visual_line_count_with_multiline_comments_and_labels() {
        let app_state = AppState::new();
        let line = make_test_line(
            0x1000,
            1,
            Some("very_long_label_that_exceeds_column_width_threshold"),
            Some("First comment line\nSecond comment line"),
        );
        let counts = get_visual_line_counts(&line, &app_state);
        assert_eq!(counts.comments, 2);
        assert_eq!(counts.instruction, 2); // instruction + label own line
        assert_eq!(counts.total(), 4);
    }

    #[test]
    fn test_calculate_content_height_collapsed_blocks() {
        let mut app_state = AppState::new();
        let mut line1 = make_test_line(0x1000, 1, None, None);
        line1.is_collapsed = true;
        let line2 = make_test_line(0x1001, 1, None, None);
        app_state.disassembly = vec![line1, line2];

        let total_height: usize = app_state
            .disassembly
            .iter()
            .map(|l| get_visual_line_count_for_instruction(l, &app_state))
            .sum();

        assert_eq!(total_height, 2);
    }
}
