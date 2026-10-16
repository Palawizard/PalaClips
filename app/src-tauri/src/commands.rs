use palaclips_engine::EngineConfig;
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
