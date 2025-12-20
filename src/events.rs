use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use std::io;
use crate::state::AppState;
use crate::ui::ui;

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut state: AppState) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => {
                    return Ok(());
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if state.cursor_index < state.disassembly.len().saturating_sub(1) {
                        state.cursor_index += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if state.cursor_index > 0 {
                        state.cursor_index -= 1;
                    }
                }
                KeyCode::PageDown => {
                    state.cursor_index = (state.cursor_index + 10).min(state.disassembly.len().saturating_sub(1));
                }
                KeyCode::PageUp => {
                    state.cursor_index = state.cursor_index.saturating_sub(10);
                }
                KeyCode::Home => {
                    state.cursor_index = 0;
                }
                KeyCode::End => {
                    state.cursor_index = state.disassembly.len().saturating_sub(1);
                }
                _ => {}
            }
        }
    }
}
