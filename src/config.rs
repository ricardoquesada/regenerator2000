use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub open_last_project: bool,
    pub last_project_path: Option<PathBuf>,
    #[serde(default = "default_theme")]
    pub theme: String,
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
