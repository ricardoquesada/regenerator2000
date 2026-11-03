//! Visual layout calculations, sub-line height breakdown, and disassembly line index mapping.
//!
//! This module bridges [`regenerator2000_core::disassembler`] sub-line geometry
//! ([`VisualLineCounts`] and [`DisassemblyLine::visual_line_counts`]) with the TUI disassembly
//! viewport. By delegating per-line height and sub-cursor resolution to the active assembler's
//! [`Disassembler::formatter`], keyboard navigation, mouse hit-testing, and viewport rendering
//! share identical row counts for mid-instruction relative labels, multi-line comments, and
//! [`LabelPlacement::OwnLine`](regenerator2000_core::disassembler::LabelPlacement::OwnLine) labels.

use regenerator2000_core::disassembler::{Disassembler, DisassemblyLine};
use regenerator2000_core::state::AppState;

pub use regenerator2000_core::disassembler::VisualLineCounts;

/// Computes the [`VisualLineCounts`] breakdown (`labels`, `comments`, and `instruction`)
/// for a single [`DisassemblyLine`] using the active assembler's [`Disassembler::formatter`].
///
/// # Arguments
///
/// * `line` - Target [`DisassemblyLine`] to measure.
/// * `app_state` - Application state [`AppState`] providing intermediate-byte labels and assembler settings.
///
/// # Returns
///
/// A [`VisualLineCounts`] struct detailing the vertical sub-row layout of `line`.
#[must_use]
pub fn get_visual_line_counts(line: &DisassemblyLine, app_state: &AppState) -> VisualLineCounts {
    line.visual_line_counts(
        app_state,
        Disassembler::formatter(app_state.settings.assembler),
    )
}

/// Returns the total number of visual terminal rows occupied by `line` ([`VisualLineCounts::total`]).
///
/// # Arguments
///
/// * `line` - Target [`DisassemblyLine`] to measure.
/// * `app_state` - Application state [`AppState`] providing intermediate-byte labels and assembler settings.
///
/// # Returns
///
/// Total visual row count (`labels + comments + instruction`).
#[must_use]
pub fn get_visual_line_count_for_instruction(
    line: &DisassemblyLine,
    app_state: &AppState,
) -> usize {
    get_visual_line_counts(line, app_state).total()
}

/// Maps a 1-indexed visual line number across the entire disassembly listing to its
/// corresponding instruction index in `app_state.disassembly`.
///
/// # Arguments
///
/// * `app_state` - Application state [`AppState`].
/// * `target_line` - 1-indexed visual line number to resolve.
///
/// # Returns
///
/// `Some(index)` into `app_state.disassembly`, or `None` if `target_line` is `0` or past the end of the listing.
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

