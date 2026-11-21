//! Transient TUI-only view state, layout hit-test rectangles, and graphics caches.

use crate::state::{AppState, BlockItem, EntryAnalysisMode, LoadedProjectData};
use crate::terminal_graphics::{self, TerminalImageProtocol};
use crate::theme::Theme;
use crate::ui::dialog_import_context::ImportContextDialog;
use crate::ui::navigable::AlignedGridRange;
use crate::ui::statusbar::StatusBarState;
use crate::ui::widget::Widget;
use crate::utils;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use regenerator2000_core::disassembler::DisassemblyLine;
use std::collections::HashMap;
use std::env;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub use crate::ui::menu::MenuState;
pub use regenerator2000_core::state::actions::AppAction;
// Re-export core enums so existing `crate::ui_state::ActivePane` etc. still work.
pub use regenerator2000_core::view_state::{
    ActivePane, CoreViewState, NavigationTarget, RightPane, ScreenRamMode,
};

/// Cache key identifying a decoded C64 bitmap and its Screen RAM configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BitmapCacheKey {
    /// Program load origin address.
    pub origin: usize,
    /// Base memory address of the 8 KB bitmap buffer.
    pub bitmap_address: usize,
    /// `true` for multicolor mode (`160x200`), `false` for high-res mode (`320x200`).
    pub multicolor_mode: bool,
    /// Base memory address of the 1 KB Screen RAM attribute block.
    pub screen_ram_address: usize,
}

/// Cached graphics protocol slot for `BitmapView`, paired with the key that produced it.
pub struct CachedBitmapProtocol {
    /// Key of the encoded bitmap image, or `None` when invalidated.
    pub key: Option<BitmapCacheKey>,
    /// Terminal image protocol instance reused across frames to avoid leaking GPU IDs.
    pub protocol: TerminalImageProtocol,
}

/// Cached live disassembly lines for `DebuggerView` keyed by base address and memory bytes.
#[derive(Debug, Clone)]
pub struct DebuggerDisasmCache {
    /// Start address of [`Self::live_bytes`].
    pub mem_start: u16,
    /// Snapshot of live VICE memory bytes from which [`Self::lines`] were disassembled.
    pub live_bytes: Vec<u8>,
    /// Disassembled instruction lines displayed in the live debugger panel.
    pub lines: Vec<DisassemblyLine>,
}

/// Mutable transient TUI state wrapping [`CoreViewState`] via [`Deref`]/[`DerefMut`].
pub struct UIState {
    /// Frontend-agnostic state (cursors, selections, panes, modes).
    /// Accessible directly via `Deref`/`DerefMut`.
    pub core: CoreViewState,

    // --- TUI-only state below ---
    /// Currently visible modal dialog widget, if any.
    pub active_dialog: Option<Box<dyn Widget>>,
    /// FIFO queue of pending modal dialogs shown sequentially after [`Self::active_dialog`] closes.
    pub dialog_queue: Vec<Box<dyn Widget>>,
    /// Working directory currently displayed in the file browser dialog.
    pub file_dialog_current_dir: PathBuf,

    /// Top menu bar navigation and availability state.
    pub menu: MenuState,

    /// Ratatui selection state for the Blocks view list.
    pub blocks_list_state: ListState,
    /// Ratatui selection state for the Bookmarks dialog list.
    pub bookmarks_list_state: ListState,
    /// Ratatui selection state for the Open Recent Projects dialog list.
    pub recent_list_state: ListState,

    /// Set to `true` when the application event loop should exit.
    pub should_quit: bool,
    /// Bottom status bar text and transient highlight timer state.
    pub status_bar: StatusBarState,

    /// Decoded application logo image shown on the About dialog and empty welcome screen.
    pub logo: Option<DynamicImage>,
    /// Active graphics protocol picker.
    ///
    /// [`UIState::new`] initializes this field to a headless `Halfblocks` picker so tests avoid
    /// stdio I/O. Interactive startup replaces it with the probed terminal [`Picker`].
    pub picker: Option<Picker>,
    /// Cached [`TerminalImageProtocol`] slot for `AboutDialog`.
    ///
    /// Reusing this slot across `Help -> About` openings double-buffers the Kitty image ID pair
    /// (`id` / `id ^ 1`) while re-transmitting when `AboutDialog` opens (avoiding stale textures
    /// after `tmux` reattach or terminal eviction).
    pub about_logo_protocol: Option<TerminalImageProtocol>,
    /// `true` after the user dismisses the startup welcome logo.
    pub dismiss_logo: bool,
    /// Numeric prefix buffer accumulated for `[N]G` jump shortcuts.
    pub input_buffer: String,

    /// Active TUI color palette.
    pub theme: Theme,

    // Vim-like search
    /// `true` while incremental `/` vim search input is open in the status bar.
    pub vim_search_active: bool,
    /// Incremental search pattern typed during vim search.
    pub vim_search_input: String,

