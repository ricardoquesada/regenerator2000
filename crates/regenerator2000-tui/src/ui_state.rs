use crate::theme::Theme;
pub use crate::ui::menu::MenuState;
use crate::ui::widget::Widget;
use image::DynamicImage;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
pub use regenerator2000_core::state::actions::AppAction;
use std::collections::HashMap;
use std::path::PathBuf;

// Re-export core enums so existing `crate::ui_state::ActivePane` etc. still work.
pub use regenerator2000_core::view_state::{
    ActivePane, CoreViewState, NavigationTarget, RightPane, ScreenRamMode,
};

pub struct UIState {
    /// Frontend-agnostic state (cursors, selections, panes, modes).
    /// Accessible directly via `Deref`/`DerefMut`.
    pub core: CoreViewState,

    // --- TUI-only state below ---
    pub active_dialog: Option<Box<dyn Widget>>,
    pub dialog_queue: Vec<Box<dyn Widget>>,
    pub file_dialog_current_dir: PathBuf,

    pub menu: MenuState,

    #[allow(dead_code)]
    pub disassembly_state: ListState,

    pub blocks_list_state: ListState,
    pub bookmarks_list_state: ListState,
    pub recent_list_state: ListState,

    pub should_quit: bool,
    pub status_bar: crate::ui::statusbar::StatusBarState,

    pub logo: Option<DynamicImage>,
    pub picker: Option<Picker>,
    pub dismiss_logo: bool,
    pub input_buffer: String,

    pub theme: Theme,

    // Vim-like search
    pub vim_search_active: bool,
    pub vim_search_input: String,

    // Bitmap cache: key is (bitmap_address, multicolor_mode, screen_ram_address)
    pub bitmap_cache: HashMap<(usize, bool, usize), DynamicImage>,

    // Version update notification
    pub new_version_available: Option<String>,

    // Flash countdown for debugger status line when breakpoint/watchpoint is hit.
    // Decremented each render frame; while > 0, renders with attention-grabbing style.
    pub debugger_flash_remaining: u8,

    // Layout Areas for Mouse Interaction
    pub menu_area: ratatui::layout::Rect,
    pub main_area: ratatui::layout::Rect,
    pub status_bar_area: ratatui::layout::Rect,
    pub disassembly_area: ratatui::layout::Rect,
    pub right_pane_area: ratatui::layout::Rect,
    pub active_dialog_area: ratatui::layout::Rect,
    pub minimap_area: ratatui::layout::Rect,
}

// ---------------------------------------------------------------------------
// Deref to CoreViewState so `ui_state.cursor_index` keeps working unchanged
// ---------------------------------------------------------------------------

impl std::ops::Deref for UIState {
    type Target = CoreViewState;
    fn deref(&self) -> &CoreViewState {
        &self.core
    }
}

impl std::ops::DerefMut for UIState {
    fn deref_mut(&mut self) -> &mut CoreViewState {
        &mut self.core
    }
}

impl UIState {
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self {
            core: CoreViewState::new(),

            active_dialog: None,
            dialog_queue: Vec::new(),
            file_dialog_current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),

            menu: MenuState::new(),
            disassembly_state: ListState::default(),
            blocks_list_state: ListState::default(),
            bookmarks_list_state: ListState::default(),
            recent_list_state: ListState::default(),
            should_quit: false,
            status_bar: crate::ui::statusbar::StatusBarState::new(),
            logo: crate::utils::load_logo(),
            picker: crate::utils::create_picker(),
            dismiss_logo: false,
            input_buffer: String::new(),
            vim_search_active: false,
            vim_search_input: String::new(),
            bitmap_cache: HashMap::new(),
            theme,

            new_version_available: None,
            debugger_flash_remaining: 0,

