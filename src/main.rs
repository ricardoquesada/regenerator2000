mod analyzer;
pub mod assets;
mod parser_crt;
mod parser_t64;
mod parser_vsf;

mod commands;
mod cpu;
mod dialog_about;
mod dialog_document_settings;
mod dialog_keyboard_shortcut;
mod dialog_settings;
mod disassembler;
mod events;
mod exporter;
mod state;
mod ui;
mod ui_state;

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
            Ok(loaded_data) => {
                let loaded_cursor = loaded_data.cursor_address;
                let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
                let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
                let loaded_right_pane = loaded_data.right_pane_visible;
                let loaded_charset_cursor = loaded_data.charset_cursor_address;

                ui_state.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
                ui_state.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
                ui_state.petscii_mode = loaded_data.petscii_mode;
                let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
                if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
                    ui_state.cursor_index = idx;
                }

                // Also restore hex cursor if present
                if let Some(hex_addr) = loaded_hex_cursor
                    && !app_state.raw_data.is_empty()
                {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let target = hex_addr as usize;

                    if target >= aligned_origin {
                        let offset = target - aligned_origin;
                        let row = offset / 16;
                        // Ensure row is within bounds
                        let total_len = app_state.raw_data.len() + alignment_padding;
                        let max_rows = total_len.div_ceil(16);
                        if row < max_rows {
                            ui_state.hex_cursor_index = row;
                        }
                    }
                }

                // Restore Right Pane and Sprites Cursor
                if let Some(pane_str) = loaded_right_pane {
                    match pane_str.as_str() {
                        "HexDump" => ui_state.right_pane = crate::ui_state::RightPane::HexDump,
                        "Sprites" => ui_state.right_pane = crate::ui_state::RightPane::Sprites,
                        "Charset" => ui_state.right_pane = crate::ui_state::RightPane::Charset,
                        "Blocks" => ui_state.right_pane = crate::ui_state::RightPane::Blocks,
                        _ => {}
                    }
                }
                if let Some(idx) = loaded_data.blocks_view_cursor {
                    ui_state.blocks_list_state.select(Some(idx));
                }
                if let Some(sprites_addr) = loaded_sprites_cursor {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    let addr = sprites_addr as usize;
                    if addr >= origin + padding {
                        let offset = addr - (origin + padding);
                        ui_state.sprites_cursor_index = offset / 64;
                    }
                }
                if let Some(charset_addr) = loaded_charset_cursor {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    let addr = charset_addr as usize;
                    if addr >= aligned_start_addr {
                        let offset = addr - aligned_start_addr;
                        ui_state.charset_cursor_index = offset / 8;
                    }
                }
            }
        }
    } else if app_state.system_config.open_last_project
        && let Some(last_path) = app_state.system_config.last_project_path.clone()
        && last_path.exists()
    {
        match app_state.load_file(last_path.clone()) {
            Ok(loaded_data) => {
                let loaded_cursor = loaded_data.cursor_address;
                let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
                let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
                let loaded_right_pane = loaded_data.right_pane_visible;
                let loaded_charset_cursor = loaded_data.charset_cursor_address;

                ui_state.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
                ui_state.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
                ui_state.petscii_mode = loaded_data.petscii_mode;
                let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
                if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
                    ui_state.cursor_index = idx;
                }

                // Also restore hex cursor if present
                // Also restore hex cursor if present
                if let Some(hex_addr) = loaded_hex_cursor
                    && !app_state.raw_data.is_empty()
                {
                    let origin = app_state.origin as usize;
                    let alignment_padding = origin % 16;
                    let aligned_origin = origin - alignment_padding;
                    let target = hex_addr as usize;

                    if target >= aligned_origin {
                        let offset = target - aligned_origin;
                        let row = offset / 16;
                        let total_len = app_state.raw_data.len() + alignment_padding;
                        let max_rows = total_len.div_ceil(16);
                        if row < max_rows {
                            ui_state.hex_cursor_index = row;
                        }
                    }
                }

                // Restore Right Pane and Sprites Cursor
                if let Some(pane_str) = loaded_right_pane {
                    match pane_str.as_str() {
                        "HexDump" => ui_state.right_pane = crate::ui_state::RightPane::HexDump,
                        "Sprites" => ui_state.right_pane = crate::ui_state::RightPane::Sprites,
                        "Charset" => ui_state.right_pane = crate::ui_state::RightPane::Charset,
                        "Blocks" => ui_state.right_pane = crate::ui_state::RightPane::Blocks,
                        _ => {}
                    }
                }
                if let Some(idx) = loaded_data.blocks_view_cursor {
                    ui_state.blocks_list_state.select(Some(idx));
                }
                if let Some(sprites_addr) = loaded_sprites_cursor {
                    let origin = app_state.origin as usize;
                    let padding = (64 - (origin % 64)) % 64;
                    let addr = sprites_addr as usize;
                    if addr >= origin + padding {
                        let offset = addr - (origin + padding);
                        ui_state.sprites_cursor_index = offset / 64;
                    }
                }
                if let Some(charset_addr) = loaded_charset_cursor {
                    let origin = app_state.origin as usize;
                    let base_alignment = 0x400;
                    let aligned_start_addr = (origin / base_alignment) * base_alignment;
                    let addr = charset_addr as usize;
                    if addr >= aligned_start_addr {
                        let offset = addr - aligned_start_addr;
                        ui_state.charset_cursor_index = offset / 8;
                    }
                }

                ui_state.status_message = format!("Loaded recent project: {:?}", last_path);
            }
            Err(e) => {
                ui_state.status_message = format!("Failed to load recent: {}", e);
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
