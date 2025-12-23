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

pub fn petscii_to_unicode(byte: u8) -> char {
    match byte {
        0x00..=0x1F => '.',                   // Control codes
        0x20..=0x40 => byte as char,          // Space + Punctuation + Numbers + @
        0x41..=0x5A => (byte + 0x20) as char, // Lowercase letters (unshifted state)
        // 0x5B..=0x5F -> [, pound, ], arrow up, arrow left
        0x5B => '[',
        0x5C => '£',
        0x5D => ']',
        0x5E => '↑',
        0x5F => '←',
        // 0x60..=0x7F -> Graphics
        0x60..=0x7F => '░', // Placeholder for graphics
        // 0x80..=0x9F -> Control codes?
        0x80..=0x9F => '.',
        // 0xA0..=0xBF -> Shifted Space + Graphics
        0xA0 => ' ',
        0xA1..=0xBF => '▒',
        // 0xC0..=0xDF -> Uppercase?
        0xC1..=0xDA => (byte - 0x80) as char, // A-Z (uppercase)
        // Others
        _ => '.',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_petscii_to_unicode_alphanumeric() {
        assert_eq!(petscii_to_unicode(0x41), 'a'); // 'A' unshifted -> 'a'
        assert_eq!(petscii_to_unicode(0x5A), 'z');
        assert_eq!(petscii_to_unicode(0xC1), 'A'); // 'A' shifted -> 'A'
        assert_eq!(petscii_to_unicode(0xDA), 'Z');
        assert_eq!(petscii_to_unicode(0x30), '0');
        assert_eq!(petscii_to_unicode(0x39), '9');
    }

    #[test]
    fn test_petscii_to_unicode_graphics() {
        assert_eq!(petscii_to_unicode(0x60), '░');
        assert_eq!(petscii_to_unicode(0xA0), ' ');
        assert_eq!(petscii_to_unicode(0x5B), '[');
    }

    #[test]
    fn test_petscii_to_unicode_control() {
        assert_eq!(petscii_to_unicode(0x00), '.');
        assert_eq!(petscii_to_unicode(0x1F), '.');
        assert_eq!(petscii_to_unicode(0x90), '.');
    }
}
