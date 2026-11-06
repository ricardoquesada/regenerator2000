//! Embedded and disk asset manager for system ROMs, predefined labels, comments, and enums.

use crate::state::{EnumDefinition, Label, LabelKind, LabelType, RawEnumDefinition};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

static SYSTEMS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/systems");
static ENUMS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/enums");

/// Default 8 KB Commodore 64 BASIC V2 ROM image (`$A000`–`$BFFF`).
pub static C64_BASIC_ROM: &[u8] = include_bytes!("../assets/roms/c64/basic-901226-01.bin");
/// Default 8 KB Commodore 64 KERNAL ROM image (`$E000`–`$FFFF`).
pub static C64_KERNAL_ROM: &[u8] = include_bytes!("../assets/roms/c64/kernal-901227-03.bin");
/// Default 4 KB Commodore 64 Character Generator ROM image (`$D000`–`$DFFF`).
pub static C64_CHAR_ROM: &[u8] = include_bytes!("../assets/roms/c64/characters-901225-01.bin");

/// Returns the default BASIC ROM image for `system`, if available.
#[must_use]
pub fn default_basic_rom(system: &crate::state::types::System) -> Option<Vec<u8>> {
    system.is_c64().then(|| C64_BASIC_ROM.to_vec())
}

/// Returns the default KERNAL ROM image for `system`, if available.
#[must_use]
pub fn default_kernal_rom(system: &crate::state::types::System) -> Option<Vec<u8>> {
    system.is_c64().then(|| C64_KERNAL_ROM.to_vec())
}

/// Returns the default Character Generator ROM image for `system`, if available.
#[must_use]
pub fn default_char_rom(system: &crate::state::types::System) -> Option<Vec<u8>> {
    system.is_c64().then(|| C64_CHAR_ROM.to_vec())
}

/// Describes a toggleable predefined label category for a target system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelOption {
    /// Unique feature identifier (for example, `"KERNAL"` or `"VIC_II"`).
    pub id: String,
    /// Display name shown in the UI settings dialog.
    pub name: String,
    /// Enables this label category by default for new projects when `true`.
    #[serde(default)]
    pub default: bool,
}

/// Summarizes the available predefined label categories, comments, and exclusions for a system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemConfig {
    /// Available label categories for the system.
    pub features: Vec<LabelOption>,
    /// Indicates that the system asset file defines address comments.
    pub has_comments: bool,
    /// Indicates that the system asset file defines excluded address ranges.
    pub has_excludes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemData {
    #[serde(alias = "platform_name")]
    system_name: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    labels: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    comments: BTreeMap<String, String>,
    #[serde(default)]
    excluded: Vec<String>,
}

/// Parses a [`SystemData`] definition from TOML, or falls back to JSON for legacy files.
fn parse_system_data(content: &str) -> Option<SystemData> {
    toml::from_str::<SystemData>(content)
        .ok()
        .or_else(|| serde_json::from_str::<SystemData>(content).ok())
}

/// Returns `true` when `feature_id` is enabled by default for a system.
#[must_use]
fn is_default_feature(feature_id: &str) -> bool {
    matches!(feature_id, "KERNAL" | "SYSTEM")
}

/// Returns the user configuration directory where custom `system-*.toml` files reside.
#[must_use]
pub fn user_config_systems_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "regenerator2000").map(|d| d.config_dir().to_path_buf())
}

/// Reads the raw configuration file content for `system`, checking `config_dir` before embedded assets.
fn get_system_file_content_with_config_dir(
    system: &str,
    config_dir: Option<&Path>,
) -> Option<String> {
    let normalized = system.to_lowercase().replace(' ', "_");

    // Check `config_dir` first so user files override embedded definitions.
    if let Some(dir) = config_dir {
        let user_candidates = [
            format!("system-{system}.toml"),
            format!("system-{normalized}.toml"),
            format!("platform-{system}.toml"),
            format!("platform-{normalized}.toml"),
            format!("platform-{system}.json"),
            format!("platform-{normalized}.json"),
        ];
        for (idx, name) in user_candidates.iter().enumerate() {
            if user_candidates[..idx].contains(name) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(dir.join(name)) {
                return Some(content);
            }
        }
    }

    // Fall back to embedded system assets.
    let embedded_candidates = [
        format!("system-{system}.toml"),
        format!("system-{normalized}.toml"),
    ];
    for (idx, name) in embedded_candidates.iter().enumerate() {
        if embedded_candidates[..idx].contains(name) {
            continue;
        }
        if let Some(file) = SYSTEMS_DIR.get_file(name) {
            return file.contents_utf8().map(str::to_owned);
        }
    }

    None
}

