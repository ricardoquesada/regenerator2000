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
    pub block_splitter_fg: Color,

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
    pub block_splitter_bg: Color,
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

// Dracula Palette
struct Dracula;
impl Dracula {
    const BACKGROUND: Color = Color::Rgb(40, 42, 54);
    const CURRENT_LINE: Color = Color::Rgb(68, 71, 90);
    const FOREGROUND: Color = Color::Rgb(248, 248, 242);
    const COMMENT: Color = Color::Rgb(98, 114, 164);
    const CYAN: Color = Color::Rgb(139, 233, 253);
    const GREEN: Color = Color::Rgb(80, 250, 123);
    const ORANGE: Color = Color::Rgb(255, 184, 108);
    const PINK: Color = Color::Rgb(255, 121, 198);
    const PURPLE: Color = Color::Rgb(189, 147, 249);
    const RED: Color = Color::Rgb(255, 85, 85);
    const YELLOW: Color = Color::Rgb(241, 250, 140);
}

// Gruvbox Dark Palette
struct Gruvbox;
impl Gruvbox {
    const BG0: Color = Color::Rgb(40, 40, 40);
    const BG1: Color = Color::Rgb(60, 56, 54);
    const FG: Color = Color::Rgb(235, 219, 178);
    const GRAY: Color = Color::Rgb(146, 131, 116);
    const RED: Color = Color::Rgb(251, 73, 52);
    const GREEN: Color = Color::Rgb(184, 187, 38);
    const YELLOW: Color = Color::Rgb(250, 189, 47);
    const BLUE: Color = Color::Rgb(131, 165, 152);
    const PURPLE: Color = Color::Rgb(211, 134, 155);
    const AQUA: Color = Color::Rgb(142, 192, 124);
    const ORANGE: Color = Color::Rgb(254, 128, 25);
}

