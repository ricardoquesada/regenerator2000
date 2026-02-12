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

use simplelog::*;
use std::fs::File;
use std::panic;

fn main() -> Result<()> {
    // 1. Initialize Logging
    let log_path = std::env::temp_dir().join("regenerator2000.log");
    let _ = WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create(&log_path).or_else(|_| File::create("regenerator2000.log"))?,
    );
    log::info!("Regenerator 2000 started");

    // Check args and load initial project/file
    let args: Vec<String> = std::env::args().collect();

    let mut file_to_load = None;
    let mut labels_to_import = None;

    let mut headless = false;
    let mut mcp_server = false;
    let mut export_lbl_path = None;
    let mut export_asm_path = None;

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
                    "Supported file types: .prg, .crt, .t64, .d64, .d71, .d81, .vsf, .bin, .raw, .regen2000proj"
                );
                println!();
                println!("Options:");
                println!("    --help                    Print this help message");
                println!("    --version                 Print version information");
                println!(
                    "    --import_lbl <PATH>       Import VICE labels from the specified file"
                );
                println!(
                    "    --export_lbl <PATH>       Export labels to the specified file (after analysis/import)"
                );
                println!(
                    "    --export_asm <PATH>       Export assembly to the specified file (after analysis/import)"
                );
                println!("    --headless                Run in headless mode (no TUI)");
                println!("                              Only .regen2000proj files supported");
                println!("    --mcp-server              Run MCP server (HTTP port 3000)");
                println!("    --mcp-server-stdio        Run MCP server (stdio)");
                return Ok(());
            }
            "--mcp-server" => {
                mcp_server = true;
                i += 1;
            }
            "--mcp-server-stdio" => {
                mcp_server = true;
                headless = true; // stdio MCP is always headless
                i += 1;
                // We'll need a special mode for this
                // Let's use a local flag
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
            "--export_lbl" => {
                if i + 1 < args.len() {
                    export_lbl_path = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --export_lbl requires a file path");
                    std::process::exit(1);
                }
            }
            "--export_asm" => {
                if i + 1 < args.len() {
                    export_asm_path = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --export_asm requires a file path");
                    std::process::exit(1);
                }
            }
            "--headless" => {
                headless = true;
                i += 1;
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

    // Validate headless mode restrictions
    if headless && let Some(file_str) = &file_to_load {
        let path = std::path::Path::new(file_str);
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && !ext.eq_ignore_ascii_case("regen2000proj")
        {
            eprintln!("Error: Headless mode only supports .regen2000proj files");
            eprintln!("File provided: {}", file_str);
            eprintln!("Reason: Other formats require interactive UI for configuration");
            eprintln!("Solution: Load file in UI mode, configure, then save as .regen2000proj");
            std::process::exit(1);
        }
    }

    // Create AppState first (needed for logic before UI)
    let mut app_state = AppState::new();

    // 1. Load File / Project
    let mut initial_load_result = None;
    let mut disk_image_data = None;
    let mut is_disk_image = false;

    if let Some(file_str) = &file_to_load {
        let path = std::path::Path::new(file_str);
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("d64")
                    || ext.eq_ignore_ascii_case("d71")
                    || ext.eq_ignore_ascii_case("d81")
            })
        {
            is_disk_image = true;
        }
    }

    if is_disk_image {
        if let Some(file_str) = &file_to_load {
            let path = std::path::PathBuf::from(file_str);
            match std::fs::read(&path) {
                Ok(data) => match regenerator2000::parser::d64::parse_d64_directory(&data) {
                    Ok(files) => {
                        disk_image_data = Some((files, data, path));
                    }
                    Err(e) => {
                        eprintln!("Error parsing D64/D71/D81 file: {}", e);
                        if headless {
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                    if headless {
                        std::process::exit(1);
                    }
                }
            }
        }
    } else {
        // We capture the result to use it later for UI initialization if needed
        initial_load_result = app_state.resolve_initial_load(file_to_load.as_deref());

        if let Some(result) = &initial_load_result {
            match result {
                Ok((_, path)) => {
                    if headless && !mcp_server {
                        println!("Loaded: {:?}", path);
                    }
                }
                Err(e) => {
                    eprintln!("Error loading file: {}", e);
                    // In headless mode we should exit on error
                    if headless {
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    // 2. Import Labels
    if let Some(lbl_path) = labels_to_import {
        match app_state.import_vice_labels(std::path::PathBuf::from(lbl_path)) {
            Ok(msg) => {
                if headless && !mcp_server {
                    println!("{}", msg);
                }
            }
            Err(e) => {
                eprintln!("Error importing labels: {}", e);
                if headless {
                    std::process::exit(1);
                }
            }
        }
    }

    // 3. Export Labels
    if let Some(path_str) = export_lbl_path {
        let path = std::path::PathBuf::from(path_str);
        match app_state.export_vice_labels(path) {
            Ok(msg) => {
                if headless && !mcp_server {
                    println!("{}", msg);
                }
            }
            Err(e) => {
                eprintln!("Error exporting labels: {}", e);
                if headless {
                    std::process::exit(1);
                }
            }
        }
    }

    // 4. Export Assembly
    if let Some(path_str) = export_asm_path {
        let path = std::path::PathBuf::from(path_str);
        match regenerator2000::exporter::export_asm(&app_state, &path) {
            Ok(_) => {
                if headless && !mcp_server {
                    println!("Assembly exported to {:?}", path);
                }
            }
            Err(e) => {
                eprintln!("Error exporting assembly: {}", e);
                if headless {
                    std::process::exit(1);
                }
            }
        }
    }

    if headless && !mcp_server {
        return Ok(());
    }

    if headless && mcp_server {
        // Run as standalone MCP server (stdio or HTTP without TUI)
        // Check if stdout is being redirected or if we want stdio mode
        let args: Vec<String> = std::env::args().collect();
        let use_stdio = args.iter().any(|a| a == "--mcp-server-stdio");

        if use_stdio {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
            let ui_state = UIState::new(theme);
            rt.block_on(async {
                regenerator2000::mcp::stdio::run_headless_stdio_loop(app_state, ui_state).await;
            });
            return Ok(());
        } else {
            // Headless HTTP server
            let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
            let ui_state = UIState::new(theme);
            let (mcp_req_tx, mut mcp_req_rx) = tokio::sync::mpsc::channel(100);

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                tokio::spawn(async move {
                    if let Err(e) = regenerator2000::mcp::http::run_server(3000, mcp_req_tx).await {
                        eprintln!("Failed to start MCP server: {}", e);
                        std::process::exit(1);
                    }
                });

                while let Some(req) = mcp_req_rx.recv().await {
                    let response = regenerator2000::mcp::handler::handle_request(
                        &req,
                        &mut app_state,
                        &ui_state,
                    );
                    let _ = req.response_sender.send(response);
                }
            });
            return Ok(());
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Critical: Set Panic Hook to restore terminal
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);

        // Log the panic
        log::error!("Panic: {:?}", info);

        // Print to stderr
        eprintln!("Panic: {:?}", info);

        default_hook(info);
    }));

    // Critical setup: Alternate Screen & Mouse
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture,)?;

    // Optional setup: Keyboard Enhancement (might fail on legacy Windows)
    let keyboard_enhancement_result = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
    let mut ui_state = UIState::new(theme);

    if let Some((files, disk_data, disk_path)) = disk_image_data {
        let dialog = regenerator2000::ui::dialog_d64_picker::D64FilePickerDialog::new(
            files, disk_data, disk_path,
        );
        ui_state.active_dialog = Some(Box::new(dialog));
    }

    // Report keyboard enhancement error if any
    if let Err(ref e) = keyboard_enhancement_result {
        let error_msg = format!("Warning: Keyboard enhancement failed: {}", e);
        ui_state.set_status_message(error_msg);
    }

    // Apply the initial load result to the UI state
    if let Some(result) = initial_load_result {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, &app_state);
                if file_to_load.is_none() {
                    ui_state.set_status_message(format!("Loaded recent project: {:?}", path));
                }
            }
            Err(e) => {
                if file_to_load.is_some() {
                    // We already printed to stderr above, but UI needs feedback too
                    ui_state.set_status_message(format!("Error loading file: {}", e));
                } else {
                    ui_state.set_status_message(format!("Failed to load recent: {}", e));
                }
            }
        }
    }

    // Unified Event Channel
    let (event_tx, event_rx) = std::sync::mpsc::channel::<events::AppEvent>();

    // Spawn TUI Input Thread
    let tui_tx = event_tx.clone();
    std::thread::spawn(move || {
        // Loop until explicitly exited
        while let Ok(event) = crossterm::event::read() {
            if tui_tx.send(events::AppEvent::Crossterm(event)).is_err() {
                break;
            }
        }
    });

    // Start MCP Server if requested
    if mcp_server {
        let (mcp_req_tx, mut mcp_req_rx) = tokio::sync::mpsc::channel(100);
        let mcp_bridge_tx = event_tx.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Spawn the actual server
                let server_tx = mcp_req_tx.clone();
                let error_tx = mcp_bridge_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = regenerator2000::mcp::http::run_server(3000, server_tx).await {
                        let _ = error_tx.send(events::AppEvent::McpError(e.to_string()));
                    }
                });

                // Bridge MCP requests to Main Thread
                while let Some(req) = mcp_req_rx.recv().await {
                    if mcp_bridge_tx.send(events::AppEvent::Mcp(req)).is_err() {
                        break;
                    }
                }
            });
        });
    }

    // Run app
    let res = events::run_app(&mut terminal, app_state, ui_state, event_rx);

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
