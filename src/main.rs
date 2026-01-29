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

    let mut file_to_load = None;
    let mut labels_to_import = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--version" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" => {
                println!("Usage: {} [OPTIONS] [FILE]", env!("CARGO_PKG_NAME"));
                println!();
                println!(
                    "Supported file types: .prg, .crt, .t64, .vsf, .bin, .raw, .regen2000proj"
                );
                println!();
                println!("Options:");
                println!("    --help                    Print this help message");
                println!("    --version                 Print version information");
                println!(
                    "    --import_lbl <PATH>       Import VICE labels from the specified file"
                );
                return Ok(());
            }
            "--import_lbl" => {
                if i + 1 < args.len() {
                    labels_to_import = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --import_lbl requires a file path");
                    std::process::exit(1);
                }
            }
            arg if arg.starts_with('-') => {
                eprintln!("Error: Invalid command line option: {}", arg);
                std::process::exit(1);
            }
            arg => {
                file_to_load = Some(arg.to_string());
                i += 1;
            }
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

    if let Some(result) = app_state.resolve_initial_load(file_to_load.as_deref()) {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, &app_state);
                if file_to_load.is_none() {
                    ui_state.set_status_message(format!("Loaded recent project: {:?}", path));
                }
            }
            Err(e) => {
                if file_to_load.is_some() {
                    eprintln!("Error loading file: {}", e);
                    ui_state.set_status_message(format!("Error loading file: {}", e));
                } else {
                    ui_state.set_status_message(format!("Failed to load recent: {}", e));
                }
            }
        }
    }

    // Handle label import if requested
    if let Some(lbl_path) = labels_to_import {
        match app_state.import_vice_labels(std::path::PathBuf::from(lbl_path)) {
            Ok(msg) => ui_state.set_status_message(msg),
            Err(e) => ui_state.set_status_message(format!("Error importing labels: {}", e)),
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
