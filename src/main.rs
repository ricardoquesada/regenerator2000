mod analyzer;
pub mod assets;
mod parser_crt;
mod parser_t64;
mod parser_vsf;

mod commands;
mod cpu;
mod dialog_about;
mod dialog_comment;
mod dialog_confirmation;
mod dialog_document_settings;
mod dialog_export_as;
mod dialog_jump_to_address;
mod dialog_jump_to_line;
mod dialog_keyboard_shortcut;
mod dialog_label;
mod dialog_open;
mod dialog_origin;
mod dialog_save_as;
mod dialog_search;
mod dialog_settings;
mod disassembler;
mod events;
mod exporter;
mod state;
mod ui;
mod ui_state;
mod view_blocks;
mod view_charset;
mod view_disassembly;
mod view_hexdump;
mod view_sprites;

mod config;
mod theme;
mod utils;

#[cfg(test)]
mod cursor_persistence_test;

#[cfg(test)]
mod load_project_test;
#[cfg(test)]
mod serialization_stability_test;

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
    let theme = crate::theme::Theme::from_name(&app_state.system_config.theme);
    let mut ui_state = UIState::new(theme);

    // Check args and load initial project/file
    let args: Vec<String> = std::env::args().collect();
    if let Some(result) = app_state.resolve_initial_load(&args) {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, &app_state);
                if args.len() <= 1 {
                    ui_state.status_message = format!("Loaded recent project: {:?}", path);
                }
            }
            Err(e) => {
                if args.len() > 1 {
                    eprintln!("Error loading file: {}", e);
                    ui_state.status_message = format!("Error loading file: {}", e);
                } else {
                    ui_state.status_message = format!("Failed to load recent: {}", e);
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
