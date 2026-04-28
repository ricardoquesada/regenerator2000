use crate::state::{Label, LabelKind, LabelType};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

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
    pub has_comments: bool,
    pub has_excludes: bool,
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

/// Returns the path to the user's config directory where custom `system-*.json`
/// files can be placed to override or extend the built-in platform definitions.
#[must_use]
pub fn user_config_systems_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "regenerator2000").map(|d| d.config_dir().to_path_buf())
}

/// Read a `system-*.json` file for `platform` as an owned `String`.
///
/// The user's config directory is checked first; if a matching file is found
/// there it takes precedence over the built-in embedded version.  Both the
/// exact filename and a normalized variant (lowercase, spaces → underscores)
/// are tried in that order.
fn get_system_file_content_with_config_dir(
    platform: &str,
    config_dir: Option<&Path>,
) -> Option<String> {
    let filename = format!("system-{platform}.json");
    let normalized = platform.to_lowercase().replace(' ', "_");
    let normalized_filename = format!("system-{normalized}.json");

    // 1. Check the provided config directory first (user files take precedence).
    if let Some(dir) = config_dir {
        for name in [filename.as_str(), normalized_filename.as_str()] {
            let path = dir.join(name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                return Some(content);
            }
        }
    }

    // 2. Fall back to the embedded assets.
    if let Some(file) = SYSTEMS_DIR.get_file(&filename) {
        return file.contents_utf8().map(str::to_owned);
    }
    if normalized != platform
        && let Some(file) = SYSTEMS_DIR.get_file(&normalized_filename)
    {
        return file.contents_utf8().map(str::to_owned);
    }

    None
}

fn get_system_file_content(platform: &str) -> Option<String> {
    get_system_file_content_with_config_dir(platform, user_config_systems_dir().as_deref())
}

/// Dump all embedded `system-*.json` files into `dest_dir`.
///
/// The destination directory is created automatically if it does not exist.
/// Each file is written with its original filename (e.g. `system-commodore_64.json`).
///
/// # Errors
///
/// Returns an error if the directory cannot be created or if any file write fails.
pub fn dump_system_config_files(dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create directory {:?}", dest_dir))?;

    for file in SYSTEMS_DIR.files() {
        let Some(filename) = file.path().file_name() else {
            continue;
        };
        let filename_str = filename.to_string_lossy();
        if !filename_str.starts_with("system-") || !filename_str.ends_with(".json") {
            continue;
        }
        let dest_path = dest_dir.join(filename);
        std::fs::write(&dest_path, file.contents())
            .with_context(|| format!("Failed to write {:?}", dest_path))?;
        println!("Wrote {dest_path:?}");
    }

    Ok(())
}

/// Collect enabled platform names from an iterator of `(filename, content)` pairs.
fn collect_platforms_from_iter<'a>(
    iter: impl Iterator<Item = (&'a str, String)>,
    platforms: &mut Vec<String>,
) {
    for (filename, content) in iter {
        if !filename.starts_with("system-") || !filename.ends_with(".json") {
            continue;
        }
        if let Ok(data) = serde_json::from_str::<SystemData>(&content)
            && data.enabled
        {
            platforms.push(data.platform_name);
        }
    }
}

fn get_available_platforms_with_config_dir(config_dir: Option<&Path>) -> Vec<String> {
    let mut platforms: Vec<String> = Vec::new();

    // 1. Collect from the provided config directory.
    if let Some(dir) = config_dir
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        let iter = entries.filter_map(|e| {
            let entry = e.ok()?;
            let path = entry.path();
            let filename = path.file_name()?.to_str()?.to_owned();
            let content = std::fs::read_to_string(&path).ok()?;
            Some((filename, content))
        });
        // Collect via a vec so lifetimes work out
        let pairs: Vec<(String, String)> = iter.collect();
        collect_platforms_from_iter(
            pairs.iter().map(|(f, c)| (f.as_str(), c.clone())),
            &mut platforms,
        );
    }

    // 2. Collect from built-in embedded assets, skipping names already added
    //    by the config directory.
    for file in SYSTEMS_DIR.files() {
        if let Some(filename) = file.path().file_name().and_then(|s| s.to_str())
            && filename.starts_with("system-")
            && filename.ends_with(".json")
            && let Some(content) = file.contents_utf8()
            && let Ok(data) = serde_json::from_str::<SystemData>(content)
            && data.enabled
            && !platforms.contains(&data.platform_name)
        {
            platforms.push(data.platform_name);
        }
    }

    platforms.sort();
    platforms
}

/// Return the list of all available platform names.
///
/// Platforms defined in the user's config directory are included and, when a
/// platform name matches a built-in one, the user's version takes precedence
/// (the built-in duplicate is excluded).
#[must_use]
pub fn get_available_platforms() -> Vec<String> {
    get_available_platforms_with_config_dir(user_config_systems_dir().as_deref())
}

