use crate::state::HexdumpViewMode;

// ---------------------------------------------------------------------------
// Enums shared across all frontends
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    Disassembly,
    HexDump,
    Sprites,
    Charset,
    Bitmap,
    Blocks,
    Debugger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTarget {
    Index(usize),
    Address(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPane {
    None,
    #[default]
    HexDump,
    Sprites,
    Charset,
    Bitmap,
    Blocks,
    Debugger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRamMode {
    AfterBitmap,
    BankOffset(u8), // 0-15
}

// ---------------------------------------------------------------------------
// CoreViewState — frontend-agnostic view/cursor state
// ---------------------------------------------------------------------------

/// Shared view state that any frontend (TUI, GUI, web) needs.
///
/// Contains cursor positions, selections, active pane, view modes, and
/// navigation history. Does **not** include rendering primitives (Rect, Theme,
/// ListState) or TUI widgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreViewState {
    // Navigation
    pub navigation_history: Vec<(ActivePane, NavigationTarget)>,
    pub active_pane: ActivePane,
    pub right_pane: RightPane,

    // Disassembly cursors
    pub cursor_index: usize,
    pub sub_cursor_index: usize,
    pub scroll_index: usize,
    pub scroll_sub_index: usize,
    pub disassembly_viewport_height: usize,
    pub selection_start: Option<usize>,
    pub is_visual_mode: bool,

    // Hex view
    pub hex_cursor_index: usize,
    pub hex_col_cursor: usize,
    pub hex_selection_start: Option<usize>,
    pub hex_selection_start_col: usize,
    pub hex_scroll_index: usize,
    pub hexdump_view_mode: HexdumpViewMode,

    // Sprites / Charset / Bitmap cursors
    pub sprites_cursor_index: usize,
    pub sprites_selection_start: Option<usize>,
    pub charset_cursor_index: usize,
    pub charset_selection_start: Option<usize>,
    pub bitmap_cursor_index: usize,
    pub bitmap_screen_ram_mode: ScreenRamMode,

    // Persisted modes
    pub sprite_multicolor_mode: bool,
    pub charset_multicolor_mode: bool,
    pub bitmap_multicolor_mode: bool,

    // Blocks view (plain index, not ratatui ListState)
    pub blocks_selected_index: Option<usize>,

    // Search
    pub last_search_query: String,
    pub search_filters: crate::state::search::SearchFilters,

    /// Status message for the UI.
    pub status_message: Option<String>,
}

impl CoreViewState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            navigation_history: Vec::new(),
            active_pane: ActivePane::Disassembly,
            right_pane: RightPane::HexDump,

            cursor_index: 0,
            sub_cursor_index: 0,
            scroll_index: 0,
            scroll_sub_index: 0,
            disassembly_viewport_height: 0,
            selection_start: None,
            is_visual_mode: false,

            hex_cursor_index: 0,
            hex_col_cursor: 0,
            hex_selection_start: None,
            hex_selection_start_col: 0,
            hex_scroll_index: 0,
            hexdump_view_mode: HexdumpViewMode::ScreencodeShifted,

            sprites_cursor_index: 0,
            sprites_selection_start: None,
            charset_cursor_index: 0,
            charset_selection_start: None,
            bitmap_cursor_index: 0,
            bitmap_screen_ram_mode: ScreenRamMode::AfterBitmap,

            sprite_multicolor_mode: false,
            charset_multicolor_mode: false,
            bitmap_multicolor_mode: false,

            blocks_selected_index: None,
            last_search_query: String::new(),
            search_filters: crate::state::search::SearchFilters::default(),
            status_message: None,
        }
    }
}

impl Default for CoreViewState {
    fn default() -> Self {
        Self::new()
    }
}