fn get_system_file_content(system: &str) -> Option<String> {
    get_system_file_content_with_config_dir(system, user_config_systems_dir().as_deref())
}

fn dump_embedded_toml_files(embedded_dir: &Dir<'_>, prefix: &str, dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir)
        .with_context(|| format!("Failed to create directory {dest_dir:?}"))?;

    for file in embedded_dir.files() {
        let Some(filename) = file.path().file_name() else {
            continue;
        };
        let filename_str = filename.to_string_lossy();
        if !filename_str.starts_with(prefix) || !filename_str.ends_with(".toml") {
            continue;
        }
        let dest_path = dest_dir.join(filename);
        std::fs::write(&dest_path, file.contents())
            .with_context(|| format!("Failed to write {dest_path:?}"))?;
        println!("Wrote {dest_path:?}");
    }

    Ok(())
}

/// Writes all embedded `system-*.toml` files into `dest_dir`.
///
/// Creates `dest_dir` if it does not exist.
///
/// # Errors
///
/// Returns an error if `dest_dir` cannot be created or if writing any file fails.
pub fn dump_system_config_files(dest_dir: &Path) -> Result<()> {
    dump_embedded_toml_files(&SYSTEMS_DIR, "system-", dest_dir)
}

/// Returns `true` when `filename` matches a supported system configuration file pattern.
#[must_use]
fn is_system_config_file(filename: &str) -> bool {
    (filename.starts_with("system-") || filename.starts_with("platform-"))
        && (filename.ends_with(".toml") || filename.ends_with(".json"))
}

fn get_available_systems_with_config_dir(config_dir: Option<&Path>) -> Vec<String> {
    let mut systems: Vec<String> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // 1. Collect from `config_dir` first, recording both enabled and disabled system names.
    if let Some(dir) = config_dir
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !is_system_config_file(filename) {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(data) = parse_system_data(&content) else {
                continue;
            };
            if seen_names.insert(data.system_name.clone()) && data.enabled {
                systems.push(data.system_name);
            }
        }
    }

    // 2. Collect from embedded assets, skipping any system name already defined in `config_dir`.
    for file in SYSTEMS_DIR.files() {
        if let Some(filename) = file.path().file_name().and_then(|s| s.to_str())
            && is_system_config_file(filename)
            && let Some(content) = file.contents_utf8()
            && let Some(data) = parse_system_data(content)
            && seen_names.insert(data.system_name.clone())
            && data.enabled
        {
            systems.push(data.system_name);
        }
    }

    systems.sort();
    systems
}

/// Returns the sorted list of all enabled system names from user and embedded assets.
///
/// User definitions in the configuration directory override embedded definitions with the same name.
#[must_use]
pub fn get_available_systems() -> Vec<String> {
    get_available_systems_with_config_dir(user_config_systems_dir().as_deref())
}

/// Loads the [`SystemConfig`] metadata (label categories, comments flag, excludes flag) for `system`.
#[must_use]
pub fn load_system_config(system: &str) -> SystemConfig {
    let mut features = Vec::new();
    let mut has_comments = false;
    let mut has_excludes = false;

    if let Some(content) = get_system_file_content(system)
        && let Some(data) = parse_system_data(&content)
    {
        has_comments = !data.comments.is_empty();
        has_excludes = !data.excluded.is_empty();

        for key in data.labels.keys() {
            features.push(LabelOption {
                id: key.clone(),
                name: key.clone(),
                default: is_default_feature(key),
            });
        }
    }

    SystemConfig {
        features,
        has_comments,
        has_excludes,
    }
}

