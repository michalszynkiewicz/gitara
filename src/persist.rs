//! Per-window settings + per-repo state. JSON in
//! `directories::ProjectDirs::from("dev","ordo","Ordo").config_dir()`.

use serde::{Deserialize, Serialize};
use crate::theme::ThemeMode;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub sidebar_collapsed: bool,
    pub inspector_w: Option<f64>,
    pub inspector_collapsed: bool,
    pub recent_repos: Vec<String>,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::path()?;
        if !path.exists() { return Ok(Self::default()); }
        let s = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&s)?)
    }

    #[allow(dead_code)] // wired up once we persist runtime settings (theme, sidebar, show_all_refs).
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    fn path() -> anyhow::Result<std::path::PathBuf> {
        let dirs = directories::ProjectDirs::from("dev", "ordo", "Ordo")
            .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        Ok(dirs.config_dir().join("settings.json"))
    }
}
