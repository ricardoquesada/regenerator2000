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

    // Highlights
    pub highlight_fg: Color, // e.g. bold yellow/green text
    pub highlight_bg: Color,
    pub error_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Light" => Self::light(),
            _ => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            name: "Dark",
            background: Color::Reset,
            foreground: Color::White,
            border_active: Color::Green,
            border_inactive: Color::DarkGray,
            selection_bg: Color::DarkGray,
            selection_fg: Color::White,
            status_bar_bg: Color::Rgb(45, 45, 45),
            status_bar_fg: Color::Rgb(220, 220, 220),

            address: Color::Yellow,
            bytes: Color::DarkGray,
            mnemonic: Color::Cyan, // Or a lighter blue
            operand: Color::White,
            label: Color::Magenta,
            label_def: Color::Magenta,
            comment: Color::Gray, // Or DarkGray
            arrow: Color::DarkGray,

            hex_bytes: Color::White,
            hex_ascii: Color::Green,

            dialog_bg: Color::DarkGray,
            dialog_fg: Color::White,
            dialog_border: Color::White,
            menu_bg: Color::Rgb(45, 128, 128),
            menu_fg: Color::Rgb(240, 240, 240),
            menu_selected_bg: Color::Rgb(32, 64, 64),
            menu_selected_fg: Color::White,
            menu_disabled_fg: Color::Gray,

            highlight_fg: Color::Yellow,
            highlight_bg: Color::DarkGray,
            error_fg: Color::Red,
        }
    }

    pub fn light() -> Self {
        // VS Code Light ish
        Self {
            name: "Light",
            background: Color::White,
            foreground: Color::Black,
            border_active: Color::Blue,
            border_inactive: Color::Gray,
            selection_bg: Color::Rgb(220, 220, 220), // Light Gray
            selection_fg: Color::Black,
            status_bar_bg: Color::Rgb(0, 122, 204), // VS Code Blue
            status_bar_fg: Color::White,

            address: Color::Rgb(100, 100, 100), // Dark Gray
            bytes: Color::Rgb(100, 100, 100),
            mnemonic: Color::Rgb(0, 0, 255), // Blue
            operand: Color::Black,
            label: Color::Rgb(128, 0, 128), // Purple
            label_def: Color::Rgb(128, 0, 128),
            comment: Color::Rgb(0, 128, 0), // Green for comments
            arrow: Color::DarkGray,

            hex_bytes: Color::Black,
            hex_ascii: Color::Rgb(0, 0, 255), // Blue for ASCII

            dialog_bg: Color::White,
            dialog_fg: Color::Black,
            dialog_border: Color::Black,
            menu_bg: Color::Rgb(240, 240, 240),
            menu_fg: Color::Black,
            menu_selected_bg: Color::Rgb(0, 122, 204),
            menu_selected_fg: Color::White,
            menu_disabled_fg: Color::Gray,

            highlight_fg: Color::Rgb(255, 140, 0), // Dark Orange
            highlight_bg: Color::Rgb(240, 240, 240),
            error_fg: Color::Red,
        }
    }

    pub fn all_names() -> Vec<&'static str> {
        vec!["Dark", "Light"]
    }
}
