use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

/// User settings, persisted in `%APPDATA%\Sonixx\settings.json` on Windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub tray: bool,
    pub startup: bool,
    pub theme: String,
    pub master_volume: f32,
    pub monitor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tray: true,
            startup: false,
            theme: "dark".to_string(),
            master_volume: 1.0,
            monitor: false,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join("Sonixx").join("settings.json")
        } else {
            PathBuf::from("settings.json")
        }
    }

    pub fn load() -> Self {
        fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, text)
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();
        assert!(settings.tray);
        assert!(!settings.startup);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.master_volume, 1.0);
    }

    #[test]
    fn round_trips_through_json() {
        let settings = Settings {
            tray: false,
            startup: true,
            theme: "dark".to_string(),
            master_volume: 1.4,
            monitor: true,
        };
        let text = serde_json::to_string(&settings).expect("serialize");
        let parsed: Settings = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(parsed.master_volume, 1.4);
        assert!(parsed.monitor);
        assert!(parsed.startup);
    }
}
