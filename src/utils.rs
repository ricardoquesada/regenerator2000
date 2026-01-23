use image::DynamicImage;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui_image::picker::Picker;
use std::fs;
use std::path::{Path, PathBuf};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn list_files(dir: &Path, extensions: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if extensions.iter().any(|e| e.to_lowercase() == ext_lower) {
                    files.push(path);
                }
            }
        }
    }

    // Sort: directories first, then files alpha
    files.sort_by(|a, b| {
        let a_is_dir = a.is_dir();
        let b_is_dir = b.is_dir();
        if a_is_dir && !b_is_dir {
            std::cmp::Ordering::Less
        } else if !a_is_dir && b_is_dir {
            std::cmp::Ordering::Greater
        } else {
            a.file_name().cmp(&b.file_name())
        }
    });

    files
}

pub fn load_logo() -> Option<DynamicImage> {
    let logo_bytes = include_bytes!("../docs/regenerator2000_logo.png");
    if let Ok(img) = image::load_from_memory(logo_bytes) {
        return Some(img);
    }
    None
}
pub fn create_picker() -> Option<Picker> {
    let font_size = (8, 16);
    // Force Kitty protocol for Ghostty if autodetection fails/blurs.
    // ratatui-image 0.9 Picker::new(font_size) might be available.
    #[allow(deprecated)]
    let picker = Picker::from_fontsize(font_size);

    // Attempt to force Kitty for Ghostty
    if std::env::var("TERM_PROGRAM").unwrap_or_default() == "ghostty" {
        // This is a guess at the API since autodetection is failing.
        // We'll see if this compiles.
        // picker.protocol_type = ProtocolType::Kitty;
    }

    Some(picker)
}

pub fn screencode_to_petscii(byte: u8) -> u8 {
    // Basic Screencode to PETSCII mapping
    // This is a simplification, but covers the main displayable range
    match byte {
        0x00..=0x1F => byte + 0x40, // @ABC... -> @ABC... (40-5F)
        0x20..=0x3F => byte,        //  !"#... ->  !"#... (20-3F)
        0x40..=0x5F => byte + 0x20, // ─♠│... -> ─♠│... (60-7F)
        0x60..=0x7F => byte + 0x40, //  ▌▄▔... ->  ▌▄▔... (A0-BF)
        // Reverse characters (bit 7 set)
        0x80..=0x9F => byte - 0x40 + 0x80, // Rev @ABC... -> Rev @ABC... (C0-DF ?) - actually PETSCII reverse is typically +$80
        // But let's check our PETSCII map.
        // Our PETSCII map handles 00-FF.
        // Screencode $80 (Rev @) -> PETSCII $C0?
        // Let's assume standard behavior:
        // Screencode = PETSCII & 0x7F? No.
        //
        // Let's stick to the upper/lower case rules logic usually:
        //
        // Bank 1 (Unshifted / Uppercase/Graphics):
        // SC 00-1F (@..) -> PETSCII 40-5F
        // SC 20-3F ( !..) -> PETSCII 20-3F
        // SC 40-5F (Graph) -> PETSCII 60-7F
        // SC 60-7F (Graph) -> PETSCII A0-BF
        //
        // Bank 2 (Shifted / Lowercase):
        // SC 00-1F (a..z) -> PETSCII 40-5F (But displayed as LOWER case if in Shifted mode)
        // Actually, if we use the same PETSCII code but "Shifted" mode in rendering, it handles cases.
        //
        // So for "Screencode Unshifted" (Uppercase/Graphics):
        // 00 -> 40 (@)
        //
        // For "Screencode Shifted" (Lowercase):
        // 01 (A) -> Should look like 'a'. PETSCII 41 is 'A'.
        // BUT petscii_to_unicode(0x41, true) -> 'a'.
        // So if we convert SC 01 -> PETSCII 41, and pass shifted=true, we get 'a'. Correct.
        //
        // So the mapping is consistent regardless of shifted state, provided we pass the shifted state to `petscii_to_unicode`.
        // The mapping is mostly:
        // 00-1F -> +40 -> 40-5F
        // 20-3F -> +00 -> 20-3F
        // 40-5F -> +20 -> 60-7F
        // 60-7F -> +40 -> A0-BF
        //
        // What about 80-FF? (Reverse)
        // Usually ignored in simple hex dumps or mapped to non-reverse.
        // Let's map them to their non-reverse counterparts for now (mod 128) and apply reverse style?
        // Or just map them linearly if possible.
        // For now let's just handle the base 00-7F and map 80-FF to the same (stripped).
        _ => {
            let b = byte & 0x7F;
            match b {
                0x00..=0x1F => b + 0x40,
                0x20..=0x3F => b,
                0x40..=0x5F => b + 0x20,
                0x60..=0x7F => b + 0x40,
                _ => b,
            }
        }
    }
}

