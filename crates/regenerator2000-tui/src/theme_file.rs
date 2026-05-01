//! TOML-based theme file loading, saving, and serialization.
//!
//! Provides a serializable representation of [`Theme`] that can be read from
//! and written to `theme-*.toml` files, enabling user-created custom themes.
//!
//! # File format
//!
//! Theme files are TOML documents whose keys mirror the fields of [`Theme`].
//! Colors are specified as hex strings (`"#RRGGBB"`).  An optional `base` key
//! names a built-in theme whose values are used for any omitted fields.
//!
//! ```text
//! name = "Green Screen"
//! base = "Solarized Dark"
//! background = "#001100"
//! foreground = "#33FF33"
//! ```

use crate::theme::Theme;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Global cache of custom themes loaded from the user's config directory.
static CUSTOM_THEMES: OnceLock<Vec<Theme>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Hex color helpers
// ---------------------------------------------------------------------------

/// Parse a hex color string (`#RRGGBB` or `RRGGBB`) into a ratatui [`Color`].
///
/// Returns `None` if the string is not a valid 6-digit hex color.
#[must_use]
pub fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

/// Convert a ratatui [`Color`] to a hex string (`#RRGGBB`).
///
/// Named colors that don't have an explicit RGB value are returned as
/// `#000000` (black) for `Color::Black`, `#FFFFFF` for `Color::White`,
/// and `#000000` as a fallback for other indexed colors.
#[must_use]
pub fn color_to_hex(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02X}{g:02X}{b:02X}"),
        Color::Black => "#000000".to_string(),
        Color::White => "#FFFFFF".to_string(),
        _ => "#000000".to_string(),
    }
}

// ---------------------------------------------------------------------------
// ThemeFile — TOML-serializable theme representation
// ---------------------------------------------------------------------------

/// A TOML-serializable representation of a [`Theme`].
///
/// Every color field is `Option<String>` so that partial theme files can
/// inherit unset values from a base theme.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeFile {
    /// Display name of the theme.
    pub name: String,

    /// Optional base theme name.  Unset fields inherit from this built-in.
    /// Defaults to `"Solarized Dark"` when absent.
    #[serde(default)]
    pub base: Option<String>,

    // -- Base colors --
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub border_active: Option<String>,
    pub border_inactive: Option<String>,
    pub selection_bg: Option<String>,
    pub selection_fg: Option<String>,
    pub block_selection_bg: Option<String>,
    pub block_selection_fg: Option<String>,
    pub status_bar_bg: Option<String>,
    pub status_bar_fg: Option<String>,

    // -- Code / Disassembly --
    pub address: Option<String>,
    pub bytes: Option<String>,
    pub mnemonic: Option<String>,
    pub operand: Option<String>,
    pub label: Option<String>,
    pub label_def: Option<String>,
    pub comment: Option<String>,
    pub arrow: Option<String>,
    pub collapsed_block: Option<String>,
    pub collapsed_block_bg: Option<String>,

    // -- Hex View --
    pub hex_bytes: Option<String>,
    pub hex_ascii: Option<String>,
    pub hex_color_palette: Option<Vec<String>>,

    // -- UI Elements --
    pub dialog_bg: Option<String>,
    pub dialog_fg: Option<String>,
    pub dialog_border: Option<String>,
    pub menu_bg: Option<String>,
    pub menu_fg: Option<String>,
    pub menu_selected_bg: Option<String>,
    pub menu_selected_fg: Option<String>,
    pub menu_disabled_fg: Option<String>,

    pub sprite_multicolor_1: Option<String>,
    pub sprite_multicolor_2: Option<String>,
    pub charset_multicolor_1: Option<String>,
    pub charset_multicolor_2: Option<String>,

    // -- Highlights --
    pub highlight_fg: Option<String>,
    pub highlight_bg: Option<String>,
    pub error_fg: Option<String>,

    // -- Block Types (Foregrounds) --
    pub block_code_fg: Option<String>,
    pub block_scope_fg: Option<String>,
    pub block_data_byte_fg: Option<String>,
    pub block_data_word_fg: Option<String>,
    pub block_address_fg: Option<String>,
    pub block_petscii_text_fg: Option<String>,
    pub block_screencode_text_fg: Option<String>,
    pub block_lohi_fg: Option<String>,
    pub block_hilo_fg: Option<String>,
    pub block_external_file_fg: Option<String>,
    pub block_undefined_fg: Option<String>,
    pub block_splitter_fg: Option<String>,

    // -- Block Types (Backgrounds) --
    pub block_code_bg: Option<String>,
    pub block_scope_bg: Option<String>,
    pub block_data_byte_bg: Option<String>,
    pub block_data_word_bg: Option<String>,
    pub block_address_bg: Option<String>,
    pub block_petscii_text_bg: Option<String>,
    pub block_screencode_text_bg: Option<String>,
    pub block_lohi_bg: Option<String>,
    pub block_hilo_bg: Option<String>,
    pub block_external_file_bg: Option<String>,
    pub block_undefined_bg: Option<String>,
    pub block_splitter_bg: Option<String>,
    pub minimap_cursor_fg: Option<String>,
}

