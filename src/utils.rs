use image::DynamicImage;
use ratatui_image::picker::Picker;
use std::fs;
use std::path::{Path, PathBuf};

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
    let path = Path::new("docs/regenerator2000_logo.png");
    if path.exists() {
        if let Ok(img) = image::open(path) {
            return Some(img);
        }
    }
    None
}
pub fn create_picker() -> Option<Picker> {
    let font_size = (8, 16);
    Some(Picker::from_fontsize(font_size))
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
