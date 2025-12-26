use crate::state::{Label, LabelKind, LabelType, Platform};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn get_assets_path(platform: Platform) -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("assets");
    path.push("systems");
    path.push(platform.to_string());
    path
}

pub fn load_comments(platform: Platform) -> HashMap<u16, String> {
    let mut comments = HashMap::new();
    let mut path = get_assets_path(platform);
    path.push("comments.txt");

    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // content format looks like:
            // FF81 $FF81 - init VIC & screen editor
            // ...

            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() < 2 {
                continue;
            }

            if let Ok(addr) = u16::from_str_radix(parts[0], 16) {
                let remaining = parts[1].trim();
                let comment_start = if remaining.starts_with('$') {
                    if let Some(idx) = remaining.find(' ') {
                        &remaining[idx + 1..]
                    } else {
                        ""
                    }
                } else {
                    remaining
                };

                let comment = comment_start
                    .trim_start_matches(|c| c == '-' || c == ' ' || c == ':')
                    .trim();

                if !comment.is_empty() {
                    comments.insert(addr, comment.to_string());
                }
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_comment_line() {
        // Placeholder for future tests
    }
}