/// Loads the predefined system comment map (`address -> comment`) for `system`.
#[must_use]
pub fn load_comments(system: &str) -> BTreeMap<u16, String> {
    let mut comments = BTreeMap::new();

    if let Some(content) = get_system_file_content(system)
        && let Some(data) = parse_system_data(&content)
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

/// Loads predefined [`Label`] entries for `system`, filtered by `enabled_features`.
#[must_use]
pub fn load_labels(
    system: &str,
    enabled_features: Option<&HashMap<String, bool>>,
) -> Vec<(u16, Label)> {
    let mut labels = Vec::new();

    if let Some(content) = get_system_file_content(system)
        && let Some(data) = parse_system_data(&content)
    {
        for (feature_id, label_map) in data.labels {
            let default_enabled = is_default_feature(&feature_id);
            let is_enabled = enabled_features
                .and_then(|features| features.get(&feature_id).copied())
                .unwrap_or(default_enabled);

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

/// Loads the list of addresses excluded from automatic label generation for `system`.
#[must_use]
pub fn load_excludes(system: &str) -> Vec<u16> {
    let mut excludes = Vec::new();

    if let Some(content) = get_system_file_content(system)
        && let Some(data) = parse_system_data(&content)
    {
        for line in data.excluded {
            let line = line.trim();
            if let Some((start_str, end_str)) = line.split_once('-') {
                if let (Ok(start), Ok(end)) = (
                    u16::from_str_radix(start_str.trim(), 16),
                    u16::from_str_radix(end_str.trim(), 16),
                ) {
                    excludes.extend(start..=end);
                }
            } else if let Ok(addr) = u16::from_str_radix(line, 16) {
                excludes.push(addr);
            }
        }
    }

    excludes
}

// =============================================================================
// Enum Assets Loading & Dumping
// =============================================================================

/// Returns the user configuration directory where custom `enum-*.toml` files reside.
#[must_use]
pub fn user_config_enums_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "regenerator2000").map(|d| d.config_dir().to_path_buf())
}

/// Converts an enum name into a sanitized `enum-<snake_case>.toml` file name.
#[must_use]
fn enum_filename(name: &str) -> String {
    let mut snake = String::new();
    let mut prev_was_upper = true;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if !prev_was_upper && !snake.ends_with('_') {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
            prev_was_upper = true;
        } else if ch.is_ascii_alphanumeric() {
            snake.push(ch);
            prev_was_upper = false;
        } else {
            if !snake.is_empty() && !snake.ends_with('_') {
                snake.push('_');
            }
            prev_was_upper = true;
        }
    }
    let safe_name = snake.trim_matches('_');
    let safe_name = if safe_name.is_empty() {
        "unnamed"
    } else {
        safe_name
    };
    format!("enum-{safe_name}.toml")
}

/// Loads all built-in enums from the embedded `assets/enums/` directory.
#[must_use]
pub fn load_builtin_enums() -> BTreeMap<String, EnumDefinition> {
    let mut enums = BTreeMap::new();
    for file in ENUMS_DIR.files() {
        let Some(filename) = file.path().file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !filename.starts_with("enum-") || !filename.ends_with(".toml") {
            continue;
        };
        let Some(content) = file.contents_utf8() else {
            continue;
        };
        match toml::from_str::<RawEnumDefinition>(content) {
            Ok(raw) => {
                let mut def = EnumDefinition::from(raw);
                def.source_file = Some(filename.to_string());
                enums.insert(def.name.clone(), def);
            }
            Err(e) => {
                log::warn!("Failed to parse built-in enum {filename}: {e}");
            }
        }
    }
    enums
}

/// Loads all custom global enums from `dir`.
#[must_use]
pub fn load_global_enums(dir: &Path) -> BTreeMap<String, EnumDefinition> {
    let mut enums = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return enums;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !filename.starts_with("enum-") || !filename.ends_with(".toml") {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<RawEnumDefinition>(&content) {
                Ok(raw) => {
                    let mut def = EnumDefinition::from(raw);
                    def.source_file = Some(filename.to_string());
                    log::info!("Loaded global enum: {} from {path:?}", def.name);
                    enums.insert(def.name.clone(), def);
                }
                Err(e) => {
                    log::warn!("Failed to parse enum file {path:?}: {e}");
                }
            },
            Err(e) => {
                log::warn!("Failed to read enum file {path:?}: {e}");
            }
        }
    }

    enums
}

