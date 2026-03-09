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
    pub block_petscii_text_fg: Color,
    pub block_screencode_text_fg: Color,
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
    pub block_petscii_text_bg: Color,
    pub block_screencode_text_bg: Color,
    pub block_lohi_bg: Color,
    pub block_hilo_bg: Color,
    pub block_external_file_bg: Color,
    pub block_undefined_bg: Color,
    pub block_splitter_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dracula()
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

// Gruvbox Palette
struct Gruvbox;
impl Gruvbox {
    // Dark base colors
    const BG0: Color = Color::Rgb(40, 40, 40);
    const BG1: Color = Color::Rgb(60, 56, 54);
    const FG: Color = Color::Rgb(235, 219, 178);
    const GRAY: Color = Color::Rgb(146, 131, 116);

    // Light base colors
    const BG0_LIGHT: Color = Color::Rgb(251, 241, 199);
    const BG1_LIGHT: Color = Color::Rgb(235, 219, 178);
    const FG_LIGHT: Color = Color::Rgb(60, 56, 54);

    // Shared accent colors (same for dark and light)
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

// Nord Palette
#[allow(dead_code)]
struct Nord;
#[allow(dead_code)]
impl Nord {
    // Polar Night (dark backgrounds)
    const NORD0: Color = Color::Rgb(46, 52, 64);
    const NORD1: Color = Color::Rgb(59, 66, 82);
    const NORD2: Color = Color::Rgb(67, 76, 94);
    const NORD3: Color = Color::Rgb(76, 86, 106);

    // Snow Storm (light foregrounds)
    const NORD4: Color = Color::Rgb(216, 222, 233);
    const NORD5: Color = Color::Rgb(229, 233, 240);
    const NORD6: Color = Color::Rgb(236, 239, 244);

    // Frost (blue accents)
    const NORD7: Color = Color::Rgb(143, 188, 187);
    const NORD8: Color = Color::Rgb(136, 192, 208);
    const NORD9: Color = Color::Rgb(129, 161, 193);
    const NORD10: Color = Color::Rgb(94, 129, 172);