/// Resolve an optional hex string against a fallback color.
fn resolve_color(opt: &Option<String>, fallback: Color) -> Color {
    opt.as_deref().and_then(parse_hex_color).unwrap_or(fallback)
}

impl ThemeFile {
    /// Convert a built-in [`Theme`] into a fully-populated [`ThemeFile`].
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            name: theme.name.clone(),
            base: None,
            background: Some(color_to_hex(theme.background)),
            foreground: Some(color_to_hex(theme.foreground)),
            border_active: Some(color_to_hex(theme.border_active)),
            border_inactive: Some(color_to_hex(theme.border_inactive)),
            selection_bg: Some(color_to_hex(theme.selection_bg)),
            selection_fg: Some(color_to_hex(theme.selection_fg)),
            block_selection_bg: Some(color_to_hex(theme.block_selection_bg)),
            block_selection_fg: Some(color_to_hex(theme.block_selection_fg)),
            status_bar_bg: Some(color_to_hex(theme.status_bar_bg)),
            status_bar_fg: Some(color_to_hex(theme.status_bar_fg)),
            address: Some(color_to_hex(theme.address)),
            bytes: Some(color_to_hex(theme.bytes)),
            mnemonic: Some(color_to_hex(theme.mnemonic)),
            operand: Some(color_to_hex(theme.operand)),
            label: Some(color_to_hex(theme.label)),
            label_def: Some(color_to_hex(theme.label_def)),
            comment: Some(color_to_hex(theme.comment)),
            arrow: Some(color_to_hex(theme.arrow)),
            collapsed_block: Some(color_to_hex(theme.collapsed_block)),
            collapsed_block_bg: Some(color_to_hex(theme.collapsed_block_bg)),
            hex_bytes: Some(color_to_hex(theme.hex_bytes)),
            hex_ascii: Some(color_to_hex(theme.hex_ascii)),
            hex_color_palette: Some(
                theme
                    .hex_color_palette
                    .iter()
                    .map(|c| color_to_hex(*c))
                    .collect(),
            ),
            dialog_bg: Some(color_to_hex(theme.dialog_bg)),
            dialog_fg: Some(color_to_hex(theme.dialog_fg)),
            dialog_border: Some(color_to_hex(theme.dialog_border)),
            menu_bg: Some(color_to_hex(theme.menu_bg)),
            menu_fg: Some(color_to_hex(theme.menu_fg)),
            menu_selected_bg: Some(color_to_hex(theme.menu_selected_bg)),
            menu_selected_fg: Some(color_to_hex(theme.menu_selected_fg)),
            menu_disabled_fg: Some(color_to_hex(theme.menu_disabled_fg)),
            sprite_multicolor_1: Some(color_to_hex(theme.sprite_multicolor_1)),
            sprite_multicolor_2: Some(color_to_hex(theme.sprite_multicolor_2)),
            charset_multicolor_1: Some(color_to_hex(theme.charset_multicolor_1)),
            charset_multicolor_2: Some(color_to_hex(theme.charset_multicolor_2)),
            highlight_fg: Some(color_to_hex(theme.highlight_fg)),
            highlight_bg: Some(color_to_hex(theme.highlight_bg)),
            error_fg: Some(color_to_hex(theme.error_fg)),
            block_code_fg: Some(color_to_hex(theme.block_code_fg)),
            block_scope_fg: Some(color_to_hex(theme.block_scope_fg)),
            block_data_byte_fg: Some(color_to_hex(theme.block_data_byte_fg)),
            block_data_word_fg: Some(color_to_hex(theme.block_data_word_fg)),
            block_address_fg: Some(color_to_hex(theme.block_address_fg)),
            block_petscii_text_fg: Some(color_to_hex(theme.block_petscii_text_fg)),
            block_screencode_text_fg: Some(color_to_hex(theme.block_screencode_text_fg)),
            block_lohi_fg: Some(color_to_hex(theme.block_lohi_fg)),
            block_hilo_fg: Some(color_to_hex(theme.block_hilo_fg)),
            block_external_file_fg: Some(color_to_hex(theme.block_external_file_fg)),
            block_undefined_fg: Some(color_to_hex(theme.block_undefined_fg)),
            block_splitter_fg: Some(color_to_hex(theme.block_splitter_fg)),
            block_code_bg: Some(color_to_hex(theme.block_code_bg)),
            block_scope_bg: Some(color_to_hex(theme.block_scope_bg)),
            block_data_byte_bg: Some(color_to_hex(theme.block_data_byte_bg)),
            block_data_word_bg: Some(color_to_hex(theme.block_data_word_bg)),
            block_address_bg: Some(color_to_hex(theme.block_address_bg)),
            block_petscii_text_bg: Some(color_to_hex(theme.block_petscii_text_bg)),
            block_screencode_text_bg: Some(color_to_hex(theme.block_screencode_text_bg)),
            block_lohi_bg: Some(color_to_hex(theme.block_lohi_bg)),
            block_hilo_bg: Some(color_to_hex(theme.block_hilo_bg)),
            block_external_file_bg: Some(color_to_hex(theme.block_external_file_bg)),
            block_undefined_bg: Some(color_to_hex(theme.block_undefined_bg)),
            block_splitter_bg: Some(color_to_hex(theme.block_splitter_bg)),
            minimap_cursor_fg: Some(color_to_hex(theme.minimap_cursor_fg)),
        }
    }

    /// Convert this file representation into a runtime [`Theme`].
    ///
    /// Fields that are `None` fall back to the base theme identified by
    /// [`Self::base`].  If `base` is not set, `"Solarized Dark"` is used.
    #[must_use]
    pub fn to_theme(&self) -> Theme {
        let fallback = self
            .base
            .as_deref()
            .map(builtin_theme_by_name)
            .unwrap_or_else(Theme::dark);

        // Resolve the hex palette, falling back entry-by-entry.
        let hex_palette = if let Some(ref palette) = self.hex_color_palette {
            let mut result = fallback.hex_color_palette;
            for (i, hex) in palette.iter().enumerate().take(18) {
                if let Some(c) = parse_hex_color(hex) {
                    result[i] = c;
                }
            }
            result
        } else {
            fallback.hex_color_palette
        };

        Theme {
            name: self.name.clone(),
            background: resolve_color(&self.background, fallback.background),
            foreground: resolve_color(&self.foreground, fallback.foreground),
            border_active: resolve_color(&self.border_active, fallback.border_active),
            border_inactive: resolve_color(&self.border_inactive, fallback.border_inactive),
            selection_bg: resolve_color(&self.selection_bg, fallback.selection_bg),
            selection_fg: resolve_color(&self.selection_fg, fallback.selection_fg),
            block_selection_bg: resolve_color(
                &self.block_selection_bg,
                fallback.block_selection_bg,
            ),
            block_selection_fg: resolve_color(
                &self.block_selection_fg,
                fallback.block_selection_fg,
            ),
            status_bar_bg: resolve_color(&self.status_bar_bg, fallback.status_bar_bg),
            status_bar_fg: resolve_color(&self.status_bar_fg, fallback.status_bar_fg),
            address: resolve_color(&self.address, fallback.address),
            bytes: resolve_color(&self.bytes, fallback.bytes),
            mnemonic: resolve_color(&self.mnemonic, fallback.mnemonic),
            operand: resolve_color(&self.operand, fallback.operand),
            label: resolve_color(&self.label, fallback.label),
            label_def: resolve_color(&self.label_def, fallback.label_def),
            comment: resolve_color(&self.comment, fallback.comment),
            arrow: resolve_color(&self.arrow, fallback.arrow),
            collapsed_block: resolve_color(&self.collapsed_block, fallback.collapsed_block),
            collapsed_block_bg: resolve_color(
                &self.collapsed_block_bg,
                fallback.collapsed_block_bg,
            ),
            hex_bytes: resolve_color(&self.hex_bytes, fallback.hex_bytes),
            hex_ascii: resolve_color(&self.hex_ascii, fallback.hex_ascii),
            hex_color_palette: hex_palette,
            dialog_bg: resolve_color(&self.dialog_bg, fallback.dialog_bg),
            dialog_fg: resolve_color(&self.dialog_fg, fallback.dialog_fg),
            dialog_border: resolve_color(&self.dialog_border, fallback.dialog_border),
            menu_bg: resolve_color(&self.menu_bg, fallback.menu_bg),
            menu_fg: resolve_color(&self.menu_fg, fallback.menu_fg),
            menu_selected_bg: resolve_color(&self.menu_selected_bg, fallback.menu_selected_bg),
            menu_selected_fg: resolve_color(&self.menu_selected_fg, fallback.menu_selected_fg),
            menu_disabled_fg: resolve_color(&self.menu_disabled_fg, fallback.menu_disabled_fg),
            sprite_multicolor_1: resolve_color(
                &self.sprite_multicolor_1,
                fallback.sprite_multicolor_1,
            ),
            sprite_multicolor_2: resolve_color(
                &self.sprite_multicolor_2,
                fallback.sprite_multicolor_2,
            ),
            charset_multicolor_1: resolve_color(
                &self.charset_multicolor_1,
                fallback.charset_multicolor_1,
            ),
            charset_multicolor_2: resolve_color(
                &self.charset_multicolor_2,
                fallback.charset_multicolor_2,
            ),
            highlight_fg: resolve_color(&self.highlight_fg, fallback.highlight_fg),
            highlight_bg: resolve_color(&self.highlight_bg, fallback.highlight_bg),
            error_fg: resolve_color(&self.error_fg, fallback.error_fg),
            block_code_fg: resolve_color(&self.block_code_fg, fallback.block_code_fg),
            block_scope_fg: resolve_color(&self.block_scope_fg, fallback.block_scope_fg),
            block_data_byte_fg: resolve_color(
                &self.block_data_byte_fg,
                fallback.block_data_byte_fg,
            ),
            block_data_word_fg: resolve_color(
                &self.block_data_word_fg,
                fallback.block_data_word_fg,
            ),
            block_address_fg: resolve_color(&self.block_address_fg, fallback.block_address_fg),
            block_petscii_text_fg: resolve_color(
                &self.block_petscii_text_fg,
                fallback.block_petscii_text_fg,
            ),
            block_screencode_text_fg: resolve_color(
                &self.block_screencode_text_fg,
                fallback.block_screencode_text_fg,
            ),
            block_lohi_fg: resolve_color(&self.block_lohi_fg, fallback.block_lohi_fg),
            block_hilo_fg: resolve_color(&self.block_hilo_fg, fallback.block_hilo_fg),
            block_external_file_fg: resolve_color(
                &self.block_external_file_fg,
                fallback.block_external_file_fg,
            ),
            block_undefined_fg: resolve_color(
                &self.block_undefined_fg,
                fallback.block_undefined_fg,
            ),
            block_splitter_fg: resolve_color(&self.block_splitter_fg, fallback.block_splitter_fg),
            block_code_bg: resolve_color(&self.block_code_bg, fallback.block_code_bg),
            block_scope_bg: resolve_color(&self.block_scope_bg, fallback.block_scope_bg),
            block_data_byte_bg: resolve_color(
                &self.block_data_byte_bg,
                fallback.block_data_byte_bg,
            ),
            block_data_word_bg: resolve_color(
                &self.block_data_word_bg,
                fallback.block_data_word_bg,
            ),
            block_address_bg: resolve_color(&self.block_address_bg, fallback.block_address_bg),
            block_petscii_text_bg: resolve_color(
                &self.block_petscii_text_bg,
                fallback.block_petscii_text_bg,
            ),
            block_screencode_text_bg: resolve_color(
                &self.block_screencode_text_bg,
                fallback.block_screencode_text_bg,
            ),
            block_lohi_bg: resolve_color(&self.block_lohi_bg, fallback.block_lohi_bg),
            block_hilo_bg: resolve_color(&self.block_hilo_bg, fallback.block_hilo_bg),
            block_external_file_bg: resolve_color(
                &self.block_external_file_bg,
                fallback.block_external_file_bg,
            ),
            block_undefined_bg: resolve_color(
                &self.block_undefined_bg,
                fallback.block_undefined_bg,
            ),
            block_splitter_bg: resolve_color(&self.block_splitter_bg, fallback.block_splitter_bg),
            minimap_cursor_fg: resolve_color(&self.minimap_cursor_fg, fallback.minimap_cursor_fg),
        }
    }
}

