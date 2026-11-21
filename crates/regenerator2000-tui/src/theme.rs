//! TUI color palette definitions and theme lookup APIs.

use crate::theme_file;
use ratatui::style::Color;

/// Complete color scheme for TUI panes, syntax tokens, menus, dialogs, and minimap rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Human-readable display name of this theme.
    pub name: String,
    /// Primary canvas background color.
    pub background: Color,
    /// Default text foreground color.
    pub foreground: Color,
    /// Border color for the focused pane.
    pub border_active: Color,
    /// Border color for unfocused panes.
    pub border_inactive: Color,
    /// Background color for selected lines or cells.
    pub selection_bg: Color,
    /// Foreground color for selected lines or cells.
    pub selection_fg: Color,
    /// Background color for selected rows in the Blocks view.
    pub block_selection_bg: Color,
    /// Foreground color for selected rows in the Blocks view.
    pub block_selection_fg: Color,
    /// Background color for the bottom status bar.
    pub status_bar_bg: Color,
    /// Foreground color for the bottom status bar.
    pub status_bar_fg: Color,

    // Code / Disassembly
    /// Disassembly instruction address column color.
    pub address: Color,
    /// Raw machine-code hex bytes column color.
    pub bytes: Color,
    /// 6502 mnemonic token color.
    pub mnemonic: Color,
    /// Operand expression token color.
    pub operand: Color,
    /// Inline label reference color.
    pub label: Color,
    /// Standalone label definition color.
    pub label_def: Color,
    /// Side and line comment text color.
    pub comment: Color,
    /// Control-flow gutter arrow color.
    pub arrow: Color,
    /// Collapsed block summary foreground color.
    pub collapsed_block: Color,
    /// Collapsed block summary background color.
    pub collapsed_block_bg: Color,

    // Hex View
    /// Hex dump byte value default color.
    pub hex_bytes: Color,
    /// Hex dump PETSCII/ASCII gutter text color.
    pub hex_ascii: Color,
    /// 18-entry palette for byte-value coloring in the hex dump.
    /// Index 0 = byte `0x00` (special), indices 1–16 = high nibble `0x0`–`0xF`
    /// (for bytes `0x01`–`0xFE`), index 17 = byte `0xFF` (special).
    pub hex_color_palette: [Color; 18],

    // UI Elements
    /// Modal dialog background color.
    pub dialog_bg: Color,
    /// Modal dialog default foreground color.
    pub dialog_fg: Color,
    /// Modal dialog border color.
    pub dialog_border: Color,
    /// Top menu bar and popup background color.
    pub menu_bg: Color,
    /// Top menu bar and popup foreground color.
    pub menu_fg: Color,
    /// Selected menu item background color.
    pub menu_selected_bg: Color,
    /// Selected menu item foreground color.
    pub menu_selected_fg: Color,
    /// Disabled menu item and muted widget foreground color.
    pub menu_disabled_fg: Color,

    /// Multicolor sprite bit-pair `10` accent color.
    pub sprite_multicolor_1: Color,
    /// Multicolor sprite bit-pair `11` accent color.
    pub sprite_multicolor_2: Color,
    /// Multicolor charset bit-pair `10` accent color.
    pub charset_multicolor_1: Color,
    /// Multicolor charset bit-pair `11` accent color.
    pub charset_multicolor_2: Color,

    // Highlights
    /// Emphasized foreground color for active inputs, headers, and focused markers.
    pub highlight_fg: Color,
    /// Emphasized background color for active inputs and badges.
    pub highlight_bg: Color,
    /// Validation and runtime error message foreground color.
    pub error_fg: Color,

    // Block Types (Foregrounds)
    /// Minimap and block-list foreground for Code blocks.
    pub block_code_fg: Color,
    /// Minimap and block-list foreground for Scope blocks.
    pub block_scope_fg: Color,
    /// Minimap and block-list foreground for Data Byte blocks.
    pub block_data_byte_fg: Color,
    /// Minimap and block-list foreground for Data Word blocks.
    pub block_data_word_fg: Color,
    /// Minimap and block-list foreground for Address table blocks.
    pub block_address_fg: Color,
    /// Minimap and block-list foreground for PETSCII text blocks.
    pub block_petscii_text_fg: Color,
    /// Minimap and block-list foreground for Screencode text blocks.
    pub block_screencode_text_fg: Color,
    /// Minimap and block-list foreground for Lo/Hi table blocks.
    pub block_lohi_fg: Color,
    /// Minimap and block-list foreground for Hi/Lo table blocks.
    pub block_hilo_fg: Color,
    /// Minimap and block-list foreground for External File regions.
    pub block_external_file_fg: Color,
    /// Minimap and block-list foreground for Undefined regions.
    pub block_undefined_fg: Color,
    /// Minimap and block-list foreground for Splitter markers.
    pub block_splitter_fg: Color,

    // Block Types (Backgrounds)
    /// Minimap and block-list background for Code blocks.
    pub block_code_bg: Color,
    /// Minimap and block-list background for Scope blocks.
    pub block_scope_bg: Color,
    /// Minimap and block-list background for Data Byte blocks.
    pub block_data_byte_bg: Color,
    /// Minimap and block-list background for Data Word blocks.
    pub block_data_word_bg: Color,
    /// Minimap and block-list background for Address table blocks.
    pub block_address_bg: Color,
    /// Minimap and block-list background for PETSCII text blocks.
    pub block_petscii_text_bg: Color,
    /// Minimap and block-list background for Screencode text blocks.
    pub block_screencode_text_bg: Color,
    /// Minimap and block-list background for Lo/Hi table blocks.
    pub block_lohi_bg: Color,
    /// Minimap and block-list background for Hi/Lo table blocks.
    pub block_hilo_bg: Color,
    /// Minimap and block-list background for External File regions.
    pub block_external_file_bg: Color,
    /// Minimap and block-list background for Undefined regions.
    pub block_undefined_bg: Color,
    /// Minimap and block-list background for Splitter markers.
    pub block_splitter_bg: Color,
    /// Indicator color for the current cursor block in the minimap bar.
    pub minimap_cursor_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_name("Dracula")
    }
}

impl Theme {
    /// Loads a theme by display name.
    ///
    /// Custom themes in the user configuration directory override built-in
    /// themes. Unknown names fall back to `"Solarized Dark"`.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        theme_file::find_custom_theme(name)
            .unwrap_or_else(|| theme_file::builtin_theme_by_name(name))
    }

    /// Returns the display names of all built-in and custom themes.
    #[must_use]
    pub fn all_names() -> Vec<String> {
        let mut names = theme_file::builtin_theme_names();

        // Append custom theme names, skipping any that match built-in names
        for name in theme_file::custom_theme_names() {
            if !names.iter().any(|n| n == &name) {
                names.push(name);
            }
        }

        names
    }
}