pub fn petscii_to_unicode(byte: u8, shifted: bool) -> char {
    let (unshifted_char, shifted_char) = PETSCII_MAP[byte as usize];
    if shifted {
        shifted_char
    } else {
        unshifted_char
    }
}

// Mapping: (Unshifted, Shifted)
// Based on https://github.com/9999years/Unicode-PETSCII/blob/master/table.txt
// Control codes (0x00-0x1F, 0x80-0x9F) are mapped to '.' unless they have a specific meaning visualization like space
static PETSCII_MAP: [(char, char); 256] = [
    // $00 - $0F
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    // $10 - $1F
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    // $20 - $2F
    (' ', ' '),
    ('!', '!'),
    ('\"', '\"'),
    ('#', '#'),
    ('$', '$'),
    ('%', '%'),
    ('&', '&'),
    ('\'', '\''),
    ('(', '('),
    (')', ')'),
    ('*', '*'),
    ('+', '+'),
    (',', ','),
    ('-', '-'),
    ('.', '.'),
    ('/', '/'),
    // $30 - $3F
    ('0', '0'),
    ('1', '1'),
    ('2', '2'),
    ('3', '3'),
    ('4', '4'),
    ('5', '5'),
    ('6', '6'),
    ('7', '7'),
    ('8', '8'),
    ('9', '9'),
    (':', ':'),
    (';', ';'),
    ('<', '<'),
    ('=', '='),
    ('>', '>'),
    ('?', '?'),
    // $40 - $4F
    ('@', '@'),
    ('A', 'a'),
    ('B', 'b'),
    ('C', 'c'),
    ('D', 'd'),
    ('E', 'e'),
    ('F', 'f'),
    ('G', 'g'),
    ('H', 'h'),
    ('I', 'i'),
    ('J', 'j'),
    ('K', 'k'),
    ('L', 'l'),
    ('M', 'm'),
    ('N', 'n'),
    ('O', 'o'),
    // $50 - $5F
    ('P', 'p'),
    ('Q', 'q'),
    ('R', 'r'),
    ('S', 's'),
    ('T', 't'),
    ('U', 'u'),
    ('V', 'v'),
    ('W', 'w'),
    ('X', 'x'),
    ('Y', 'y'),
    ('Z', 'z'),
    ('[', '['),
    ('£', '£'),
    (']', ']'),
    ('↑', '↑'),
    ('←', '←'),
    // $60 - $6F
    ('─', '─'),
    ('♠', 'A'),
    ('│', 'B'),
    ('─', 'C'),
    ('.', 'D'),
    ('.', 'E'),
    ('.', 'F'),
    ('.', 'G'),
    ('.', 'H'),
    ('╮', 'I'),
    ('╰', 'J'),
    ('╯', 'K'),
    ('.', 'L'),
    ('╲', 'M'),
    ('╱', 'N'),
    ('.', 'O'),
    // $70 - $7F
    ('.', 'P'),
    ('●', 'Q'),
    ('.', 'R'),
    ('♥', 'S'),
    ('.', 'T'),
    ('╭', 'U'),
    ('╳', 'V'),
    ('○', 'W'),
    ('♣', 'X'),
    ('.', 'Y'),
    ('♦', 'Z'),
    ('┼', '┼'),
    ('|', '|'),
    ('│', '│'),
    ('π', '▒'),
    ('◥', '.'),
    // $80 - $8F
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    // $90 - $9F
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    // $A0 - $AF
    (' ', ' '),
    ('▌', '▌'),
    ('▄', '▄'),
    ('▔', '▔'),
    (' ', ' '),
    ('▏', '▏'),
    ('▒', '▒'),
    ('▕', '▕'),
    ('.', '.'),
    ('◤', '◤'),
    ('.', '.'),
    ('├', '├'),
    ('▗', '▗'),
    ('└', '└'),
    ('┐', '┐'),
    ('▂', '▂'),
    // $B0 - $BF
    ('┌', '┌'),
    ('┴', '┴'),
    ('┬', '┬'),
    ('┤', '┤'),
    ('▎', '▎'),
    ('▍', '▍'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('▃', '▃'),
    ('✓', '✓'),
    ('▖', '▖'),
    ('▝', '▝'),
    ('┘', '┘'),
    ('▘', '▘'),
    ('▚', '▚'),
    // $C0 - $CF
    ('─', '─'),
    ('♠', 'A'),
    ('│', 'B'),
    ('─', 'C'),
    ('.', 'D'),
    ('.', 'E'),
    ('.', 'F'),
    ('.', 'G'),
    ('.', 'H'),
    ('╮', 'I'),
    ('╰', 'J'),
    ('╯', 'K'),
    ('.', 'L'),
    ('╲', 'M'),
    ('╱', 'N'),
    ('.', 'O'),
    // $D0 - $DF
    ('.', 'P'),
    ('●', 'Q'),
    ('.', 'R'),
    ('♥', 'S'),
    ('.', 'T'),
    ('╭', 'U'),
    ('╳', 'V'),
    ('○', 'W'),
    ('♣', 'X'),
    ('.', 'Y'),
    ('♦', 'Z'),
    ('┼', '┼'),
    ('|', '|'),
    ('│', '│'),
    ('π', '▒'),
    ('◥', '.'),
    // $E0 - $EF
    (' ', ' '),
    ('▌', '▌'),
    ('▄', '▄'),
    ('▔', '▔'),
    (' ', ' '),
    ('▏', '▏'),
    ('▒', '▒'),
    ('▕', '▕'),
    ('.', '.'),
    ('◤', '◤'),
    ('.', '.'),
    ('├', '├'),
    ('▗', '▗'),
    ('└', '└'),
    ('┐', '┐'),
    ('▂', '▂'),
    // $F0 - $FF
    ('┌', '┌'),
    ('┴', '┴'),
    ('┬', '┬'),
    ('┤', '┤'),
    ('▎', '▎'),
    ('▍', '▍'),
    ('.', '.'),
    ('.', '.'),
    ('.', '.'),
    ('▃', '▃'),
    ('✓', '✓'),
    ('▖', '▖'),
    ('▝', '▝'),
    ('┘', '┘'),
    ('▘', '▘'),
    ('π', '▒'),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_petscii_to_unicode_alphanumeric() {
        // Unshifted: 0x41 is 'A'
        assert_eq!(petscii_to_unicode(0x41, false), 'A');
        assert_eq!(petscii_to_unicode(0x5A, false), 'Z');

        // Shifted: 0x41 is 'a'
        assert_eq!(petscii_to_unicode(0x41, true), 'a');
        assert_eq!(petscii_to_unicode(0x5A, true), 'z');

        assert_eq!(petscii_to_unicode(0x30, false), '0');
        assert_eq!(petscii_to_unicode(0x39, false), '9');
    }

    #[test]
    fn test_petscii_to_unicode_graphics() {
        // 0x61: Unshifted ♠ (Spade), Shifted 'A'
        assert_eq!(petscii_to_unicode(0x61, false), '♠');
        assert_eq!(petscii_to_unicode(0x61, true), 'A');

        // 0x60: ─
        assert_eq!(petscii_to_unicode(0x60, false), '─');

        // 0x5E: ↑
        assert_eq!(petscii_to_unicode(0x5E, false), '↑');
    }

    #[test]
    fn test_petscii_to_unicode_control() {
        assert_eq!(petscii_to_unicode(0x00, false), '.');
        assert_eq!(petscii_to_unicode(0x00, true), '.');
    }

    #[test]
    fn test_petscii_to_unicode_upper_range() {
        // 0xA0: Space
        assert_eq!(petscii_to_unicode(0xA0, false), ' ');
        // 0xFF: π / ▒
        assert_eq!(petscii_to_unicode(0xFF, false), 'π');
        assert_eq!(petscii_to_unicode(0xFF, true), '▒');
    }
}
