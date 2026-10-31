//! Disassembly view rendering, line formatting, and Ratatui list construction.
//!
//! This module handles the visual rendering of 6502 disassembly lines, control-flow arrows,
//! lexical scope gutter brackets (`┌─`, `│ `, `└─`), collapsed block summaries, comments,
//! labels, VICE debugger indicators, breakpoints, and cursor selection highlights.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use regenerator2000_core::disassembler::LABEL_COLUMN_WIDTH;
use regenerator2000_core::state::AppState;

use super::layout::get_visual_line_count_for_instruction;
use crate::ui_state::{ActivePane, UIState};

/// Formats a byte slice as space-separated uppercase two-digit hexadecimal values (e.g. `"A9 00"`).
fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Renders the disassembly view widget into the specified frame area.
///
/// # Arguments
///
/// * `f` - Frame reference for rendering ratatui widgets.
/// * `area` - Target rectangle [`Rect`] area for view output.
/// * `app_state` - Active application state [`AppState`].
/// * `ui_state` - Mutable UI state [`UIState`] for tracking cursor, scroll, and theme.
#[allow(clippy::too_many_lines)]
pub fn render_disassembly(f: &mut Frame, area: Rect, app_state: &AppState, ui_state: &mut UIState) {
    let is_active = ui_state.active_pane == ActivePane::Disassembly;
    let border_style = if is_active {
        Style::default().fg(ui_state.theme.border_active)
    } else {
        Style::default().fg(ui_state.theme.border_inactive)
    };

    let title = if app_state.vice_state.live_memory.is_some() && app_state.vice_state.connected {
        " Disassembly [LIVE] "
    } else {
        " Disassembly "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title)
        .style(
            Style::default()
                .bg(ui_state.theme.background)
                .fg(ui_state.theme.foreground),
        );
    let list_area = block.inner(area);

    // Render outer border block first (separately from the list)
    f.render_widget(block.clone(), area);

    let formatter = app_state.get_formatter();

    let visible_height = list_area.height as usize;
    let total_items = app_state.disassembly.len();

    // Ensure cursor is within valid bounds to avoid panic
    if ui_state.cursor_index >= total_items {
        ui_state.cursor_index = total_items.saturating_sub(1);
        ui_state.sub_cursor_index = 0;
    }

    // 1. Calculate Cursor Visual Offset (distance from top of file)
    // This is expensive O(N) if we scan from 0. Ideally we only care about relative position
    // between scroll_index and cursor_index.

    let mut scroll_inst_idx = ui_state.scroll_index;
    let mut scroll_sub_idx = ui_state.scroll_sub_index;

    // Ensure scroll indices are valid
    if scroll_inst_idx >= total_items {
        scroll_inst_idx = total_items.saturating_sub(1);
        scroll_sub_idx = 0;
    }

    // --- Scrolloff Logic Start ---
    // We calculate the VALID range for the scroll position relative to the cursor.
    // The scroll position (top of view) must be:
    // 1. Not too "below" the cursor (cursor - margin). Ideally <= (cursor - margin).
    // 2. Not too "above" the cursor (cursor - (height - margin)). Ideally >= (cursor - (height - margin)).

    let margin = 5.min(visible_height / 3);

    // Helper to walk backwards visual lines
    let walk_back = |start_inst: usize, start_sub: usize, steps: usize| -> (usize, usize) {
        let mut curr_inst = start_inst;
        let mut curr_sub = start_sub;
        for _ in 0..steps {
            if curr_sub > 0 {
                curr_sub -= 1;
            } else if curr_inst > 0 {
                // Find previous valid instruction
                let mut prev_idx = curr_inst - 1;
                while prev_idx > 0 {
                    let prev_line = &app_state.disassembly[prev_idx];
                    if !prev_line.bytes.is_empty()
                        || prev_line.is_collapsed
                        || prev_line.label.is_some()
                        || !prev_line.mnemonic.is_empty()
                    {
                        break;
                    }
                    prev_idx -= 1;
                }
                curr_inst = prev_idx;
                // Check if valid line found (handle file start case where loop might fail if nothing valid)
                let prev_line = &app_state.disassembly[curr_inst];
                // Re-verify validity (esp for index 0)
                if !prev_line.bytes.is_empty()
                    || prev_line.is_collapsed
                    || prev_line.label.is_some()
                    || !prev_line.mnemonic.is_empty()
                {
                    let prev_lines = get_visual_line_count_for_instruction(prev_line, app_state);
                    curr_sub = prev_lines.saturating_sub(1);
                } else {
                    // Fallback to start of file
                    return (0, 0);
                }
            } else {
                return (0, 0);
            }
        }
        (curr_inst, curr_sub)
    };

    let (max_scroll_inst, max_scroll_sub) =
        walk_back(ui_state.cursor_index, ui_state.sub_cursor_index, margin);

    // Calculate min_scroll (furthest UP scroll can be)
    // Since we want cursor to be at most (visible_height - margin - 1) lines away from top,
    // min_scroll is (cursor) walked back by (visible_height - margin - 1).
    // However, max_scroll is (cursor) walked back by (margin).
    // So min_scroll is max_scroll walked back by (visible_height - margin - 1 - margin).

    let window_size = visible_height.saturating_sub(2 * margin).saturating_sub(1);
    let (min_scroll_inst, min_scroll_sub) = if window_size > 0 {
        walk_back(max_scroll_inst, max_scroll_sub, window_size)
    } else {
        (max_scroll_inst, max_scroll_sub)
    };

    // Clamp current scroll (scroll_inst_idx, scroll_sub_idx) to [min_scroll, max_scroll]

    // 1. Check if too high (scroll > max_scroll) -> Move UP to max_scroll
    if scroll_inst_idx > max_scroll_inst
        || (scroll_inst_idx == max_scroll_inst && scroll_sub_idx > max_scroll_sub)
    {
        scroll_inst_idx = max_scroll_inst;
        scroll_sub_idx = max_scroll_sub;
    }

    // 2. Check if too low (scroll < min_scroll) -> Move DOWN to min_scroll
    if scroll_inst_idx < min_scroll_inst
        || (scroll_inst_idx == min_scroll_inst && scroll_sub_idx < min_scroll_sub)
    {
        scroll_inst_idx = min_scroll_inst;
        scroll_sub_idx = min_scroll_sub;
    }

    // --- Scrolloff Logic End ---

    // --- Arrow Calculation Start ---
    // We want to find all arrows that overlap with the visible range: [offset, offset + visible_height]
    #[derive(Clone, Copy)]
    struct ArrowInfo {
        start: usize,
        end: usize,
        col: usize,
        target_addr: Option<u16>,
        start_visible: bool,
        end_visible: bool,
    }

    // Compute the accurate last visible instruction index by walking the disassembly
    // and counting visual rows (accounting for line comments and mid-address labels),
    // exactly like the render loop does. Using a naive `scroll_inst_idx + visible_height`
    // overestimates when instructions have multi-row comments, causing arrows to be
    // marked as visible when they are actually off-screen.
    let end_view = {
        let mut visible_rows_used = 0;
        let mut inst = scroll_inst_idx;
        // First instruction: only partial visibility when scroll_sub_idx > 0
        if inst < total_items {
            let line = &app_state.disassembly[inst];
            let count = get_visual_line_count_for_instruction(line, app_state);
            visible_rows_used += count.saturating_sub(scroll_sub_idx);
            inst += 1;
        }
        // Remaining instructions: full visibility
        while inst < total_items && visible_rows_used < visible_height {
            let line = &app_state.disassembly[inst];
            visible_rows_used += get_visual_line_count_for_instruction(line, app_state);
            inst += 1;
        }
        inst // exclusive upper bound (instruction index)
    };

    // Optimization: Use cached arrows from AppState to avoid O(N) search per frame
    let mut relevant_arrows: Vec<ArrowInfo> = Vec::with_capacity(app_state.cached_arrows.len());

    for arrow in &app_state.cached_arrows {
        let src_idx = arrow.start;
        let dst_idx = arrow.end;

        let low = std::cmp::min(src_idx, dst_idx);
        let high = std::cmp::max(src_idx, dst_idx);

        // Check if arrow overlaps with visible area
        if low < end_view && high >= scroll_inst_idx {
            let target_addr_val = arrow.target_addr.map_or(0, |a| a.0);

            // Check if it is an exact match to the line address
            // If exact match, we don't treat it as "relative target" (no special tip)
            // If it's midway (e.g. branch to +1), relative_target is Some(addr).
            let exact_match = if let Some(line) = app_state.disassembly.get(dst_idx) {
                line.address == target_addr_val
            } else {
                false
            };

            let relative_target = if exact_match { None } else { arrow.target_addr };

            relevant_arrows.push(ArrowInfo {
                start: src_idx,
                end: dst_idx,
                col: 0,
                target_addr: relative_target.map(|a| a.0),
                start_visible: src_idx >= scroll_inst_idx && src_idx < end_view,
                end_visible: dst_idx >= scroll_inst_idx && dst_idx < end_view,
            });
        }
    }

    // Step 2: Assign columns to arrows

    // Sort: shortest arrows first? Or logic from ui.rs?
    // ui.rs: sorted_arrows.sort_by_key(|(src, dst, _)| (*src as isize - *dst as isize).abs());
    // Since we used struct ArrowInfo, we need to adapt.
    // We already pushed to relevant_arrows which is ArrowInfo.
    // We need to sort it.
    relevant_arrows.sort_by_key(|a| (a.start as isize - a.end as isize).abs());

    let max_allowed_cols = app_state.settings.max_arrow_columns;
    let view_start = scroll_inst_idx;
    let view_end = end_view; // Use the accurate value computed above

    // Split into Full and Partial
    let (full_arrows, mut partial_arrows): (Vec<_>, Vec<_>) =
        relevant_arrows.into_iter().partition(|a| {
            let start_visible = a.start >= view_start && a.start < view_end;
            let end_visible = a.end >= view_start && a.end < view_end;
            start_visible && end_visible
        });

    // 1. Process Full Arrows: Prefer Inner (Rightmost) columns.
    // Logic restarted below

    let mut final_arrows = Vec::new();

    for mut arrow in full_arrows {
        let (range_low, range_high) = if arrow.start < arrow.end {
            (arrow.start, arrow.end)
        } else {
            (arrow.end, arrow.start)
        };

        let mut best_col = None;
        let mut col = (max_allowed_cols as isize) - 1;
        while col >= 0 {
            let has_conflict = final_arrows.iter().any(|a: &ArrowInfo| {
                if a.col != col as usize {
                    return false;
                }
                let (a_low, a_high) = if a.start_visible && a.end_visible {
                    if a.start < a.end {
                        (a.start, a.end)
                    } else {
                        (a.end, a.start)
                    }
                } else if a.start_visible {
                    if a.start < a.end {
                        (a.start, a.start + 1)
                    } else {
                        (a.start.saturating_sub(1), a.start)
                    }
                } else if a.end_visible {
                    if a.start < a.end {
                        (a.end.saturating_sub(1), a.end)
                    } else {
                        (a.end, a.end + 1)
                    }
                } else {
                    (0, 0)
                };
                !(a_high < range_low || a_low > range_high)
            });
            if !has_conflict {
                best_col = Some(col as usize);
                break;
            }
            col -= 1;
        }

        if let Some(c) = best_col {
            arrow.col = c;
            final_arrows.push(arrow);
        } else {
            partial_arrows.push(arrow);
        }
    }

    // 2. Process Partial Arrows
    for mut arrow in partial_arrows {
        // Re-check start/end visible from struct fields as they are correct
        let (range_low, range_high) = if arrow.start_visible {
            if arrow.start < arrow.end {
                (arrow.start, arrow.start + 1)
            } else {
                (arrow.start.saturating_sub(1), arrow.start)
            }
        } else if arrow.end_visible {
            if arrow.start < arrow.end {
                (arrow.end.saturating_sub(1), arrow.end)
            } else {
                (arrow.end, arrow.end + 1)
            }
        } else {
            continue;
        };

        let mut best_col = None;
        for col in 0..max_allowed_cols {
            let has_conflict = final_arrows.iter().any(|a| {
                if a.col != col {
                    return false;
                }
                let (a_low, a_high) = if a.start_visible && a.end_visible {
                    if a.start < a.end {
                        (a.start, a.end)
                    } else {
                        (a.end, a.start)
                    }
                } else if a.start_visible {
                    if a.start < a.end {
                        (a.start, a.start + 1)
                    } else {
                        (a.start.saturating_sub(1), a.start)
                    }
                } else if a.end_visible {
                    if a.start < a.end {
                        (a.end.saturating_sub(1), a.end)
                    } else {
                        (a.end, a.end + 1)
                    }
                } else {
                    (0, 0)
                };
                !(a_high < range_low || a_low > range_high)
            });
            if !has_conflict {
                best_col = Some(col);
                break;
            }
        }

        if let Some(c) = best_col {
            arrow.col = c;
            final_arrows.push(arrow);
        }
    }

    let active_arrows = final_arrows;
    // --- Arrow Calculation End ---

    let arrow_width = (app_state.settings.max_arrow_columns * 2) + 1;

    let get_arrow_str = |current_line: usize| -> String {
        let cols = app_state.settings.max_arrow_columns;
        let mut chars = vec![' '; cols * 2 + 1];

        if active_arrows.is_empty() {
            return chars.iter().collect();
        }

        for arrow in &active_arrows {
            let c_idx = arrow.col * 2;
            let is_down = arrow.start < arrow.end;
            let is_relative_target = arrow.target_addr.is_some() && current_line == arrow.end;

            let (low, high) = if is_down {
                (arrow.start, arrow.end)
            } else {
                (arrow.end, arrow.start)
            };

            // 1. Vertical Body
            // Draw vertical line if we are strictly inside the arrow span
            if current_line > low && current_line < high {
                if chars[c_idx] == ' ' {
                    chars[c_idx] = '│';
                } else if chars[c_idx] == '─' {
                    chars[c_idx] = '┼';
                }
            }

            // Relative target special vertical handling
            // If this is the ending line of the arrow, but it's a relative target (offset),
            // we might need to continue draw the vertical line through the instruction?
            // Logic:
            // Downward relative: target is somewhere inside this instruction or later.
            //   If later, well, `current_line` is `end`.
            //   If target is "inside", we generally draw `└` at `end`.
            //   Wait, `is_relative_target` means `target_addr` matches inside `end` line.
            //   If we are at `end`, and it's relative, do we stop here?
            //   The `target_addr` logic inside `render` handles drawing the `└` at the sub-line.
            //   For the main instruction line, if it's relative, we probably want `│`
            //   to reach the sub-line?
            //   If Unsure, check existing behavior:
            //   "if is_relative_target && !is_down && current_line == arrow.end { chars = | }"
            //   Upward relative: target is inside `end` (start < end? No, end < start).
            //   So arrow comes from below. Reaches `end`. Target is inside.
            //   The arrow line needs to go UP to the sub-line.
            //   Since instruction line is usually the "base", and sub-lines (comments/labels)
            //   are considered "above" or "inside"?
            //   Actually we index sub-lines for display.
            //   If target is inside, we need `│` on the base instruction line IF the target
            //   is "after" the base rendering?
            //   Actually, sub-lines (labels) are rendered BEFORE the instruction line.
            //   For Upward Relative:
            //      We come from below. We hit `end` (Instruction line).
            //      Target is one of the labels ABOVE.
            //      So we need `│` on the instruction line? No.
            //      The line comes from below. It passes `end+1`. It reaches `end`.
            //      It needs to reach the labels ABOVE `end`.
            //      So yes, it must pass through the instruction line to reach labels.
            //      So Upward Relative at `end` -> Need `│`.
            //   For Downward Relative:
            //      We come from above. We hit `end`. Target is label ABOVE?
            //      If target is label ABOVE instruction, we already passed it.
            //      So we don't need `│` on instruction line.
            //      If target is offset bytes INSIDE instruction (rare? jumps to middle of instruction?)
            //      Then we might need it. But typically relative targets are labels.

            if is_relative_target && !is_down && current_line == arrow.end && chars[c_idx] == ' ' {
                chars[c_idx] = '│';
            }

            // 2. Start Hook
            // Only if start is visible on this line
            if arrow.start_visible && current_line == arrow.start {
                if app_state.disassembly[current_line].target_address.is_some() {
                    chars[c_idx] = if is_down { '┌' } else { '└' };
                    chars[c_idx + 1] = '─';
                }
            }
            // 3. End Hook
            // Only if end is visible on this line
            else if arrow.end_visible && current_line == arrow.end && !is_relative_target {
                chars[c_idx] = if is_down { '└' } else { '┌' };
                chars[c_idx + 1] = '─';
            }

            // 4. Edge Pointers (Partial visibility)
            // If start visible but end NOT:
            if arrow.start_visible && !arrow.end_visible {
                // Check if we need to draw '▼' or '▲' just to indicate direction at edge?
                // Previous code:
                if is_down && current_line == arrow.start + 1 {
                    chars[c_idx] = '▼';
                } else if !is_down && current_line == arrow.start.saturating_sub(1) {
                    chars[c_idx] = '▲';
                }
            }
            // If end visible but start NOT:
            else if !arrow.start_visible
                && arrow.end_visible
                && ((is_down && current_line == arrow.end.saturating_sub(1))
                    || (!is_down && current_line == arrow.end + 1))
            {
                chars[c_idx] = '│'; // Ensure connection coming in?
            }

            // 5. Fill gaps for partial arrows that fully cross the screen
            // (Handled by Step 1 "Vertical Body" which checks low/high range)
        }

        // Horizontal connectors
        for arrow in &active_arrows {
            let is_relative_target = arrow.target_addr.is_some();
            let is_end_line = current_line == arrow.end;
            let is_start_line = current_line == arrow.start;

            let c_idx = arrow.col * 2;

            if arrow.start == arrow.end && current_line == arrow.start && arrow.start_visible {
                chars[c_idx] = '∞';
            }

            let is_valid_source = app_state.disassembly[current_line].target_address.is_some();
            // Safe start line: visible and is actually a control flow source
            let safe_is_start_line = is_start_line && arrow.start_visible && is_valid_source;

            // Draw horizontal line if:
            // 1. It's the start line (and valid)
            // 2. It's the end line (and visible), unless it's a relative target (handled elsewhere)

            let draw_horizontal = if arrow.start == arrow.end {
                safe_is_start_line
            } else {
                safe_is_start_line || (is_end_line && arrow.end_visible && !is_relative_target)
            };

            if draw_horizontal {
                for c in chars.iter_mut().skip(c_idx + 1) {
                    if *c == ' ' {
                        *c = '─';
                    } else if *c == '│' {
                        *c = '┼';
                    }
                }

                if is_end_line && arrow.end_visible {
                    let last = chars.len() - 1;
                    chars[last] = '►';
                }
            }
        }
        chars.iter().collect()
    };

    let get_comment_arrow_str = |current_line: usize, sub_addr: Option<u16>| -> String {
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

            let is_down = arrow.start < arrow.end;

            // Determine if this line should have a vertical pass-through bar.
            // 1. If strictly inside the arrow body, ALWAYS pass through.
            let strictly_inside = current_line > low && current_line < high;

            // 2. Boundary conditions for comments (which appear "above" the instruction line)
            // - If upward jump (start > end):
            //    - At `start`: Comments are above start. Arrow goes UP from start. So pass through.
            //    - At `end`: Comments are above end. Arrow comes DOWN to end. So pass through.
            // - If downward jump (start < end):
            //    - At `start`: Comments are above start. Arrow goes DOWN from start. No pass through.
            //    - At `end`: Comments are above end. Arrow comes DOWN to end. So pass through.

            let boundary_pass = if is_down {
                // Downward: Start -> End
                // Start line comments: No line (arrow starts below comments).
                // End line comments: Yes line (arrow arrives from above).
                current_line == arrow.end
            } else {
                // Upward: End <- Start
                // Start line comments: Yes line (arrow goes up from instruction, passing through comments above it).
                // End line comments: Yes line (arrow comes down to instruction, passing through comments above it... wait?)
                // Logic check: Upward arrow `└─` at start goes UP. Comments are above. So Yes.
                // Upward arrow `┌─` at end comes DOWN. Comments are above. So Yes?
                // Actually, standard representation:
                // Upward jump:
                //   End:  ┌─> Target
                //         │
                //   Start:└─ Source
                //
                // At End (Target): The arrow arrives from BELOW (it wraps around? No, it's just a line).
                // Visual:
                // JMP $1000  (Start)  └──────┐
                // ...                        │
                // $1000 NOP  (End)    <──────┘
                //
                // Wait, upward arrow usually drawn on the right in some tools, but here on left.
                // Left side upward jump:
                //    ┌─ $1000 (End)
                //    │
                //    └─ JMP $1000 (Start)
                //
                // So at End: Line comes from below? No, from Start (below) to End (above).
                // So at End, the hook is `┌─`. The vertical line is BELOW the hook.
                // Comments are ABOVE the instruction.
                // So comments at End are OUTSIDE the arrow span. No line.

                // At Start: Hook is `└─`. Vertical line is ABOVE the hook.
                // Comments are ABOVE the instruction.
                // So comments at Start are INSIDE the arrow span. Yes line.

                current_line == arrow.start
            };

            let mut passes_through = strictly_inside || boundary_pass;

            let is_relative_target_elsewhere =
                arrow.target_addr.is_some() && arrow.end == current_line;

            // Relative target adjustment
            if is_relative_target_elsewhere && let Some(target) = arrow.target_addr {
                if let Some(this_addr) = sub_addr {
                    // If inside the target line, check if we passed the specific address
                    if arrow.start < arrow.end {
                        // Downward
                        // Target is inside line. We stop at target.
                        passes_through = this_addr < target;
                    } else {
                        // Upward
                        // Target is inside line. We are coming from below.
                        // If we are at End, we are "above" the vertical span.
                        // But we logic said `boundary_pass` is FALSE for `current_line == arrow.end` (Upward).
                        // So `passes_through` is false.
                        // But checking `sub_addr`: if `this_addr < target`, are we inside?
                        // Upward jump arrives at target from below.
                        // If target is in middle of line:
                        //   [Addr 1]
                        //   [Addr 2] <- Target
                        //   [Addr 3]
                        // The line comes from below, reaches Addr 2, turns right.
                        // So Addr 3 "has line". Addr 1 "no line".
                        // So if `this_addr > target`, passes_through = true?
                        if sub_addr.is_some() {
                            passes_through = this_addr > target;
                        }
                    }
                } else {
                    // No sub-addr (comment).
                    // For upward jump at End: comments are above.
                    // Line comes from below to target.
                    // Comments are "before" target. So "no line"?
                    // Logic above said boundary_pass false for End/Upward.
                    // So this remains false. Correct.
                    // For downward jump at End: comments are above.
                    // Line comes from above to target.
                    // Comments are "before" target. So "yes line".
                    // Logic above said boundary_pass true for End/Downward.
                    // So remains true. Correct.
                }
            }

            if passes_through {
                chars[c_idx] = '│';
            }

            let is_target_here = if let Some(addr) = sub_addr
                && let Some(target) = arrow.target_addr
            {
                addr == target
            } else {
                false
            };

            if is_target_here {
                if arrow.start < arrow.end {
                    chars[c_idx] = '└';
                    chars[c_idx + 1] = '─';
                } else {
                    chars[c_idx] = '┌'; // Visual same as above logic
                    chars[c_idx + 1] = '─';
                }
            }
        }

        for arrow in &active_arrows {
            let is_target_here = if let Some(addr) = sub_addr
                && let Some(target) = arrow.target_addr
            {
                addr == target
            } else {
                false
            };

            if is_target_here {
                let c_idx = arrow.col * 2;

                for c in chars.iter_mut().skip(c_idx + 1) {
                    if *c == ' ' {
                        *c = '─';
                    } else if *c == '│' {
                        *c = '┼';
                    }
                }
                let last = chars.len() - 1;
                chars[last] = '►';
            }
        }
        chars.iter().collect()
    };

    // Render Loop: Generate ListItems starting from scroll_inst_idx, scroll_sub_idx
    let mut items = Vec::new();
    let mut current_inst = scroll_inst_idx;
    let mut current_sub = scroll_sub_idx;
    let mut processed_visual_lines = 0;

    // let mut arrow_calc_offset_map = Vec::new(); // Map visual line index (0..visible) to instruction index for arrows

    while processed_visual_lines < visible_height && current_inst < total_items {
        let line = &app_state.disassembly[current_inst];
        // let line_visual_count = get_visual_line_count_for_instruction(line, app_state);

        // We need to generate the specific visual lines for this instruction, starting from current_sub
        // Since the original generate logic produced a single ListItem with multiple Lines,
        // we now need to replicate that logic but conditionally extracting single Lines.

        // Helper specific to this instruction to get "all visual parts" in order
        // 1. Labels [1..len]
        // 2. Comments (lines)
        // 3. Instruction itself

        let mut parts = Vec::new();

        // Part generation logic (copied/adapted from previous render)
        let is_cursor_row = current_inst == ui_state.cursor_index;
        let is_selected_block = if let Some(selection_start) = ui_state.selection_start {
            let (start, end) = if selection_start < ui_state.cursor_index {
                (selection_start, ui_state.cursor_index)
            } else {
                (ui_state.cursor_index, selection_start)
            };
            current_inst >= start && current_inst <= end
        } else {
            false
        };

        let offset = line.address.0.saturating_sub(app_state.origin.0) as usize;
        let is_unexplored = line.external_label_address.is_none()
            && matches!(
                app_state.block_types.get(offset),
                Some(&regenerator2000_core::state::BlockType::Undefined)
            );

        let mut base_style = if is_selected_block {
            Style::default()
                .bg(ui_state.theme.selection_bg)
                .fg(ui_state.theme.selection_fg)
        } else {
            Style::default()
        };

        if is_unexplored && !is_selected_block {
            base_style = base_style.add_modifier(Modifier::DIM);
        }

        // 1. Labels
        if line.bytes.len() > 1 {
            for offset in 1..line.bytes.len() {
                let mid_addr = line.address.wrapping_add(offset as u16);
                if let Some(labels) = app_state.labels.get(&mid_addr) {
                    // XREF logic
                    let xref_str = if let Some(refs) = app_state.cross_refs.get(&mid_addr) {
                        if !refs.is_empty() && app_state.settings.max_xref_count > 0 {
                            format!(
                                "{} {}",
                                formatter.comment_prefix(),
                                crate::disassembler::format_cross_references(
                                    refs,
                                    app_state.settings.max_xref_count
                                )
                            )
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    for label in labels {
                        let arrow_padding = get_comment_arrow_str(current_inst, Some(mid_addr.0)); // Need accurate active_arrows calculate first?
                        // Arrows calculation depends on 'offset' which is scroll_index (inst).
                        // But with smooth scrolling, arrows should be calculated based on the window.
                        // The arrow logic "get_arrow_str" takes 'current_line' (instruction index).
                        // So it remains valid as long as we pass 'current_inst'.
                        // The VISUAL clipping in arrow logic used 'offset' and 'visible_height'.
                        // This might need tweak if we show "half an instruction".
                        // For now we assume arrow logic works on Instruction granularity.

                        let is_bookmarked = app_state
                            .annotations
                            .get(mid_addr)
                            .and_then(|e| e.bookmark.as_ref())
                            .is_some();
                        let has_breakpoint = app_state.vice_state.has_breakpoint_at(mid_addr.0);
                        let gutter = if has_breakpoint {
                            "  *  "
                        } else if is_bookmarked {
                            "  +  "
                        } else {
                            "     "
                        };
                        let gutter_style = if has_breakpoint {
                            base_style
                                .fg(ui_state.theme.error_fg)
                                .add_modifier(Modifier::BOLD)
                        } else if is_bookmarked {
                            base_style.fg(ui_state.theme.label)
                        } else {
                            base_style.fg(ui_state.theme.bytes)
                        };

                        let mut name = label.name.clone();
                        if label.label_type == crate::state::LabelType::LocalUserDefined
                            && let Some(p) = formatter.local_label_prefix()
                            && !name.starts_with(p)
                        {
                            name = format!("{}{}", p, name);
                        }
                        let label_def = format!("{} =*+${:02x}", name, offset);
                        let mut spans = vec![
                            Span::styled(gutter, gutter_style),
                            Span::styled(
                                format!("{arrow_padding:<arrow_width$} "),
                                Style::default().fg(ui_state.theme.arrow),
                            ),
                            Span::styled("                 ".to_string(), base_style),
                            Span::styled(
                                format!("{label_def:<40}"),
                                base_style.fg(ui_state.theme.label_def),
                            ),
                        ];
                        if !xref_str.is_empty() {
                            spans.push(Span::styled(
                                xref_str.clone(),
                                base_style.fg(ui_state.theme.comment),
                            ));
                        }
                        parts.push(Line::from(spans));
                    }
                }
            }
        }

        // 2. Comments
        if let Some(line_comment) = &line.line_comment {
            for comment_part in line_comment.lines() {
                let arrow_padding = get_comment_arrow_str(current_inst, None);
                parts.push(Line::from(vec![
                    Span::styled("     ", base_style.fg(ui_state.theme.bytes)),
                    Span::styled(
                        format!("{arrow_padding:arrow_width$} "),
                        Style::default().fg(ui_state.theme.arrow),
                    ),
                    Span::styled("                 ".to_string(), base_style),
                    Span::styled(
                        format!("{} {}", formatter.comment_prefix(), comment_part),
                        base_style.fg(ui_state.theme.comment),
                    ),
                ]));
            }
        }

        let prefix_parts_count = parts.len();

        // 3. Instruction
        let show_address = !line.bytes.is_empty() || line.is_collapsed || line.label.is_some();
        let address_str = if show_address {
            format!("${:04X}  ", line.address)
        } else {
            "       ".to_string()
        };

        let label_text = if let Some(label) = &line.label {
            formatter.format_label_definition(label)
        } else {
            String::new()
        };
        let arrow_padding = get_arrow_str(current_inst);

        let is_bookmarked = app_state
            .annotations
            .get(line.address)
            .and_then(|e| e.bookmark.as_ref())
            .is_some();
        let is_pc = app_state.vice_state.pc == Some(line.address.0);
        let has_breakpoint = app_state.vice_state.has_breakpoint_at(line.address.0);
        let gutter = if is_pc && has_breakpoint {
            " *>  "
        } else if is_pc {
            "  >  "
        } else if has_breakpoint {
            "  *  "
        } else if is_bookmarked {
            "  +  "
        } else {
            "     "
        };

        let gutter_style = if is_pc && has_breakpoint {
            base_style
                .fg(ui_state.theme.error_fg)
                .add_modifier(Modifier::BOLD)
        } else if is_pc {
            base_style
                .fg(ui_state.theme.border_active)
                .add_modifier(Modifier::BOLD)
        } else if has_breakpoint {
            base_style
                .fg(ui_state.theme.error_fg)
                .add_modifier(Modifier::BOLD)
        } else if is_bookmarked {
            base_style.fg(ui_state.theme.label)
        } else {
            base_style.fg(ui_state.theme.bytes)
        };

        let mut inst_spans = vec![
            Span::styled(gutter, gutter_style),
            Span::styled(
                format!("{arrow_padding:<arrow_width$} "),
                Style::default().fg(ui_state.theme.arrow),
            ),
            Span::styled(address_str.clone(), base_style.fg(ui_state.theme.address)),
            Span::styled(
                format!(
                    "{: <10}",
                    if line.show_bytes {
                        hex_bytes(&line.bytes)
                    } else {
                        String::new()
                    }
                ),
                base_style.fg(ui_state.theme.bytes),
            ),
        ];

        if line.mnemonic == "{splitter}" {
            let prefix_len = gutter.len() + arrow_width + address_str.len() + 10;
            let ruler_len = (list_area.width as usize).saturating_sub(prefix_len + 1);
            let ruler = "─".repeat(ruler_len);
            inst_spans.push(Span::styled(
                ruler,
                base_style
                    .fg(ui_state.theme.comment)
                    .add_modifier(Modifier::DIM),
            ));
        } else if line.is_collapsed {
            inst_spans.push(Span::styled(
                line.mnemonic.clone(),
                base_style
                    .fg(ui_state.theme.collapsed_block)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            let cp = formatter.comment_prefix();
            let is_enum = line.bytes.is_empty()
                && line.address == crate::state::Addr::ZERO
                && line.external_label_address.is_none()
                && !line.mnemonic.is_empty()
                && !line.mnemonic.starts_with(cp);

            let is_unindented_scope = (line.mnemonic == "{" || line.mnemonic == "}") && !is_enum;
            // If the label is too long for the column, or if it's an unindented scope and has a label, put it on its own line
            let label_on_own_line = label_text.len() >= LABEL_COLUMN_WIDTH
                || (is_unindented_scope && !label_text.is_empty());
            if label_on_own_line && !label_text.is_empty() {
                let label_arrow_padding = get_comment_arrow_str(current_inst, None);
                parts.push(Line::from(vec![
                    Span::styled("     ", base_style),
                    Span::styled(
                        format!("{label_arrow_padding:<arrow_width$} "),
                        Style::default().fg(ui_state.theme.arrow),
                    ),
                    Span::styled("                 ".to_string(), base_style),
                    Span::styled(
                        label_text.clone(),
                        base_style
                            .fg(ui_state.theme.label_def)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }

            let label_column_text = if is_unindented_scope {
                format!("{:<LABEL_COLUMN_WIDTH$}", line.mnemonic)
            } else if label_on_own_line || label_text.is_empty() {
                format!("{: <width$}", "", width = LABEL_COLUMN_WIDTH)
            } else {
                format!("{label_text: <LABEL_COLUMN_WIDTH$}")
            };

            let label_style = if is_unindented_scope {
                base_style
                    .fg(ui_state.theme.mnemonic)
                    .add_modifier(Modifier::BOLD)
            } else {
                base_style
                    .fg(ui_state.theme.label_def)
                    .add_modifier(Modifier::BOLD)
            };

            // External-label section lines: both the definition lines
            // (equates like `name = $value`) and the header/legend comment
            // lines should render at the label column position so they
            // align with code labels.  The total width spans the label,
            // mnemonic (6) and operand (15) columns so trailing x-ref
            // comments stay aligned with code comments.
            let is_ext_section_line =
                line.bytes.is_empty() && !line.mnemonic.is_empty() && !is_unindented_scope;

            if is_ext_section_line {
                const EXT_COL_WIDTH: usize = LABEL_COLUMN_WIDTH + 6 + 15;
                let style = if line.external_label_address.is_some() {
                    base_style
                        .fg(ui_state.theme.label_def)
                        .add_modifier(Modifier::BOLD)
                } else if is_enum {
                    base_style.fg(ui_state.theme.label_def)
                } else {
                    base_style.fg(ui_state.theme.comment)
                };
                inst_spans.push(Span::styled(
                    format!("{:<EXT_COL_WIDTH$}", line.mnemonic),
                    style,
                ));
            } else {
                inst_spans.push(Span::styled(label_column_text, label_style));
                let mnemonic_text = if is_unindented_scope {
                    "      ".to_string()
                } else {
                    format!("{: <5} ", line.mnemonic)
                };

                inst_spans.push(Span::styled(
                    mnemonic_text,
                    base_style
                        .fg(ui_state.theme.mnemonic)
                        .add_modifier(Modifier::BOLD),
                ));
                inst_spans.push(Span::styled(
                    format!("{: <15}", line.operand),
                    base_style.fg(ui_state.theme.operand),
                ));
            }
            inst_spans.push(Span::styled(
                if line.comment.is_empty() {
                    String::new()
                } else {
                    format!("{} {}", formatter.comment_prefix(), line.comment)
                },
                base_style.fg(ui_state.theme.comment),
            ));
        }
        parts.push(Line::from(inst_spans));

        // Calculate lexical scope visual gutter connectors (`┌─`, `│ `, `└─`).
        let mut is_scope_start = false;
        let mut is_scope_end = false;
        let mut is_in_scope = false;

        // If the target assembler emits a dedicated closing directive (e.g. `.pend` in 64tass),
        // that synthetic directive line closes the scope box with `└─`.
        if let Some(pend_str) = formatter.format_scope_end()
            && line.mnemonic == pend_str
        {
            is_scope_end = true;
            is_in_scope = true;
        }

        if !is_scope_end {
            for (&start, &end) in app_state.annotations.scopes() {
                if line.address == start
                    && current_inst > 0
                    && app_state.disassembly[current_inst - 1].address == start
                {
                    // When multiple disassembly lines share the scope start address (e.g. a scope
                    // header directive line followed by the first instruction at `start`), only
                    // the first line at `start` draws the top corner (`┌─`); subsequent lines
                    // at `start` draw the vertical continuation bar (`│ `).
                    is_in_scope = true;
                    break;
                } else if line.address == start {
                    is_scope_start = true;
                    is_in_scope = true;
                    break;
                } else if line.address == end
                    && current_inst + 1 < app_state.disassembly.len()
                    && app_state.disassembly[current_inst + 1].address == end
                {
                    // Keep drawing the vertical bar (`│ `) on non-final lines sharing `end`.
                    is_in_scope = true;
                    break;
                } else if line.address == end {
                    // Assemblers with an explicit scope-end directive (e.g. `.pend`) close the box
                    // on the trailing directive line (handled above), whereas assemblers without
                    // one (e.g. ACME) close the box (`└─`) directly on the instruction at `end`.
                    if formatter.format_scope_end().is_some() {
                        is_in_scope = true;
                    } else {
                        is_scope_end = true;
                        is_in_scope = true;
                    }
                    break;
                } else if line.address > start && line.address < end {
                    is_in_scope = true;
                    break;
                }
            }
        }

        let scope_style = base_style.fg(ui_state.theme.block_scope_fg);
        let part_count = parts.len();

        // --- Emit processed parts ---
        for (idx, part) in parts.into_iter().enumerate() {
            // Prefix sub-lines (`idx < prefix_parts_count`, such as blank separators and line
            // comments above `start`) render outside the scope box (`"  "`), while the first
            // label/instruction sub-line (`idx == prefix_parts_count`) opens the box (`"┌─"`).
            let current_scope_char = if is_scope_start {
                if idx < prefix_parts_count {
                    "  "
                } else if idx == prefix_parts_count {
                    "┌─"
                } else {
                    "│ "
                }
            } else if is_scope_end {
                if idx == part_count - 1 {
                    "└─"
                } else {
                    "│ "
                }
            } else if is_in_scope {
                "│ "
            } else {
                "  "
            };

            let mut new_spans = vec![Span::styled(current_scope_char, scope_style)];
            new_spans.extend(part.spans);
            let manipulated_part = Line::from(new_spans);

            if idx >= current_sub {
                // This sub-part is visible
                // Check highlight for this specific sub-part
                let is_cursor_sub = is_cursor_row && idx == ui_state.sub_cursor_index;

                let style = if is_cursor_sub && !is_selected_block {
                    // Sub-cursor Highlight: use selection bg but strip DIM so that
                    // arrow spans (which use Style::default()) are not dimmed by the
                    // ListItem patch.
                    base_style
                        .bg(ui_state.theme.selection_bg)
                        .remove_modifier(Modifier::DIM)
                } else {
                    // Strip DIM from the ListItem-level style: DIM is already applied
                    // via base_style on the individual content spans (address, bytes,
                    // mnemonic, etc.). Keeping DIM here would patch arrow spans that
                    // intentionally use Style::default() and should stay bright.
                    base_style.remove_modifier(Modifier::DIM)
                };

                items.push(ListItem::new(manipulated_part).style(style));
                processed_visual_lines += 1;

                if processed_visual_lines >= visible_height {
                    break;
                }
            }
        }

        // Advance to next instruction
        let mut next = current_inst + 1;
        while next < total_items {
            let l = &app_state.disassembly[next];
            if !l.bytes.is_empty() || l.is_collapsed || l.label.is_some() || !l.mnemonic.is_empty()
            {
                break;
            }
            next += 1;
        }
        current_inst = next;
        current_sub = 0; // Reset sub for next instruction
    }

    // Render the static disassembly list into list_area (block already rendered above)
    let list = List::new(items);
    f.render_widget(list, list_area);

    // Update persistent state
    ui_state.scroll_index = scroll_inst_idx;
    ui_state.scroll_sub_index = scroll_sub_idx;
    ui_state.disassembly_viewport_height = visible_height;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::ui_state::UIState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use regenerator2000_core::state::{Addr, AppState, Assembler, BlockType};

    #[test]
    fn test_render_disassembly_fills_viewport() {
        let mut app_state = AppState::new();
        let code = vec![0xEA; 100]; // 100 NOP instructions
        app_state.load_binary(Addr(0x0801), code).unwrap();

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 80, 24);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        // Inner area height is 24 - 2 (borders) = 22 lines.
        // Check that lines 1 to 22 all contain disassembly address '$' prefix
        for y in 1..=22 {
            let line_str: String = (0..80)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect();
            assert!(
                line_str.contains('$'),
                "Row {} should contain disassembly address '$', but was: '{}'",
                y,
                line_str
            );
        }
    }

    #[test]
    fn test_render_disassembly_label_alignment() {
        let mut app_state = AppState::new();
        let code = vec![0xAD, 0x00, 0x10, 0x8D, 0x00, 0x20];
        app_state.load_binary(Addr(0x1000), code).unwrap();
        app_state.block_types = vec![BlockType::Code; 6];
        app_state.disassemble();

        if let Some(line) = app_state.disassembly.get_mut(0) {
            line.label = Some("short_lbl".to_string());
        }
        if let Some(line) = app_state.disassembly.get_mut(1) {
            line.label = Some("extremely_long_label_name_exceeding_sixteen".to_string());
        }

        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let area = Rect::new(0, 0, 100, 20);
                render_disassembly(f, area, &app_state, &mut ui_state);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();

        let mut short_lbl_line = String::new();
        let mut long_lbl_own_line = String::new();
        let mut long_lbl_inst_line = String::new();

        for y in 1..19 {
            let line_str: String = (0..100)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(String::new(), |c| c.symbol().to_string())
                })
                .collect();
            if line_str.contains("short_lbl") {
                short_lbl_line = line_str.clone();
            }
            if line_str.contains("extremely_long") {
                long_lbl_own_line = line_str;
                long_lbl_inst_line = (0..100)
                    .map(|x| {
                        buffer
                            .cell((x, y + 1))
                            .map_or(String::new(), |c| c.symbol().to_string())
                    })
                    .collect();
            }
        }

        let short_lbl_lower = short_lbl_line.to_lowercase();
        assert!(
            short_lbl_lower.contains("short_lbl"),
            "Short label should be rendered"
        );
        assert!(
            short_lbl_lower.contains("lda"),
            "Short label should be on same line as instruction"
        );

        let long_lbl_own_lower = long_lbl_own_line.to_lowercase();
        let long_lbl_inst_lower = long_lbl_inst_line.to_lowercase();
        assert!(
            long_lbl_own_lower.contains("extremely_long"),
            "Long label should be on its own line"
        );
        assert!(
            !long_lbl_own_lower.contains("sta"),
            "Long label line should NOT contain instruction mnemonic"
        );
        assert!(
            long_lbl_inst_lower.contains("sta"),
            "Instruction mnemonic should be on line following long label"
        );
    }

    // T5.3: Verify scope gutter rendering (`┌─`, `│ `, `└─`) with indexed `app_state.annotations.scopes()`
    #[test]
    fn test_disassembly_render_scope_gutter_with_indexed_scopes() {
        for assembler in [Assembler::Tass64, Assembler::Acme] {
            let mut app_state = AppState::new();
            app_state.load_binary(Addr(0x1000), vec![0xEA; 8]).unwrap();
            app_state.block_types = vec![BlockType::Code; 8];
            app_state.settings.assembler = assembler;
            // Scope covering $1001..=$1003 with a line comment at $1001 (multi-part start)
            app_state.annotations.update(Addr(0x1001), |e| {
                e.scope = Some(Addr(0x1003));
                e.user_line_comment = Some("scope header comment".to_string());
            });
            app_state.disassemble();

            let mut ui_state = UIState::new(Theme::default());
            let backend = TestBackend::new(100, 20);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|f| {
                    let area = Rect::new(0, 0, 100, 20);
                    render_disassembly(f, area, &app_state, &mut ui_state);
                })
                .unwrap();

            let buffer = terminal.backend().buffer();
            let rendered_rows: Vec<String> = (1..19)
                .map(|y| {
                    (0..100)
                        .map(|x| {
                            buffer
                                .cell((x, y))
                                .map_or(String::new(), |c| c.symbol().to_string())
                        })
                        .collect()
                })
                .collect();

            assert!(
                rendered_rows.iter().any(|r| r.contains("┌─")),
                "Expected scope start gutter '┌─' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
            assert!(
                rendered_rows.iter().any(|r| r.contains("│ ")),
                "Expected scope middle gutter '│ ' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
            assert!(
                rendered_rows.iter().any(|r| r.contains("└─")),
                "Expected scope end gutter '└─' for assembler {assembler:?}, rows: {rendered_rows:#?}"
            );
        }
    }
}
