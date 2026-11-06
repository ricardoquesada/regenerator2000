//! User preferences and application-wide configuration persistence.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Stores persistent user preferences and recent project paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Reopens the most recently saved project on startup when `true`.
    pub open_last_project: bool,
    /// Path to the most recently saved project file.
    pub last_project_path: Option<PathBuf>,
    /// Name of the active UI color theme.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Synchronizes the Blocks view selection with the disassembly cursor when `true`.
    #[serde(default = "default_true")]
    pub sync_blocks_view: bool,
    /// Synchronizes the Hex Dump cursor with the disassembly cursor when `true`.
    #[serde(default = "default_true")]
    pub sync_hex_dump: bool,
    /// Synchronizes the Charset view cursor with the disassembly cursor when `true`.
    #[serde(default = "default_false")]
    pub sync_charset_view: bool,
    /// Synchronizes the Sprites view cursor with the disassembly cursor when `true`.
    #[serde(default = "default_false")]
    pub sync_sprites_view: bool,
    /// Synchronizes the Bitmap view cursor with the disassembly cursor when `true`.
    #[serde(default = "default_false")]
    pub sync_bitmap_view: bool,
    /// Shannon entropy threshold above which a binary triggers a packed-file warning.
    #[serde(default = "default_entropy_threshold")]
    pub entropy_threshold: f32,
    /// Optional path that overrides the default user configuration file location.
    #[serde(skip)]
    pub config_path_override: Option<PathBuf>,
    /// Ordered list of recently opened `.regen2000proj` file paths (newest first).
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    /// Checks for newer application releases on startup when `true`.
    #[serde(default = "default_true")]
    pub check_for_updates: bool,
    /// Initializes newly loaded binary regions as `Undefined` instead of `DataByte` when `true`.
    #[serde(default = "default_true")]
    pub default_is_unexplored: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_theme() -> String {
    "Dracula".to_string()
}

impl SystemConfig {
    /// Default Shannon entropy threshold used to detect packed or compressed binaries.
    pub const DEFAULT_ENTROPY_THRESHOLD: f32 = 7.4;
}

fn default_entropy_threshold() -> f32 {
    SystemConfig::DEFAULT_ENTROPY_THRESHOLD
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            open_last_project: true,
            last_project_path: None,
            theme: default_theme(),
            sync_blocks_view: true,
            sync_hex_dump: true,
            sync_charset_view: false,
            sync_sprites_view: false,
            sync_bitmap_view: false,
            entropy_threshold: Self::DEFAULT_ENTROPY_THRESHOLD,
            config_path_override: None,
            recent_projects: Vec::new(),
            check_for_updates: true,
            default_is_unexplored: true,
        }
    }
}

impl SystemConfig {
    /// Inserts a `.regen2000proj` path at the front of `recent_projects` and caps the list at 20 entries.
    pub fn add_recent_project(&mut self, path: PathBuf) {
        if path.extension().is_none_or(|ext| ext != "regen2000proj") {
            return;
        }
        let canon = std::fs::canonicalize(&path).unwrap_or(path);
        self.recent_projects.retain(|p| p != &canon);
        self.recent_projects.insert(0, canon);
        self.recent_projects.truncate(20);
    }

    /// Removes `path` from `recent_projects`, matching both the raw and canonicalized path.
    pub fn remove_recent_project(&mut self, path: &Path) {
        let canon = std::fs::canonicalize(path).ok();
        self.recent_projects
            .retain(|p| p.as_path() != path && canon.as_ref().is_none_or(|c| p != c));
    }

    /// Removes any entry from `recent_projects` that does not have the `.regen2000proj` extension.
    pub fn clean_recent_projects(&mut self) {
        self.recent_projects
            .retain(|p| p.extension().is_some_and(|ext| ext == "regen2000proj"));
    }

    /// Loads the user configuration from disk, or returns [`SystemConfig::default`] when no valid file exists.
    #[must_use]
    pub fn load() -> Self {
        let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") else {
            return Self::default();
        };
        let config_dir = proj_dirs.config_dir();