/// Writes all embedded `enum-*.toml` files into `dest_dir`.
///
/// Creates `dest_dir` if it does not exist.
///
/// # Errors
///
/// Returns an error if `dest_dir` cannot be created or if writing any file fails.
pub fn dump_enum_files(dest_dir: &Path) -> Result<()> {
    dump_embedded_toml_files(&ENUMS_DIR, "enum-", dest_dir)
}

/// Saves `def` as a global TOML file in the user configuration directory.
///
/// # Errors
///
/// Returns an error if the configuration directory cannot be resolved or if writing the file fails.
pub fn save_global_enum(def: &mut EnumDefinition) -> Result<()> {
    let dir =
        user_config_enums_dir().context("Failed to resolve user config directory for enums")?;
    std::fs::create_dir_all(&dir).with_context(|| format!("Failed to create directory {dir:?}"))?;

    let filename = if let Some(sf) = &def.source_file {
        sf.clone()
    } else {
        let fname = enum_filename(&def.name);
        def.source_file = Some(fname.clone());
        fname
    };

    let path = dir.join(&filename);
    let raw = RawEnumDefinition::from(def.clone());
    let toml_content =
        toml::to_string_pretty(&raw).context("Failed to serialize RawEnumDefinition to TOML")?;

    std::fs::write(&path, toml_content)
        .with_context(|| format!("Failed to write global enum to {path:?}"))?;

    Ok(())
}

