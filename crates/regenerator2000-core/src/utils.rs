use std::fs;
use std::path::{Path, PathBuf};

#[must_use]
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

#[must_use]
pub fn calculate_entropy(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }

    let mut counts = [0usize; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f32;
    let mut entropy = 0.0;

    for count in counts {
        if count > 0 {
            let p = count as f32 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}
#[must_use]
pub fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[must_use]
pub fn screencode_to_petscii(byte: u8) -> u8 {
    // Basic Screencode to PETSCII mapping
    // This is a simplification, but covers the main displayable range
    match byte {
        0x00..=0x1F => byte + 0x40, // @ABC... -> @ABC... (40-5F)
        0x20..=0x3F => byte,        //  !"#... ->  !"#... (20-3F)
        0x40..=0x5F => byte + 0x20, // ─♠│... -> ─♠│... (60-7F)
        0x60..=0x7F => byte + 0x40, //  ▌▄▔... ->  ▌▄▔... (A0-BF)
        // Reverse characters (bit 7 set)
        0x80..=0x9F => byte - 0x40 + 0x80,
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

#[must_use]
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
    ('"', '"'),
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

    #[test]
    fn test_calculate_entropy() {
        // Empty
        assert_eq!(calculate_entropy(&[]), 0.0);

        // Zero entropy (all same bytes)
        let data = vec![0; 100];
        assert_eq!(calculate_entropy(&data), 0.0);

        // Max entropy (uniform distribution)
        // For 256 bytes, max entropy is 8.0
        let mut data = Vec::with_capacity(256);
        for i in 0..=255 {
            data.push(i as u8);
        }
        assert!((calculate_entropy(&data) - 8.0).abs() < 0.001);

        // 2 distinct values, equal probability -> 1 bit
        let data = vec![0, 0, 1, 1];
        assert!((calculate_entropy(&data) - 1.0).abs() < 0.001);
    }
}
