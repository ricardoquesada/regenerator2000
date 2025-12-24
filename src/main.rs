mod analyzer;

mod commands;
mod cpu;
mod disassembler;
mod events;
mod exporter;
mod state;
mod ui;
mod ui_state;

mod utils;

#[cfg(test)]
mod cursor_persistence_test;

use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use state::AppState;
use std::io;
use ui_state::UIState;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create States
    let mut app_state = AppState::new();
    let mut ui_state = UIState::new();

    // Check args
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        match app_state.load_file(path) {
            Err(e) => {
                eprintln!("Error loading file: {}", e);
                // In a real app we might want to show this in the UI
                ui_state.status_message = format!("Error loading file: {}", e);
            }
            Ok(loaded_cursor) => {
                let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
                if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
                    ui_state.cursor_index = idx;
                }
            }
        }
    }

    // Run app
    let res = events::run_app(&mut terminal, app_state, ui_state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        PopKeyboardEnhancementFlags
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}