            menu_area: ratatui::layout::Rect::default(),
            main_area: ratatui::layout::Rect::default(),
            status_bar_area: ratatui::layout::Rect::default(),
            disassembly_area: ratatui::layout::Rect::default(),
            right_pane_area: ratatui::layout::Rect::default(),
            active_dialog_area: ratatui::layout::Rect::default(),
            minimap_area: ratatui::layout::Rect::default(),
        }
    }

    pub fn push_dialog(&mut self, dialog: Box<dyn Widget>) {
        if self.active_dialog.is_none() {
            self.active_dialog = Some(dialog);
        } else {
            self.dialog_queue.push(dialog);
        }
    }

    pub fn set_status_message(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.core.status_message = Some(msg.clone());
        self.status_bar.set_message(msg);
    }

    /// Synchronize the TUI status bar with any message set in CoreViewState.
    /// Used after calling core functions that might update status_message.
    pub fn sync_status_message(&mut self) {
        if let Some(msg) = self.core.status_message.take() {
            self.status_bar.set_message(msg);
        }
    }

    /// Sync TUI-specific state (like ListState) TO CoreViewState.
    pub fn sync_tui_to_core(&mut self) {
        self.core.blocks_selected_index = self.blocks_list_state.selected();
    }

    /// Sync CoreViewState state TO TUI-specific state (like ListState).
    pub fn sync_core_to_tui(&mut self) {
        if let Some(idx) = self.core.blocks_selected_index {
            self.blocks_list_state.select(Some(idx));
        }
    }

    pub fn restore_session(
        &mut self,
        loaded_data: &crate::state::LoadedProjectData,
        app_state: &crate::state::AppState,
    ) {
        let loaded_cursor = loaded_data.cursor_address;
        let loaded_hex_cursor = loaded_data.hex_dump_cursor_address;
        let loaded_sprites_cursor = loaded_data.sprites_cursor_address;
        let loaded_right_pane = &loaded_data.right_pane_visible;
        let loaded_charset_cursor = loaded_data.charset_cursor_address;

        self.sprite_multicolor_mode = loaded_data.sprite_multicolor_mode;
        self.charset_multicolor_mode = loaded_data.charset_multicolor_mode;
        self.bitmap_multicolor_mode = loaded_data.bitmap_multicolor_mode.unwrap_or(false);
        self.hexdump_view_mode = loaded_data.hexdump_view_mode;
        let initial_addr = loaded_cursor.unwrap_or(app_state.origin);
        if let Some(idx) = app_state.get_line_index_for_address(initial_addr) {
            self.cursor_index = idx;
            // Also reset scroll and sub-cursor so it's visible at top
            self.scroll_index = idx;
            self.sub_cursor_index = 0;
            self.scroll_sub_index = 0;
        }

        // Update file dialog directory to the project/file location
        if let Some(path) = &app_state.project_path {
            if let Some(parent) = path.parent() {
                self.file_dialog_current_dir = parent.to_path_buf();
            }
        } else if let Some(path) = &app_state.file_path
            && let Some(parent) = path.parent()
        {
            // `parent` may be an empty path when the file was opened with a
            // bare filename (no directory component, e.g. `cargo run -- foo.prg`).
            // In that case fall back to the actual current working directory so
            // that `fs::read_dir` works and `..` is shown correctly.
            let resolved = if parent.as_os_str().is_empty() {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            } else {
                parent
                    .canonicalize()
                    .unwrap_or_else(|_| parent.to_path_buf())
            };
            self.file_dialog_current_dir = resolved;
        }

        // Also restore hex cursor if present
        if let Some(hex_addr) = loaded_hex_cursor
            && !app_state.raw_data.is_empty()
        {
            let origin = app_state.origin.0 as usize;
            let alignment_padding = origin % 16;
            let aligned_origin = origin - alignment_padding;
            let target = hex_addr.0 as usize;

            if target >= aligned_origin {
                let offset = target - aligned_origin;
                let row = offset / 16;
                // Ensure row is within bounds
                let total_len = app_state.raw_data.len() + alignment_padding;
                let max_rows = total_len.div_ceil(16);
                if row < max_rows {
                    self.hex_cursor_index = row;
                }
            }
        }

        // Restore Right Pane and Sprites Cursor
        if let Some(pane_str) = loaded_right_pane {
            match pane_str.as_str() {
                "HexDump" => self.right_pane = RightPane::HexDump,
                "None" => self.right_pane = RightPane::None,
                "Sprites" => self.right_pane = RightPane::Sprites,
                "Charset" => self.right_pane = RightPane::Charset,
                "Bitmap" => self.right_pane = RightPane::Bitmap,
                "Blocks" => self.right_pane = RightPane::Blocks,
                _ => {}
            }
        }
        if let Some(idx) = loaded_data.blocks_view_cursor {
            self.blocks_list_state.select(Some(idx));
            self.blocks_selected_index = Some(idx);
        }
        if let Some(sprites_addr) = loaded_sprites_cursor {
            let origin = app_state.origin.0 as usize;
            let aligned_origin = (origin / 64) * 64;
            let addr = sprites_addr.0 as usize;
            if addr >= aligned_origin {
                let offset = addr - aligned_origin;
                self.sprites_cursor_index = offset / 64;
            }
        }
        if let Some(charset_addr) = loaded_charset_cursor {
            let origin = app_state.origin.0 as usize;
            let base_alignment = 0x400;
            let aligned_start_addr = (origin / base_alignment) * base_alignment;
            let addr = charset_addr.0 as usize;
            if addr >= aligned_start_addr {
                let offset = addr - aligned_start_addr;
                self.charset_cursor_index = offset / 8;
            }
        }

        // Restore Bitmap Cursor
        if let Some(bitmap_addr) = loaded_data.bitmap_cursor_address {
            let origin = app_state.origin.0 as usize;
            // Bitmaps must be aligned to 8192-byte boundaries
            let aligned_start_addr = (origin / 8192) * 8192;
            let addr = bitmap_addr.0 as usize;
            if addr >= aligned_start_addr {
                let offset = addr - aligned_start_addr;
                self.bitmap_cursor_index = offset / 8192;
            }
        }

        // --- Centralized File Load hooks ---

        // Import Context Setup (Wizard for raw files)
        let ext = app_state
            .file_path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if ext != "regen2000proj" {
            self.push_dialog(Box::new(
                crate::ui::dialog_import_context::ImportContextDialog::new(
                    &app_state.settings.platform.to_string(),
                    app_state.origin,
                    loaded_data.suggested_entry_point,
                    loaded_data.suggested_platform.clone(),
                    loaded_data.entropy_warning,
                ),
            ));
        }
    }
}
