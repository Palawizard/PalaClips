use palaclips_engine::EngineConfig;
use palaclips_engine::calibrate::Calibration;
use palaclips_engine::capture::{PreviewCapture, TargetInfo, list_targets};
use palaclips_engine::config::CaptureTarget;
use palaclips_engine::encode::{HardwareEncoder, hardware_video_encoders};
use palaclips_engine::engine::ReplayEstimate;
use palaclips_engine::events::Localized;
use palaclips_engine::library::{ClipSummary, disk_space, recent_clips};
use palaclips_engine::sources::{AppSession, AudioDevice, app_sessions, audio_devices};
use serde::Serialize;
use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody, Response};
use tauri_plugin_autostart::ManagerExt;

use crate::hotkeys::{HotkeyAction, Hotkeys};
use crate::i18n::{self, Language, LanguageSetting};
use crate::settings::{Feedback, Theme};
use crate::{AppStatus, EngineSlot, internal_error};

#[tauri::command]
pub fn engine_version() -> &'static str {
    palaclips_engine::VERSION
}

#[tauri::command]
pub fn default_config() -> EngineConfig {
    EngineConfig::default()
}

#[tauri::command]
pub fn get_state(slot: State<'_, EngineSlot>) -> Result<AppStatus, Localized> {
    slot.status
        .lock()
        .map(|status| status.clone())
        .map_err(|_| internal_error())
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
) -> Result<(), Localized> {
    slot.set_hotkeys(&app, hotkeys)
}

#[tauri::command]
pub fn get_autostart(app: tauri::AppHandle) -> Result<bool, Localized> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| Localized::new("autostart_unreadable").with("detail", error.to_string()))
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), Localized> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
    .map_err(|error| Localized::new("autostart_not_changed").with("detail", error.to_string()))
}

#[tauri::command]
pub fn get_feedback(slot: State<'_, EngineSlot>) -> Feedback {
    slot.feedback()
}

#[tauri::command]
pub fn set_feedback(slot: State<'_, EngineSlot>, feedback: Feedback) -> Result<(), Localized> {
    slot.update(|settings| settings.feedback = feedback)
}

#[tauri::command]
pub async fn set_config(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
    config: EngineConfig,
) -> Result<(), Localized> {
    let current = slot.config();
    if current.video != config.video || current.capture != config.capture {
        let encoders = cached_encoders(&slot)?;
        palaclips_engine::engine::try_video_output(&config, &encoders)?;
    }
    slot.apply(&app, config)
}

#[tauri::command]
pub fn reset_settings(app: tauri::AppHandle, slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    slot.reset(&app)
}

#[tauri::command]
pub fn get_theme(slot: State<'_, EngineSlot>) -> Theme {
    slot.theme()
}

#[tauri::command]
pub fn set_theme(slot: State<'_, EngineSlot>, theme: Theme) -> Result<(), Localized> {
    slot.update(|settings| settings.theme = theme)
}

#[derive(Debug, Serialize)]
pub struct LanguageView {
    setting: LanguageSetting,
    resolved: Language,
}

#[tauri::command]
pub fn get_language(slot: State<'_, EngineSlot>) -> LanguageView {
    let setting = slot
        .settings
        .lock()
        .map(|settings| settings.language)
        .unwrap_or_default();
    LanguageView {
        setting,
        resolved: setting.resolve(),
    }
}

#[tauri::command]
pub fn set_language(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
    language: LanguageSetting,
) -> Result<Language, Localized> {
    slot.set_language(&app, language)
}

#[tauri::command]
pub fn replay_arm(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    Ok(slot.handle.arm()?)
}

#[tauri::command]
pub fn replay_disarm(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    Ok(slot.handle.disarm()?)
}

#[tauri::command]
pub fn replay_save(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    Ok(slot.handle.save_replay()?)
}

#[tauri::command]
pub fn record_start(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    Ok(slot.handle.start_recording()?)
}

#[tauri::command]
pub fn record_stop(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    Ok(slot.handle.stop_recording()?)
}

#[tauri::command]
pub async fn list_clips(
    slot: State<'_, EngineSlot>,
    query: palaclips_engine::library::ClipQuery,
) -> Result<Vec<palaclips_engine::library::Clip>, Localized> {
    slot.list_clips(&query)
}

#[tauri::command]
pub async fn clip_games(slot: State<'_, EngineSlot>) -> Result<Vec<String>, Localized> {
    slot.clip_games()
}

#[tauri::command]
pub async fn set_clip_favorite(
    slot: State<'_, EngineSlot>,
    path: String,
    favorite: bool,
) -> Result<(), Localized> {
    slot.set_clip_favorite(&path, favorite)
}

#[tauri::command]
pub async fn rename_clip(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
    path: String,
    name: String,
) -> Result<String, Localized> {
    slot.rename_clip(&app, &path, &name)
}

#[tauri::command]
pub async fn trash_clip(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
    path: String,
) -> Result<(), Localized> {
    slot.trash_clip(&app, &path)
}

#[tauri::command]
pub fn reveal_clip(slot: State<'_, EngineSlot>, path: String) -> Result<(), Localized> {
    slot.reveal_clip(&path)
}

#[tauri::command]
pub async fn quota_preview(
    slot: State<'_, EngineSlot>,
    quota_gb: u32,
) -> Result<crate::clips::QuotaPreview, Localized> {
    slot.quota_preview(quota_gb)
}

#[tauri::command]
pub async fn prepare_player(
    slot: State<'_, EngineSlot>,
    path: String,
) -> Result<Vec<palaclips_engine::library::TrackFile>, Localized> {
    slot.prepare_player(&path)
}

