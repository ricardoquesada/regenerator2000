mod cpu;
mod disassembler;
mod state;
mod ui;
mod events;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use state::AppState;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create AppState
    let mut app_state = AppState::new();
    
    // For now, load some dummy or user can load via cli args later.
    // Ideally we parse CLI args here.
    // Let's check args
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        if let Err(e) = app_state.load_file(path) {
            eprintln!("Error loading file: {}", e);
            // We might want to show this in UI, but for now just log it or fail?
            // Let's just start empty if fail, or panic? 
            // Better to let the UI show empty state.
        }
    }

    // Run app
    let res = events::run_app(&mut terminal, app_state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}
