#![deny(clippy::unwrap_used, clippy::panic)]

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

use simplelog::{Config, LevelFilter, WriteLogger};
use std::fs::File;
use std::panic;

use clap::Parser;

/// An interactive disassembler for the MOS 6502, focused on Commodore 8-bit computers.
#[derive(Parser)]
#[command(
    version,
    after_help = "Supported file types: .prg, .crt, .t64, .d64, .d71, .d81, .vsf, .bin, .raw, .regen2000proj"
)]
struct Cli {
    /// File to load (.prg, .crt, .d64, .d71, .d81, .t64, .vsf, .bin, .raw, .regen2000proj)
    file: Option<String>,

    /// Import VICE labels from the specified file
    #[arg(long = "import_lbl", value_name = "PATH")]
    import_lbl: Option<String>,

    /// Export labels to the specified file (after analysis/import)
    #[arg(long = "export_lbl", value_name = "PATH")]
    export_lbl: Option<String>,

    /// Export assembly to the specified file (after analysis/import)
    #[arg(long = "export_asm", value_name = "PATH")]
    export_asm: Option<String>,

    /// Override assembler format (64tass, acme, ca65, kick)
    #[arg(long, value_name = "NAME")]
    assembler: Option<String>,

    /// Run in headless mode (no TUI, only .regen2000proj files supported)
    #[arg(long)]
    headless: bool,

    /// Verify export roundtrip (export → assemble → diff). Implies --headless
    #[arg(long)]
    verify: bool,

    /// Run MCP server (HTTP port 3000)
    #[arg(long = "mcp-server")]
    mcp_server: bool,

    /// Run MCP server (stdio, headless)
    #[arg(long = "mcp-server-stdio")]
    mcp_server_stdio: bool,

    /// Auto-connect to VICE binary monitor at HOST:PORT (e.g. localhost:6502)
    #[arg(long, value_name = "HOST:PORT")]
    vice: Option<String>,
}

async fn check_for_new_version() -> Option<String> {
    const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
    const API_URL: &str =
        "https://api.github.com/repos/ricardoquesada/regenerator2000/releases/latest";

    let client = reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let response = client.get(API_URL).send().await.ok()?;
    let json: serde_json::Value = response.json().await.ok()?;
    let tag_name = json["tag_name"].as_str()?;
    let remote_version = tag_name.trim_start_matches('v');

    if is_newer_version(CURRENT_VERSION, remote_version) {
        Some(remote_version.to_string())
    } else {
        None
    }
}

fn is_newer_version(current: &str, remote: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let mut parts = v.split('.').filter_map(|p| p.parse::<u32>().ok());
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    parse(remote) > parse(current)
}