#[tauri::command]
pub fn copy_clip(slot: State<'_, EngineSlot>, path: String) -> Result<(), Localized> {
    slot.copy_clip(&path)
}

#[tauri::command]
pub async fn list_recent_clips(
    slot: State<'_, EngineSlot>,
    limit: usize,
) -> Result<Vec<ClipSummary>, Localized> {
    let root = slot.clips_root()?;
    Ok(recent_clips(&root, limit))
}

#[derive(Debug, Serialize)]
pub struct StorageStatus {
    clips_dir: String,
    free_bytes: u64,
    total_bytes: u64,
}

#[tauri::command]
pub async fn storage_status(slot: State<'_, EngineSlot>) -> Result<StorageStatus, Localized> {
    let root = slot.clips_root()?;
    let space = disk_space(&root)?;
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
) -> Result<Option<String>, Localized> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    let mut dialog = app
        .dialog()
        .file()
        .set_title(i18n::text(slot.language(), "dialog.clipsDir"))
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
                .map_err(|error| {
                    Localized::new("folder_unreadable").with("detail", error.to_string())
                })
        })
        .transpose()
}

#[tauri::command]
pub fn open_clips_dir(slot: State<'_, EngineSlot>) -> Result<(), Localized> {
    let root = slot.clips_root()?;
    std::fs::create_dir_all(&root).map_err(|error| {
        Localized::new("clips_dir_not_created").with("detail", error.to_string())
    })?;
    std::process::Command::new("explorer")
        .arg(&root)
        .spawn()
        .map(|_| ())
        .map_err(|error| Localized::new("explorer_failed").with("detail", error.to_string()))
}

#[tauri::command]
pub async fn calibrate_audio(slot: State<'_, EngineSlot>) -> Result<Calibration, Localized> {
    let handle = slot.handle.clone();
    tauri::async_runtime::spawn_blocking(move || handle.calibrate())
        .await
        .map_err(|_| internal_error())?
        .map_err(|error| Localized::new("calibration_failed").with("reason", error.localized()))
}

#[tauri::command]
pub async fn list_audio_devices(input: bool) -> Result<Vec<AudioDevice>, Localized> {
    Ok(audio_devices(input)?)
}

#[tauri::command]
pub async fn list_app_sessions() -> Result<Vec<AppSession>, Localized> {
    Ok(app_sessions()?)
}

#[derive(Debug, Serialize)]
pub struct PickedApp {
    exe_name: String,
    app_name: String,
}

#[tauri::command]
pub async fn pick_app_exe(
    app: tauri::AppHandle,
    slot: State<'_, EngineSlot>,
) -> Result<Option<PickedApp>, Localized> {
    use tauri::Manager;
    use tauri_plugin_dialog::DialogExt;

    let language = slot.language();
    let mut dialog = app
        .dialog()
        .file()
        .set_title(i18n::text(language, "dialog.pickApp"))
        .add_filter(i18n::text(language, "dialog.applications"), &["exe"]);
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    let Some(picked) = dialog.blocking_pick_file() else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|error| Localized::new("file_unreadable").with("detail", error.to_string()))?;
    let exe_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| Localized::new("not_an_application"))?;
    Ok(Some(PickedApp {
        app_name: palaclips_engine::games::app_name(&path, &exe_name),
        exe_name,
    }))
}

#[tauri::command]
pub async fn list_capture_targets() -> Result<Vec<TargetInfo>, Localized> {
    Ok(list_targets()?)
}

#[tauri::command]
pub async fn app_icon(process_id: u32) -> Result<Response, Localized> {
    palaclips_engine::capture::app_icon(process_id)
        .map(Response::new)
        .ok_or_else(|| Localized::new("icon_missing"))
}

#[tauri::command]
pub async fn start_preview(
    slot: State<'_, EngineSlot>,
    target: CaptureTarget,
    channel: Channel<InvokeResponseBody>,
) -> Result<u32, Localized> {
    let preview = PreviewCapture::start(
        &target,
        Box::new(move |jpeg| {
            if let Err(error) = channel.send(InvokeResponseBody::Raw(jpeg)) {
                tracing::debug!(%error, "aperçu non transmis");
            }
        }),
    )?;
    slot.add_preview(preview)
}

#[tauri::command]
pub fn stop_preview(slot: State<'_, EngineSlot>, id: u32) {
    slot.stop_preview(id);
}

#[tauri::command]
pub fn stop_previews(slot: State<'_, EngineSlot>) {
    slot.stop_previews();
}

#[tauri::command]
pub async fn list_encoders(slot: State<'_, EngineSlot>) -> Result<Vec<HardwareEncoder>, Localized> {
    cached_encoders(&slot)
}

fn cached_encoders(slot: &EngineSlot) -> Result<Vec<HardwareEncoder>, Localized> {
    if let Some(encoders) = slot.encoders.lock().ok().and_then(|cache| cache.clone()) {
        return Ok(encoders);
    }
    let encoders = hardware_video_encoders()?;
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
pub fn memory_status() -> Result<MemoryStatus, Localized> {
    let (total_bytes, available_bytes) = palaclips_engine::paths::physical_memory()
        .ok_or_else(|| Localized::new("memory_unreadable"))?;
    Ok(MemoryStatus {
        total_bytes,
        available_bytes,
    })
}

#[tauri::command]
pub async fn estimate_replay(config: EngineConfig) -> Result<ReplayEstimate, Localized> {
    Ok(palaclips_engine::engine::estimate_replay(&config)?)
}
