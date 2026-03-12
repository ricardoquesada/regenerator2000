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
use std::path::{Path, PathBuf};

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

// ---------------------------------------------------------------------------
// File type classification
// ---------------------------------------------------------------------------

/// Detected file type from its extension.
enum InputFileType {
    DiskImage,
    TapeImage,
    CartImage,
    Other,
}

/// Classify a file path by its extension into one of the supported image types.
fn classify_input_file(file_str: &str) -> InputFileType {
    let path = Path::new(file_str);
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext)
            if ext.eq_ignore_ascii_case("d64")
                || ext.eq_ignore_ascii_case("d71")
                || ext.eq_ignore_ascii_case("d81") =>
        {
            InputFileType::DiskImage
        }
        Some(ext) if ext.eq_ignore_ascii_case("t64") => InputFileType::TapeImage,
        Some(ext) if ext.eq_ignore_ascii_case("crt") => InputFileType::CartImage,
        _ => InputFileType::Other,
    }
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Initialize the global logger, writing to a temp-dir log file.
fn init_logging() -> Result<()> {
    let log_path = std::env::temp_dir().join("regenerator2000.log");
    let _ = WriteLogger::init(
        LevelFilter::Debug,
        Config::default(),
        File::create(&log_path).or_else(|_| File::create("regenerator2000.log"))?,
    );
    log::info!("Regenerator 2000 started");
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI validation helpers
// ---------------------------------------------------------------------------

/// Validate that headless mode is only used with `.regen2000proj` files.
fn validate_headless_mode(file_str: &str) {
    let path = Path::new(file_str);
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

/// Parse an assembler name string into the corresponding `Assembler` enum variant.
fn parse_assembler_name(name: &str) -> regenerator2000::state::Assembler {
    match name.to_ascii_lowercase().as_str() {
        "64tass" | "tass64" | "tass" => regenerator2000::state::Assembler::Tass64,
        "acme" => regenerator2000::state::Assembler::Acme,
        "ca65" | "cc65" => regenerator2000::state::Assembler::Ca65,
        "kick" | "kickassembler" | "kickasm" => regenerator2000::state::Assembler::Kick,
        _ => {
            eprintln!("Error: Unknown assembler '{name}'. Valid values: 64tass, acme, ca65, kick");
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Image loading helpers
// ---------------------------------------------------------------------------

/// Parsed data from a disk image (D64/D71/D81).
type DiskImageData = (
    Vec<regenerator2000::parser::d64::D64FileEntry>,
    Vec<u8>,
    PathBuf,
);

/// Parsed data from a tape image (T64).
type TapeImageData = (
    Vec<regenerator2000::parser::t64::T64Entry>,
    Vec<u8>,
    PathBuf,
);

/// Parsed data from a cartridge image (CRT).
type CartImageData = (regenerator2000::parser::crt::CrtHeader, PathBuf);

/// Load and parse a D64/D71/D81 disk image. Returns `None` on error (with
/// messages printed to stderr). In headless mode, errors cause `exit(1)`.
fn load_disk_image(file_str: &str, headless: bool) -> Option<DiskImageData> {
    let path = PathBuf::from(file_str);
    match std::fs::read(&path) {
        Ok(data) => match regenerator2000::parser::d64::parse_d64_directory(&data) {
            Ok(files) => Some((files, data, path)),
            Err(e) => {
                eprintln!("Error parsing D64/D71/D81 file: {e}");
                if headless {
                    std::process::exit(1);
                }
                None
            }
        },
        Err(e) => {
            eprintln!("Error reading file: {e}");
            if headless {
                std::process::exit(1);
            }
            None
        }
    }
}

/// Load and parse a T64 tape image.
fn load_tape_image(file_str: &str, headless: bool) -> Option<TapeImageData> {
    let path = PathBuf::from(file_str);
    match std::fs::read(&path) {
        Ok(data) => match regenerator2000::parser::t64::parse_t64_directory(&data) {
            Ok(files) => Some((files, data, path)),
            Err(e) => {
                eprintln!("Error parsing T64 file: {e}");
                if headless {
                    std::process::exit(1);
                }
                None
            }
        },
        Err(e) => {
            eprintln!("Error reading file: {e}");
            if headless {
                std::process::exit(1);
            }
            None
        }
    }
}

/// Load and parse a CRT cartridge image.
fn load_cart_image(file_str: &str, headless: bool) -> Option<CartImageData> {
    let path = PathBuf::from(file_str);
    match std::fs::read(&path) {
        Ok(data) => match regenerator2000::parser::crt::parse_crt_chips(&data) {
            Ok(header) => Some((header, path)),
            Err(e) => {
                eprintln!("Error parsing CRT file: {e}");
                if headless {
                    std::process::exit(1);
                }
                None
            }
        },
        Err(e) => {
            eprintln!("Error reading file: {e}");
            if headless {
                std::process::exit(1);
            }
            None
        }
    }
}

// ---------------------------------------------------------------------------
// CLI "batch" operations (import/export labels, export asm)
// ---------------------------------------------------------------------------

/// Import VICE labels from a file into `app_state`.
fn import_labels(app_state: &mut AppState, lbl_path: String, headless: bool, mcp_server: bool) {
    match app_state.import_vice_labels(PathBuf::from(lbl_path)) {
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

/// Export VICE labels from `app_state` to a file.
fn export_labels(app_state: &mut AppState, path_str: String, headless: bool, mcp_server: bool) {
    let path = PathBuf::from(path_str);
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

/// Export assembly source from `app_state` to a file.
fn export_assembly(app_state: &AppState, path_str: String, headless: bool, mcp_server: bool) {
    let path = PathBuf::from(path_str);
    match regenerator2000::exporter::export_asm(app_state, &path) {
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

// ---------------------------------------------------------------------------
// Roundtrip verification
// ---------------------------------------------------------------------------

/// Run roundtrip verification (export → assemble → diff) and exit.
fn run_verify(app_state: &AppState) -> Result<()> {
    println!("\nRoundtrip Export Verification");
    println!("=============================");
    let results = regenerator2000::exporter::verify_all_assemblers(app_state);
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Headless MCP server
// ---------------------------------------------------------------------------

/// Run the MCP server in headless mode (stdio or HTTP, no TUI).
fn run_headless_mcp(mut app_state: AppState, use_stdio: bool) -> Result<()> {
    let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);

    if use_stdio {
        let ui_state = UIState::new(theme);
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            regenerator2000::mcp::stdio::run_headless_stdio_loop(app_state, ui_state).await;
        });
    } else {
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
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Terminal setup / teardown
// ---------------------------------------------------------------------------

/// Convenience alias for the TUI terminal type.
type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Set up the terminal for TUI mode: raw mode, panic hook, alternate screen,
/// mouse capture, and keyboard enhancement. Returns the terminal and the
/// keyboard-enhancement result (which may have failed on some platforms).
fn setup_terminal() -> Result<(Term, Result<(), io::Error>)> {
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
    let terminal = Terminal::new(backend)?;

    Ok((terminal, keyboard_enhancement_result))
}

/// Restore the terminal to its original state after TUI mode.
fn restore_terminal(terminal: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
    )?;

    // Try to pop enhancement flags, ignore error
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);

    terminal.show_cursor()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// UI initialisation helpers
// ---------------------------------------------------------------------------

/// If image data was parsed (disk/tape/cart), open the appropriate picker
/// dialog in the UI state.
fn open_image_picker_dialog(
    ui_state: &mut UIState,
    disk_image_data: Option<DiskImageData>,
    tape_image_data: Option<TapeImageData>,
    cart_image_data: Option<CartImageData>,
) {
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
}

/// Apply the initial load result (from `resolve_initial_load`) to the UI state,
/// restoring the session or setting an error status message.
fn apply_initial_load_result(
    ui_state: &mut UIState,
    app_state: &AppState,
    initial_load_result: Option<
        anyhow::Result<(regenerator2000::state::LoadedProjectData, PathBuf)>,
    >,
    file_was_specified: bool,
) {
    if let Some(result) = initial_load_result {
        match result {
            Ok((loaded_data, path)) => {
                ui_state.restore_session(&loaded_data, app_state);
                if !file_was_specified {
                    ui_state.set_status_message(format!("Loaded recent project: {path:?}"));
                }
            }
            Err(e) => {
                if file_was_specified {
                    ui_state.set_status_message(format!("Error loading file: {e}"));
                } else {
                    ui_state.set_status_message(format!("Failed to load recent: {e}"));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Background thread spawning
// ---------------------------------------------------------------------------

/// Spawn the TUI input reader thread.
fn spawn_input_thread(event_tx: &std::sync::mpsc::Sender<events::AppEvent>) {
    let tui_tx = event_tx.clone();
    std::thread::spawn(move || {
        while let Ok(event) = crossterm::event::read() {
            if tui_tx.send(events::AppEvent::Crossterm(event)).is_err() {
                break;
            }
        }
    });
}

/// Spawn the version-update check thread (async HTTP call).
fn spawn_update_check(event_tx: &std::sync::mpsc::Sender<events::AppEvent>) {
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

/// Spawn the MCP HTTP server thread and bridge its requests into the main
/// event channel.
fn spawn_mcp_server(event_tx: &std::sync::mpsc::Sender<events::AppEvent>) {
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

/// Auto-connect to the VICE binary monitor at the given address.
fn connect_vice(
    app_state: &mut AppState,
    ui_state: &mut UIState,
    vice_addr: &str,
    event_tx: &std::sync::mpsc::Sender<events::AppEvent>,
) {
    let vice_tx = vice_event_adapter(event_tx);
    match regenerator2000::vice::ViceClient::connect(vice_addr, vice_tx) {
        Ok(client) => {
            app_state.vice_client = Some(client);
            ui_state.set_status_message(format!("Connecting to VICE at {vice_addr}..."));
        }
        Err(e) => {
            ui_state.set_status_message(format!("Failed to connect to VICE at {vice_addr}: {e}"));
        }
    }
}

/// Create a `Sender<ViceEvent>` that wraps events into `AppEvent::Vice` and
/// forwards them to the given `Sender<AppEvent>`.
fn vice_event_adapter(
    app_tx: &std::sync::mpsc::Sender<events::AppEvent>,
) -> std::sync::mpsc::Sender<regenerator2000::vice::ViceEvent> {
    let (vice_tx, vice_rx) = std::sync::mpsc::channel();
    let app_tx = app_tx.clone();
    std::thread::spawn(move || {
        while let Ok(event) = vice_rx.recv() {
            if app_tx.send(events::AppEvent::Vice(event)).is_err() {
                break;
            }
        }
    });
    vice_tx
}

// ---------------------------------------------------------------------------
// Version check
// ---------------------------------------------------------------------------

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

// ===========================================================================
// main
// ===========================================================================

fn main() -> Result<()> {
    init_logging()?;

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
        validate_headless_mode(file_str);
    }

    // Create AppState and load the real system config from disk
    let mut app_state = AppState::new();
    app_state.system_config = regenerator2000::config::SystemConfig::load();

    // 1. Load file / project based on detected file type
    let mut initial_load_result = None;
    let mut disk_image_data = None;
    let mut tape_image_data = None;
    let mut cart_image_data = None;

    if let Some(file_str) = &file_to_load {
        match classify_input_file(file_str) {
            InputFileType::DiskImage => {
                disk_image_data = load_disk_image(file_str, headless);
            }
            InputFileType::TapeImage => {
                tape_image_data = load_tape_image(file_str, headless);
            }
            InputFileType::CartImage => {
                cart_image_data = load_cart_image(file_str, headless);
            }
            InputFileType::Other => {
                initial_load_result = app_state.resolve_initial_load(Some(file_str));
                if let Some(Ok((_, path))) = &initial_load_result
                    && headless
                    && !mcp_server
                {
                    println!("Loaded: {path:?}");
                }
                if let Some(Err(e)) = &initial_load_result {
                    eprintln!("Error loading file: {e}");
                    if headless || file_to_load.is_some() {
                        std::process::exit(1);
                    }
                }
            }
        }
    } else {
        // No file specified — try to load the most recent project
        initial_load_result = app_state.resolve_initial_load(None);
        if let Some(Err(e)) = &initial_load_result {
            eprintln!("Error loading file: {e}");
            // Don't exit — just proceed without a loaded file
        }
    }

    // 2. Import labels
    if let Some(lbl_path) = labels_to_import {
        import_labels(&mut app_state, lbl_path, headless, mcp_server);
    }

    // 3. Export labels
    if let Some(path_str) = export_lbl_path {
        export_labels(&mut app_state, path_str, headless, mcp_server);
    }

    // 4. Apply assembler override (before export or verify)
    if let Some(ref name) = assembler_override {
        let assembler = parse_assembler_name(name);
        app_state.settings.assembler = assembler;
        if headless && !mcp_server {
            println!("Assembler overridden to: {assembler}");
        }
    }

    // 5. Export assembly
    if let Some(path_str) = export_asm_path {
        export_assembly(&app_state, path_str, headless, mcp_server);
    }

    // 6. Verify roundtrip (export → assemble → diff)
    if verify {
        return run_verify(&app_state);
    }

    // Headless without MCP — nothing more to do
    if headless && !mcp_server {
        return Ok(());
    }

    // Headless MCP server (stdio or HTTP, no TUI)
    if headless && mcp_server {
        return run_headless_mcp(app_state, mcp_server_stdio);
    }

    // --- TUI mode ---

    let (mut terminal, keyboard_enhancement_result) = setup_terminal()?;

    let theme = regenerator2000::theme::Theme::from_name(&app_state.system_config.theme);
    let mut ui_state = UIState::new(theme);

    // Open an image picker dialog if we loaded a container image
    open_image_picker_dialog(
        &mut ui_state,
        disk_image_data,
        tape_image_data,
        cart_image_data,
    );

    // Report keyboard enhancement error if any
    if let Err(ref e) = keyboard_enhancement_result {
        ui_state.set_status_message(format!("Warning: Keyboard enhancement failed: {e}"));
    }

    // Apply the initial load result to the UI state
    apply_initial_load_result(
        &mut ui_state,
        &app_state,
        initial_load_result,
        file_to_load.is_some(),
    );

    // Unified Event Channel
    let (event_tx, event_rx) = std::sync::mpsc::channel::<events::AppEvent>();

    // Spawn background threads
    spawn_input_thread(&event_tx);

    if app_state.system_config.check_for_updates {
        spawn_update_check(&event_tx);
    }

    if mcp_server {
        spawn_mcp_server(&event_tx);
    }

    // Auto-connect to VICE if --vice flag provided
    if let Some(ref vice_addr) = cli.vice {
        connect_vice(&mut app_state, &mut ui_state, vice_addr, &event_tx);
    }

    // Run the main event loop
    let res = events::run_app(&mut terminal, app_state, ui_state, event_tx, event_rx);

    // Restore terminal
    restore_terminal(&mut terminal)?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
