use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};

/// A recent history entry (Place ID or PS Link)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub value: String,
    pub label: String,
    pub timestamp: u64,
}

/// A named preset containing Place ID + PS Link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinPreset {
    pub id: String,
    pub name: String,
    pub place_id: String,
    pub ps_link: String,
    pub created_at: u64,
}

/// All join-related persistent data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JoinData {
    pub recent_place_ids: Vec<RecentEntry>,
    pub recent_ps_links: Vec<RecentEntry>,
    pub presets: Vec<JoinPreset>,
}

const MAX_RECENT: usize = 10;

/// Path to join_data.json
pub struct JoinDataPath(pub PathBuf);

fn load_join_data(path: &PathBuf) -> JoinData {
    if path.exists() {
        let data = std::fs::read_to_string(path).unwrap_or_default();
        match serde_json::from_str::<JoinData>(&data) {
            Ok(parsed) => return parsed,
            Err(e) => {
                eprintln!("[presets] Failed to parse {:?}: {}", path, e);
            }
        }
    }

    // Primary failed or doesn't exist — try backup
    let bak_path = path.with_extension("json.bak");
    if bak_path.exists() {
        let data = std::fs::read_to_string(&bak_path).unwrap_or_default();
        match serde_json::from_str::<JoinData>(&data) {
            Ok(parsed) => {
                eprintln!("[presets] Recovered from backup {:?}", bak_path);
                // Restore primary from backup
                let _ = std::fs::copy(&bak_path, path);
                return parsed;
            }
            Err(e) => {
                eprintln!("[presets] Backup also corrupt {:?}: {}", bak_path, e);
            }
        }
    }

    JoinData::default()
}

fn save_join_data(path: &PathBuf, data: &JoinData) -> AppResult<()> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| AppError::Other(format!("Serialization error: {}", e)))?;

    // Create backup of current file before overwriting
    if path.exists() {
        let bak_path = path.with_extension("json.bak");
        let _ = std::fs::copy(path, &bak_path);
    }

    // Atomic write: write to .tmp then rename
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)?;
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}

// ─── Recent History Commands ───

#[tauri::command]
pub async fn get_recent_place_ids(
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let data = load_join_data(&join_data_path.0);
    Ok(data.recent_place_ids)
}

#[tauri::command]
pub async fn get_recent_ps_links(
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let data = load_join_data(&join_data_path.0);
    Ok(data.recent_ps_links)
}

#[tauri::command]
pub async fn add_recent_place_id(
    value: String,
    label: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let mut data = load_join_data(&join_data_path.0);
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Ok(data.recent_place_ids);
    }

    // Remove duplicate
    data.recent_place_ids.retain(|e| e.value.to_lowercase() != trimmed.to_lowercase());

    // Insert at front
    data.recent_place_ids.insert(0, RecentEntry {
        value: trimmed,
        label,
        timestamp: now_millis(),
    });

    // Trim to max
    data.recent_place_ids.truncate(MAX_RECENT);
    save_join_data(&join_data_path.0, &data)?;
    Ok(data.recent_place_ids)
}

#[tauri::command]
pub async fn add_recent_ps_link(
    value: String,
    label: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let mut data = load_join_data(&join_data_path.0);
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Ok(data.recent_ps_links);
    }

    data.recent_ps_links.retain(|e| e.value.to_lowercase() != trimmed.to_lowercase());
    data.recent_ps_links.insert(0, RecentEntry {
        value: trimmed,
        label,
        timestamp: now_millis(),
    });
    data.recent_ps_links.truncate(MAX_RECENT);
    save_join_data(&join_data_path.0, &data)?;
    Ok(data.recent_ps_links)
}

#[tauri::command]
pub async fn remove_recent_place_id(
    value: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let mut data = load_join_data(&join_data_path.0);
    data.recent_place_ids.retain(|e| e.value.to_lowercase() != value.to_lowercase());
    save_join_data(&join_data_path.0, &data)?;
    Ok(data.recent_place_ids)
}

#[tauri::command]
pub async fn remove_recent_ps_link(
    value: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<RecentEntry>> {
    let mut data = load_join_data(&join_data_path.0);
    data.recent_ps_links.retain(|e| e.value.to_lowercase() != value.to_lowercase());
    save_join_data(&join_data_path.0, &data)?;
    Ok(data.recent_ps_links)
}

// ─── Preset Commands ───

#[tauri::command]
pub async fn get_presets(
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<JoinPreset>> {
    let data = load_join_data(&join_data_path.0);
    Ok(data.presets)
}

#[tauri::command]
pub async fn add_preset(
    name: String,
    place_id: String,
    ps_link: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<JoinPreset>> {
    let mut data = load_join_data(&join_data_path.0);

    let id = format!("{:016x}", rand::random::<u64>());
    data.presets.push(JoinPreset {
        id,
        name,
        place_id,
        ps_link,
        created_at: now_millis(),
    });

    save_join_data(&join_data_path.0, &data)?;
    Ok(data.presets)
}

#[tauri::command]
pub async fn update_preset(
    id: String,
    name: String,
    place_id: String,
    ps_link: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<JoinPreset>> {
    let mut data = load_join_data(&join_data_path.0);

    if let Some(preset) = data.presets.iter_mut().find(|p| p.id == id) {
        preset.name = name;
        preset.place_id = place_id;
        preset.ps_link = ps_link;
    } else {
        return Err(AppError::Other(format!("Preset not found: {}", id)));
    }

    save_join_data(&join_data_path.0, &data)?;
    Ok(data.presets)
}

#[tauri::command]
pub async fn remove_preset(
    id: String,
    join_data_path: State<'_, JoinDataPath>,
) -> AppResult<Vec<JoinPreset>> {
    let mut data = load_join_data(&join_data_path.0);
    data.presets.retain(|p| p.id != id);
    save_join_data(&join_data_path.0, &data)?;
    Ok(data.presets)
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