#[must_use]
pub fn load_system_config(platform: &str) -> SystemConfig {
    let mut features = Vec::new();
    let mut has_comments = false;
    let mut has_excludes = false;

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
    {
        has_comments = !data.comments.is_empty();
        has_excludes = !data.excluded.is_empty();

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

    SystemConfig {
        features,
        has_comments,
        has_excludes,
    }
}

#[must_use]
pub fn load_comments(platform: &str) -> BTreeMap<u16, String> {
    let mut comments = BTreeMap::new();

    if let Some(content) = get_system_file_content(platform)
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

#[must_use]
pub fn load_labels(
    platform: &str,
    enabled_features: Option<&HashMap<String, bool>>,
) -> Vec<(u16, Label)> {
    let mut labels = Vec::new();

    if let Some(content) = get_system_file_content(platform)
        && let Ok(data) = serde_json::from_str::<SystemData>(&content)
    {
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

#[must_use]
pub fn load_excludes(platform: &str) -> Vec<u16> {
    let mut excludes = Vec::new();

    if let Some(content) = get_system_file_content(platform)
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
    use std::fs;

    #[test]
    fn test_assets_load() {
        // Smoke test to ensure we can list platforms
        let platforms = get_available_platforms();
        println!("Platforms: {platforms:?}");
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

        // Test VIC-20 case (hyphen)
        let content_vic20 = get_system_file_content("Commodore VIC-20");
        assert!(content_vic20.is_some(), "VIC-20 config should exist");
    }

    #[test]
    fn test_load_system_config() {
        let config = load_system_config("Commodore 64");
        assert!(!config.features.is_empty(), "C64 should have features");

        // Check that KERNAL has default true
        let kernal = config.features.iter().find(|f| f.id == "KERNAL");
        assert!(kernal.is_some(), "C64 should have KERNAL feature");
        assert!(kernal.unwrap().default, "KERNAL should default to true");

        // Test VIC-20 case
        let config_vic20 = load_system_config("Commodore VIC-20");
        assert!(
            !config_vic20.features.is_empty(),
            "VIC-20 should have features"
        );
        assert!(config_vic20.has_comments, "VIC-20 should have comments");
        assert!(config_vic20.has_excludes, "VIC-20 should have excludes");
    }

    /// Minimal valid `SystemData` JSON for a custom test platform.
    fn make_custom_system_json(platform_name: &str) -> String {
        format!(
            r#"{{"platform_name":"{platform_name}","enabled":true,"labels":{{"CUSTOM":{{"1000":"MY_LABEL"}}}},"comments":{{}},"excluded":[]}}"#
        )
    }

    /// Verify that a `system-*.json` placed in the config directory:
    ///   1. Is returned by `get_system_file_content_with_config_dir` (overriding built-ins).
    ///   2. Appears in `get_available_platforms_with_config_dir` as a new platform.
    ///   3. Built-in platforms that were NOT overridden are still listed.
    ///   4. An overridden built-in (same `platform_name`) is NOT duplicated.
    #[test]
    fn test_user_config_dir_overrides_builtin() {
        // Use a unique temp directory per test run so parallel tests never collide.
        let test_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let config_dir = std::env::temp_dir().join(format!("r2000_test_assets_{test_id}"));
        fs::create_dir_all(&config_dir).unwrap();

        // ── 1. Custom (new) platform ───────────────────────────────────────────
        let custom_json = make_custom_system_json("My Custom Platform");
        fs::write(
            config_dir.join("system-my_custom_platform.json"),
            &custom_json,
        )
        .unwrap();

        // get_system_file_content_with_config_dir must find it.
        let content =
            get_system_file_content_with_config_dir("my_custom_platform", Some(&config_dir));
        assert!(
            content.is_some(),
            "Custom platform file should be found in config dir"
        );
        assert_eq!(content.unwrap(), custom_json);

        // ── 2. Custom platform appears in platform list ────────────────────────
        let platforms = get_available_platforms_with_config_dir(Some(&config_dir));
        assert!(
            platforms.contains(&"My Custom Platform".to_string()),
            "Custom platform should appear in available platforms"
        );

        // ── 3. Built-in platforms are still listed ─────────────────────────────
        assert!(
            platforms.contains(&"Commodore 64".to_string()),
            "Built-in Commodore 64 should still be listed"
        );

        // ── 4. Override: config-dir file wins over built-in ───────────────────
        // Write a file that shadows the built-in Commodore 64 definition.
        let override_json = make_custom_system_json("Commodore 64");
        fs::write(config_dir.join("system-commodore_64.json"), &override_json).unwrap();

        let overridden = get_system_file_content_with_config_dir("Commodore 64", Some(&config_dir));
        assert!(
            overridden.is_some(),
            "Overridden Commodore 64 file should be found"
        );
        assert_eq!(
            overridden.unwrap(),
            override_json,
            "Config-dir version should take precedence over built-in"
        );

        // Platform list must not contain duplicate "Commodore 64" entries.
        let platforms_after_override = get_available_platforms_with_config_dir(Some(&config_dir));
        let c64_count = platforms_after_override
            .iter()
            .filter(|p| p.as_str() == "Commodore 64")
            .count();
        assert_eq!(c64_count, 1, "Commodore 64 should appear exactly once");

        // Cleanup
        let _ = fs::remove_dir_all(&config_dir);
    }
}