// Monokai Palette
struct Monokai;
impl Monokai {
    const BACKGROUND: Color = Color::Rgb(39, 40, 34);
    const FOREGROUND: Color = Color::Rgb(248, 248, 242);
    const COMMENT: Color = Color::Rgb(117, 113, 94);
    const RED: Color = Color::Rgb(249, 38, 114);
    const ORANGE: Color = Color::Rgb(253, 151, 31);
    const YELLOW: Color = Color::Rgb(230, 219, 116);
    const GREEN: Color = Color::Rgb(166, 226, 46);
    const BLUE: Color = Color::Rgb(102, 217, 239);
    const PURPLE: Color = Color::Rgb(174, 129, 255);
}

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Solarized Light" => Self::light(),
            "Dracula" => Self::dracula(),
            "Gruvbox Dark" => Self::gruvbox_dark(),
            "Monokai" => Self::monokai(),
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
            block_splitter_fg: Solarized::BASE1,
            block_splitter_bg: Solarized::BASE02,
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
            block_splitter_fg: Solarized::BASE1,
            block_splitter_bg: Solarized::BASE2,
        }
    }

    pub fn all_names() -> Vec<&'static str> {
        vec![
            "Solarized Dark",
            "Solarized Light",
            "Dracula",
            "Gruvbox Dark",
            "Monokai",
        ]
    }

    pub fn dracula() -> Self {
        Self {
            name: "Dracula",
            background: Dracula::BACKGROUND,
            foreground: Dracula::FOREGROUND,
            border_active: Dracula::PURPLE,
            border_inactive: Dracula::COMMENT,
            selection_bg: Dracula::CURRENT_LINE,
            selection_fg: Dracula::FOREGROUND,
            status_bar_bg: Dracula::CURRENT_LINE,
            status_bar_fg: Dracula::CYAN,

            address: Dracula::PURPLE,
            bytes: Dracula::COMMENT,
            mnemonic: Dracula::PINK,
            operand: Dracula::FOREGROUND,
            label: Dracula::CYAN,
            label_def: Dracula::CYAN,
            comment: Dracula::COMMENT,
            arrow: Dracula::COMMENT,
            collapsed_block: Dracula::PURPLE,
            collapsed_block_bg: Dracula::CURRENT_LINE,

            hex_bytes: Dracula::FOREGROUND,
            hex_ascii: Dracula::CYAN,

            dialog_bg: Dracula::BACKGROUND,
            dialog_fg: Dracula::FOREGROUND,
            dialog_border: Dracula::PURPLE,
            menu_bg: Dracula::BACKGROUND,
            menu_fg: Dracula::FOREGROUND,
            menu_selected_bg: Dracula::CURRENT_LINE,
            menu_selected_fg: Dracula::CYAN,
            menu_disabled_fg: Dracula::COMMENT,

            sprite_multicolor_1: Dracula::ORANGE,
            sprite_multicolor_2: Dracula::RED,
            charset_multicolor_1: Dracula::YELLOW,
            charset_multicolor_2: Dracula::PURPLE,

            highlight_fg: Dracula::YELLOW,
            highlight_bg: Dracula::CURRENT_LINE,
            error_fg: Dracula::RED,

            block_code_fg: Dracula::PINK,
            block_code_bg: Dracula::BACKGROUND,
            block_data_byte_fg: Dracula::CYAN,
            block_data_byte_bg: Dracula::BACKGROUND,
            block_data_word_fg: Dracula::PURPLE,
            block_data_word_bg: Dracula::BACKGROUND,
            block_address_fg: Dracula::GREEN,
            block_address_bg: Dracula::BACKGROUND,
            block_text_fg: Dracula::YELLOW,
            block_text_bg: Dracula::BACKGROUND,
            block_screencode_fg: Dracula::ORANGE,
            block_screencode_bg: Dracula::BACKGROUND,
            block_lohi_fg: Dracula::RED,
            block_lohi_bg: Dracula::BACKGROUND,
            block_hilo_fg: Dracula::RED,
            block_hilo_bg: Dracula::BACKGROUND,
            block_external_file_fg: Dracula::COMMENT,
            block_external_file_bg: Dracula::BACKGROUND,
            block_undefined_fg: Dracula::COMMENT,
            block_undefined_bg: Dracula::BACKGROUND,
            block_splitter_fg: Dracula::COMMENT,
            block_splitter_bg: Dracula::BACKGROUND,
        }
    }

    pub fn gruvbox_dark() -> Self {
        Self {
            name: "Gruvbox Dark",
            background: Gruvbox::BG0,
            foreground: Gruvbox::FG,
            border_active: Gruvbox::ORANGE,
            border_inactive: Gruvbox::BG1,
            selection_bg: Gruvbox::BG1,
            selection_fg: Gruvbox::FG,
            status_bar_bg: Gruvbox::BG1,
            status_bar_fg: Gruvbox::FG,

            address: Gruvbox::YELLOW,
            bytes: Gruvbox::GRAY,
            mnemonic: Gruvbox::RED,
            operand: Gruvbox::FG,
            label: Gruvbox::AQUA,
            label_def: Gruvbox::AQUA,
            comment: Gruvbox::GRAY,
            arrow: Gruvbox::GRAY,
            collapsed_block: Gruvbox::ORANGE,
            collapsed_block_bg: Gruvbox::BG1,

            hex_bytes: Gruvbox::FG,
            hex_ascii: Gruvbox::AQUA,

            dialog_bg: Gruvbox::BG1,
            dialog_fg: Gruvbox::FG,
            dialog_border: Gruvbox::ORANGE,
            menu_bg: Gruvbox::BG0,
            menu_fg: Gruvbox::FG,
            menu_selected_bg: Gruvbox::BG1,
            menu_selected_fg: Gruvbox::ORANGE,
            menu_disabled_fg: Gruvbox::GRAY,

            sprite_multicolor_1: Gruvbox::RED,
            sprite_multicolor_2: Gruvbox::PURPLE,
            charset_multicolor_1: Gruvbox::YELLOW,
            charset_multicolor_2: Gruvbox::GREEN,

            highlight_fg: Gruvbox::ORANGE,
            highlight_bg: Gruvbox::BG1,
            error_fg: Gruvbox::RED,

            block_code_fg: Gruvbox::RED,
            block_code_bg: Gruvbox::BG0,
            block_data_byte_fg: Gruvbox::PURPLE,
            block_data_byte_bg: Gruvbox::BG0,
            block_data_word_fg: Gruvbox::BLUE,
            block_data_word_bg: Gruvbox::BG0,
            block_address_fg: Gruvbox::YELLOW,
            block_address_bg: Gruvbox::BG0,
            block_text_fg: Gruvbox::GREEN,
            block_text_bg: Gruvbox::BG0,
            block_screencode_fg: Gruvbox::AQUA,
            block_screencode_bg: Gruvbox::BG0,
            block_lohi_fg: Gruvbox::ORANGE,
            block_lohi_bg: Gruvbox::BG0,
            block_hilo_fg: Gruvbox::ORANGE,
            block_hilo_bg: Gruvbox::BG0,
            block_external_file_fg: Gruvbox::GRAY,
            block_external_file_bg: Gruvbox::BG0,
            block_undefined_fg: Gruvbox::GRAY,
            block_undefined_bg: Gruvbox::BG0,
            block_splitter_fg: Gruvbox::GRAY,
            block_splitter_bg: Gruvbox::BG0,
        }
    }

    pub fn monokai() -> Self {
        Self {
            name: "Monokai",
            background: Monokai::BACKGROUND,
            foreground: Monokai::FOREGROUND,
            border_active: Monokai::YELLOW,
            border_inactive: Monokai::COMMENT,
            selection_bg: Monokai::COMMENT,
            selection_fg: Monokai::FOREGROUND,
            status_bar_bg: Monokai::BACKGROUND,
            status_bar_fg: Monokai::FOREGROUND,

            address: Monokai::PURPLE,
            bytes: Monokai::COMMENT,
            mnemonic: Monokai::RED,
            operand: Monokai::FOREGROUND,
            label: Monokai::GREEN,
            label_def: Monokai::GREEN,
            comment: Monokai::COMMENT,
            arrow: Monokai::COMMENT,
            collapsed_block: Monokai::YELLOW,
            collapsed_block_bg: Monokai::BACKGROUND, // Slightly different if needed, but BG is okay

            hex_bytes: Monokai::FOREGROUND,
            hex_ascii: Monokai::YELLOW,

            dialog_bg: Monokai::BACKGROUND,
            dialog_fg: Monokai::FOREGROUND,
            dialog_border: Monokai::YELLOW,
            menu_bg: Monokai::BACKGROUND,
            menu_fg: Monokai::FOREGROUND,
            menu_selected_bg: Monokai::COMMENT,
            menu_selected_fg: Monokai::YELLOW,
            menu_disabled_fg: Monokai::COMMENT,

            sprite_multicolor_1: Monokai::ORANGE,
            sprite_multicolor_2: Monokai::RED,
            charset_multicolor_1: Monokai::YELLOW,
            charset_multicolor_2: Monokai::GREEN,

            highlight_fg: Monokai::ORANGE,
            highlight_bg: Monokai::COMMENT,
            error_fg: Monokai::RED,

            block_code_fg: Monokai::RED,
            block_code_bg: Monokai::BACKGROUND,
            block_data_byte_fg: Monokai::PURPLE,
            block_data_byte_bg: Monokai::BACKGROUND,
            block_data_word_fg: Monokai::BLUE,
            block_data_word_bg: Monokai::BACKGROUND,
            block_address_fg: Monokai::ORANGE,
            block_address_bg: Monokai::BACKGROUND,
            block_text_fg: Monokai::YELLOW,
            block_text_bg: Monokai::BACKGROUND,
            block_screencode_fg: Monokai::GREEN,
            block_screencode_bg: Monokai::BACKGROUND,
            block_lohi_fg: Monokai::RED,
            block_lohi_bg: Monokai::BACKGROUND,
            block_hilo_fg: Monokai::RED,
            block_hilo_bg: Monokai::BACKGROUND,
            block_external_file_fg: Monokai::COMMENT,
            block_external_file_bg: Monokai::BACKGROUND,
            block_undefined_fg: Monokai::COMMENT,
            block_undefined_bg: Monokai::BACKGROUND,
            block_splitter_fg: Monokai::COMMENT,
            block_splitter_bg: Monokai::BACKGROUND,
        }
    }
}
