//! Interactive TUI modal dialog for disambiguating multi-address "Jump to Operand" actions.
//!
//! When a disassembly line resolves to two or more distinct [`OperandJumpTarget`] destinations,
//! the core emits [`DialogType::JumpToOperand`](regenerator2000_core::event::DialogType::JumpToOperand)
//! and the TUI opens [`JumpToOperandDialog`]. Each row shows a `[1]`–`[9]` hotkey badge (or a
//! blank slot from item 10 onward) and the uppercase [`Addr`]. When present, it also shows an
//! aligned operand expression and a single-line preview after `│`. Supports `Up`/`Down`/`j`/`k`,
//! `PageUp`/`PageDown`, `Home`/`End`, `1`–`9`, `Enter`, `Esc`, mouse wheel, row click, `[x]`,
//! and outside left-click close.

use crate::state::AppState;
use crate::state::actions::AppAction;
use crate::ui::widget::{Widget, WidgetResult, create_dialog_block};
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, ListState, Paragraph};
use regenerator2000_core::state::types::{Addr, OperandJumpTarget};
use std::cell::{Cell, RefCell};

/// Fixed character width budget reserved per row in [`JumpToOperandDialog`] when a row shows an
/// expression column: `highlight (4) + badge (4) + addr (5) + gap (2) + divider (3) + min_preview (16)`.
const FIXED_ROW_OVERHEAD: usize = 34;

/// Prefix character width before the preview column when rows omit the expression column:
/// `highlight (4) + badge (4) + addr (5) + divider (3)`.
const NO_EXPR_PREVIEW_PREFIX: usize = 16;

/// Prefix character width before the expression column when rows show the expression column:
/// `highlight (4) + badge (4) + addr (5) + gap (2) + divider (3)`.
const EXPR_FIXED_OVERHEAD: usize = 18;

const MIN_MODAL_WIDTH: u16 = 64;
const MAX_MODAL_WIDTH: u16 = 96;
/// Minimum modal height: 2 borders + at least 2 target rows + 1 blank spacer + 1 footer row.
const MIN_MODAL_HEIGHT: u16 = 6;
/// Maximum modal height: 2 borders + up to 13 visible target rows + 1 blank spacer + 1 footer row.
const MAX_MODAL_HEIGHT: u16 = 17;
const FOOTER_TEXT: &str = "[\u{2191}/\u{2193}] Move  [Enter/1-9] Jump  [Esc] Cancel";

/// Interactive modal dialog for picking a destination when a disassembly line resolves to
/// multiple candidate operand addresses.
pub(crate) struct JumpToOperandDialog {
    /// Canonical 6502 [`Addr`] of the source line shown in the modal title.
    pub(crate) source_address: Addr,
    /// Ordered, deduplicated [`OperandJumpTarget`] choices in left-to-right operand order.
    pub(crate) targets: Vec<OperandJumpTarget>,
    /// Index of the currently highlighted item inside [`Self::targets`].
    pub(crate) selected_index: usize,
    /// Ratatui [`ListState`] cached across frames via [`RefCell`] so `render(&self, ...)` keeps
    /// viewport scroll `offset()` synchronized for scrolled lists and mouse click hit-testing.
    list_state: RefCell<ListState>,
    /// Outer dialog [`Rect`] recorded on each render to guard outside-click dismissal before the
    /// first frame and ignore stale `UIState::active_dialog_area` values from earlier dialogs.
    dialog_area: Cell<Rect>,
    /// Inner target list [`Rect`] recorded on each render for scrolled row hit-testing.
    list_area: Cell<Rect>,
}

