use crate::state::AppState;
use crate::ui::widget::WidgetResult;
use crate::ui_state::UIState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub trait Navigable {
    fn len(&self, app_state: &AppState) -> usize;
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
            let len = target.len(app_state);
            target.jump_to(app_state, ui_state, len.saturating_sub(1));
            WidgetResult::Handled
        }
        KeyCode::Char('G') if key.modifiers == KeyModifiers::SHIFT => {
            let entered_number = ui_state.input_buffer.parse::<usize>().unwrap_or(0);
            let is_buffer_empty = ui_state.input_buffer.is_empty();
            let active_pane = ui_state.active_pane;
            let current_idx = target.current_index(app_state, ui_state);

            ui_state.input_buffer.clear();

            // Push history before jumping
            ui_state.navigation_history.push((active_pane, current_idx));

            if is_buffer_empty {
                let len = target.len(app_state);
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

pub fn jump_to_disassembly_at_address(
    app_state: &AppState,
    ui_state: &mut UIState,
    target_addr: u16,
) -> WidgetResult {
    if let Some(line_idx) = app_state.get_line_index_containing_address(target_addr) {
        ui_state.navigation_history.push((
            crate::ui_state::ActivePane::Disassembly,
            ui_state.cursor_index,
        ));
        ui_state.cursor_index = line_idx;
        ui_state.active_pane = crate::ui_state::ActivePane::Disassembly;
        ui_state.sub_cursor_index = 0;
        ui_state.set_status_message(format!("Jumped to ${:04X}", target_addr));
    } else {
        ui_state.set_status_message(format!("Address ${:04X} not found", target_addr));
    }
    WidgetResult::Handled
}
