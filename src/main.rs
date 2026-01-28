use regenerator2000::events;
use regenerator2000::state::AppState;
use regenerator2000::ui_state::UIState;

use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

fn main() -> Result<()> {
    // Check args and load initial project/file
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        // Handle command line arguments
        if args[1] == "--version" {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if args[1] == "--help" {
            println!("Usage: {} [OPTIONS] [FILE]", env!("CARGO_PKG_NAME"));
            println!();
            println!("Supported file types: .prg, .crt, .t64, .vsf, .bin, .raw, .regen2000proj");
            println!();
            println!("Options:");
            println!("    --help       Print this help message");
            println!("    --version    Print version information");
            return Ok(());
        } else if args[1].starts_with('-') {
            eprintln!("Error: Invalid command line option: {}", args[1]);
            std::process::exit(1);
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Critical setup: Alternate Screen & Mouse
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture,)?;

    // Optional setup: Keyboard Enhancement (might fail on legacy Windows)
    let keyboard_enhancement_result = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create States
    let mut app_state = AppState::new();
    let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
    let mut ui_state = UIState::new(theme);

    // Report keyboard enhancement error if any
    if let Err(ref e) = keyboard_enhancement_result {
        let error_msg = format!("Warning: Keyboard enhancement failed: {}", e);
        ui_state.set_status_message(error_msg);
    }

    if let Some(result) = app_state.resolve_initial_load(&args) {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, &app_state);
                if args.len() <= 1 {
                    ui_state.set_status_message(format!("Loaded recent project: {:?}", path));
                }
            }
            Err(e) => {
                if args.len() > 1 {
                    eprintln!("Error loading file: {}", e);
                    ui_state.set_status_message(format!("Error loading file: {}", e));
                } else {
                    ui_state.set_status_message(format!("Failed to load recent: {}", e));
                }
            }
        }
    }

    if let Err(ref e) = keyboard_enhancement_result {
        eprintln!("Keyboard enhancement failed: {}", e);
    }

    // Run app
    let res = events::run_app(&mut terminal, app_state, ui_state);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;

    // Try to pop enhancement flags, ignore error
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);

    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}