    /// Cached native `320x200` decoded bitmaps keyed by [`BitmapCacheKey`].
    pub bitmap_cache: HashMap<BitmapCacheKey, DynamicImage>,
    /// Cached [`TerminalImageProtocol`] slot for `BitmapView`.
    ///
    /// Reusing this slot double-buffers the Kitty image ID pair (`id` / `id ^ 1`) across bitmap,
    /// Screen RAM, theme, origin, and project-reload transitions without leaking GPU IDs.
    pub bitmap_protocol: Option<CachedBitmapProtocol>,

    /// Latest upstream release version string when a newer version is detected.
    pub new_version_available: Option<String>,

    /// Remaining timer ticks for flashing the debugger pause indicator.
    pub debugger_flash_remaining: u8,
    /// Cached live disassembly lines for `DebuggerView`.
    pub debugger_disasm_cache: Option<DebuggerDisasmCache>,
    /// Per-frame cached `BlockItem` list computed during `sync_views_before_render` and consumed by `BlocksView::render`.
    pub cached_blocks_items: Option<Vec<BlockItem>>,

    // Layout Areas for Mouse Interaction
    /// Rendered bounds of the top menu bar.
    pub menu_area: Rect,
    /// Rendered bounds of the main split workspace (excluding menu, minimap, and status bar).
    pub main_area: Rect,
    /// Rendered bounds of the bottom status bar.
    pub status_bar_area: Rect,
    /// Rendered bounds of the left disassembly pane.
    pub disassembly_area: Rect,
    /// Rendered bounds of the right auxiliary pane.
    pub right_pane_area: Rect,
    /// Rendered bounds of the active modal dialog for `[x]` close hit-testing.
    pub active_dialog_area: Rect,
    /// Rendered bounds of the top memory minimap bar.
    pub minimap_area: Rect,
    /// The rendered area of the textarea inside the line-comment dialog.
    /// Used to translate mouse clicks to textarea row/col coordinates.
    pub comment_textarea_area: Rect,

    /// Entry-point analysis mode carried from a pre-unpack `ImportContextDialog` into the
    /// post-unpack `ImportContextDialog` so the user's `[ ] Disassemble from Entry Point Address`
    /// toggle persists across unpacking.
    pub pending_unpack_entry_mode: Option<EntryAnalysisMode>,
}

// ---------------------------------------------------------------------------
// Deref to CoreViewState so `ui_state.cursor_index` keeps working unchanged
// ---------------------------------------------------------------------------

impl Deref for UIState {
    type Target = CoreViewState;
    fn deref(&self) -> &CoreViewState {
        &self.core
    }
}

impl DerefMut for UIState {
    fn deref_mut(&mut self) -> &mut CoreViewState {
        &mut self.core
    }
}

impl UIState {
    /// Creates transient UI state with a deterministic headless
    /// [`Halfblocks`](ratatui_image::picker::ProtocolType::Halfblocks) picker.
    ///
    /// Interactive callers overwrite `picker` with the result of
    /// [`probe_terminal_picker`](crate::terminal_graphics::probe_terminal_picker).
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self {
            core: CoreViewState::new(),

            active_dialog: None,
            dialog_queue: Vec::new(),
            file_dialog_current_dir: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),

            menu: MenuState::new(),
            blocks_list_state: ListState::default(),
            bookmarks_list_state: ListState::default(),
            recent_list_state: ListState::default(),
            should_quit: false,
            status_bar: StatusBarState::new(),
            logo: utils::load_logo(),
            // Avoid stdio probing here so headless `TestBackend` tests stay deterministic and fast.
            picker: Some(terminal_graphics::create_headless_picker()),
            about_logo_protocol: None,
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