// ---------------------------------------------------------------------------
// Built-in theme lookup (without custom theme check to avoid recursion)
// ---------------------------------------------------------------------------

/// Look up a built-in theme by name, without checking custom themes.
/// Used as the fallback for the `base` field in theme files.
#[must_use]
fn builtin_theme_by_name(name: &str) -> Theme {
    match name {
        "Solarized Light" => Theme::light(),
        "Dracula" => Theme::dracula(),
        "Gruvbox Dark" => Theme::gruvbox_dark(),
        "Gruvbox Light" => Theme::gruvbox_light(),
        "Monokai" => Theme::monokai(),
        "Nord" => Theme::nord(),
        "Catppuccin Mocha" => Theme::catppuccin_mocha(),
        "Catppuccin Latte" => Theme::catppuccin_latte(),
        _ => Theme::dark(),
    }
}

// ---------------------------------------------------------------------------
// Config directory helpers
// ---------------------------------------------------------------------------

/// Returns the path to the user's config directory for theme files.
#[must_use]
fn user_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "regenerator2000").map(|d| d.config_dir().to_path_buf())
}

/// Load all custom themes from the user's config directory.
///
/// Scans for files matching `theme-*.toml` and parses them.
fn load_custom_themes_from_dir(dir: &Path) -> Vec<Theme> {
    let mut themes = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return themes;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !filename.starts_with("theme-") || !filename.ends_with(".toml") {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<ThemeFile>(&content) {
                Ok(tf) => {
                    log::info!("Loaded custom theme: {} from {path:?}", tf.name);
                    themes.push(tf.to_theme());
                }
                Err(e) => {
                    log::warn!("Failed to parse theme file {path:?}: {e}");
                }
            },
            Err(e) => {
                log::warn!("Failed to read theme file {path:?}: {e}");
            }
        }
    }

    themes
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Find a custom theme by name.
///
/// Returns `None` if no custom theme with the given name is loaded.
#[must_use]
pub fn find_custom_theme(name: &str) -> Option<Theme> {
    get_custom_themes().iter().find(|t| t.name == name).cloned()
}

