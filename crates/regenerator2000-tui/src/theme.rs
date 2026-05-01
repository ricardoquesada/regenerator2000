use ratatui::style::Color;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub border_active: Color,
    pub border_inactive: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub block_selection_bg: Color,
    pub block_selection_fg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,

    // Code / Disassembly
    pub address: Color,
    pub bytes: Color,
    pub mnemonic: Color,
    pub operand: Color,
    pub label: Color,
    pub label_def: Color,
    pub comment: Color,
    pub arrow: Color,
    pub collapsed_block: Color,
    pub collapsed_block_bg: Color,

    // Hex View
    pub hex_bytes: Color,
    pub hex_ascii: Color,
    /// 18-entry palette for byte-value coloring in the hex dump.
    /// Index 0 = byte 0x00 (special), indices 1–16 = high nibble 0x0–0xF
    /// (for bytes 0x01–0xFE), index 17 = byte 0xFF (special).
    pub hex_color_palette: [Color; 18],

    // UI Elements
    pub dialog_bg: Color,
    pub dialog_fg: Color,
    pub dialog_border: Color,
    pub menu_bg: Color,
    pub menu_fg: Color,
    pub menu_selected_bg: Color,
    pub menu_selected_fg: Color,
    pub menu_disabled_fg: Color,

    pub sprite_multicolor_1: Color,
    pub sprite_multicolor_2: Color,
    pub charset_multicolor_1: Color,
    pub charset_multicolor_2: Color,

    // Highlights
    pub highlight_fg: Color, // e.g. bold yellow/green text
    pub highlight_bg: Color,
    pub error_fg: Color,

    // Block Types (Foregrounds)
    pub block_code_fg: Color,
    pub block_scope_fg: Color,
    pub block_data_byte_fg: Color,
    pub block_data_word_fg: Color,
    pub block_address_fg: Color,
    pub block_petscii_text_fg: Color,
    pub block_screencode_text_fg: Color,
    pub block_lohi_fg: Color,
    pub block_hilo_fg: Color,
    pub block_external_file_fg: Color,
    pub block_undefined_fg: Color,
    pub block_splitter_fg: Color,

    // Block Types (Backgrounds)
    pub block_code_bg: Color,
    pub block_scope_bg: Color,
    pub block_data_byte_bg: Color,
    pub block_data_word_bg: Color,
    pub block_address_bg: Color,
    pub block_petscii_text_bg: Color,
    pub block_screencode_text_bg: Color,
    pub block_lohi_bg: Color,
    pub block_hilo_bg: Color,
    pub block_external_file_bg: Color,
    pub block_undefined_bg: Color,
    pub block_splitter_bg: Color,
    pub minimap_cursor_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::from_name("Dracula")
    }
}

impl Theme {
    /// Load a theme by display name.
    ///
    /// Custom themes from the user's config directory take precedence over
    /// built-in embedded themes.  If the name is not found, falls back to
    /// "Solarized Dark".
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        // Check custom themes first (user overrides take precedence)
        if let Some(theme) = crate::theme_file::find_custom_theme(name) {
            return theme;
        }
        // Then check built-in embedded themes
        if let Some(theme) = crate::theme_file::find_builtin_theme(name) {
            return theme;
        }
        // Fallback: load "Solarized Dark" from embedded assets
        crate::theme_file::find_builtin_theme("Solarized Dark")
            .unwrap_or_else(crate::theme_file::default_fallback_theme)
    }

    /// Return the display names of all available themes (built-in + custom).
    #[must_use]
    pub fn all_names() -> Vec<String> {
        let mut names = crate::theme_file::builtin_theme_names();

        // Append custom theme names, skipping any that match built-in names
        for name in crate::theme_file::custom_theme_names() {
            if !names.iter().any(|n| n == &name) {
                names.push(name);
            }
        }

        names
    }
}
