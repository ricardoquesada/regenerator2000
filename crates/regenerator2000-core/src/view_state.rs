//! Frontend-agnostic view, pane, and cursor state.

use crate::state::HexdumpViewMode;

/// Currently focused UI pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    /// Primary 6502 disassembly listing pane.
    Disassembly,
    /// Hexadecimal and character dump pane.
    HexDump,
    /// Hardware sprite viewer pane.
    Sprites,
    /// Character set glyph viewer pane.
    Charset,
    /// High-resolution and multicolor bitmap viewer pane.
    Bitmap,
    /// Memory block classification overview pane.
    Blocks,
    /// Live VICE debugger pane.
    Debugger,
}

/// Saved cursor destination in the navigation history stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationTarget {
    /// Zero-based item or row index within a pane.
    Index(usize),
    /// 16-bit 6502 memory address.
    Address(u16),
}

/// Active display mode for the right-hand auxiliary pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RightPane {
    /// Auxiliary pane hidden.
    None,
    #[default]
    /// 16-column hex dump (standard width).
    HexDump16,
    /// 8-column hex dump (narrow width).
    HexDump8,
    /// 2-column sprite view (side-by-side sprites).
    Sprites2Col,
    /// 1-column sprite view (one sprite per row).
    Sprites1Col,
    /// 8-column character set view (standard width).
    Charset8Col,
    /// 4-column character set view (narrow width).
    Charset4Col,
    /// Bitmap graphics viewer.
    Bitmap,
    /// Memory block list viewer.
    Blocks,
    /// Live VICE debugger panel.
    Debugger,
}

/// Screen RAM color source for the bitmap viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRamMode {
    /// Reads screen RAM immediately after the 8,000-byte bitmap data.
    AfterBitmap,
    /// Reads screen RAM from a 1 KB offset (`0..=15`) within the VIC bank.
    BankOffset(u8),
}

/// Shared view state for any frontend (TUI, GUI, or MCP).
///
/// Stores cursor positions, active selections, pane modes, and navigation history.
/// Excludes terminal rendering primitives such as rectangles, themes, and widget states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreViewState {
    /// Back-navigation stack of `(pane, target)` entries.
    pub navigation_history: Vec<(ActivePane, NavigationTarget)>,
    /// Currently focused pane.
    pub active_pane: ActivePane,
    /// Active right-hand pane mode.
    pub right_pane: RightPane,
    /// Last active hex dump column mode (`HexDump16` or `HexDump8`).
    pub last_hexdump_pane: RightPane,
    /// Last active sprite column mode (`Sprites2Col` or `Sprites1Col`).
    pub last_sprites_pane: RightPane,
    /// Last active character set column mode (`Charset8Col` or `Charset4Col`).
    pub last_charset_pane: RightPane,

    /// Current line index in `AppState::disassembly`.
    pub cursor_index: usize,
    /// Sub-line offset within the current disassembly line.
    pub sub_cursor_index: usize,
    /// Top visible disassembly line index.
    pub scroll_index: usize,
    /// Top visible sub-line offset within `scroll_index`.
    pub scroll_sub_index: usize,
    /// Last known disassembly viewport height in visual rows.
    pub disassembly_viewport_height: usize,
    /// Anchor line index for range selection in the disassembly pane.
    pub selection_start: Option<usize>,
    /// `true` when persistent visual selection mode (`Shift+V`) is active.
    pub is_visual_mode: bool,

    /// Current row index in the hex dump pane.
    pub hex_cursor_index: usize,
    /// Current byte column offset within the active hex dump row.
    pub hex_col_cursor: usize,
    /// Anchor row index for hex dump selection.
    pub hex_selection_start: Option<usize>,
    /// Anchor byte column offset for hex dump selection.
    pub hex_selection_start_col: usize,
    /// Top visible row index in the hex dump pane.
    pub hex_scroll_index: usize,
    /// Character decoding mode for the hex dump text column.
    pub hexdump_view_mode: HexdumpViewMode,

    /// Selected sprite index in the sprites pane.
    pub sprites_cursor_index: usize,
    /// Top visible row index in the sprites pane.
    pub sprites_scroll_index: usize,
    /// Anchor sprite index for range selection in the sprites pane.
    pub sprites_selection_start: Option<usize>,
    /// Selected character glyph index in the charset pane.
    pub charset_cursor_index: usize,
    /// Anchor character glyph index for range selection in the charset pane.
    pub charset_selection_start: Option<usize>,
    /// Top visible row index in the charset pane.
    pub charset_scroll_row: usize,
    /// Selected 8 KB slot index in the bitmap pane.
    pub bitmap_cursor_index: usize,
    /// Active screen RAM address mode in the bitmap pane.
    pub bitmap_screen_ram_mode: ScreenRamMode,

    /// `true` when multicolor rendering is enabled in the sprites pane.
    pub sprite_multicolor_mode: bool,
    /// `true` when multicolor rendering is enabled in the charset pane.
    pub charset_multicolor_mode: bool,
    /// `true` when multicolor rendering is enabled in the bitmap pane.
    pub bitmap_multicolor_mode: bool,

    /// Selected block index in the blocks pane.
    pub blocks_selected_index: Option<usize>,

    /// Most recent search query string.
    pub last_search_query: String,
    /// Active search filters.
    pub search_filters: crate::state::search::SearchFilters,

    /// Latest status message emitted by core actions.
    pub status_message: Option<String>,
}

impl CoreViewState {
    /// Creates a new [`CoreViewState`] with default pane and cursor positions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            navigation_history: Vec::new(),
            active_pane: ActivePane::Disassembly,
            right_pane: RightPane::HexDump16,
            last_hexdump_pane: RightPane::HexDump16,
            last_sprites_pane: RightPane::Sprites2Col,
            last_charset_pane: RightPane::Charset8Col,

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
            sprites_scroll_index: 0,
            sprites_selection_start: None,
            charset_cursor_index: 0,
            charset_selection_start: None,
            charset_scroll_row: 0,
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