/// Return the names of all loaded custom themes.
#[must_use]
pub fn custom_theme_names() -> Vec<String> {
    get_custom_themes().iter().map(|t| t.name.clone()).collect()
}

/// Access the lazily-initialized custom theme cache.
fn get_custom_themes() -> &'static Vec<Theme> {
    CUSTOM_THEMES.get_or_init(|| {
        user_config_dir()
            .map(|dir| load_custom_themes_from_dir(&dir))
            .unwrap_or_default()
    })
}

/// Dump all built-in themes as TOML files into `dest_dir`.
///
/// Creates one file per built-in theme, named `theme-<normalized_name>.toml`.
/// The directory is created if it does not exist.
///
/// # Errors
///
/// Returns an error if the directory cannot be created or if any file write fails.
pub fn dump_theme_files(dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create directory {dest_dir:?}"))?;

    let themes = [
        Theme::dark(),
        Theme::light(),
        Theme::dracula(),
        Theme::gruvbox_dark(),
        Theme::gruvbox_light(),
        Theme::monokai(),
        Theme::nord(),
        Theme::catppuccin_mocha(),
        Theme::catppuccin_latte(),
    ];

    for theme in &themes {
        let tf = ThemeFile::from_theme(theme);
        let toml_str = toml::to_string_pretty(&tf)
            .with_context(|| format!("Failed to serialize theme {:?}", theme.name))?;
        let normalized = theme.name.to_lowercase().replace(' ', "_");
        let filename = format!("theme-{normalized}.toml");
        let dest_path = dest_dir.join(&filename);
        std::fs::write(&dest_path, toml_str)
            .with_context(|| format!("Failed to write {dest_path:?}"))?;
        println!("Wrote {dest_path:?}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#FF0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_hex_color("00FF00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_hex_color("#0000ff"), Some(Color::Rgb(0, 0, 255)));
    }

    #[test]
    fn test_parse_hex_color_invalid() {
        assert_eq!(parse_hex_color("#FFF"), None);
        assert_eq!(parse_hex_color(""), None);
        assert_eq!(parse_hex_color("#GGGGGG"), None);
    }

    #[test]
    fn test_color_to_hex_roundtrip() {
        let color = Color::Rgb(40, 42, 54);
        let hex = color_to_hex(color);
        assert_eq!(hex, "#282A36");
        assert_eq!(parse_hex_color(&hex), Some(color));
    }

    #[test]
    fn test_color_to_hex_named() {
        assert_eq!(color_to_hex(Color::Black), "#000000");
        assert_eq!(color_to_hex(Color::White), "#FFFFFF");
    }

    #[test]
    fn test_theme_file_from_theme_roundtrip() {
        let original = Theme::dracula();
        let tf = ThemeFile::from_theme(&original);
        let restored = tf.to_theme();

        assert_eq!(restored.name, original.name);
        assert_eq!(restored.background, original.background);
        assert_eq!(restored.foreground, original.foreground);
        assert_eq!(restored.mnemonic, original.mnemonic);
        assert_eq!(restored.hex_color_palette, original.hex_color_palette);
    }

    #[test]
    fn test_all_builtin_themes_roundtrip() {
        let themes = [
            Theme::dark(),
            Theme::light(),
            Theme::dracula(),
            Theme::gruvbox_dark(),
            Theme::gruvbox_light(),
            Theme::monokai(),
            Theme::nord(),
            Theme::catppuccin_mocha(),
            Theme::catppuccin_latte(),
        ];

        for theme in &themes {
            let tf = ThemeFile::from_theme(theme);
            let toml_str = toml::to_string_pretty(&tf).unwrap();
            let parsed: ThemeFile = toml::from_str(&toml_str).unwrap();
            let restored = parsed.to_theme();

            assert_eq!(
                restored.name, theme.name,
                "Name mismatch for {}",
                theme.name
            );
            assert_eq!(
                restored.background, theme.background,
                "Background mismatch for {}",
                theme.name
            );
            assert_eq!(
                restored.foreground, theme.foreground,
                "Foreground mismatch for {}",
                theme.name
            );
            assert_eq!(
                restored.border_active, theme.border_active,
                "Border active mismatch for {}",
                theme.name
            );
        }
    }

    #[test]
    fn test_partial_theme_with_base() {
        let toml_str = r##"
name = "Custom Green"
base = "Dracula"
background = "#001100"
foreground = "#33FF33"
"##;
        let tf: ThemeFile = toml::from_str(toml_str).unwrap();
        let theme = tf.to_theme();

        assert_eq!(theme.name, "Custom Green");
        assert_eq!(theme.background, Color::Rgb(0, 17, 0));
        assert_eq!(theme.foreground, Color::Rgb(51, 255, 51));
        // Inherited from Dracula
        let dracula = Theme::dracula();
        assert_eq!(theme.mnemonic, dracula.mnemonic);
        assert_eq!(theme.border_active, dracula.border_active);
    }

    #[test]
    fn test_partial_theme_default_base() {
        let toml_str = r##"
name = "Minimal"
background = "#112233"
"##;
        let tf: ThemeFile = toml::from_str(toml_str).unwrap();
        let theme = tf.to_theme();

        assert_eq!(theme.name, "Minimal");
        assert_eq!(theme.background, Color::Rgb(17, 34, 51));
        // Everything else inherited from Solarized Dark (default)
        let dark = Theme::dark();
        assert_eq!(theme.foreground, dark.foreground);
    }

    #[test]
    fn test_dump_and_reload_themes() {
        let dir = std::env::temp_dir().join(format!(
            "r2000_theme_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));

        dump_theme_files(&dir).unwrap();

        // Verify files were created
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(entries.len(), 9, "Should dump 9 built-in themes");

        // Load them back and verify
        let loaded = load_custom_themes_from_dir(&dir);
        assert_eq!(loaded.len(), 9, "Should load 9 themes back");

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