/// Deletes the global enum file for `name` from the user configuration directory.
///
/// # Errors
///
/// Returns an error if the configuration directory cannot be resolved or if removing the file fails.
pub fn delete_global_enum(name: &str, state_def: Option<&EnumDefinition>) -> Result<()> {
    let dir =
        user_config_enums_dir().context("Failed to resolve user config directory for enums")?;
    let filename = if let Some(def) = state_def.and_then(|d| d.source_file.as_ref()) {
        def.clone()
    } else {
        enum_filename(name)
    };
    let path = dir.join(filename);
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to delete global enum at {path:?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_assets_load() {
        let systems = get_available_systems();
        assert!(!systems.is_empty(), "Should have at least one system");
    }

    #[test]
    fn test_get_system_file_content() {
        let content_nes = get_system_file_content("NES");
        assert!(content_nes.is_some(), "NES config should exist");

        let content_c64 = get_system_file_content("Commodore 64");
        assert!(content_c64.is_some(), "Commodore 64 config should exist");

        let content_atari = get_system_file_content("Atari 8bit");
        assert!(content_atari.is_some(), "Atari 8bit config should exist");

        let content_vic20 = get_system_file_content("Commodore VIC-20");
        assert!(content_vic20.is_some(), "VIC-20 config should exist");
    }

    #[test]
    fn test_load_system_config() {
        let config = load_system_config("Commodore 64");
        assert!(!config.features.is_empty(), "C64 should have features");

        let kernal = config.features.iter().find(|f| f.id == "KERNAL");
        assert!(kernal.is_some(), "C64 should have KERNAL feature");
        assert!(kernal.unwrap().default, "KERNAL should default to true");

        let config_vic20 = load_system_config("Commodore VIC-20");
        assert!(
            !config_vic20.features.is_empty(),
            "VIC-20 should have features"
        );
        assert!(config_vic20.has_comments, "VIC-20 should have comments");
        assert!(config_vic20.has_excludes, "VIC-20 should have excludes");
    }

    /// Builds a minimal valid `SystemData` TOML string for a test system.
    fn make_custom_system_toml(system_name: &str) -> String {
        format!(
            "system_name = \"{system_name}\"\nenabled = true\nexcluded = []\n\n[labels.CUSTOM]\n\"1000\" = \"MY_LABEL\"\n\n[comments]\n"
        )
    }

    /// Builds a minimal valid `SystemData` JSON string using the legacy `platform_name` alias.
    fn make_custom_system_json(system_name: &str) -> String {
        format!(
            r#"{{"platform_name":"{system_name}","enabled":true,"labels":{{"CUSTOM":{{"1000":"MY_LABEL"}}}},"comments":{{}},"excluded":[]}}"#
        )
    }

    /// Verifies that `system-*.toml` files in `config_dir` override built-in definitions
    /// without duplicating or re-enabling disabled systems.
    #[test]
    fn test_user_config_dir_overrides_builtin() {
        let test_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let config_dir = std::env::temp_dir().join(format!("r2000_test_assets_{test_id}"));
        fs::create_dir_all(&config_dir).unwrap();

        let custom_toml = make_custom_system_toml("My Custom System");
        fs::write(
            config_dir.join("system-my_custom_system.toml"),
            &custom_toml,
        )
        .unwrap();

        let content =
            get_system_file_content_with_config_dir("my_custom_system", Some(&config_dir));
        assert_eq!(content.as_deref(), Some(custom_toml.as_str()));

        let systems = get_available_systems_with_config_dir(Some(&config_dir));
        assert!(systems.contains(&"My Custom System".to_string()));
        assert!(systems.contains(&"Commodore 64".to_string()));

        let override_toml = make_custom_system_toml("Commodore 64");
        fs::write(config_dir.join("system-commodore_64.toml"), &override_toml).unwrap();

        let overridden = get_system_file_content_with_config_dir("Commodore 64", Some(&config_dir));
        assert_eq!(overridden.as_deref(), Some(override_toml.as_str()));

        let systems_after_override = get_available_systems_with_config_dir(Some(&config_dir));
        let c64_count = systems_after_override
            .iter()
            .filter(|p| p.as_str() == "Commodore 64")
            .count();
        assert_eq!(c64_count, 1, "Commodore 64 should appear exactly once");

        // Disabling a built-in system in `config_dir` must suppress the embedded fallback.
        fs::write(
            config_dir.join("system-commodore_64.toml"),
            "system_name = \"Commodore 64\"\nenabled = false\n",
        )
        .unwrap();
        let systems_after_disable = get_available_systems_with_config_dir(Some(&config_dir));
        assert!(
            !systems_after_disable.contains(&"Commodore 64".to_string()),
            "Disabled override in config_dir must not fall back to enabled built-in"
        );

        let _ = fs::remove_dir_all(&config_dir);
    }

    /// Verifies that legacy `platform-*.json` files in `config_dir` remain supported.
    #[test]
    fn test_legacy_json_backward_compatibility() {
        let test_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let config_dir = std::env::temp_dir().join(format!("r2000_test_json_compat_{test_id}"));
        fs::create_dir_all(&config_dir).unwrap();

        let custom_json = make_custom_system_json("Legacy JSON System");
        fs::write(
            config_dir.join("platform-legacy_json_platform.json"),
            &custom_json,
        )
        .unwrap();

        let systems = get_available_systems_with_config_dir(Some(&config_dir));
        assert!(
            systems.contains(&"Legacy JSON System".to_string()),
            "Legacy JSON system should appear in available systems"
        );

        let _ = fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn test_enum_filename_sanitization_and_consistency() {
        assert_eq!(enum_filename("VicColors"), "enum-vic_colors.toml");
        assert_eq!(enum_filename("CIA 1 Modes"), "enum-cia_1_modes.toml");
        assert_eq!(enum_filename("../bad/path"), "enum-bad_path.toml");
        assert_eq!(enum_filename("---"), "enum-unnamed.toml");
    }
}
