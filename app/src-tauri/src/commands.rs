use palaclips_engine::EngineConfig;
use palaclips_engine::encode::{HardwareEncoder, hardware_video_encoders};
use palaclips_engine::engine::ReplayEstimate;
use palaclips_engine::library::{ClipSummary, disk_space, recent_clips};
use serde::Serialize;
use tauri::State;
use tauri_plugin_autostart::ManagerExt;

use crate::hotkeys::{HotkeyAction, Hotkeys};
use crate::settings::{Feedback, Theme};
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
pub fn get_config(slot: State<'_, EngineSlot>) -> EngineConfig {
    slot.config()
}

#[derive(Debug, Serialize)]
pub struct HotkeysView {
    hotkeys: Hotkeys,
    unavailable: Vec<HotkeyAction>,
}

#[tauri::command]
pub fn get_hotkeys(slot: State<'_, EngineSlot>) -> HotkeysView {
    HotkeysView {
        hotkeys: slot.hotkeys(),
        unavailable: slot
            .unavailable
            .lock()
            .map(|unavailable| unavailable.clone())
            .unwrap_or_default(),
    }
}

#[tauri::command]
pub fn suspend_hotkeys(app: tauri::AppHandle, slot: State<'_, EngineSlot>) {
    slot.unregister_hotkeys(&app);
}

#[tauri::command]
pub fn resume_hotkeys(app: tauri::AppHandle, slot: State<'_, EngineSlot>) {
    slot.resume_hotkeys(&app);
}

#[tauri::command]
pub fn set_hotkeys(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
    hotkeys: Hotkeys,
) -> Result<(), String> {
    slot.set_hotkeys(&app, hotkeys)
}

#[tauri::command]
pub fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| format!("Démarrage avec Windows illisible : {error}."))
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(|error| format!("Démarrage avec Windows non modifié : {error}."))
}

#[tauri::command]
pub fn get_feedback(slot: State<'_, EngineSlot>) -> Feedback {
    slot.feedback()
}

#[tauri::command]
pub fn set_feedback(slot: State<'_, EngineSlot>, feedback: Feedback) -> Result<(), String> {
    slot.update(|settings| settings.feedback = feedback)
}

#[tauri::command]
pub async fn set_config(slot: State<'_, EngineSlot>, config: EngineConfig) -> Result<(), String> {
    let current = slot.config();
    if current.video != config.video || current.capture != config.capture {
        let encoders = cached_encoders(&slot)?;
        palaclips_engine::engine::try_video_output(&config, &encoders)
            .map_err(|error| error.to_string())?;
    }
    slot.apply(config)
}

#[tauri::command]
pub fn reset_settings(app: tauri::AppHandle, slot: State<'_, EngineSlot>) -> Result<(), String> {
    slot.reset(&app)
}

#[tauri::command]
pub fn get_theme(slot: State<'_, EngineSlot>) -> Theme {
    slot.theme()
}

#[tauri::command]
pub fn set_theme(slot: State<'_, EngineSlot>, theme: Theme) -> Result<(), String> {
    slot.update(|settings| settings.theme = theme)
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

#[tauri::command]
pub async fn pick_clips_dir(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
) -> Result<Option<String>, String> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    let mut dialog = app
        .dialog()
        .file()
        .set_title("Dossier des clips")
        .set_can_create_directories(true);
    if let Ok(root) = slot.clips_root() {
        dialog = dialog.set_directory(root);
    }
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog
        .blocking_pick_folder()
        .map(|picked| {
            picked
                .into_path()
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| format!("Dossier illisible : {error}."))
        })
        .transpose()
}

#[tauri::command]
pub fn open_clips_dir(slot: State<'_, EngineSlot>) -> Result<(), String> {
    let root = slot.clips_root().map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("Dossier des clips impossible à créer : {error}."))?;
    std::process::Command::new("explorer")
        .arg(&root)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Explorateur impossible à ouvrir : {error}."))
}

#[tauri::command]
pub async fn list_encoders(slot: State<'_, EngineSlot>) -> Result<Vec<HardwareEncoder>, String> {
    cached_encoders(&slot)
}

fn cached_encoders(slot: &EngineSlot) -> Result<Vec<HardwareEncoder>, String> {
    if let Some(encoders) = slot.encoders.lock().ok().and_then(|cache| cache.clone()) {
        return Ok(encoders);
    }
    let encoders = hardware_video_encoders().map_err(|error| error.to_string())?;
    if let Ok(mut cache) = slot.encoders.lock() {
        *cache = Some(encoders.clone());
    }
    Ok(encoders)
}

#[derive(Debug, Serialize)]
pub struct MemoryStatus {
    total_bytes: u64,
    available_bytes: u64,
}

#[tauri::command]
pub fn memory_status() -> Result<MemoryStatus, String> {
    let (total_bytes, available_bytes) = palaclips_engine::paths::physical_memory()
        .ok_or_else(|| "Mémoire du PC illisible.".to_owned())?;
    Ok(MemoryStatus {
        total_bytes,
        available_bytes,
    })
}

#[tauri::command]
pub async fn estimate_replay(config: EngineConfig) -> Result<ReplayEstimate, String> {
    palaclips_engine::engine::estimate_replay(&config).map_err(|error| error.to_string())
}
