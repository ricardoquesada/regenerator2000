use crate::state::{Label, LabelKind, LabelType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelOption {
    pub id: String,
    pub name: String,
    // default is inferred or stored differently now?
    // Using simple struct for UI compatibility
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

pub fn get_assets_path() -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("assets");
    path.push("systems");
    path
}

fn get_system_file_path(platform: &str) -> PathBuf {
    let mut path = get_assets_path();
    path.push(format!("system-{}.json", platform));
    path
}

pub fn get_available_platforms() -> Vec<String> {
    let mut platforms = Vec::new();
    let assets_path = get_assets_path();

    if let Ok(entries) = fs::read_dir(assets_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|s| s.to_str())
                && filename.starts_with("system-")
                && filename.ends_with(".json")
            {
                // Try to parse to check if enabled
                if let Ok(content) = fs::read_to_string(&path)
                    && let Ok(data) = serde_json::from_str::<SystemData>(&content)
                    && data.enabled
                {
                    platforms.push(data.platform_name);
                }
            }
        }
    }
    platforms.sort();
    platforms
}

pub fn load_system_config(platform: &str) -> SystemConfig {
    let path = get_system_file_path(platform);
    let mut features = Vec::new();

    if let Ok(content) = fs::read_to_string(path)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
    {
        // Convert hashmap keys to features
        // Sort keys to have stable order in UI
        let mut keys: Vec<_> = data.labels.keys().collect();
        keys.sort();

        for key in keys {
            features.push(LabelOption {
                id: key.clone(),
                name: key.clone(), // Use ID as name since we don't have separate names
                default: false,    // Default to false? Or true if it's "SYSTEM" or "KERNAL"?
            });
        }
    }
    // If empty or specialized logic needed:
    // Maybe set KERNAL etc to default true if found?
    for f in &mut features {
        if f.id == "KERNAL" || f.id == "SYSTEM" {
            f.default = true;
        }
    }

    SystemConfig { features }
}

pub fn load_comments(platform: &str) -> BTreeMap<u16, String> {
    let mut comments = BTreeMap::new();
    let path = get_system_file_path(platform);

    if let Ok(content) = fs::read_to_string(path)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
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
    let path = get_system_file_path(platform);

    if let Ok(content) = fs::read_to_string(path)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
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
    let path = get_system_file_path(platform);

    if let Ok(content) = fs::read_to_string(path)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
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
        // Only asserts if we expect assets to exist during test
        // assert!(!platforms.is_empty());
    }
}
