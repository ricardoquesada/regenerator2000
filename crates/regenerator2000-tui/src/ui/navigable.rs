use crate::state::AppState;
use crate::ui::widget::WidgetResult;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Shared cursor-navigation contract implemented by all scrollable/grid TUI views
/// (`DisassemblyView`, `HexDumpView`, `SpritesView`, `CharsetView`, `BitmapView`, `BlocksView`).
pub trait Navigable {
    fn len(&self, app_state: &AppState) -> usize;

    fn len_with_ui(&self, app_state: &AppState, _ui_state: &UIState) -> usize {
        self.len(app_state)
    }

    fn current_index(&self, app_state: &AppState, ui_state: &UIState) -> usize;

    fn move_down(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize);
    fn move_up(&self, app_state: &AppState, ui_state: &mut UIState, amount: usize);
    fn page_down(&self, app_state: &AppState, ui_state: &mut UIState);
    fn page_up(&self, app_state: &AppState, ui_state: &mut UIState);

    // Jump to absolute index (e.g. from Home/End or computed target)
    fn jump_to(&self, app_state: &AppState, ui_state: &mut UIState, index: usize);

    // Jump based on user input (e.g. from G)
    fn jump_to_user_input(&self, app_state: &AppState, ui_state: &mut UIState, input: usize);

    fn item_name(&self) -> &str;
}

