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
    pub auto_analyze: bool,
    #[serde(default = "default_true")]
    pub sync_hex_dump: bool,
    #[serde(default = "default_false")]
    pub sync_charset_view: bool,
    #[serde(default = "default_false")]
    pub sync_sprites_view: bool,
    #[serde(default = "default_false")]
    pub sync_bitmap_view: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_theme() -> String {
    "Solarized Dark".to_string()
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            open_last_project: true,
            last_project_path: None,
            theme: "Solarized Dark".to_string(),
            sync_blocks_view: true,
            auto_analyze: true,
            sync_hex_dump: true,
            sync_charset_view: false,
            sync_sprites_view: false,
            sync_bitmap_view: false,
        }
    }
}

impl SystemConfig {
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") {
            let config_path = proj_dirs.config_dir().join("config.json");
            if config_path.exists()
                && let Ok(data) = std::fs::read_to_string(config_path)
                && let Ok(config) = serde_json::from_str(&data)
            {
                return config;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(proj_dirs) = ProjectDirs::from("", "", "regenerator2000") {
            let config_dir = proj_dirs.config_dir();
            std::fs::create_dir_all(config_dir)?;
            let config_path = config_dir.join("config.json");
            let data = serde_json::to_string_pretty(self)?;
            std::fs::write(config_path, data)?;
        }
        Ok(())
    }
}