        // Read config.toml first.
        let toml_path = config_dir.join("config.toml");
        if toml_path.exists()
            && let Ok(data) = std::fs::read_to_string(&toml_path)
        {
            match toml::from_str::<Self>(&data) {
                Ok(mut config) => {
                    config.clean_recent_projects();
                    return config;
                }
                Err(e) => {
                    let backup_path = toml_path.with_extension("toml.bak");
                    let _ = std::fs::copy(&toml_path, &backup_path);
                    log::error!(
                        "Failed to parse config file: {}. Backed up to {:?}. Error: {}",
                        toml_path.display(),
                        backup_path,
                        e
                    );
                }
            }
        }

        // Fall back to legacy config.json and migrate it to config.toml.
        let json_path = config_dir.join("config.json");
        if json_path.exists()
            && let Ok(data) = std::fs::read_to_string(&json_path)
        {
            match serde_json::from_str::<Self>(&data) {
                Ok(mut config) => {
                    config.clean_recent_projects();
                    if config.save().is_ok() {
                        let _ = std::fs::remove_file(&json_path);
                        log::info!(
                            "Migrated config from {} to {}",
                            json_path.display(),
                            toml_path.display()
                        );
                    }
                    return config;
                }
                Err(e) => {
                    let backup_path = json_path.with_extension("json.bak");
                    let _ = std::fs::copy(&json_path, &backup_path);
                    log::error!(
                        "Failed to parse legacy config file: {}. Backed up to {:?}. Error: {}",
                        json_path.display(),
                        backup_path,
                        e
                    );
                }
            }
        }

        Self::default()
    }

    /// Saves the configuration as TOML to `config_path_override` or the default user config directory.
    ///
    /// # Errors
    ///
    /// Returns an error if TOML serialization fails, the parent directory cannot be created,
    /// or the configuration file cannot be written.
    pub fn save(&self) -> anyhow::Result<()> {
        let target_path = if let Some(path) = &self.config_path_override {
            path.clone()
        } else if let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") {
            proj_dirs.config_dir().join("config.toml")
        } else {
            return Ok(());
        };

        if let Some(parent) = target_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let data = toml::to_string_pretty(self)?;
        std::fs::write(target_path, data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recent_projects_filtering_dedup_and_cap() {
        let mut config = SystemConfig::default();

        // Ignore non-project extensions.
        config.add_recent_project(PathBuf::from("/tmp/demo.prg"));
        assert!(config.recent_projects.is_empty());

        // Add 25 valid project paths and verify the 20-item cap and MRU ordering.
        for i in 0..25 {
            config.add_recent_project(PathBuf::from(format!("/tmp/proj_{i}.regen2000proj")));
        }
        assert_eq!(config.recent_projects.len(), 20);
        assert_eq!(
            config.recent_projects[0],
            PathBuf::from("/tmp/proj_24.regen2000proj")
        );

        // Re-adding an existing path moves it to index 0 without duplicating.
        let existing = PathBuf::from("/tmp/proj_10.regen2000proj");
        config.add_recent_project(existing.clone());
        assert_eq!(config.recent_projects.len(), 20);
        assert_eq!(config.recent_projects[0], existing);

        // Removing the path deletes it even when the file does not exist on disk.
        config.remove_recent_project(&existing);
        assert!(!config.recent_projects.contains(&existing));
    }

    #[test]
    fn test_save_to_override_path_and_clean_recent_projects() {
        let temp_dir = std::env::temp_dir().join(format!("r2000_cfg_test_{}", std::process::id()));
        let override_path = temp_dir.join("nested").join("config.toml");

        let config = SystemConfig {
            config_path_override: Some(override_path.clone()),
            recent_projects: vec![
                PathBuf::from("/tmp/valid.regen2000proj"),
                PathBuf::from("/tmp/invalid.prg"),
            ],
            ..SystemConfig::default()
        };

        config.save().unwrap();
        let saved = std::fs::read_to_string(&override_path).unwrap();
        let mut loaded: SystemConfig = toml::from_str(&saved).unwrap();
        loaded.clean_recent_projects();

        assert_eq!(
            loaded.recent_projects,
            vec![PathBuf::from("/tmp/valid.regen2000proj")]
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
