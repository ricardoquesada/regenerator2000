use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub open_last_project: bool,
    pub last_project_path: Option<PathBuf>,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_true")]
    pub sync_blocks_view: bool,
    #[serde(default = "default_true")]
    pub sync_hex_dump: bool,
    #[serde(default = "default_false")]
    pub sync_charset_view: bool,
    #[serde(default = "default_false")]
    pub sync_sprites_view: bool,
    #[serde(default = "default_false")]
    pub sync_bitmap_view: bool,
    #[serde(default = "default_entropy_threshold")]
    pub entropy_threshold: f32,
    #[serde(skip)]
    pub config_path_override: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default = "default_true")]
    pub check_for_updates: bool,
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

fn default_entropy_threshold() -> f32 {
    7.5
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            open_last_project: true,
            last_project_path: None,
            theme: "Dracula".to_string(),
            sync_blocks_view: true,
            sync_hex_dump: true,
            sync_charset_view: false,
            sync_sprites_view: false,
            sync_bitmap_view: false,
            entropy_threshold: 7.5,
            config_path_override: None,
            recent_projects: Vec::new(),
            check_for_updates: true,
            default_is_unexplored: true,
        }
    }
}

impl SystemConfig {
    pub fn add_recent_project(&mut self, path: PathBuf) {
        if path.extension().is_none_or(|ext| ext != "regen2000proj") {
            return;
        }
        let canon = std::fs::canonicalize(&path).unwrap_or(path);
        self.recent_projects.retain(|p| p != &canon);
        self.recent_projects.insert(0, canon);
        self.recent_projects.truncate(20);
    }

    pub fn remove_recent_project(&mut self, path: &std::path::Path) {
        let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent_projects.retain(|p| p != &canon);
    }

    pub fn clean_recent_projects(&mut self) {
        self.recent_projects
            .retain(|p| p.extension().is_some_and(|ext| ext == "regen2000proj"));
    }

    #[must_use]
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") {
            let config_dir = proj_dirs.config_dir();

            // Try config.toml first (preferred format).
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

            // Fall back to legacy config.json and migrate.
            let json_path = config_dir.join("config.json");
            if json_path.exists()
                && let Ok(data) = std::fs::read_to_string(&json_path)
            {
                match serde_json::from_str::<Self>(&data) {
                    Ok(mut config) => {
                        config.clean_recent_projects();
                        // Migrate: save as TOML and remove the old JSON file.
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
        }
        Self::default()
    }

    /// Saves the configuration to the user's config directory or override path.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be created or the file cannot be written.
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(path) = &self.config_path_override {
            let data = toml::to_string_pretty(self)?;
            std::fs::write(path, data)?;
            return Ok(());
        }
        if let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") {
            let config_dir = proj_dirs.config_dir();
            std::fs::create_dir_all(config_dir)?;
            let config_path = config_dir.join("config.toml");
            let data = toml::to_string_pretty(self)?;
            std::fs::write(config_path, data)?;
        }
        Ok(())
    }
}