fn main() -> Result<()> {
    // 1. Initialize Logging
    let log_path = std::env::temp_dir().join("regenerator2000.log");
    let _ = WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create(&log_path).or_else(|_| File::create("regenerator2000.log"))?,
    );
    log::info!("Regenerator 2000 started");

    // Parse command-line arguments
    let cli = Cli::parse();

    let file_to_load = cli.file;
    let labels_to_import = cli.import_lbl;
    let export_lbl_path = cli.export_lbl;
    let export_asm_path = cli.export_asm;
    let assembler_override = cli.assembler;
    let verify = cli.verify;
    let mcp_server_stdio = cli.mcp_server_stdio;
    let headless = cli.headless || cli.verify || cli.mcp_server_stdio;
    let mcp_server = cli.mcp_server || cli.mcp_server_stdio;

    // Validate headless mode restrictions
    if headless && let Some(file_str) = &file_to_load {
        let path = std::path::Path::new(file_str);
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && !ext.eq_ignore_ascii_case("regen2000proj")
        {
            eprintln!("Error: Headless mode only supports .regen2000proj files");
            eprintln!("File provided: {file_str}");
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
    let mut tape_image_data = None;
    let mut cart_image_data = None;
    let mut is_disk_image = false;
    let mut is_tape_image = false;
    let mut is_cart_image = false;

    if let Some(file_str) = &file_to_load {
        let path = std::path::Path::new(file_str);
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("d64")
                || ext.eq_ignore_ascii_case("d71")
                || ext.eq_ignore_ascii_case("d81")
            {
                is_disk_image = true;
            } else if ext.eq_ignore_ascii_case("t64") {
                is_tape_image = true;
            } else if ext.eq_ignore_ascii_case("crt") {
                is_cart_image = true;
            }
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
                        eprintln!("Error parsing D64/D71/D81 file: {e}");
                        if headless {
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error reading file: {e}");
                    if headless {
                        std::process::exit(1);
                    }
                }
            }
        }
    } else if is_tape_image {
        if let Some(file_str) = &file_to_load {
            let path = std::path::PathBuf::from(file_str);
            match std::fs::read(&path) {
                Ok(data) => match regenerator2000::parser::t64::parse_t64_directory(&data) {
                    Ok(files) => {
                        tape_image_data = Some((files, data, path));
                    }
                    Err(e) => {
                        eprintln!("Error parsing T64 file: {e}");
                        if headless {
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error reading file: {e}");
                    if headless {
                        std::process::exit(1);
                    }
                }
            }
        }
    } else if is_cart_image {
        if let Some(file_str) = &file_to_load {
            let path = std::path::PathBuf::from(file_str);
            match std::fs::read(&path) {
                Ok(data) => match regenerator2000::parser::crt::parse_crt_chips(&data) {
                    Ok(chips) => {
                        cart_image_data = Some((chips, path));
                    }
                    Err(e) => {
                        eprintln!("Error parsing CRT file: {e}");
                        if headless {
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Error reading file: {e}");
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
                        println!("Loaded: {path:?}");
                    }
                }
                Err(e) => {
                    eprintln!("Error loading file: {e}");
                    // In headless mode or if explicit file failed, we should exit
                    if headless || file_to_load.is_some() {
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
                    println!("{msg}");
                }
            }
            Err(e) => {
                eprintln!("Error importing labels: {e}");
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
                    println!("{msg}");
                }
            }
            Err(e) => {
                eprintln!("Error exporting labels: {e}");
                if headless {
                    std::process::exit(1);
                }
            }
        }
    }

    // Apply assembler override (before export or verify)
    if let Some(ref name) = assembler_override {
        let assembler = match name.to_ascii_lowercase().as_str() {
            "64tass" | "tass64" | "tass" => regenerator2000::state::Assembler::Tass64,
            "acme" => regenerator2000::state::Assembler::Acme,
            "ca65" | "cc65" => regenerator2000::state::Assembler::Ca65,
            "kick" | "kickassembler" | "kickasm" => regenerator2000::state::Assembler::Kick,
            _ => {
                eprintln!(
                    "Error: Unknown assembler '{name}'. Valid values: 64tass, acme, ca65, kick"
                );
                std::process::exit(1);
            }
        };
        app_state.settings.assembler = assembler;
        if headless && !mcp_server {
            println!("Assembler overridden to: {assembler}");
        }
    }

    // 4. Export Assembly
    if let Some(path_str) = export_asm_path {
        let path = std::path::PathBuf::from(path_str);
        match regenerator2000::exporter::export_asm(&app_state, &path) {
            Ok(()) => {
                if headless && !mcp_server {
                    println!("Assembly exported to {path:?}");
                }
            }
            Err(e) => {
                eprintln!("Error exporting assembly: {e}");
                if headless {
                    std::process::exit(1);
                }
            }
        }
    }

    // 5. Verify roundtrip (export → assemble → diff)
    if verify {
        println!("\nRoundtrip Export Verification");
        println!("=============================");
        let results = regenerator2000::exporter::verify_all_assemblers(&app_state);
        let mut all_passed = true;
        let mut any_ran = false;
        for r in &results {
            println!("{r}");
            if r.success {
                any_ran = true;
            } else {
                // "not found in PATH" means the assembler simply isn't installed — not a failure
                if !r.message.contains("not found in PATH") {
                    all_passed = false;
                }
                if r.message.contains("not found in PATH") {
                    // skipped
                } else {
                    any_ran = true;
                }
            }
        }
        if !any_ran {
            eprintln!("\nNo assemblers found — install at least one to verify.");
            std::process::exit(1);
        }
        if all_passed {
            println!("\n✓ All roundtrip verifications passed.");
        } else {
            eprintln!("\n✗ Some roundtrip verifications failed.");
            std::process::exit(1);
        }
        return Ok(());
    }

    if headless && !mcp_server {
        return Ok(());
    }

    if headless && mcp_server {
        // Run as standalone MCP server (stdio or HTTP without TUI)
        // Check if stdout is being redirected or if we want stdio mode
        let use_stdio = mcp_server_stdio;

        if use_stdio {
            let rt = tokio::runtime::Runtime::new()?;
            let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
            let ui_state = UIState::new(theme);
            rt.block_on(async {
                regenerator2000::mcp::stdio::run_headless_stdio_loop(app_state, ui_state).await;
            });
            return Ok(());
        } else {
            // Headless HTTP server
            let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
            let mut ui_state = UIState::new(theme);
            let (mcp_req_tx, mut mcp_req_rx) = tokio::sync::mpsc::channel(100);

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                tokio::spawn(async move {
                    if let Err(e) = regenerator2000::mcp::http::run_server(3000, mcp_req_tx).await {
                        eprintln!("Failed to start MCP server: {e}");
                        std::process::exit(1);
                    }
                });

                while let Some(req) = mcp_req_rx.recv().await {
                    let response = regenerator2000::mcp::handler::handle_request(
                        &req,
                        &mut app_state,
                        &mut ui_state,
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
        log::error!("Panic: {info:?}");

        // Print to stderr
        eprintln!("Panic: {info:?}");

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
    } else if let Some((files, tape_data, tape_path)) = tape_image_data {
        let dialog = regenerator2000::ui::dialog_t64_picker::T64FilePickerDialog::new(
            files, tape_data, tape_path,
        );
        ui_state.active_dialog = Some(Box::new(dialog));
    } else if let Some((crt_header, path)) = cart_image_data {
        let dialog =
            regenerator2000::ui::dialog_crt_picker::CrtBankPickerDialog::new(crt_header, path);
        ui_state.active_dialog = Some(Box::new(dialog));
    }

    // Report keyboard enhancement error if any
    if let Err(ref e) = keyboard_enhancement_result {
        let error_msg = format!("Warning: Keyboard enhancement failed: {e}");
        ui_state.set_status_message(error_msg);
    }

    // Apply the initial load result to the UI state
    if let Some(result) = initial_load_result {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, &app_state);
                if file_to_load.is_none() {
                    ui_state.set_status_message(format!("Loaded recent project: {path:?}"));
                }
            }
            Err(e) => {
                if file_to_load.is_some() {
                    // We already printed to stderr above, but UI needs feedback too
                    ui_state.set_status_message(format!("Error loading file: {e}"));
                } else {
                    ui_state.set_status_message(format!("Failed to load recent: {e}"));
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

    // Spawn version update check thread
    if app_state.system_config.check_for_updates {
        let update_tx = event_tx.clone();
        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
            rt.block_on(async {
                if let Some(version) = check_for_new_version().await {
                    let _ = update_tx.send(events::AppEvent::UpdateAvailable(version));
                }
            });
        });
    }

    // Start MCP Server if requested
    if mcp_server {
        let (mcp_req_tx, mut mcp_req_rx) = tokio::sync::mpsc::channel(100);
        let mcp_bridge_tx = event_tx.clone();

        std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
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

    // Auto-connect to VICE if --vice flag provided
    if let Some(ref vice_addr) = cli.vice {
        match regenerator2000::vice::ViceClient::connect(vice_addr, event_tx.clone()) {
            Ok(client) => {
                app_state.vice_client = Some(client);
                ui_state.set_status_message(format!("Connecting to VICE at {vice_addr}..."));
            }
            Err(e) => {
                ui_state
                    .set_status_message(format!("Failed to connect to VICE at {vice_addr}: {e}"));
            }
        }
    }

    // Run app
    let res = events::run_app(&mut terminal, app_state, ui_state, event_tx, event_rx);

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
        println!("{err:?}");
    }

    Ok(())
}