/// Processes standard navigation keys (`Up`/`Down`/`j`/`k`, `PageUp`/`PageDown`, `Ctrl+u`/`Ctrl+d`,
/// `Home`/`End`, numeric prefix counts `0`–`9`, and `[N]G` / `Shift+G`) for any [`Navigable`] view.
///
/// Unmodified `Up` and `Down` are strictly guarded by `key.modifiers.is_empty()` so that modified
/// arrow keys (`Shift+Up`/`Down` for selection extension and `Alt+Up`/`Down` for scope boundary
/// nudging) fall through to the view-specific handler.
pub fn handle_nav_input<T: Navigable>(
    target: &T,
    key: KeyEvent,
    app_state: &mut AppState,
    ui_state: &mut UIState,
) -> WidgetResult {
    match key.code {
        KeyCode::Char(c)
            if c.is_ascii_digit()
                && !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
        {
            if ui_state.input_buffer.len() < 10 {
                ui_state.input_buffer.push(c);
                ui_state.set_status_message(format!(":{}", ui_state.input_buffer));
            }
            WidgetResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j')
            if key.modifiers.is_empty()
                || (key.code == KeyCode::Down && key.modifiers.is_empty()) =>
        {
            ui_state.input_buffer.clear();
            target.move_down(app_state, ui_state, 1);
            WidgetResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k')
            if key.modifiers.is_empty()
                || (key.code == KeyCode::Up && key.modifiers.is_empty()) =>
        {
            ui_state.input_buffer.clear();
            target.move_up(app_state, ui_state, 1);
            WidgetResult::Handled
        }
        KeyCode::PageDown => {
            ui_state.input_buffer.clear();
            target.page_down(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => {
            ui_state.input_buffer.clear();
            target.page_down(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::PageUp => {
            ui_state.input_buffer.clear();
            target.page_up(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
            ui_state.input_buffer.clear();
            target.page_up(app_state, ui_state);
            WidgetResult::Handled
        }
        KeyCode::Home => {
            ui_state.input_buffer.clear();
            target.jump_to(app_state, ui_state, 0);
            WidgetResult::Handled
        }
        KeyCode::End => {
            ui_state.input_buffer.clear();
            let len = target.len_with_ui(app_state, ui_state);
            target.jump_to(app_state, ui_state, len.saturating_sub(1));
            WidgetResult::Handled
        }
        // Accept both `KeyModifiers::NONE` and `KeyModifiers::SHIFT` for uppercase `'G'` across
        // different terminal backends while rejecting `Ctrl+Shift+G` / `Alt+Shift+G` (`JumpToLine`).
        KeyCode::Char('G') if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
            let is_buffer_empty = ui_state.input_buffer.is_empty();
            let active_pane = ui_state.active_pane;
            let current_idx = target.current_index(app_state, ui_state);

            ui_state.input_buffer.clear();

            // Push history before jumping
            ui_state.core.navigation_history.push((
                active_pane,
                crate::ui_state::NavigationTarget::Index(current_idx),
            ));

            if is_buffer_empty {
                let len = target.len_with_ui(app_state, ui_state);
                let end_idx = len.saturating_sub(1);
                target.jump_to(app_state, ui_state, end_idx);
                ui_state.set_status_message("Jumped to end");
            } else {
                target.jump_to_user_input(app_state, ui_state, entered_number);
                ui_state.set_status_message(format!(
                    "Jumped to {} {}",
                    target.item_name(),
                    entered_number
                ));
            }
            WidgetResult::Handled
        }
        _ => WidgetResult::Ignored,
    }
}

/// Switches focus to [`crate::ui_state::ActivePane::Disassembly`] at the line containing
/// `target_addr` and records the caller pane and index in `navigation_history`.
pub fn jump_to_disassembly_at_address(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: crate::state::Addr,
) -> WidgetResult {
    regenerator2000_core::navigation::perform_jump_to_address(
        app_state,
        &mut ui_state.core,
        target_addr,
    );
    ui_state.sync_status_message();
    WidgetResult::Handled
}

/// Switches focus to [`crate::ui_state::ActivePane::Disassembly`] at `line_idx` (if `Some`),
/// recording the previous pane's cursor position in `navigation_history` for `Backspace` navigation.
pub fn jump_to_disassembly_at_line_idx(
    ui_state: &mut UIState,
    line_idx: Option<usize>,
    target_addr: crate::state::Addr,
) -> WidgetResult {
    if let Some(line_idx) = line_idx {
        let active_pane = ui_state.active_pane;
        let target = match active_pane {
            crate::ui_state::ActivePane::Disassembly => {
                crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index)
            }
            crate::ui_state::ActivePane::HexDump => {
                crate::ui_state::NavigationTarget::Index(ui_state.hex_cursor_index)
            }
            crate::ui_state::ActivePane::Sprites => {
                crate::ui_state::NavigationTarget::Index(ui_state.sprites_cursor_index)
            }
            crate::ui_state::ActivePane::Charset => {
                crate::ui_state::NavigationTarget::Index(ui_state.charset_cursor_index)
            }
            crate::ui_state::ActivePane::Bitmap => {
                crate::ui_state::NavigationTarget::Index(ui_state.bitmap_cursor_index)
            }
            crate::ui_state::ActivePane::Blocks => crate::ui_state::NavigationTarget::Index(
                ui_state.blocks_selected_index.unwrap_or(0),
            ),
            crate::ui_state::ActivePane::Debugger => {
                crate::ui_state::NavigationTarget::Index(ui_state.core.cursor_index)
            }
        };
        ui_state.core.navigation_history.push((active_pane, target));
        ui_state.cursor_index = line_idx;
        ui_state.active_pane = crate::ui_state::ActivePane::Disassembly;
        ui_state.sub_cursor_index = 0;

        // Also reset scroll if not visible
        let is_visible = ui_state.disassembly_viewport_height > 0
            && line_idx >= ui_state.scroll_index
            && line_idx < ui_state.scroll_index + ui_state.disassembly_viewport_height;

        if !is_visible {
            ui_state.scroll_index = line_idx;
            ui_state.scroll_sub_index = 0;
        }

        ui_state.set_status_message(format!("Jumped to ${target_addr:04X}"));
    } else {
        ui_state.set_status_message(format!("Address ${target_addr:04X} not found"));
    }
    WidgetResult::Handled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, BlockType};
    use crate::theme::Theme;
    use crate::ui_state::ActivePane;

    #[test]
    fn test_jump_to_disassembly_at_address_skips_splitter_and_sets_sub_cursor() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x1000);
        app_state.raw_data = vec![0xEA; 8];
        app_state.block_types = vec![BlockType::Code; 8];
        app_state.splitters.insert(Addr(0x1004));
        app_state.annotations.update(Addr(0x1004), |e| {
            e.user_line_comment = Some("line1\nline2".to_string());
        });
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::HexDump;
        ui_state.hex_cursor_index = 3;

        let res = jump_to_disassembly_at_address(&app_state, &mut ui_state, Addr(0x1004));
        assert_eq!(res, WidgetResult::Handled);
        assert_eq!(ui_state.active_pane, ActivePane::Disassembly);

        let landed = &app_state.disassembly[ui_state.cursor_index];
        assert_eq!(landed.address, Addr(0x1004));
        assert!(
            !landed.bytes.is_empty(),
            "Jump must advance past zero-byte splitter header to the instruction line"
        );
        assert!(
            ui_state.sub_cursor_index > 0,
            "Jump must set sub_cursor_index past line comments"
        );
        assert_eq!(
            ui_state.core.navigation_history,
            vec![(
                ActivePane::HexDump,
                crate::ui_state::NavigationTarget::Index(3)
            )]
        );
    }
}