            menu_area: Rect::default(),
            main_area: Rect::default(),
            status_bar_area: Rect::default(),
            disassembly_area: Rect::default(),
            right_pane_area: Rect::default(),
            active_dialog_area: Rect::default(),
            minimap_area: Rect::default(),
            comment_textarea_area: Rect::default(),
            pending_unpack_entry_mode: None,
        }
    }

    /// Shows `dialog` immediately if no modal is active, or queues it in [`Self::dialog_queue`].
    pub fn push_dialog(&mut self, dialog: Box<dyn Widget>) {
        if self.active_dialog.is_none() {
            // Clear any bounds recorded by a prior modal before the newly activated dialog renders.
            self.active_dialog_area = Rect::default();
            self.active_dialog = Some(dialog);
        } else {
            self.dialog_queue.push(dialog);
        }
    }

    /// Promotes the next queued dialog into `active_dialog` when no dialog is currently active.
    pub fn promote_next_dialog(&mut self) {
        if self.active_dialog.is_none() {
            // Drop stale modal bounds regardless of whether another queued dialog is promoted so
            // pre-render mouse `[x]` hit-testing cannot trigger before the next frame renders.
            self.active_dialog_area = Rect::default();
            if !self.dialog_queue.is_empty() {
                self.active_dialog = Some(self.dialog_queue.remove(0));
            }
        }
    }

    /// Updates both core status message and the TUI status bar display string.
    pub fn set_status_message(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.core.status_message = Some(msg.clone());
        self.status_bar.set_message(msg);
    }

    /// Synchronizes the TUI status bar with any message stored in `CoreViewState`.
    pub fn sync_status_message(&mut self) {
        if let Some(msg) = self.core.status_message.take() {
            self.status_bar.set_message(msg);
        }
    }

    /// Synchronizes TUI-managed widget state into `CoreViewState`.
    pub fn sync_tui_to_core(&mut self) {
        self.core.blocks_selected_index = self.blocks_list_state.selected();
    }

    /// Synchronizes `CoreViewState` fields into TUI-managed widget state.
    pub fn sync_core_to_tui(&mut self) {
        if let Some(idx) = self.core.blocks_selected_index {
            self.blocks_list_state.select(Some(idx));
        }
    }

    /// Resets the debugger flash counter and clears the live disassembly cache.
    pub fn clear_debugger_state(&mut self) {
        self.debugger_flash_remaining = 0;
        self.debugger_disasm_cache = None;
    }

    /// Infers the full terminal bounds from the recorded layout rectangles.
    /// Falls back to `u16::MAX` height when only `menu_area` is set, such as in unit tests.
    #[must_use]
    pub fn terminal_area(&self) -> Rect {
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
        Rect::new(0, 0, width, height)
    }

    /// Clears cached decoded bitmaps and marks any existing [`UIState::bitmap_protocol`] as
    /// stale (`None` key) so the next render re-encodes in place without leaking Kitty IDs.
    pub fn invalidate_bitmap_cache(&mut self) {
        self.bitmap_cache.clear();
        if let Some(cached) = &mut self.bitmap_protocol {
            cached.key = None;
        }
    }

    /// Resets transient UI state, clears all selections, and restores saved pane cursors from
    /// `loaded_data`.
    pub fn restore_session(&mut self, loaded_data: &LoadedProjectData, app_state: &AppState) {
        self.invalidate_bitmap_cache();
        self.cached_blocks_items = None;
        self.pending_unpack_entry_mode = None;
        self.navigation_history.clear();
        self.active_pane = ActivePane::Disassembly;
        // Clear visual mode and all pane selection anchors.
        self.core.clear_all_selections();

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
        if let Some(idx) = app_state.line_index_for_address(initial_addr) {
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
                env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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
            && idx < app_state.blocks_view_items().len()
        {
            self.blocks_list_state.select(Some(idx));
            self.blocks_selected_index = Some(idx);
        }

        if !app_state.raw_data.is_empty() {
            if let Some(hex_addr) = loaded_hex_cursor
                && let Some(row) = AlignedGridRange::hex_dump(app_state, self.right_pane)
                    .index_for_addr(hex_addr.0 as usize)
            {
                self.hex_cursor_index = row;
            }

            if let Some(sprites_addr) = loaded_sprites_cursor
                && let Some(idx) =
                    AlignedGridRange::sprites(app_state).index_for_addr(sprites_addr.0 as usize)
            {
                self.sprites_cursor_index = idx;
            }

            if let Some(charset_addr) = loaded_charset_cursor
                && let Some(idx) =
                    AlignedGridRange::charset(app_state).index_for_addr(charset_addr.0 as usize)
            {
                self.charset_cursor_index = idx;
            }

            if let Some(bitmap_addr) = loaded_data.bitmap_cursor_address
                && let Some(idx) =
                    AlignedGridRange::bitmap(app_state).index_for_addr(bitmap_addr.0 as usize)
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
            self.push_dialog(Box::new(ImportContextDialog::new(
                app_state.settings.system.as_str(),
                app_state.origin,
                loaded_data.suggested_entry_point,
                loaded_data.suggested_system.as_ref(),
                loaded_data.entropy_warning,
                detected_packer,
            )));
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
        ui_state.hex_selection_start_col = 3;
        ui_state.sprites_selection_start = Some(3);
        ui_state.charset_selection_start = Some(7);
        ui_state.bitmap_selection_start = Some(1);
        ui_state.sprites_cursor_index = 99;
        ui_state.charset_cursor_index = 99;
        ui_state.bitmap_cursor_index = 99;
        ui_state.blocks_list_state.select(Some(99));
        ui_state.blocks_selected_index = Some(99);
        ui_state
            .navigation_history
            .push((ActivePane::Disassembly, NavigationTarget::Address(0x0800)));
        ui_state.pending_unpack_entry_mode = Some(EntryAnalysisMode::SkipEntryAnalysis);

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
        assert_eq!(ui_state.hex_selection_start_col, 0);
        assert!(ui_state.sprites_selection_start.is_none());
        assert!(ui_state.charset_selection_start.is_none());
        assert!(ui_state.bitmap_selection_start.is_none());
        assert!(ui_state.navigation_history.is_empty());

        assert_eq!(ui_state.hex_cursor_index, 1);
        assert_eq!(ui_state.charset_cursor_index, 1);
        assert_eq!(ui_state.sprites_cursor_index, 0);
        assert_eq!(ui_state.bitmap_cursor_index, 0);
        assert_eq!(ui_state.blocks_selected_index, None);
        assert_eq!(ui_state.blocks_list_state.selected(), None);
        assert_eq!(ui_state.pending_unpack_entry_mode, None);
    }
}
