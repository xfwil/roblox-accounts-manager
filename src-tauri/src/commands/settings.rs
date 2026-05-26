use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Web API server port (0 = disabled)
    pub api_port: u16,
    /// Nexus WebSocket port (0 = disabled)
    pub nexus_port: u16,
    /// Theme name
    pub theme: String,
    /// Auto-refresh interval in minutes (0 = disabled)
    pub auto_refresh_minutes: u32,
    /// Enable watcher (auto-detect dead processes)
    pub watcher_enabled: bool,
    /// Watcher poll interval in seconds
    pub watcher_interval_seconds: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_port: 7963,
            nexus_port: 7964,
            theme: "dark".into(),
            auto_refresh_minutes: 0,
            watcher_enabled: true,
            watcher_interval_seconds: 30,
        }
    }
}

/// Settings file path state
pub struct SettingsPath(pub PathBuf);

/// Get current settings
#[tauri::command]
pub async fn get_settings(
    settings_path: State<'_, SettingsPath>,
) -> AppResult<AppSettings> {
    let path = &settings_path.0;
    if path.exists() {
        let data = std::fs::read_to_string(path)?;
        let settings: AppSettings = serde_json::from_str(&data)
            .map_err(|e| AppError::Other(format!("Invalid settings: {}", e)))?;
        Ok(settings)
    } else {
        Ok(AppSettings::default())
    }
}

/// Save settings
#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    settings_path: State<'_, SettingsPath>,
) -> AppResult<()> {
    let data = serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::Other(format!("Serialization error: {}", e)))?;
    std::fs::write(&settings_path.0, data)?;
    Ok(())
}

/// Get available themes
#[tauri::command]
pub async fn get_themes() -> AppResult<Vec<String>> {
    Ok(vec![
        "dark".into(),
        "light".into(),
        "midnight".into(),
        "cyberpunk".into(),
    ])
}
