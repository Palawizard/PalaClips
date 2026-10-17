use palaclips_engine::EngineConfig;
use palaclips_engine::library::{ClipSummary, disk_space, recent_clips};
use serde::Serialize;
use tauri::State;

use crate::{AppStatus, EngineSlot};

#[tauri::command]
pub fn engine_version() -> &'static str {
    palaclips_engine::VERSION
}

#[tauri::command]
pub fn default_config() -> EngineConfig {
    EngineConfig::default()
}

#[tauri::command]
pub fn get_state(slot: State<'_, EngineSlot>) -> Result<AppStatus, String> {
    slot.status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| "état du moteur illisible".to_owned())
}

#[tauri::command]
pub fn get_config(slot: State<'_, EngineSlot>) -> Result<EngineConfig, String> {
    slot.config
        .lock()
        .map(|config| config.clone())
        .map_err(|_| "réglages illisibles".to_owned())
}

#[tauri::command]
pub fn replay_arm(slot: State<'_, EngineSlot>) -> Result<(), String> {
    slot.handle.arm().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn replay_disarm(slot: State<'_, EngineSlot>) -> Result<(), String> {
    slot.handle.disarm().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn replay_save(slot: State<'_, EngineSlot>) -> Result<(), String> {
    slot.handle.save_replay().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_recent_clips(
    slot: State<'_, EngineSlot>,
    limit: usize,
) -> Result<Vec<ClipSummary>, String> {
    let root = slot.clips_root().map_err(|error| error.to_string())?;
    Ok(recent_clips(&root, limit))
}

#[derive(Debug, Serialize)]
pub struct StorageStatus {
    clips_dir: String,
    free_bytes: u64,
    total_bytes: u64,
}

#[tauri::command]
pub async fn storage_status(slot: State<'_, EngineSlot>) -> Result<StorageStatus, String> {
    let root = slot.clips_root().map_err(|error| error.to_string())?;
    let space = disk_space(&root).map_err(|error| error.to_string())?;
    Ok(StorageStatus {
        clips_dir: root.to_string_lossy().into_owned(),
        free_bytes: space.free_bytes,
        total_bytes: space.total_bytes,
    })
}