impl JumpToOperandDialog {
    /// Creates a [`JumpToOperandDialog`] for `source_address` with the given candidate `targets`.
    #[must_use]
    pub(crate) fn new(source_address: Addr, targets: Vec<OperandJumpTarget>) -> Self {
        let mut list_state = ListState::default();
        if !targets.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            source_address,
            targets,
            selected_index: 0,
            list_state: RefCell::new(list_state),
            dialog_area: Cell::new(Rect::default()),
            list_area: Cell::new(Rect::default()),
        }
    }

    /// Clamps `index` into `self.targets` and synchronizes [`Self::list_state`].
    fn select_index(&mut self, index: usize) {
        if self.targets.is_empty() {
            self.selected_index = 0;
            self.list_state.borrow_mut().select(None);
            return;
        }
        let clamped = index.min(self.targets.len() - 1);
        self.selected_index = clamped;
        self.list_state.borrow_mut().select(Some(clamped));
    }

    /// Moves selection down by one entry, wrapping around to index `0`.
    pub(crate) fn next(&mut self) {
        if self.targets.is_empty() {
            return;
        }
        self.select_index((self.selected_index + 1) % self.targets.len());
    }

    /// Moves selection up by one entry, wrapping around to the last entry.
    pub(crate) fn previous(&mut self) {
        if self.targets.is_empty() {
            return;
        }
        let prev = if self.selected_index == 0 {
            self.targets.len() - 1
        } else {
            self.selected_index - 1
        };
        self.select_index(prev);
    }

    /// Jumps selection to the first candidate target.
    pub(crate) fn select_first(&mut self) {
        self.select_index(0);
    }

    /// Jumps selection to the last candidate target.
    pub(crate) fn select_last(&mut self) {
        if self.targets.is_empty() {
            self.select_index(0);
            return;
        }
        self.select_index(self.targets.len() - 1);
    }

    /// Moves selection down by one visible list page, clamping at the last candidate target.
    pub(crate) fn page_down(&mut self) {
        let step = usize::from(self.list_area.get().height.max(1));
        self.select_index(self.selected_index.saturating_add(step));
    }

    /// Moves selection up by one visible list page, clamping at the first candidate target.
    pub(crate) fn page_up(&mut self) {
        let step = usize::from(self.list_area.get().height.max(1));
        self.select_index(self.selected_index.saturating_sub(step));
    }

    /// Returns [`AppAction::NavigateToAddress`] for [`Self::selected_index`], or closes if empty.
    #[must_use]
    fn confirm_selected(&self) -> WidgetResult {
        if let Some(target) = self.targets.get(self.selected_index) {
            WidgetResult::Action(AppAction::NavigateToAddress(target.target_address))
        } else {
            WidgetResult::Close
        }
    }

    /// Computes the centered modal [`Rect`] clamped to terminal `area` dimensions.
    #[must_use]
    fn compute_dialog_rect(&self, area: Rect) -> Rect {
        let pct_width = u16::try_from(u32::from(area.width) * 65 / 100).unwrap_or(u16::MAX);
        let dialog_width = pct_width
            .clamp(MIN_MODAL_WIDTH, MAX_MODAL_WIDTH)
            .min(area.width);

        // Add 4 rows for top/bottom borders, the trailing blank spacer row, and the footer hint.
        let count_u16 = u16::try_from(self.targets.len()).unwrap_or(u16::MAX);
        let dialog_height = count_u16
            .saturating_add(4)
            .clamp(MIN_MODAL_HEIGHT, MAX_MODAL_HEIGHT)
            .min(area.height);

        let x = area
            .x
            .saturating_add(area.width.saturating_sub(dialog_width) / 2);
        let y = area
            .y
            .saturating_add(area.height.saturating_sub(dialog_height) / 2);
        Rect::new(x, y, dialog_width, dialog_height)
    }

    /// Formats the modal title, appending `[selected/total]` only when items overflow `list_height`.
    #[must_use]
    fn format_title(&self, list_height: u16) -> String {
        let overflows = !self.targets.is_empty() && self.targets.len() > usize::from(list_height);
        if overflows {
            let current = self.selected_index.min(self.targets.len() - 1) + 1;
            format!(
                " Jump to Operand from ${:04X} [{current}/{}] ",
                self.source_address.0,
                self.targets.len()
            )
        } else {
            format!(" Jump to Operand from ${:04X} ", self.source_address.0)
        }
    }

    /// Computes the aligned expression column width inside `inner_width`, or `0` when every entry
    /// repeats its raw `$XXXX` address.
    #[must_use]
    fn expr_column_width(&self, inner_width: usize) -> usize {
        let non_redundant_max = self
            .targets
            .iter()
            .filter(|t| !is_redundant_expr(t))
            .map(|t| display_width(t.operand_expr.trim()))
            .max()
            .unwrap_or(0);

        if non_redundant_max == 0 {
            return 0;
        }

        let budget = inner_width.saturating_sub(FIXED_ROW_OVERHEAD).max(8);
        let max_fit = inner_width.saturating_sub(EXPR_FIXED_OVERHEAD);
        non_redundant_max.min(budget).min(max_fit)
    }
}

/// Returns the terminal cell width of `text`.
#[must_use]
fn display_width(text: &str) -> usize {
    Span::raw(text).width()
}

/// Truncates `text` at a char boundary so its rendered width fits within `max_width` cells,
/// appending `'…'` when clipped.
#[must_use]
fn truncate_to_display_width(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width == 1 {
        return "\u{2026}".to_string();
    }
    let budget = max_width - 1;
    let mut out = String::new();
    let mut used = 0usize;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        let ch_str = ch.encode_utf8(&mut buf);
        let ch_w = Span::raw(&*ch_str).width();
        if used + ch_w > budget {
            break;
        }
        out.push(ch);
        used += ch_w;
    }
    out.push('\u{2026}');
    out
}

/// Pads `text` on the right with ASCII spaces to reach `target_width` terminal cells.
#[must_use]
fn pad_right_to_width(text: &str, target_width: usize) -> String {
    let w = display_width(text);
    if w >= target_width {
        return text.to_string();
    }
    let pad = target_width - w;
    let mut out = String::with_capacity(text.len() + pad);
    out.push_str(text);
    for _ in 0..pad {
        out.push(' ');
    }
    out
}

/// Returns `true` when `target.operand_expr` repeats `target.target_address` as bare `$XXXX` hex.
///
/// Compares case-insensitively because all four assembler formatters emit lowercase `$xxxx`
/// whereas the address column shows uppercase `$XXXX`.
#[must_use]
fn is_redundant_expr(target: &OperandJumpTarget) -> bool {
    let addr_hex = format!("${:04X}", target.target_address.0);
    target.operand_expr.trim().eq_ignore_ascii_case(&addr_hex)
}

/// Returns `true` when terminal column/row `(col, row)` falls inside `rect`.
#[must_use]
fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

