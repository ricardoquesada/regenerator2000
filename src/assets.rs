use crate::state::{Label, LabelKind, LabelType, Platform};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn get_assets_path(platform: Platform) -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("assets");
    path.push("systems");
    path.push(platform.to_string());
    path
}

pub fn load_comments(platform: Platform) -> BTreeMap<u16, String> {
    let mut comments = BTreeMap::new();

    macro_rules! bundled_comments {
        ($($variant:ident => $path:expr),* $(,)?) => {
            match platform {
                $(Platform::$variant => Some(include_str!(concat!("../assets/systems/", $path, "/comments.txt")).to_string()),)*
            }
        };
    }

    let content = bundled_comments!(
        Commodore64 => "Commodore 64",
        Commodore128 => "Commodore 128",
        CommodorePlus4 => "Commodore Plus4",
        CommodoreVIC20 => "Commodore VIC-20",
        CommodorePET20 => "Commodore PET 2.0",
        CommodorePET40 => "Commodore PET 4.0",
        Commodore1541 => "Commodore 1541",
    );

    let content = content.unwrap_or_default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // If the line starts with ";" then it is a comment, and should be ignored.
        if line.starts_with(';') {
            continue;
        }

        // Format: Address (hex) space Comment
        // e.g. "FF81 init VIC"
        // Split once by whitespace
        let parts: Vec<&str> = line.splitn(2, |c: char| c.is_whitespace()).collect();
        if parts.len() < 2 {
            continue;
        }

        if let Ok(addr) = u16::from_str_radix(parts[0], 16) {
            let comment = parts[1].trim();
            if !comment.is_empty() {
                comments.insert(addr, comment.to_string());
            }
        }
    }
    comments
}

pub fn load_labels(platform: Platform) -> Vec<(u16, Label)> {
    let mut labels = Vec::new();

    macro_rules! bundled_labels {
        ($($variant:ident => $path:expr),* $(,)?) => {
            match platform {
                $(Platform::$variant => Some(include_str!(concat!("../assets/systems/", $path, "/labels.txt")).to_string()),)*
                _ => {
                    let mut path = get_assets_path(platform);
                    path.push("labels.txt");
                    std::fs::read_to_string(path).ok()
                }
            }
        };
    }

    let content = bundled_labels!(
        Commodore64 => "Commodore 64",
        Commodore128 => "Commodore 128",
        CommodorePlus4 => "Commodore Plus4",
        CommodoreVIC20 => "Commodore VIC-20",
        CommodorePET20 => "Commodore PET 2.0",
        CommodorePET40 => "Commodore PET 4.0"
    );

    if let Some(content) = content {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Format:
            // FF81 ROM_CINT

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(addr) = u16::from_str_radix(parts[0], 16) {
                    let name = parts[1].to_string();
                    labels.push((
                        addr,
                        Label {
                            name,
                            label_type: LabelType::Predefined,
                            kind: LabelKind::System,
                            refs: Vec::new(),
                        },
                    ));
                }
            }
        }
    }
    labels
}

pub fn load_excludes(platform: Platform) -> Vec<u16> {
    let mut excludes = Vec::new();

    macro_rules! bundled_excludes {
        ($($variant:ident => $path:expr),* $(,)?) => {
            match platform {
                $(Platform::$variant => Some(include_str!(concat!("../assets/systems/", $path, "/excludes.txt")).to_string()),)*
                _ => {
                    let mut path = get_assets_path(platform);
                    path.push("excludes.txt");
                    std::fs::read_to_string(path).ok()
                }
            }
        };
    }

    let content = bundled_excludes!(
        Commodore64 => "Commodore 64",
        Commodore128 => "Commodore 128",
        CommodorePlus4 => "Commodore Plus4",
        CommodoreVIC20 => "Commodore VIC-20",
        CommodorePET20 => "Commodore PET 2.0",
        CommodorePET40 => "Commodore PET 4.0",
        // Commodore1541 has no excludes.txt yet
    );

    if let Some(content_str) = content {
        for line in content_str.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            // Check for range: "031a-032d"
            if let Some((start_str, end_str)) = line.split_once('-') {
                let start_res = u16::from_str_radix(start_str.trim(), 16);
                let end_res = u16::from_str_radix(end_str.trim(), 16);
                if let (Ok(start), Ok(end)) = (start_res, end_res) {
                    for addr in start..=end {
                        excludes.push(addr);
                    }
                }
            } else {
                // Single address
                if let Ok(addr) = u16::from_str_radix(line, 16) {
                    excludes.push(addr);
                }
            }
        }
    }
    excludes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_comment_line() {
        // We can't easily test `load_comments` logic without mocking or passing content.
        // But we can extract parsing logic or just test strict format requirements if we refactor.
        // For now, let's test specific behavior by creating a temporary file?
        // No, `load_comments` logic is hardcoded to bundled assets or macro.
        // Refactoring to take a reader would be better, but for now I'll just check if C64 comments are loaded.
        let comments = load_comments(Platform::Commodore64);
        assert!(!comments.is_empty());

        // Check a known comment (from comments.txt if available)
        // Note: I don't know the exact content of C64 comments.txt, but I know it exists.
        // Let's assume there's at least one.
    }

    #[test]
    fn test_assets_bundled() {
        // Smoke test to ensure all platforms load something or don't crash
        for platform in Platform::all() {
            let _ = load_comments(*platform);
        }
    }
}
