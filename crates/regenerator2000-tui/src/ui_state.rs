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
    pub bitmap_protocol: Option<(
        (usize, bool, usize),
        ratatui_image::protocol::StatefulProtocol,
    )>,

    // Version update notification
    pub new_version_available: Option<String>,

    // Flash countdown for debugger status line when breakpoint/watchpoint is hit.
    // Decremented on timer ticks; while > 0, renders with attention-grabbing style.
    pub debugger_flash_remaining: u8,
    /// Cached live disassembly lines for `DebuggerView` keyed by `(mem_start, live_bytes)`.
    pub debugger_disasm_cache: Option<(
        u16,
        Vec<u8>,
        Vec<regenerator2000_core::disassembler::DisassemblyLine>,
    )>,
    /// Per-frame cached `BlockItem` list computed during `sync_views_before_render` and consumed by `BlocksView::render`.
    pub cached_blocks_items: Option<Vec<regenerator2000_core::state::BlockItem>>,

    // Layout Areas for Mouse Interaction
    pub menu_area: ratatui::layout::Rect,
    pub main_area: ratatui::layout::Rect,
    pub status_bar_area: ratatui::layout::Rect,
    pub disassembly_area: ratatui::layout::Rect,
    pub right_pane_area: ratatui::layout::Rect,
    pub active_dialog_area: ratatui::layout::Rect,
    pub minimap_area: ratatui::layout::Rect,
    /// The rendered area of the textarea inside the line-comment dialog.
    /// Used to translate mouse clicks to textarea row/col coordinates.
    pub comment_textarea_area: ratatui::layout::Rect,
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
            bitmap_protocol: None,
            theme,

            new_version_available: None,
            debugger_flash_remaining: 0,
            debugger_disasm_cache: None,
            cached_blocks_items: None,

            menu_area: ratatui::layout::Rect::default(),
            main_area: ratatui::layout::Rect::default(),
            status_bar_area: ratatui::layout::Rect::default(),
            disassembly_area: ratatui::layout::Rect::default(),
            right_pane_area: ratatui::layout::Rect::default(),
            active_dialog_area: ratatui::layout::Rect::default(),
            minimap_area: ratatui::layout::Rect::default(),
            comment_textarea_area: ratatui::layout::Rect::default(),
        }
    }

    pub fn push_dialog(&mut self, dialog: Box<dyn Widget>) {
        if self.active_dialog.is_none() {
            self.active_dialog = Some(dialog);
        } else {
            self.dialog_queue.push(dialog);
        }
    }

    /// Promotes the next queued dialog into `active_dialog` when no dialog is currently active.
    pub fn promote_next_dialog(&mut self) {
        if self.active_dialog.is_none() && !self.dialog_queue.is_empty() {
            self.active_dialog = Some(self.dialog_queue.remove(0));
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

    /// Infers the full terminal bounds from the recorded layout rects, falling back to
    /// `u16::MAX` height when only `menu_area` has been populated (e.g. in unit tests).
    #[must_use]
    pub fn terminal_area(&self) -> ratatui::layout::Rect {
        let areas = [
            self.menu_area,
            self.minimap_area,
            self.main_area,
            self.status_bar_area,
        ];
        let width = areas
            .iter()
            .map(|r| r.x.saturating_add(r.width))
            .max()
            .unwrap_or(0);
        let known_bottom = areas
            .iter()
            .map(|r| r.y.saturating_add(r.height))
            .max()
            .unwrap_or(0);
        let height = if known_bottom > self.menu_area.y.saturating_add(1) {
            known_bottom
        } else {
            u16::MAX
        };
        ratatui::layout::Rect::new(0, 0, width, height)
    }

    pub fn restore_session(
        &mut self,
        loaded_data: &crate::state::LoadedProjectData,
        app_state: &crate::state::AppState,
    ) {
        use crate::ui::navigable::AlignedGridRange;

        self.bitmap_cache.clear();
        self.bitmap_protocol = None;
        self.cached_blocks_items = None;
        self.navigation_history.clear();
        self.active_pane = ActivePane::Disassembly;
        self.is_visual_mode = false;
        self.selection_start = None;
        self.hex_selection_start = None;
        self.hex_selection_start_col = 0;
        self.sprites_selection_start = None;
        self.charset_selection_start = None;

        self.cursor_index = 0;
        self.scroll_index = 0;
        self.sub_cursor_index = 0;
        self.scroll_sub_index = 0;
        self.hex_cursor_index = 0;
        self.hex_col_cursor = 0;
        self.hex_scroll_index = 0;
        self.sprites_cursor_index = 0;
        self.sprites_scroll_index = 0;
        self.charset_cursor_index = 0;
        self.charset_scroll_row = 0;
        self.bitmap_cursor_index = 0;
        self.blocks_list_state.select(None);
        self.blocks_selected_index = None;

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

        // Restore Right Pane first so hex cursor restoration uses the correct
        // bytes_per_row (8 vs 16) based on the saved pane mode.
        if let Some(pane_str) = loaded_right_pane {
            match pane_str.as_str() {
                "HexDump" | "HexDump16" => self.right_pane = RightPane::HexDump16,
                "HexDump8" => self.right_pane = RightPane::HexDump8,
                "None" => self.right_pane = RightPane::None,
                "Sprites" | "Sprites2Col" => self.right_pane = RightPane::Sprites2Col,
                "Sprites1Col" => self.right_pane = RightPane::Sprites1Col,
                "Charset" | "Charset8Col" => self.right_pane = RightPane::Charset8Col,
                "Charset4Col" => self.right_pane = RightPane::Charset4Col,
                "Bitmap" => self.right_pane = RightPane::Bitmap,
                "Blocks" => self.right_pane = RightPane::Blocks,
                "Debugger" => self.right_pane = RightPane::Debugger,
                _ => {}
            }
        }

        if let Some(idx) = loaded_data.blocks_view_cursor
            && idx < app_state.get_blocks_view_items().len()
        {
            self.blocks_list_state.select(Some(idx));
            self.blocks_selected_index = Some(idx);
        }

        if !app_state.raw_data.is_empty() {
            if let Some(hex_addr) = loaded_hex_cursor {
                let bytes_per_row = match self.right_pane {
                    RightPane::HexDump8 => 8,
                    _ => 16,
                };
                if let Some(row) = AlignedGridRange::uniform(app_state, bytes_per_row)
                    .index_for_addr(hex_addr.0 as usize)
                {
                    self.hex_cursor_index = row;
                }
            }

            if let Some(sprites_addr) = loaded_sprites_cursor
                && let Some(idx) =
                    AlignedGridRange::uniform(app_state, 64).index_for_addr(sprites_addr.0 as usize)
            {
                self.sprites_cursor_index = idx;
            }

            if let Some(charset_addr) = loaded_charset_cursor
                && let Some(idx) = AlignedGridRange::new(app_state, 0x400, 8, 8)
                    .index_for_addr(charset_addr.0 as usize)
            {
                self.charset_cursor_index = idx;
            }

            if let Some(bitmap_addr) = loaded_data.bitmap_cursor_address
                && let Some(idx) = AlignedGridRange::new(app_state, 8192, 8192, 8000)
                    .index_for_addr(bitmap_addr.0 as usize)
            {
                self.bitmap_cursor_index = idx;
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
            let detected_packer = loaded_data
                .detected_packer
                .as_deref()
                .or_else(|| app_state.file_info().packer_name);
            self.push_dialog(Box::new(
                crate::ui::dialog_import_context::ImportContextDialog::new(
                    app_state.settings.system.as_str(),
                    app_state.origin,
                    loaded_data.suggested_entry_point,
                    loaded_data.suggested_system.as_ref(),
                    loaded_data.entropy_warning,
                    detected_packer,
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Addr, AppState, BlockType, HexdumpViewMode, LoadedProjectData};

    #[test]
    fn restore_session_resets_stale_selections_active_pane_and_out_of_bounds_cursors() {
        let mut app_state = AppState::new();
        app_state.origin = Addr(0x0800);
        app_state.raw_data = vec![0xEA; 128];
        app_state.block_types = vec![BlockType::Code; 128];
        app_state.file_path = Some(PathBuf::from("test.regen2000proj"));
        app_state.disassemble();

        let mut ui_state = UIState::new(Theme::default());
        ui_state.active_pane = ActivePane::Sprites;
        ui_state.is_visual_mode = true;
        ui_state.selection_start = Some(10);
        ui_state.hex_selection_start = Some(5);
        ui_state.sprites_selection_start = Some(3);
        ui_state.charset_selection_start = Some(7);
        ui_state.sprites_cursor_index = 99;
        ui_state.charset_cursor_index = 99;
        ui_state.bitmap_cursor_index = 99;
        ui_state.blocks_list_state.select(Some(99));
        ui_state.blocks_selected_index = Some(99);
        ui_state
            .navigation_history
            .push((ActivePane::Disassembly, NavigationTarget::Address(0x0800)));

        let loaded = LoadedProjectData {
            cursor_address: Some(Addr(0x0800)),
            hex_dump_cursor_address: Some(Addr(0x0810)),
            sprites_cursor_address: Some(Addr(0x9000)), // out of bounds for 128-byte binary at $0800
            right_pane_visible: Some("Debugger".to_string()),
            charset_cursor_address: Some(Addr(0x0808)),
            bitmap_cursor_address: Some(Addr(0x6000)), // out of bounds
            sprite_multicolor_mode: true,
            charset_multicolor_mode: false,
            bitmap_multicolor_mode: Some(true),
            hexdump_view_mode: HexdumpViewMode::PETSCIIUnshifted,
            blocks_view_cursor: Some(50), // out of bounds (only 1 block)
            entropy_warning: None,
            suggested_entry_point: None,
            suggested_system: None,
            detected_packer: None,
        };

        ui_state.restore_session(&loaded, &app_state);

        assert_eq!(ui_state.active_pane, ActivePane::Disassembly);
        assert_eq!(ui_state.right_pane, RightPane::Debugger);
        assert!(!ui_state.is_visual_mode);
        assert!(ui_state.selection_start.is_none());
        assert!(ui_state.hex_selection_start.is_none());
        assert!(ui_state.sprites_selection_start.is_none());
        assert!(ui_state.charset_selection_start.is_none());
        assert!(ui_state.navigation_history.is_empty());

        assert_eq!(ui_state.hex_cursor_index, 1);
        assert_eq!(ui_state.charset_cursor_index, 1);
        assert_eq!(ui_state.sprites_cursor_index, 0);
        assert_eq!(ui_state.bitmap_cursor_index, 0);
        assert_eq!(ui_state.blocks_selected_index, None);
        assert_eq!(ui_state.blocks_list_state.selected(), None);
    }
}
