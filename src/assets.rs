use crate::state::{Label, LabelKind, LabelType};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

static SYSTEMS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/systems");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelOption {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemConfig {
    pub features: Vec<LabelOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemData {
    platform_name: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    labels: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    comments: HashMap<String, String>,
    #[serde(default)]
    excluded: Vec<String>,
}

fn get_system_file_content(platform: &str) -> Option<&'static str> {
    let filename = format!("system-{}.json", platform);

    if let Some(file) = SYSTEMS_DIR.get_file(&filename) {
        return file.contents_utf8();
    }

    // Fallback: Try normalized filename (lowercase, spaces to underscores)
    let normalized = platform.to_lowercase().replace(' ', "_");
    if normalized != platform {
        let normalized_filename = format!("system-{}.json", normalized);
        if let Some(file) = SYSTEMS_DIR.get_file(&normalized_filename) {
            return file.contents_utf8();
        }
    }

    None
}

pub fn get_available_platforms() -> Vec<String> {
    let mut platforms = Vec::new();

    for file in SYSTEMS_DIR.files() {
        if let Some(filename) = file.path().file_name().and_then(|s| s.to_str())
            && filename.starts_with("system-")
            && filename.ends_with(".json")
            && let Some(content) = file.contents_utf8()
            && let Ok(data) = serde_json::from_str::<SystemData>(content)
            && data.enabled
        {
            platforms.push(data.platform_name);
        }
    }

    platforms.sort();
    platforms
}

pub fn load_system_config(platform: &str) -> SystemConfig {
    let mut features = Vec::new();

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(content)
    {
        // Convert hashmap keys to features
        let mut keys: Vec<_> = data.labels.keys().collect();
        keys.sort();

        for key in keys {
            features.push(LabelOption {
                id: key.clone(),
                name: key.clone(),
                default: false,
            });
        }
    }

    // Set defaults for specific features
    for f in &mut features {
        if f.id == "KERNAL" || f.id == "SYSTEM" {
            f.default = true;
        }
    }

    SystemConfig { features }
}

pub fn load_comments(platform: &str) -> BTreeMap<u16, String> {
    let mut comments = BTreeMap::new();

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(content)
    {
        for (addr_str, comment) in data.comments {
            if let Ok(addr) = u16::from_str_radix(&addr_str, 16)
                && !comment.is_empty()
            {
                comments.insert(addr, comment);
            }
        }
    }

    comments
}

pub fn load_labels(
    platform: &str,
    enabled_features: Option<&HashMap<String, bool>>,
) -> Vec<(u16, Label)> {
    let mut labels = Vec::new();

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(content)
    {
        // Determine defaults
        let mut defaults = HashMap::new();
        for key in data.labels.keys() {
            let default_val = key == "KERNAL" || key == "SYSTEM";
            defaults.insert(key.clone(), default_val);
        }

        for (feature_id, label_map) in data.labels {
            let is_enabled = if let Some(features) = enabled_features {
                *features
                    .get(&feature_id)
                    .unwrap_or(defaults.get(&feature_id).unwrap_or(&false))
            } else {
                *defaults.get(&feature_id).unwrap_or(&false)
            };

            if is_enabled {
                for (addr_str, name) in label_map {
                    if let Ok(addr) = u16::from_str_radix(&addr_str, 16) {
                        labels.push((
                            addr,
                            Label {
                                name,
                                label_type: LabelType::Predefined,
                                kind: LabelKind::System,
                            },
                        ));
                    }
                }
            }
        }
    }

    labels
}

pub fn load_excludes(platform: &str) -> Vec<u16> {
    let mut excludes = Vec::new();

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(content)
    {
        for line in data.excluded {
            let line = line.trim();
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
    fn test_assets_load() {
        // Smoke test to ensure we can list platforms
        let platforms = get_available_platforms();
        println!("Platforms: {:?}", platforms);
        assert!(!platforms.is_empty(), "Should have at least one platform");
    }

    #[test]
    fn test_get_system_file_content() {
        // Test simple case
        let content_nes = get_system_file_content("NES");
        assert!(content_nes.is_some(), "NES config should exist");

        // Test normalization case (Spaces -> Underscores)
        let content_c64 = get_system_file_content("Commodore 64");
        assert!(content_c64.is_some(), "Commodore 64 config should exist");

        // Test another normalization case
        let content_atari = get_system_file_content("Atari 8bit");
        assert!(content_atari.is_some(), "Atari 8bit config should exist");
    }

    #[test]
    fn test_load_system_config() {
        let config = load_system_config("Commodore 64");
        assert!(!config.features.is_empty(), "C64 should have features");

        // Check that KERNAL has default true
        let kernal = config.features.iter().find(|f| f.id == "KERNAL");
        assert!(kernal.is_some(), "C64 should have KERNAL feature");
        assert!(kernal.unwrap().default, "KERNAL should default to true");
    }
}