/// Resolves the visual sub-cursor index within `line` corresponding to `target_addr`.
///
/// Delegates to [`DisassemblyLine::get_sub_index_for_address`], returning either the matching
/// mid-instruction relative label sub-index (`0..labels`) or [`VisualLineCounts::opcode_sub_index`].
///
/// # Arguments
///
/// * `line` - Target [`DisassemblyLine`].
/// * `app_state` - Application state [`AppState`].
/// * `target_addr` - Raw 16-bit memory address to locate within `line`.
///
/// # Returns
///
/// Visual sub-cursor index (`0..counts.total()`).
#[must_use]
pub fn get_sub_index_for_address(
    line: &DisassemblyLine,
    app_state: &AppState,
    target_addr: u16,
) -> usize {
    line.get_sub_index_for_address(app_state, regenerator2000_core::state::Addr(target_addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use regenerator2000_core::disassembler::DisassemblyLine;
    use regenerator2000_core::state::{Addr, Assembler};

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

    #[test]
    fn test_visual_line_counts_19_char_label_assembler_dependent() {
        let mut app_state = AppState::new();
        let line = make_test_line(0x1000, 1, Some("nineteen_char_label"), None);

        for assembler in [Assembler::Tass64, Assembler::Acme] {
            app_state.settings.assembler = assembler;
            let counts = get_visual_line_counts(&line, &app_state);
            assert_eq!(counts.instruction, 1);
            assert!(!counts.has_own_line_label());
            assert_eq!(counts.opcode_sub_index(), 0);
        }

        for assembler in [Assembler::Ca65, Assembler::Kick] {
            app_state.settings.assembler = assembler;
            let counts = get_visual_line_counts(&line, &app_state);
            assert_eq!(counts.instruction, 2);
            assert!(counts.has_own_line_label());
            assert_eq!(counts.opcode_sub_index(), 1);
        }
    }

    #[test]
    fn test_visual_line_counts_named_scope_vs_kickasm_enum_brace() {
        let mut app_state = AppState::new();
        app_state.settings.assembler = Assembler::Kick;

        let mut scope_line = make_test_line(0x1000, 0, Some("scope_a"), None);
        scope_line.mnemonic = "{".to_string();
        let scope_counts = get_visual_line_counts(&scope_line, &app_state);
        assert_eq!(scope_counts.instruction, 2);
        assert_eq!(scope_counts.opcode_sub_index(), 1);

        let mut enum_brace = make_test_line(0x0000, 0, None, None);
        enum_brace.mnemonic = "}".to_string();
        let enum_counts = get_visual_line_counts(&enum_brace, &app_state);
        assert_eq!(enum_counts.instruction, 1);
        assert_eq!(enum_counts.opcode_sub_index(), 0);
    }

    #[test]
    fn test_cross_layer_visual_row_count_matches_render_output() {
        use crate::theme::Theme;
        use crate::ui::view_disassembly::render::render_disassembly;
        use crate::ui_state::UIState;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        use ratatui::layout::Rect;
        use regenerator2000_core::disassembler::SPLITTER_MNEMONIC;
        use regenerator2000_core::state::{Label, LabelKind, LabelType};

        for assembler in [
            Assembler::Tass64,
            Assembler::Acme,
            Assembler::Ca65,
            Assembler::Kick,
        ] {
            let mut app_state = AppState::new();
            app_state.settings.assembler = assembler;
            app_state.labels.insert(
                Addr(0x1001),
                vec![Label {
                    name: "mid_lbl".to_string(),
                    kind: LabelKind::User,
                    label_type: LabelType::UserDefined,
                }],
            );

            let mut splitter = make_test_line(0x1000, 0, None, Some("splitter note"));
            splitter.mnemonic = SPLITTER_MNEMONIC.to_string();

            let mut named_scope = make_test_line(0x1000, 0, Some("my_scope"), None);
            named_scope.mnemonic = "{".to_string();

            let mut mid_and_own =
                make_test_line(0x1000, 3, Some("nineteen_char_label"), Some("c1\nc2"));
            mid_and_own.show_bytes = true;

            for fixture in [
                make_test_line(0x1000, 1, None, None),
                make_test_line(0x1000, 1, Some("nineteen_char_label"), None),
                splitter,
                named_scope,
                mid_and_own,
            ] {
                let expected_rows = get_visual_line_counts(&fixture, &app_state).total();
                let mut sentinel = make_test_line(0xFFFF, 1, None, None);
                sentinel.show_bytes = true;
                app_state.disassembly = vec![fixture, sentinel];

                let mut ui_state = UIState::new(Theme::default());
                let backend = TestBackend::new(120, 25);
                let mut terminal = Terminal::new(backend).unwrap();

                terminal
                    .draw(|f| {
                        render_disassembly(f, Rect::new(0, 0, 120, 25), &app_state, &mut ui_state);
                    })
                    .unwrap();

                let buffer = terminal.backend().buffer();
                let sentinel_y = (1..24)
                    .find(|&y| {
                        let row_str: String = (0..120)
                            .map(|x| {
                                buffer
                                    .cell((x, y))
                                    .map_or(String::new(), |c| c.symbol().to_string())
                            })
                            .collect();
                        row_str.to_lowercase().contains("$ffff")
                    })
                    .expect("sentinel $FFFF row should be rendered");

                assert_eq!((sentinel_y - 1) as usize, expected_rows);
            }
        }
    }
}