impl Widget for JumpToOperandDialog {
    fn render(&self, f: &mut Frame, area: Rect, _app_state: &AppState, ui_state: &mut UIState) {
        let theme = &ui_state.theme;
        let dialog_area = self.compute_dialog_rect(area);
        // Mirror `dialog_area` into both `self` and `ui_state` so outside-click bounds and `[x]`
        // close-button hit-testing in `events.rs` stay synchronized.
        self.dialog_area.set(dialog_area);
        ui_state.active_dialog_area = dialog_area;

        if dialog_area.width == 0 || dialog_area.height == 0 {
            self.list_area.set(Rect::default());
            return;
        }

        f.render_widget(Clear, dialog_area);

        // Compute inner area dimensions identically to `Block::bordered().inner(dialog_area)`
        // so we know whether the list overflows before constructing the bordered title, reserving
        // a 1-row blank spacer above the footer whenever at least 3 inner rows exist.
        let inner_height = dialog_area.height.saturating_sub(2);
        let footer_height = u16::from(inner_height >= 1);
        let spacer_height = u16::from(inner_height >= 3);
        let list_height = inner_height.saturating_sub(footer_height + spacer_height);

        let title = self.format_title(list_height);
        let block = create_dialog_block(&title, theme);
        let inner = block.inner(dialog_area);
        f.render_widget(block, dialog_area);

        let list_rect = Rect::new(inner.x, inner.y, inner.width, list_height);
        let footer_rect = Rect::new(
            inner.x,
            inner.y.saturating_add(list_height + spacer_height),
            inner.width,
            footer_height,
        );
        self.list_area.set(list_rect);

        // Sync `ListState` if callers adjusted `selected_index` directly without navigation helpers.
        {
            let mut list_state = self.list_state.borrow_mut();
            let expected_sel =
                (!self.targets.is_empty()).then(|| self.selected_index.min(self.targets.len() - 1));
            if list_state.selected() != expected_sel {
                list_state.select(expected_sel);
            }
        }

        if list_rect.height > 0 && list_rect.width > 0 {
            let inner_width = usize::from(list_rect.width);
            let expr_col_width = self.expr_column_width(inner_width);
            let preview_budget = if expr_col_width > 0 {
                inner_width.saturating_sub(EXPR_FIXED_OVERHEAD + expr_col_width)
            } else {
                inner_width.saturating_sub(NO_EXPR_PREVIEW_PREFIX)
            };

            let items: Vec<ListItem<'_>> = self
                .targets
                .iter()
                .enumerate()
                .map(|(idx, target)| {
                    let badge = if idx < 9 {
                        format!("[{}] ", idx + 1)
                    } else {
                        // Keep a 4-char slot from item 10 onward so `$XXXX` columns stay aligned.
                        "    ".to_string()
                    };
                    let addr_text = format!("${:04X}", target.target_address.0);

                    let mut spans = vec![
                        Span::styled(badge, Style::default().fg(theme.highlight_fg)),
                        Span::styled(
                            addr_text,
                            Style::default()
                                .fg(theme.address)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];

                    if expr_col_width > 0 {
                        spans.push(Span::raw("  "));
                        // Suppress redundant `$xxxx` text on individual rows while keeping `│` aligned.
                        let raw_expr = if is_redundant_expr(target) {
                            ""
                        } else {
                            target.operand_expr.trim()
                        };
                        let truncated = truncate_to_display_width(raw_expr, expr_col_width);
                        let padded = pad_right_to_width(&truncated, expr_col_width);
                        spans.push(Span::styled(padded, Style::default().fg(theme.operand)));
                    }

                    if let Some(preview) = target.target_preview.as_deref() {
                        let clipped = truncate_to_display_width(preview.trim(), preview_budget);
                        spans.push(Span::styled(
                            " \u{2502} ",
                            Style::default().fg(theme.menu_disabled_fg),
                        ));
                        spans.push(Span::styled(clipped, Style::default().fg(theme.comment)));
                    }

                    ListItem::new(Line::from(spans))
                })
                .collect();

            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .bg(theme.menu_selected_bg)
                        .fg(theme.menu_selected_fg)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(" >> ");

            f.render_stateful_widget(list, list_rect, &mut *self.list_state.borrow_mut());
        }

        if footer_rect.height > 0 && footer_rect.width > 0 {
            let footer_text =
                truncate_to_display_width(FOOTER_TEXT, usize::from(footer_rect.width));
            let footer = Paragraph::new(footer_text)
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(theme.menu_disabled_fg)
                        .bg(theme.dialog_bg),
                );
            f.render_widget(footer, footer_rect);
        }
    }