    // Aurora (accent colors)
    const NORD11: Color = Color::Rgb(191, 97, 106); // Red
    const NORD12: Color = Color::Rgb(208, 135, 112); // Orange
    const NORD13: Color = Color::Rgb(235, 203, 139); // Yellow
    const NORD14: Color = Color::Rgb(163, 190, 140); // Green
    const NORD15: Color = Color::Rgb(180, 142, 173); // Purple
}

// Catppuccin Mocha Palette (darkest flavor)
#[allow(dead_code)]
struct CatppuccinMocha;
#[allow(dead_code)]
impl CatppuccinMocha {
    const BASE: Color = Color::Rgb(30, 30, 46);
    const MANTLE: Color = Color::Rgb(24, 24, 37);
    const SURFACE0: Color = Color::Rgb(49, 50, 68);
    const SURFACE1: Color = Color::Rgb(69, 71, 90);
    const OVERLAY0: Color = Color::Rgb(108, 112, 134);
    const TEXT: Color = Color::Rgb(205, 214, 244);
    const SUBTEXT0: Color = Color::Rgb(166, 173, 200);
    const ROSEWATER: Color = Color::Rgb(245, 224, 220);
    const FLAMINGO: Color = Color::Rgb(242, 205, 205);
    const PINK: Color = Color::Rgb(245, 194, 231);
    const MAUVE: Color = Color::Rgb(203, 166, 247);
    const RED: Color = Color::Rgb(243, 139, 168);
    const MAROON: Color = Color::Rgb(235, 160, 172);
    const PEACH: Color = Color::Rgb(250, 179, 135);
    const YELLOW: Color = Color::Rgb(249, 226, 175);
    const GREEN: Color = Color::Rgb(166, 227, 161);
    const TEAL: Color = Color::Rgb(148, 226, 213);
    const SKY: Color = Color::Rgb(137, 220, 235);
    const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
    const BLUE: Color = Color::Rgb(137, 180, 250);
    const LAVENDER: Color = Color::Rgb(180, 190, 254);
}

// Catppuccin Latte Palette (light flavor)
#[allow(dead_code)]
struct CatppuccinLatte;
#[allow(dead_code)]
impl CatppuccinLatte {
    const BASE: Color = Color::Rgb(239, 241, 245);
    const MANTLE: Color = Color::Rgb(230, 233, 239);
    const CRUST: Color = Color::Rgb(220, 224, 232);
    const SURFACE0: Color = Color::Rgb(204, 208, 218);
    const SURFACE1: Color = Color::Rgb(188, 192, 204);
    const OVERLAY0: Color = Color::Rgb(156, 160, 176);
    const TEXT: Color = Color::Rgb(76, 79, 105);
    const SUBTEXT0: Color = Color::Rgb(108, 111, 133);
    const PINK: Color = Color::Rgb(234, 118, 203);
    const MAUVE: Color = Color::Rgb(136, 57, 239);
    const RED: Color = Color::Rgb(210, 15, 57);
    const MAROON: Color = Color::Rgb(230, 69, 83);
    const PEACH: Color = Color::Rgb(254, 100, 11);
    const YELLOW: Color = Color::Rgb(223, 142, 29);
    const GREEN: Color = Color::Rgb(64, 160, 43);
    const TEAL: Color = Color::Rgb(23, 146, 153);
    const SKY: Color = Color::Rgb(4, 165, 229);
    const SAPPHIRE: Color = Color::Rgb(32, 159, 181);
    const BLUE: Color = Color::Rgb(30, 102, 245);
    const LAVENDER: Color = Color::Rgb(114, 135, 253);
}

impl Theme {
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "Solarized Light" => Self::light(),
            "Dracula" => Self::dracula(),
            "Gruvbox Dark" => Self::gruvbox_dark(),
            "Gruvbox Light" => Self::gruvbox_light(),
            "Monokai" => Self::monokai(),
            "Nord" => Self::nord(),
            "Catppuccin Mocha" => Self::catppuccin_mocha(),
            "Catppuccin Latte" => Self::catppuccin_latte(),
            _ => Self::dark(),
        }
    }

    #[must_use]
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
            block_selection_bg: Solarized::BASE02, // Match Hex Dump
            block_selection_fg: Solarized::BASE1,  // Match Hex Dump

            address: Solarized::YELLOW,
            bytes: Solarized::BASE01,
            mnemonic: Solarized::BLUE,
            operand: Solarized::BASE1,
            label: Solarized::MAGENTA,
            label_def: Solarized::MAGENTA,
            comment: Solarized::BASE01,
            arrow: Solarized::BASE01,
            collapsed_block: Solarized::YELLOW,
            collapsed_block_bg: Solarized::BASE03,

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
            block_code_bg: Solarized::BASE03,
            block_data_byte_fg: Solarized::CYAN,
            block_data_byte_bg: Solarized::BASE03,
            block_data_word_fg: Solarized::VIOLET,
            block_data_word_bg: Solarized::BASE03,
            block_address_fg: Solarized::YELLOW,
            block_address_bg: Solarized::BASE03,
            block_petscii_text_fg: Solarized::GREEN,
            block_petscii_text_bg: Solarized::BASE03,
            block_screencode_text_fg: Solarized::ORANGE,
            block_screencode_text_bg: Solarized::BASE03,
            block_lohi_fg: Solarized::RED,
            block_lohi_bg: Solarized::BASE03,
            block_hilo_fg: Solarized::MAGENTA,
            block_hilo_bg: Solarized::BASE03,
            block_external_file_fg: Solarized::BASE1,
            block_external_file_bg: Solarized::BASE03,
            block_undefined_fg: Solarized::BASE01,
            block_undefined_bg: Solarized::BASE03,
            block_splitter_fg: Solarized::BASE1,
            block_splitter_bg: Solarized::BASE03,
        }
    }

    #[must_use]
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
            block_selection_bg: Solarized::BASE1, // Higher contrast
            block_selection_fg: Solarized::BASE3, // Higher contrast

            address: Solarized::BASE01,
            bytes: Solarized::BASE1,
            mnemonic: Solarized::BLUE,
            operand: Solarized::BASE00,
            label: Solarized::MAGENTA,
            label_def: Solarized::MAGENTA,
            comment: Solarized::BASE1,
            arrow: Solarized::BASE1,
            collapsed_block: Solarized::MAGENTA,
            collapsed_block_bg: Solarized::BASE3,

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
            block_petscii_text_fg: Solarized::GREEN,
            block_petscii_text_bg: Solarized::BASE2,
            block_screencode_text_fg: Solarized::ORANGE,
            block_screencode_text_bg: Solarized::BASE2,
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

    #[must_use]
    pub fn all_names() -> Vec<&'static str> {
        vec![
            "Solarized Dark",
            "Solarized Light",
            "Dracula",
            "Gruvbox Dark",
            "Gruvbox Light",
            "Monokai",
            "Nord",
            "Catppuccin Mocha",
            "Catppuccin Latte",
        ]
    }

    #[must_use]
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
            block_selection_bg: Dracula::CURRENT_LINE, // Default to selection_bg
            block_selection_fg: Dracula::FOREGROUND,   // Default to selection_fg

            address: Dracula::PURPLE,
            bytes: Dracula::COMMENT,
            mnemonic: Dracula::PINK,
            operand: Dracula::FOREGROUND,
            label: Dracula::CYAN,
            label_def: Dracula::CYAN,
            comment: Dracula::COMMENT,
            arrow: Dracula::COMMENT,
            collapsed_block: Dracula::YELLOW,
            collapsed_block_bg: Dracula::BACKGROUND,

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
            block_petscii_text_fg: Dracula::YELLOW,
            block_petscii_text_bg: Dracula::BACKGROUND,
            block_screencode_text_fg: Dracula::ORANGE,
            block_screencode_text_bg: Dracula::BACKGROUND,
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

    #[must_use]
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
            block_selection_bg: Gruvbox::BG1, // Default to selection_bg
            block_selection_fg: Gruvbox::FG,  // Default to selection_fg

            address: Gruvbox::YELLOW,
            bytes: Gruvbox::GRAY,
            mnemonic: Gruvbox::RED,
            operand: Gruvbox::FG,
            label: Gruvbox::AQUA,
            label_def: Gruvbox::AQUA,
            comment: Gruvbox::GRAY,
            arrow: Gruvbox::GRAY,
            collapsed_block: Gruvbox::YELLOW,
            collapsed_block_bg: Gruvbox::BG0,

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
            block_petscii_text_fg: Gruvbox::GREEN,
            block_petscii_text_bg: Gruvbox::BG0,
            block_screencode_text_fg: Gruvbox::AQUA,
            block_screencode_text_bg: Gruvbox::BG0,
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

    #[must_use]
    pub fn gruvbox_light() -> Self {
        Self {
            name: "Gruvbox Light",
            background: Gruvbox::BG0_LIGHT,
            foreground: Gruvbox::FG_LIGHT,
            border_active: Gruvbox::ORANGE,
            border_inactive: Gruvbox::BG1_LIGHT,
            selection_bg: Gruvbox::BG1_LIGHT,
            selection_fg: Gruvbox::FG_LIGHT,
            status_bar_bg: Gruvbox::BG1_LIGHT,
            status_bar_fg: Gruvbox::FG_LIGHT,
            block_selection_bg: Gruvbox::GRAY, // Higher contrast on light bg
            block_selection_fg: Gruvbox::BG0_LIGHT, // Higher contrast on light bg

            address: Gruvbox::YELLOW,
            bytes: Gruvbox::GRAY,
            mnemonic: Gruvbox::RED,
            operand: Gruvbox::FG_LIGHT,
            label: Gruvbox::AQUA,
            label_def: Gruvbox::AQUA,
            comment: Gruvbox::GRAY,
            arrow: Gruvbox::GRAY,
            collapsed_block: Gruvbox::ORANGE,
            collapsed_block_bg: Gruvbox::BG0_LIGHT,

            hex_bytes: Gruvbox::FG_LIGHT,
            hex_ascii: Gruvbox::AQUA,

            dialog_bg: Gruvbox::BG1_LIGHT,
            dialog_fg: Gruvbox::FG_LIGHT,
            dialog_border: Gruvbox::ORANGE,
            menu_bg: Gruvbox::BG0_LIGHT,
            menu_fg: Gruvbox::FG_LIGHT,
            menu_selected_bg: Gruvbox::BG1_LIGHT,
            menu_selected_fg: Gruvbox::ORANGE,
            menu_disabled_fg: Gruvbox::GRAY,

            sprite_multicolor_1: Gruvbox::RED,
            sprite_multicolor_2: Gruvbox::PURPLE,
            charset_multicolor_1: Gruvbox::YELLOW,
            charset_multicolor_2: Gruvbox::GREEN,

            highlight_fg: Gruvbox::ORANGE,
            highlight_bg: Gruvbox::BG1_LIGHT,
            error_fg: Gruvbox::RED,

            // Blocks - Light (Bg uses slightly darker base)
            block_code_fg: Gruvbox::RED,
            block_code_bg: Gruvbox::BG1_LIGHT,
            block_data_byte_fg: Gruvbox::PURPLE,
            block_data_byte_bg: Gruvbox::BG1_LIGHT,
            block_data_word_fg: Gruvbox::BLUE,
            block_data_word_bg: Gruvbox::BG1_LIGHT,
            block_address_fg: Gruvbox::YELLOW,
            block_address_bg: Gruvbox::BG1_LIGHT,
            block_petscii_text_fg: Gruvbox::GREEN,
            block_petscii_text_bg: Gruvbox::BG1_LIGHT,
            block_screencode_text_fg: Gruvbox::AQUA,
            block_screencode_text_bg: Gruvbox::BG1_LIGHT,
            block_lohi_fg: Gruvbox::ORANGE,
            block_lohi_bg: Gruvbox::BG1_LIGHT,
            block_hilo_fg: Gruvbox::ORANGE,
            block_hilo_bg: Gruvbox::BG1_LIGHT,
            block_external_file_fg: Gruvbox::GRAY,
            block_external_file_bg: Gruvbox::BG1_LIGHT,
            block_undefined_fg: Gruvbox::GRAY,
            block_undefined_bg: Gruvbox::BG1_LIGHT,
            block_splitter_fg: Gruvbox::GRAY,
            block_splitter_bg: Gruvbox::BG1_LIGHT,
        }
    }

    #[must_use]
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
            block_selection_bg: Monokai::COMMENT, // Default to selection_bg
            block_selection_fg: Monokai::FOREGROUND, // Default to selection_fg

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
            block_petscii_text_fg: Monokai::YELLOW,
            block_petscii_text_bg: Monokai::BACKGROUND,
            block_screencode_text_fg: Monokai::GREEN,
            block_screencode_text_bg: Monokai::BACKGROUND,
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

    #[must_use]
    pub fn nord() -> Self {
        Self {
            name: "Nord",
            background: Nord::NORD0,
            foreground: Nord::NORD4,
            border_active: Nord::NORD8,
            border_inactive: Nord::NORD3,
            selection_bg: Nord::NORD2,
            selection_fg: Nord::NORD6,
            status_bar_bg: Nord::NORD1,
            status_bar_fg: Nord::NORD8,
            block_selection_bg: Nord::NORD2,
            block_selection_fg: Nord::NORD6,

            address: Nord::NORD13,
            bytes: Nord::NORD3,
            mnemonic: Nord::NORD9,
            operand: Nord::NORD4,
            label: Nord::NORD7,
            label_def: Nord::NORD7,
            comment: Nord::NORD3,
            arrow: Nord::NORD3,
            collapsed_block: Nord::NORD13,
            collapsed_block_bg: Nord::NORD0,

            hex_bytes: Nord::NORD4,
            hex_ascii: Nord::NORD8,

            dialog_bg: Nord::NORD1,
            dialog_fg: Nord::NORD4,
            dialog_border: Nord::NORD8,
            menu_bg: Nord::NORD0,
            menu_fg: Nord::NORD4,
            menu_selected_bg: Nord::NORD2,
            menu_selected_fg: Nord::NORD8,
            menu_disabled_fg: Nord::NORD3,

            sprite_multicolor_1: Nord::NORD11,
            sprite_multicolor_2: Nord::NORD10,
            charset_multicolor_1: Nord::NORD12,
            charset_multicolor_2: Nord::NORD14,

            highlight_fg: Nord::NORD12,
            highlight_bg: Nord::NORD1,
            error_fg: Nord::NORD11,

            block_code_fg: Nord::NORD9,
            block_code_bg: Nord::NORD0,
            block_data_byte_fg: Nord::NORD8,
            block_data_byte_bg: Nord::NORD0,
            block_data_word_fg: Nord::NORD15,
            block_data_word_bg: Nord::NORD0,
            block_address_fg: Nord::NORD13,
            block_address_bg: Nord::NORD0,
            block_petscii_text_fg: Nord::NORD14,
            block_petscii_text_bg: Nord::NORD0,
            block_screencode_text_fg: Nord::NORD12,
            block_screencode_text_bg: Nord::NORD0,
            block_lohi_fg: Nord::NORD11,
            block_lohi_bg: Nord::NORD0,
            block_hilo_fg: Nord::NORD11,
            block_hilo_bg: Nord::NORD0,
            block_external_file_fg: Nord::NORD3,
            block_external_file_bg: Nord::NORD0,
            block_undefined_fg: Nord::NORD3,
            block_undefined_bg: Nord::NORD0,
            block_splitter_fg: Nord::NORD3,
            block_splitter_bg: Nord::NORD0,
        }
    }

    #[must_use]
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha",
            background: CatppuccinMocha::BASE,
            foreground: CatppuccinMocha::TEXT,
            border_active: CatppuccinMocha::MAUVE,
            border_inactive: CatppuccinMocha::SURFACE1,
            selection_bg: CatppuccinMocha::SURFACE1,
            selection_fg: CatppuccinMocha::TEXT,
            status_bar_bg: CatppuccinMocha::MANTLE,
            status_bar_fg: CatppuccinMocha::SUBTEXT0,
            block_selection_bg: CatppuccinMocha::SURFACE1,
            block_selection_fg: CatppuccinMocha::TEXT,

            address: CatppuccinMocha::YELLOW,
            bytes: CatppuccinMocha::OVERLAY0,
            mnemonic: CatppuccinMocha::BLUE,
            operand: CatppuccinMocha::TEXT,
            label: CatppuccinMocha::TEAL,
            label_def: CatppuccinMocha::TEAL,
            comment: CatppuccinMocha::OVERLAY0,
            arrow: CatppuccinMocha::OVERLAY0,
            collapsed_block: CatppuccinMocha::YELLOW,
            collapsed_block_bg: CatppuccinMocha::BASE,

            hex_bytes: CatppuccinMocha::TEXT,
            hex_ascii: CatppuccinMocha::SKY,

            dialog_bg: CatppuccinMocha::SURFACE0,
            dialog_fg: CatppuccinMocha::TEXT,
            dialog_border: CatppuccinMocha::MAUVE,
            menu_bg: CatppuccinMocha::BASE,
            menu_fg: CatppuccinMocha::TEXT,
            menu_selected_bg: CatppuccinMocha::SURFACE1,
            menu_selected_fg: CatppuccinMocha::ROSEWATER,
            menu_disabled_fg: CatppuccinMocha::OVERLAY0,

            sprite_multicolor_1: CatppuccinMocha::PEACH,
            sprite_multicolor_2: CatppuccinMocha::PINK,
            charset_multicolor_1: CatppuccinMocha::YELLOW,
            charset_multicolor_2: CatppuccinMocha::GREEN,

            highlight_fg: CatppuccinMocha::PEACH,
            highlight_bg: CatppuccinMocha::SURFACE0,
            error_fg: CatppuccinMocha::RED,

            block_code_fg: CatppuccinMocha::BLUE,
            block_code_bg: CatppuccinMocha::BASE,
            block_data_byte_fg: CatppuccinMocha::SKY,
            block_data_byte_bg: CatppuccinMocha::BASE,
            block_data_word_fg: CatppuccinMocha::MAUVE,
            block_data_word_bg: CatppuccinMocha::BASE,
            block_address_fg: CatppuccinMocha::YELLOW,
            block_address_bg: CatppuccinMocha::BASE,
            block_petscii_text_fg: CatppuccinMocha::GREEN,
            block_petscii_text_bg: CatppuccinMocha::BASE,
            block_screencode_text_fg: CatppuccinMocha::PEACH,
            block_screencode_text_bg: CatppuccinMocha::BASE,
            block_lohi_fg: CatppuccinMocha::RED,
            block_lohi_bg: CatppuccinMocha::BASE,
            block_hilo_fg: CatppuccinMocha::MAROON,
            block_hilo_bg: CatppuccinMocha::BASE,
            block_external_file_fg: CatppuccinMocha::OVERLAY0,
            block_external_file_bg: CatppuccinMocha::BASE,
            block_undefined_fg: CatppuccinMocha::OVERLAY0,
            block_undefined_bg: CatppuccinMocha::BASE,
            block_splitter_fg: CatppuccinMocha::OVERLAY0,
            block_splitter_bg: CatppuccinMocha::BASE,
        }
    }

    #[must_use]
    pub fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte",
            background: CatppuccinLatte::BASE,
            foreground: CatppuccinLatte::TEXT,
            border_active: CatppuccinLatte::MAUVE,
            border_inactive: CatppuccinLatte::SURFACE1,
            selection_bg: CatppuccinLatte::SURFACE0,
            selection_fg: CatppuccinLatte::TEXT,
            status_bar_bg: CatppuccinLatte::MANTLE,
            status_bar_fg: CatppuccinLatte::SUBTEXT0,
            block_selection_bg: CatppuccinLatte::SURFACE1,
            block_selection_fg: CatppuccinLatte::TEXT,

            address: CatppuccinLatte::YELLOW,
            bytes: CatppuccinLatte::OVERLAY0,
            mnemonic: CatppuccinLatte::BLUE,
            operand: CatppuccinLatte::TEXT,
            label: CatppuccinLatte::TEAL,
            label_def: CatppuccinLatte::TEAL,
            comment: CatppuccinLatte::OVERLAY0,
            arrow: CatppuccinLatte::OVERLAY0,
            collapsed_block: CatppuccinLatte::MAUVE,
            collapsed_block_bg: CatppuccinLatte::BASE,

            hex_bytes: CatppuccinLatte::TEXT,
            hex_ascii: CatppuccinLatte::SKY,

            dialog_bg: CatppuccinLatte::MANTLE,
            dialog_fg: CatppuccinLatte::TEXT,
            dialog_border: CatppuccinLatte::MAUVE,
            menu_bg: CatppuccinLatte::BASE,
            menu_fg: CatppuccinLatte::TEXT,
            menu_selected_bg: CatppuccinLatte::SURFACE0,
            menu_selected_fg: CatppuccinLatte::MAUVE,
            menu_disabled_fg: CatppuccinLatte::OVERLAY0,

            sprite_multicolor_1: CatppuccinLatte::RED,
            sprite_multicolor_2: CatppuccinLatte::BLUE,
            charset_multicolor_1: CatppuccinLatte::PEACH,
            charset_multicolor_2: CatppuccinLatte::GREEN,

            highlight_fg: CatppuccinLatte::PEACH,
            highlight_bg: CatppuccinLatte::SURFACE0,
            error_fg: CatppuccinLatte::RED,

            block_code_fg: CatppuccinLatte::BLUE,
            block_code_bg: CatppuccinLatte::CRUST,
            block_data_byte_fg: CatppuccinLatte::SKY,
            block_data_byte_bg: CatppuccinLatte::CRUST,
            block_data_word_fg: CatppuccinLatte::MAUVE,
            block_data_word_bg: CatppuccinLatte::CRUST,
            block_address_fg: CatppuccinLatte::YELLOW,
            block_address_bg: CatppuccinLatte::CRUST,
            block_petscii_text_fg: CatppuccinLatte::GREEN,
            block_petscii_text_bg: CatppuccinLatte::CRUST,
            block_screencode_text_fg: CatppuccinLatte::PEACH,
            block_screencode_text_bg: CatppuccinLatte::CRUST,
            block_lohi_fg: CatppuccinLatte::RED,
            block_lohi_bg: CatppuccinLatte::CRUST,
            block_hilo_fg: CatppuccinLatte::MAROON,
            block_hilo_bg: CatppuccinLatte::CRUST,
            block_external_file_fg: CatppuccinLatte::OVERLAY0,
            block_external_file_bg: CatppuccinLatte::CRUST,
            block_undefined_fg: CatppuccinLatte::OVERLAY0,
            block_undefined_bg: CatppuccinLatte::CRUST,
            block_splitter_fg: CatppuccinLatte::OVERLAY0,
            block_splitter_bg: CatppuccinLatte::CRUST,
        }
    }
}
