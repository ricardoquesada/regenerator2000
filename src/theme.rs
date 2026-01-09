use ratatui::style::Color;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub background: Color,
    pub foreground: Color,
    pub border_active: Color,
    pub border_inactive: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
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
    pub block_data_byte_fg: Color,
    pub block_data_word_fg: Color,
    pub block_address_fg: Color,
    pub block_text_fg: Color,
    pub block_screencode_fg: Color,
    pub block_lohi_fg: Color,
    pub block_hilo_fg: Color,
    pub block_external_file_fg: Color,
    pub block_undefined_fg: Color,

    // Block Types (Backgrounds)
    pub block_code_bg: Color,
    pub block_data_byte_bg: Color,
    pub block_data_word_bg: Color,
    pub block_address_bg: Color,
    pub block_text_bg: Color,
    pub block_screencode_bg: Color,
    pub block_lohi_bg: Color,
    pub block_hilo_bg: Color,
    pub block_external_file_bg: Color,
    pub block_undefined_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

// Solarized Palette
struct Solarized;
impl Solarized {
    const BASE03: Color = Color::Rgb(0, 43, 54);
    const BASE02: Color = Color::Rgb(7, 54, 66);
    const BASE01: Color = Color::Rgb(88, 110, 117);
    const BASE00: Color = Color::Rgb(101, 123, 131);
    const BASE0: Color = Color::Rgb(131, 148, 150);
    const BASE1: Color = Color::Rgb(147, 161, 161);
    const BASE2: Color = Color::Rgb(238, 232, 213);
    const BASE3: Color = Color::Rgb(253, 246, 227);
    const YELLOW: Color = Color::Rgb(181, 137, 0);
    const ORANGE: Color = Color::Rgb(203, 75, 22);
    const RED: Color = Color::Rgb(220, 50, 47);
    const MAGENTA: Color = Color::Rgb(211, 54, 130);
    const VIOLET: Color = Color::Rgb(108, 113, 196);
    const BLUE: Color = Color::Rgb(38, 139, 210);
    const CYAN: Color = Color::Rgb(42, 161, 152);
    const GREEN: Color = Color::Rgb(133, 153, 0);
}

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Solarized Light" => Self::light(),
            _ => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "Solarized Dark",
            background: Solarized::BASE03,
            foreground: Solarized::BASE0,
            border_active: Solarized::BLUE,
            border_inactive: Solarized::BASE01,
            selection_bg: Solarized::BASE02,
            selection_fg: Solarized::BASE1,
            status_bar_bg: Solarized::BASE02,
            status_bar_fg: Solarized::BASE1,

            address: Solarized::YELLOW,
            bytes: Solarized::BASE01,
            mnemonic: Solarized::BLUE,
            operand: Solarized::BASE1,
            label: Solarized::MAGENTA,
            label_def: Solarized::MAGENTA,
            comment: Solarized::BASE01,
            arrow: Solarized::BASE01,
            collapsed_block: Solarized::BLUE,
            collapsed_block_bg: Solarized::BASE02,

            hex_bytes: Solarized::BASE1,
            hex_ascii: Solarized::CYAN,

            dialog_bg: Solarized::BASE02,
            dialog_fg: Solarized::BASE0,
            dialog_border: Solarized::BASE1,
            menu_bg: Solarized::BASE02,
            menu_fg: Solarized::BASE0,
            menu_selected_bg: Solarized::BASE01,
            menu_selected_fg: Solarized::BASE3,
            menu_disabled_fg: Solarized::BASE01,

            sprite_multicolor_1: Solarized::RED,
            sprite_multicolor_2: Solarized::BLUE,
            charset_multicolor_1: Solarized::ORANGE,
            charset_multicolor_2: Solarized::GREEN,

            highlight_fg: Solarized::ORANGE,
            highlight_bg: Solarized::BASE02,
            error_fg: Solarized::RED,

            // Blocks - Dark (Bg is slightly lighter than proper background)
            block_code_fg: Solarized::BLUE,
            block_code_bg: Solarized::BASE02,
            block_data_byte_fg: Solarized::CYAN,
            block_data_byte_bg: Solarized::BASE02,
            block_data_word_fg: Solarized::VIOLET,
            block_data_word_bg: Solarized::BASE02,
            block_address_fg: Solarized::YELLOW,
            block_address_bg: Solarized::BASE02,
            block_text_fg: Solarized::GREEN,
            block_text_bg: Solarized::BASE02,
            block_screencode_fg: Solarized::ORANGE,
            block_screencode_bg: Solarized::BASE02,
            block_lohi_fg: Solarized::RED,
            block_lohi_bg: Solarized::BASE02,
            block_hilo_fg: Solarized::MAGENTA,
            block_hilo_bg: Solarized::BASE02,
            block_external_file_fg: Solarized::BASE1,
            block_external_file_bg: Solarized::BASE02,
            block_undefined_fg: Solarized::BASE01,
            block_undefined_bg: Solarized::BASE02,
        }
    }

    pub fn light() -> Self {
        Self {
            name: "Solarized Light",
            background: Solarized::BASE3,
            foreground: Solarized::BASE00,
            border_active: Solarized::BLUE,
            border_inactive: Solarized::BASE1,
            selection_bg: Solarized::BASE2,
            selection_fg: Solarized::BASE01,
            status_bar_bg: Solarized::BASE2,
            status_bar_fg: Solarized::BASE01,

            address: Solarized::BASE01,
            bytes: Solarized::BASE1,
            mnemonic: Solarized::BLUE,
            operand: Solarized::BASE00,
            label: Solarized::MAGENTA,
            label_def: Solarized::MAGENTA,
            comment: Solarized::BASE1,
            arrow: Solarized::BASE1,
            collapsed_block: Solarized::BLUE,
            collapsed_block_bg: Solarized::BASE2,

            hex_bytes: Solarized::BASE00,
            hex_ascii: Solarized::CYAN,

            dialog_bg: Solarized::BASE2,
            dialog_fg: Solarized::BASE00,
            dialog_border: Solarized::BASE01,
            menu_bg: Solarized::BASE2,
            menu_fg: Solarized::BASE00,
            menu_selected_bg: Solarized::BASE1,
            menu_selected_fg: Solarized::BASE3,
            menu_disabled_fg: Solarized::BASE1,

            sprite_multicolor_1: Solarized::RED,
            sprite_multicolor_2: Solarized::BLUE,
            charset_multicolor_1: Solarized::ORANGE,
            charset_multicolor_2: Solarized::GREEN,

            highlight_fg: Solarized::ORANGE,
            highlight_bg: Solarized::BASE2,
            error_fg: Solarized::RED,

            // Blocks - Light (Bg is slightly darker than proper background)
            block_code_fg: Solarized::BLUE,
            block_code_bg: Solarized::BASE2,
            block_data_byte_fg: Solarized::CYAN,
            block_data_byte_bg: Solarized::BASE2,
            block_data_word_fg: Solarized::VIOLET,
            block_data_word_bg: Solarized::BASE2,
            block_address_fg: Solarized::YELLOW,
            block_address_bg: Solarized::BASE2,
            block_text_fg: Solarized::GREEN,
            block_text_bg: Solarized::BASE2,
            block_screencode_fg: Solarized::ORANGE,
            block_screencode_bg: Solarized::BASE2,
            block_lohi_fg: Solarized::RED,
            block_lohi_bg: Solarized::BASE2,
            block_hilo_fg: Solarized::MAGENTA,
            block_hilo_bg: Solarized::BASE2,
            block_external_file_fg: Solarized::BASE01,
            block_external_file_bg: Solarized::BASE2,
            block_undefined_fg: Solarized::BASE1,
            block_undefined_bg: Solarized::BASE2,
        }
    }

    pub fn all_names() -> Vec<&'static str> {
        vec!["Solarized Dark", "Solarized Light"]
    }
}