    fn handle_input(
        &mut self,
        key: KeyEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        match key.code {
            KeyCode::Esc => {
                ui_state.set_status_message("Ready");
                WidgetResult::Close
            }
            // Require empty modifiers so modified chords like `Ctrl+j` or `Alt+1` cannot move
            // selection or fire quick jumps unexpectedly.
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.previous();
                WidgetResult::Handled
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.next();
                WidgetResult::Handled
            }
            KeyCode::Home if key.modifiers.is_empty() => {
                self.select_first();
                WidgetResult::Handled
            }
            KeyCode::End if key.modifiers.is_empty() => {
                self.select_last();
                WidgetResult::Handled
            }
            KeyCode::PageUp if key.modifiers.is_empty() => {
                self.page_up();
                WidgetResult::Handled
            }
            KeyCode::PageDown if key.modifiers.is_empty() => {
                self.page_down();
                WidgetResult::Handled
            }
            KeyCode::Enter if key.modifiers.is_empty() => self.confirm_selected(),
            KeyCode::Char(c @ '1'..='9') if key.modifiers.is_empty() => {
                let idx = (c as usize) - ('1' as usize);
                if let Some(target) = self.targets.get(idx) {
                    WidgetResult::Action(AppAction::NavigateToAddress(target.target_address))
                } else {
                    // Leave the modal open when the pressed digit exceeds `targets.len()`.
                    WidgetResult::Handled
                }
            }
            _ => WidgetResult::Handled,
        }
    }

    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        _app_state: &mut AppState,
        ui_state: &mut UIState,
    ) -> WidgetResult {
        // Ignore mouse events before the first `render()` call populates `dialog_area`, preventing
        // stale `UIState::active_dialog_area` bounds from dismissing the dialog prematurely.
        let dialog_area = self.dialog_area.get();
        if dialog_area.width == 0 || dialog_area.height == 0 {
            return WidgetResult::Handled;
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.previous();
                WidgetResult::Handled
            }
            MouseEventKind::ScrollDown => {
                self.next();
                WidgetResult::Handled
            }
            // Only left button press outside the dialog closes; ignore hover, drag, release, and
            // non-left button clicks.
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(dialog_area, mouse.column, mouse.row) {
                    ui_state.set_status_message("Ready");
                    return WidgetResult::Close;
                }
                let list_area = self.list_area.get();
                if rect_contains(list_area, mouse.column, mouse.row) {
                    let rel_row = usize::from(mouse.row.saturating_sub(list_area.y));
                    let clicked_idx = self.list_state.borrow().offset().saturating_add(rel_row);
                    if let Some(target) = self.targets.get(clicked_idx) {
                        let addr = target.target_address;
                        self.select_index(clicked_idx);
                        return WidgetResult::Action(AppAction::NavigateToAddress(addr));
                    }
                }
                WidgetResult::Handled
            }
            _ => WidgetResult::Handled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_targets(count: usize) -> Vec<OperandJumpTarget> {
        (0..count)
            .map(|i| {
                let offset = u16::try_from(i).expect("test target index should fit in u16");
                let addr = Addr(0x1000 + offset * 0x10);
                OperandJumpTarget::new(
                    addr,
                    format!("target_{i}"),
                    Some(format!("lda #${:02X}", i + 1)),
                )
            })
            .collect()
    }

    fn render_dialog(
        dialog: &JumpToOperandDialog,
        width: u16,
        height: u16,
    ) -> (Terminal<TestBackend>, UIState) {
        let app_state = AppState::new();
        let mut ui_state = UIState::new(Theme::default());
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("creating test terminal should succeed");
        terminal
            .draw(|f| dialog.render(f, f.area(), &app_state, &mut ui_state))
            .expect("drawing JumpToOperandDialog should succeed");
        (terminal, ui_state)
    }

    fn row_text(terminal: &Terminal<TestBackend>, row: u16) -> String {
        let buf = terminal.backend().buffer();
        (0..buf.area.width)
            .map(|x| buf[(x, row)].symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn full_screen_text(terminal: &Terminal<TestBackend>) -> String {
        let h = terminal.backend().buffer().area.height;
        (0..h)
            .map(|r| row_text(terminal, r))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn render_displays_title_hotkey_badges_unselected_tenth_blank_spacer_and_footer() {
        // 3 targets on 80x24: modal width [64..=96], height = 7, list height = 4 (3 items + spacer).
        let targets = vec![
            OperandJumpTarget::new(
                Addr(0x0835),
                "<state_0_border_red".to_string(),
                Some("lda #VicColor.RED".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x083F),
                "<state_1_border_green".to_string(),
                Some("lda #VicColor.GREEN".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x0849),
                "<state_2_border_blue".to_string(),
                Some("lda #VicColor.BLUE".to_string()),
            ),
        ];
        let dialog = JumpToOperandDialog::new(Addr(0x0832), targets);
        let (terminal, _ui) = render_dialog(&dialog, 80, 24);
        let dialog_area = dialog.dialog_area.get();
        let list_area = dialog.list_area.get();

        assert!((64..=96).contains(&dialog_area.width));
        assert_eq!(dialog_area.height, 7);
        assert_eq!(list_area.height, 3);

        let screen = full_screen_text(&terminal);
        assert!(screen.contains("Jump to Operand from $0832"));
        assert!(!screen.contains("[1/3]"));
        assert!(screen.contains("[x]"));
        assert!(screen.contains("[1] $0835"));
        assert!(screen.contains("lda #VicColor.RED"));
        assert!(screen.contains("[2]"));
        assert!(screen.contains("[3]"));
        assert!(screen.contains("Move") && screen.contains("Jump") && screen.contains("Cancel"));

        // Dedicated 1-row spacer between last item (`list_area.y + 2`) and footer (`list_area.y + 4`) is blank.
        let spacer_row = row_text(&terminal, list_area.y + 3);
        let inner_slice: String = spacer_row
            .chars()
            .skip(usize::from(list_area.x))
            .take(usize::from(list_area.width))
            .collect();
        assert!(
            inner_slice.trim().is_empty(),
            "expected blank spacer row: {inner_slice:?}"
        );
        let footer_row = row_text(&terminal, list_area.y + 4);
        assert!(footer_row.contains("Move") && footer_row.contains("Cancel"));

        // 11 targets: 10th & 11th items have blank 4-char hotkey slot (no `[10]` or `[0]`).
        let eleven = JumpToOperandDialog::new(Addr(0x0900), make_targets(11));
        let (term11, _) = render_dialog(&eleven, 80, 24);
        let screen11 = full_screen_text(&term11);
        assert!(screen11.contains("[9]"));
        assert!(!screen11.contains("[10]"));
        assert!(!screen11.contains("[0]"));
        assert!(screen11.contains("$1090"));
    }

    #[test]
    fn render_suppresses_case_insensitive_redundant_expr_and_omits_column_when_all_redundant() {
        // (a) All redundant (`$0835`, lowercase `$083f`): expr column and 2-char gap omitted completely.
        let all_redundant = vec![
            OperandJumpTarget::new(
                Addr(0x0835),
                "$0835".to_string(),
                Some("lda #VicColor.RED".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x083F),
                "$083f".to_string(),
                Some("lda #VicColor.GREEN".to_string()),
            ),
        ];
        let dlg_a = JumpToOperandDialog::new(Addr(0x082F), all_redundant);
        let (term_a, _) = render_dialog(&dlg_a, 80, 24);
        let screen_a = full_screen_text(&term_a);
        assert!(screen_a.contains("$0835 \u{2502} lda #VicColor.RED"));
        assert!(screen_a.contains("$083F \u{2502} lda #VicColor.GREEN"));
        assert!(!screen_a.contains("$083f"));

        // (b) Mixed redundant + symbolic: divider columns align vertically, `$083f` suppressed.
        let mixed = vec![
            OperandJumpTarget::new(
                Addr(0x0835),
                "state_0".to_string(),
                Some("lda #VicColor.RED".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x083F),
                "$083f".to_string(),
                Some("lda #VicColor.GREEN".to_string()),
            ),
        ];
        let dlg_b = JumpToOperandDialog::new(Addr(0x082F), mixed);
        let (term_b, _) = render_dialog(&dlg_b, 80, 24);
        let list_area = dlg_b.list_area.get();
        let row0 = row_text(&term_b, list_area.y);
        let row1 = row_text(&term_b, list_area.y + 1);
        assert!(row0.contains("state_0"));
        assert!(!row1.contains("$083f"));

        let div0 = row0.chars().position(|c| c == '\u{2502}');
        let div1 = row1.chars().position(|c| c == '\u{2502}');
        assert!(div0.is_some());
        assert_eq!(div0, div1);
    }

    #[test]
    fn render_clamps_long_expression_at_budget_with_min_16_preview() {
        let long_expr = "a_very_long_symbolic_offset_expression_identifier_over_50_chars";
        let targets = vec![OperandJumpTarget::new(
            Addr(0x1020),
            long_expr.to_string(),
            Some("sta $D020,x ; border color update".to_string()),
        )];
        let dialog = JumpToOperandDialog::new(Addr(0x1000), targets);
        let (terminal, _) = render_dialog(&dialog, 80, 24);
        let list_area = dialog.list_area.get();
        let line = row_text(&terminal, list_area.y);

        assert!(line.contains('\u{2026}'));
        // At least 16 chars must remain after "│ " for the preview text.
        let after_div = line
            .split(" \u{2502} ")
            .nth(1)
            .expect("rendered row should contain column divider");
        let preview_part = after_div
            .split('\u{2502}')
            .next()
            .expect("rendered row should contain trailing border")
            .trim_end();
        assert!(
            display_width(preview_part) >= 16,
            "preview width too narrow: {preview_part:?}"
        );
    }

    #[test]
    fn render_truncates_utf8_and_wide_emoji_without_panicking_or_overflowing_border() {
        let targets = vec![
            OperandJumpTarget::new(
                Addr(0x2000),
                "\u{1F680}\u{1F525}\u{1F47E}\u{1F3AE}_caf\u{00E9}_na\u{00EF}ve_r\u{00E9}sum\u{00E9}_very_long_expression_label"
                    .to_string(),
                Some("lda #$01 ; \u{1F680}\u{1F525}\u{1F47E}\u{1F3AE}_wide_utf8_comment_overflow_check".to_string()),
            ),
            OperandJumpTarget::new(
                Addr(0x2010),
                "short_expr".to_string(),
                Some("rts".to_string()),
            ),
        ];
        let dialog = JumpToOperandDialog::new(Addr(0x1800), targets);
        let (terminal, _) = render_dialog(&dialog, 64, 24);
        let dialog_area = dialog.dialog_area.get();
        let buf = terminal.backend().buffer();

        // Every row of the dialog retains its right box-drawing border cell cleanly.
        let right_x = dialog_area.right() - 1;
        for row in dialog_area.y..dialog_area.bottom() {
            let sym = buf[(right_x, row)].symbol();
            assert!(
                matches!(sym, "\u{2510}" | "\u{2502}" | "\u{2518}"),
                "right border corrupted at ({right_x}, {row}): {sym:?}"
            );
        }
        let screen = full_screen_text(&terminal);
        assert!(screen.contains('\u{2026}'));
    }

    #[test]
    fn render_shows_overflow_counter_only_when_list_overflows_height() {
        let assert_spacer_row_blank =
            |terminal: &Terminal<TestBackend>, list_area: Rect, label: &str| {
                let spacer_row = row_text(terminal, list_area.y + list_area.height);
                let inner_slice: String = spacer_row
                    .chars()
                    .skip(usize::from(list_area.x))
                    .take(usize::from(list_area.width))
                    .collect();
                assert!(
                    inner_slice.trim().is_empty(),
                    "expected blank spacer row for {label}: {inner_slice:?}"
                );
                let footer_row = row_text(terminal, list_area.y + list_area.height + 1);
                assert!(
                    footer_row.contains("Move") && footer_row.contains("Cancel"),
                    "expected footer directly below spacer for {label}: {footer_row:?}"
                );
            };

        // (a) 14 targets on 80x24 -> height capped at 17, list height = 13 -> overflows, [10/14], spacer preserved.
        let mut dlg14 = JumpToOperandDialog::new(Addr(0x0900), make_targets(14));
        dlg14.selected_index = 9;
        let (term14, _) = render_dialog(&dlg14, 80, 24);
        assert_eq!(dlg14.dialog_area.get().height, 17);
        assert_eq!(dlg14.list_area.get().height, 13);
        assert_spacer_row_blank(&term14, dlg14.list_area.get(), "14 targets at max height");
        let screen14 = full_screen_text(&term14);
        assert!(screen14.contains("Jump to Operand from $0900 [10/14]"));
        assert!(screen14.contains("$1090"));

        // (b) 13 targets on 80x24 -> height 17, list height = 13 -> fits without counter suffix, spacer preserved.
        let dlg13 = JumpToOperandDialog::new(Addr(0x0900), make_targets(13));
        let (term13, _) = render_dialog(&dlg13, 80, 24);
        assert_eq!(dlg13.dialog_area.get().height, 17);
        assert_eq!(dlg13.list_area.get().height, 13);
        assert_spacer_row_blank(&term13, dlg13.list_area.get(), "13 targets exact fit");
        let top_border13 = row_text(&term13, dlg13.dialog_area.get().y);
        assert!(top_border13.contains("Jump to Operand from $0900"));
        assert!(!top_border13.contains("[1/13]"));

        // (c) 10 targets on 80x10 -> compressed terminal overflows, [1/10].
        let dlg10_short = JumpToOperandDialog::new(Addr(0x0900), make_targets(10));
        let (term10_short, _) = render_dialog(&dlg10_short, 80, 10);
        let screen10_short = full_screen_text(&term10_short);
        assert!(screen10_short.contains("Jump to Operand from $0900 [1/10]"));

        // (d) 10 targets on 80x24 -> fits within list height (13), no counter suffix!
        let dlg10_fit = JumpToOperandDialog::new(Addr(0x0900), make_targets(10));
        let (term10_fit, _) = render_dialog(&dlg10_fit, 80, 24);
        let top_border = row_text(&term10_fit, dlg10_fit.dialog_area.get().y);
        assert!(top_border.contains("Jump to Operand from $0900"));
        assert!(!top_border.contains("[1/10]"));
    }

    #[test]
    fn render_short_and_narrow_terminals_never_panic_or_underflow() {
        for (w, h) in [(80u16, 3u16), (20u16, 3u16), (20u16, 2u16)] {
            let dialog = JumpToOperandDialog::new(Addr(0x0832), make_targets(10));
            let (terminal, _) = render_dialog(&dialog, w, h);
            let list_area = dialog.list_area.get();
            assert_eq!(list_area.height, 0);
            assert!(dialog.dialog_area.get().bottom() <= h);
            if h >= 3 {
                let screen = full_screen_text(&terminal);
                assert!(!screen.is_empty());
            }
        }

        // Empty targets on standard 80x24 renders cleanly with minimum height 6.
        let empty = JumpToOperandDialog::new(Addr(0x1000), Vec::new());
        let (empty_term, _) = render_dialog(&empty, 80, 24);
        assert_eq!(empty.dialog_area.get().height, 6);
        assert!(full_screen_text(&empty_term).contains("Jump to Operand from $1000"));
    }

    #[test]
    fn keyboard_up_down_j_k_home_end_wrap_and_sync_list_state() {
        let mut dialog = JumpToOperandDialog::new(Addr(0x0832), make_targets(4));
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());

        assert_eq!(dialog.selected_index, 0);
        assert_eq!(dialog.list_state.borrow().selected(), Some(0));

        // Up from 0 wraps to 3.
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                &mut app,
                &mut ui
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.selected_index, 3);
        assert_eq!(dialog.list_state.borrow().selected(), Some(3));

        // Down from 3 wraps to 0.
        assert_eq!(
            dialog.handle_input(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &mut app,
                &mut ui
            ),
            WidgetResult::Handled
        );
        assert_eq!(dialog.selected_index, 0);
        assert_eq!(dialog.list_state.borrow().selected(), Some(0));

        // 'j' moves to 1, 'k' moves back to 0.
        dialog.handle_input(
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dialog.selected_index, 1);
        assert_eq!(dialog.list_state.borrow().selected(), Some(1));

        dialog.handle_input(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dialog.selected_index, 0);
        assert_eq!(dialog.list_state.borrow().selected(), Some(0));

        // End / Home.
        dialog.handle_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dialog.selected_index, 3);
        assert_eq!(dialog.list_state.borrow().selected(), Some(3));

        dialog.handle_input(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dialog.selected_index, 0);
        assert_eq!(dialog.list_state.borrow().selected(), Some(0));

        // Empty targets never panic.
        let mut empty = JumpToOperandDialog::new(Addr(0x0832), Vec::new());
        empty.handle_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        empty.handle_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        empty.handle_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(empty.selected_index, 0);
    }

    #[test]
    fn keyboard_page_up_and_page_down_step_by_visible_height_and_clamp_at_ends() {
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        let mut dlg = JumpToOperandDialog::new(Addr(0x0900), make_targets(20));

        // Before rendering (`list_area.height == 0`), step falls back to `max(1)` cleanly.
        assert_eq!(
            dlg.handle_input(
                KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
                &mut app,
                &mut ui
            ),
            WidgetResult::Handled
        );
        assert_eq!(dlg.selected_index, 1);
        assert_eq!(
            dlg.handle_input(
                KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
                &mut app,
                &mut ui
            ),
            WidgetResult::Handled
        );
        assert_eq!(dlg.selected_index, 0);

        // Render at 80x24 -> `dialog_area.height == 17`, `list_area.height == 13`.
        let (_term, _ui) = render_dialog(&dlg, 80, 24);
        assert_eq!(dlg.list_area.get().height, 13);

        // PageDown steps `0 -> 13`, then clamps to `19` (never wrapping to `0`).
        dlg.handle_input(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dlg.selected_index, 13);
        assert_eq!(dlg.list_state.borrow().selected(), Some(13));

        dlg.handle_input(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dlg.selected_index, 19);
        assert_eq!(dlg.list_state.borrow().selected(), Some(19));

        // PageUp steps `19 -> 6`, then clamps to `0` (never wrapping to `19`).
        dlg.handle_input(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dlg.selected_index, 6);
        assert_eq!(dlg.list_state.borrow().selected(), Some(6));

        dlg.handle_input(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(dlg.selected_index, 0);
        assert_eq!(dlg.list_state.borrow().selected(), Some(0));

        // Modified PageUp/PageDown keys are ignored without moving selection.
        dlg.handle_input(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL),
            &mut app,
            &mut ui,
        );
        dlg.handle_input(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::ALT),
            &mut app,
            &mut ui,
        );
        assert_eq!(dlg.selected_index, 0);
    }

    #[test]
    fn keyboard_numeric_hotkeys_1_through_9_jump_and_out_of_range_ignored() {
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());

        let mut dlg11 = JumpToOperandDialog::new(Addr(0x0900), make_targets(11));
        for (ch, expected_idx) in [('1', 0usize), ('2', 1), ('9', 8)] {
            let res = dlg11.handle_input(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app,
                &mut ui,
            );
            assert_eq!(
                res,
                WidgetResult::Action(AppAction::NavigateToAddress(
                    dlg11.targets[expected_idx].target_address
                ))
            );
        }

        // Out-of-range hotkeys and non-1..9 chars stay open without mutating selection.
        let mut dlg2 = JumpToOperandDialog::new(Addr(0x0832), make_targets(2));
        for ch in ['3', '9', '0', 'a'] {
            let res = dlg2.handle_input(
                KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                &mut app,
                &mut ui,
            );
            assert_eq!(res, WidgetResult::Handled);
            assert_eq!(dlg2.selected_index, 0);
        }
    }

    #[test]
    fn keyboard_enter_confirms_selected_and_esc_closes() {
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        ui.set_status_message("Jump pending");
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(3));

        dlg.handle_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut app,
            &mut ui,
        );
        assert_eq!(
            dlg.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app,
                &mut ui,
            ),
            WidgetResult::Action(AppAction::NavigateToAddress(Addr(0x1010)))
        );

        assert_eq!(
            dlg.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app,
                &mut ui,
            ),
            WidgetResult::Close
        );
        assert_eq!(ui.status_message.as_deref(), Some("Ready"));

        // 0-target dialog: Enter closes, '1' is handled safely, Esc closes and resets status.
        let mut empty = JumpToOperandDialog::new(Addr(0x0832), Vec::new());
        assert_eq!(
            empty.handle_input(
                KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
                &mut app,
                &mut ui,
            ),
            WidgetResult::Handled
        );
        assert_eq!(
            empty.handle_input(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut app,
                &mut ui,
            ),
            WidgetResult::Close
        );
        ui.set_status_message("Jump pending");
        assert_eq!(
            empty.handle_input(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &mut app,
                &mut ui,
            ),
            WidgetResult::Close
        );
        assert_eq!(ui.status_message.as_deref(), Some("Ready"));
    }

    #[test]
    fn keyboard_modified_keys_ctrl_j_alt_2_do_not_trigger_vim_or_hotkey() {
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(4));

        for (code, mods) in [
            (KeyCode::Char('j'), KeyModifiers::CONTROL),
            (KeyCode::Char('k'), KeyModifiers::ALT),
            (KeyCode::Char('1'), KeyModifiers::CONTROL),
            (KeyCode::Char('2'), KeyModifiers::ALT),
        ] {
            assert_eq!(
                dlg.handle_input(KeyEvent::new(code, mods), &mut app, &mut ui),
                WidgetResult::Handled
            );
            assert_eq!(dlg.selected_index, 0);
        }
    }

    #[test]
    fn mouse_scroll_up_and_down_moves_selection_and_syncs_list_state() {
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(4));
        let (_term, _ui) = render_dialog(&dlg, 80, 24);
        let area = dlg.dialog_area.get();
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());

        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: area.x + 2,
            row: area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg.handle_mouse(scroll_down, &mut app, &mut ui),
            WidgetResult::Handled
        );
        assert_eq!(dlg.selected_index, 1);
        assert_eq!(dlg.list_state.borrow().selected(), Some(1));

        let scroll_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: area.x + 2,
            row: area.y + 2,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg.handle_mouse(scroll_up, &mut app, &mut ui),
            WidgetResult::Handled
        );
        assert_eq!(dlg.selected_index, 0);
        assert_eq!(dlg.list_state.borrow().selected(), Some(0));
    }

    #[test]
    fn mouse_left_click_row_on_scrolled_list_uses_offset_plus_rel_row() {
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());

        // (a) Un-scrolled click on row 1 navigates to targets[1].
        let mut dlg_unscrolled = JumpToOperandDialog::new(Addr(0x0832), make_targets(4));
        let (_term4, _ui4) = render_dialog(&dlg_unscrolled, 80, 24);
        let list_area4 = dlg_unscrolled.list_area.get();
        let click_unscrolled = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: list_area4.x + 2,
            row: list_area4.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg_unscrolled.handle_mouse(click_unscrolled, &mut app, &mut ui),
            WidgetResult::Action(AppAction::NavigateToAddress(
                dlg_unscrolled.targets[1].target_address
            ))
        );
        assert_eq!(dlg_unscrolled.selected_index, 1);

        // (b) Scrolled list click at `list_area.y` uses `offset + 0` (not targets[0]).
        let mut dlg = JumpToOperandDialog::new(Addr(0x0900), make_targets(20));
        dlg.selected_index = 16;
        let (_term, _ui) = render_dialog(&dlg, 80, 24);
        let offset = dlg.list_state.borrow().offset();
        assert!(
            offset >= 4,
            "expected scrolled list offset >= 4, got {offset}"
        );

        let list_area = dlg.list_area.get();
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: list_area.x + 3,
            row: list_area.y,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg.handle_mouse(click, &mut app, &mut ui),
            WidgetResult::Action(AppAction::NavigateToAddress(
                dlg.targets[offset].target_address
            ))
        );
        assert_eq!(dlg.selected_index, offset);
        assert_eq!(dlg.list_state.borrow().selected(), Some(offset));
    }

    #[test]
    fn mouse_left_click_on_trailing_spacer_row_leaves_selection_unchanged() {
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(3));
        dlg.select_index(1);
        let (_term, _ui) = render_dialog(&dlg, 80, 24);
        let dialog_area = dlg.dialog_area.get();
        let list_area = dlg.list_area.get();
        assert_eq!(list_area.height, 3);

        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        for (col, row) in [
            // Dedicated blank spacer row between `list_area` and `footer_rect`.
            (list_area.x + 2, list_area.y + list_area.height),
            // Footer row inside `dialog_area` below the spacer row.
            (list_area.x + 2, list_area.y + list_area.height + 1),
            // Left border inside `dialog_area`.
            (dialog_area.x, dialog_area.y + 2),
        ] {
            let click = MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: col,
                row,
                modifiers: KeyModifiers::NONE,
            };
            assert_eq!(
                dlg.handle_mouse(click, &mut app, &mut ui),
                WidgetResult::Handled
            );
            assert_eq!(dlg.selected_index, 1);
        }

        // Unpopulated trailing row inside `list_area` when `targets.len() == 1` (`list_area.height == 2`).
        let mut dlg1 = JumpToOperandDialog::new(Addr(0x0832), make_targets(1));
        let (_term1, _ui1) = render_dialog(&dlg1, 80, 24);
        let list_area1 = dlg1.list_area.get();
        assert_eq!(dlg1.dialog_area.get().height, 6);
        assert_eq!(list_area1.height, 2);
        let empty_list_click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: list_area1.x + 2,
            row: list_area1.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg1.handle_mouse(empty_list_click, &mut app, &mut ui),
            WidgetResult::Handled
        );
        assert_eq!(dlg1.selected_index, 0);
    }

    #[test]
    fn mouse_outside_click_closes_only_on_left_button_down() {
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(3));
        let (_term, _ui) = render_dialog(&dlg, 80, 24);
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        ui.set_status_message("Jump pending");

        for kind in [
            MouseEventKind::Moved,
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Down(MouseButton::Middle),
        ] {
            let ev = MouseEvent {
                kind,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            };
            assert_eq!(
                dlg.handle_mouse(ev, &mut app, &mut ui),
                WidgetResult::Handled
            );
        }

        let left_outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            dlg.handle_mouse(left_outside, &mut app, &mut ui),
            WidgetResult::Close
        );
        assert_eq!(ui.status_message.as_deref(), Some("Ready"));
    }

    #[test]
    fn mouse_before_render_never_closes_or_mutates_selection() {
        let mut dlg = JumpToOperandDialog::new(Addr(0x0832), make_targets(3));
        let mut app = AppState::new();
        let mut ui = UIState::new(Theme::default());
        // Simulate stale `active_dialog_area` from a previous dialog before this modal renders.
        ui.active_dialog_area = Rect::new(10, 5, 40, 10);

        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::ScrollDown,
            MouseEventKind::Moved,
        ] {
            let ev = MouseEvent {
                kind,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            };
            assert_eq!(
                dlg.handle_mouse(ev, &mut app, &mut ui),
                WidgetResult::Handled
            );
            assert_eq!(dlg.selected_index, 0);
        }
    }
}
