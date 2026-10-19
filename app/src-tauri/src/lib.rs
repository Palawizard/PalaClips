mod clips;
mod commands;
mod feedback;
mod hotkeys;
mod i18n;
mod settings;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use palaclips_engine::EngineConfig;
use palaclips_engine::engine::{Engine, EngineHandle, LevelRate, track_descriptors};
use palaclips_engine::events::{
    EngineEvent, EngineState, Localized, Severity, TrackDescriptor, TrackKind, TrackState,
};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::hotkeys::{HotkeyAction, Hotkeys};
use crate::i18n::Language;

pub const MAIN_WINDOW: &str = "main";
pub const ENGINE_EVENT: &str = "engine-event";
const MAX_PREVIEWS: usize = 12;
pub const AUTOSTART_ARG: &str = "--autostart";

#[derive(Debug, Clone, Serialize)]
pub struct ActiveAlert {
    pub severity: Severity,
    #[serde(flatten)]
    pub text: Localized,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureInfo {
    pub source: Localized,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub title: String,
    pub kind: TrackKind,
    pub state: Option<TrackState>,
    pub source: Option<String>,
}

impl TrackInfo {
    fn named(descriptor: TrackDescriptor) -> Self {
        Self {
            title: descriptor.title,
            kind: descriptor.kind,
            state: None,
            source: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AppStatus {
    pub state: EngineState,
    pub buffered_secs: f64,
    pub buffer_bytes: u64,
    pub recording_secs: f64,
    pub recording_bytes: u64,
    pub capture: Option<CaptureInfo>,
    pub tracks: Vec<TrackInfo>,
    pub alerts: Vec<ActiveAlert>,
}

impl AppStatus {
    fn apply(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::StateChanged { state } => {
                self.state = *state;
                if *state != EngineState::Recording {
                    self.recording_secs = 0.0;
                    self.recording_bytes = 0;
                }
                if *state != EngineState::Idle {
                    self.alerts.retain(|alert| {
                        alert.text.code != "replay_arm_failed"
                            && alert.text.code != "replay_stopped"
                    });
                }
                if *state == EngineState::Idle {
                    self.buffered_secs = 0.0;
                    self.buffer_bytes = 0;
                    self.capture = None;
                    for track in &mut self.tracks {
                        track.state = None;
                        track.source = None;
                    }
                }
            }
            EngineEvent::CaptureStarted {
                source,
                width,
                height,
                fps,
                tracks,
            } => {
                self.capture = Some(CaptureInfo {
                    source: source.clone(),
                    width: *width,
                    height: *height,
                    fps: *fps,
                });
                self.tracks = tracks.iter().cloned().map(TrackInfo::named).collect();
            }
            EngineEvent::TracksRenamed { tracks } => {
                for (track, renamed) in self.tracks.iter_mut().zip(tracks) {
                    track.title = renamed.title.clone();
                }
            }
            EngineEvent::TrackChanged {
                track,
                state,
                source,
                ..
            } => {
                if let Some(track) = self.tracks.get_mut(*track) {
                    track.state = Some(*state);
                    track.source = source.clone();
                }
            }
            EngineEvent::ReplayStats {
                buffered_secs,
                bytes,
            } => {
                self.buffered_secs = *buffered_secs;
                self.buffer_bytes = *bytes;
            }
            EngineEvent::RecordingStats {
                elapsed_secs,
                bytes,
            } => {
                self.recording_secs = *elapsed_secs;
                self.recording_bytes = *bytes;
            }
            EngineEvent::ClipSaved { .. } => {
                self.alerts
                    .retain(|alert| alert.text.code != "clip_too_short");
            }
            EngineEvent::Alert {
                severity,
                text,
                message,
            } => {
                self.alerts.retain(|alert| alert.text.code != text.code);
                self.alerts.push(ActiveAlert {
                    severity: *severity,
                    text: text.clone(),
                    message: message.clone(),
                });
            }
            _ => {}
        }
    }
}

#[derive(Debug)]
pub struct EngineSlot {
    settings_path: Option<std::path::PathBuf>,
    settings_writable: bool,
    settings: Mutex<settings::Settings>,
    shortcuts: Mutex<Vec<(HotkeyAction, Shortcut)>>,
    encoders: Mutex<Option<Vec<palaclips_engine::encode::HardwareEncoder>>>,
    previews: Mutex<HashMap<u32, palaclips_engine::capture::PreviewCapture>>,
    next_preview: AtomicU32,
    performance_panel: AtomicBool,
    unavailable: Mutex<Vec<HotkeyAction>>,
    engine: Mutex<Option<Engine>>,
    handle: EngineHandle,
    status: Arc<Mutex<AppStatus>>,
    library: Mutex<Option<palaclips_engine::library::Library>>,
    library_watch: Mutex<Option<palaclips_engine::library::LibraryWatch>>,
    track_cache: Mutex<Option<palaclips_engine::library::TrackCache>>,
    extracting: Mutex<()>,
    exports: Mutex<HashMap<u32, Arc<std::sync::atomic::AtomicBool>>>,
    next_export: AtomicU32,
}

impl EngineSlot {
    fn start(
        app: &tauri::AppHandle,
        settings: settings::Settings,
        settings_path: Option<std::path::PathBuf>,
        settings_writable: bool,
    ) -> palaclips_engine::Result<Self> {
        let mut config = settings.engine.clone();
        config.audio.default_names = settings::track_names(settings.language.resolve());
        let arm = config.replay.arm_on_startup;
        let engine = Engine::start(config.clone())?;
        let handle = engine.handle();
        let status = Arc::new(Mutex::new(AppStatus {
            tracks: track_descriptors(&config)
                .into_iter()
                .map(TrackInfo::named)
                .collect(),
            ..AppStatus::default()
        }));
        let events = engine.events().clone();
        let relay_status = status.clone();
        let app = app.clone();
        std::thread::Builder::new()
            .name("palaclips-events".into())
            .spawn(move || {
                for event in events.iter() {
                    if let Some(slot) = app.try_state::<EngineSlot>() {
                        slot.react(&app, &event);
                    }
                    if let Ok(mut status) = relay_status.lock() {
                        status.apply(&event);
                    }
                    app.emit(ENGINE_EVENT, &event).ok();
                }
            })?;
        if arm {
            handle.arm()?;
        }
        Ok(Self {
            settings_path,
            settings_writable,
            settings: Mutex::new(settings),
            shortcuts: Mutex::new(Vec::new()),
            encoders: Mutex::new(None),
            previews: Mutex::new(HashMap::new()),
            next_preview: AtomicU32::new(1),
            performance_panel: AtomicBool::new(false),
            unavailable: Mutex::new(Vec::new()),
            engine: Mutex::new(Some(engine)),
            handle,
            status,
            library: Mutex::new(None),
            library_watch: Mutex::new(None),
            track_cache: Mutex::new(None),
            extracting: Mutex::new(()),
            exports: Mutex::new(HashMap::new()),
            next_export: AtomicU32::new(1),
        })
    }

    fn config(&self) -> EngineConfig {
        self.settings
            .lock()
            .map(|settings| settings.engine.clone())
            .unwrap_or_default()
    }

    fn feedback(&self) -> settings::Feedback {
        self.settings
            .lock()
            .map(|settings| settings.feedback)
            .unwrap_or_default()
    }

    fn state(&self) -> EngineState {
        self.status
            .lock()
            .map(|status| status.state)
            .unwrap_or_default()
    }

    fn language(&self) -> Language {
        self.settings
            .lock()
            .map(|settings| settings.language)
            .unwrap_or_default()
            .resolve()
    }

    fn update(&self, change: impl FnOnce(&mut settings::Settings)) -> Result<(), Localized> {
        if !self.settings_writable {
            return Err(Localized::new("settings_read_only"));
        }
        let mut current = self.settings.lock().map_err(|_| internal_error())?;
        let mut changed = current.clone();
        change(&mut changed);
        if let Some(path) = &self.settings_path {
            settings::save(path, &changed).map_err(|error| {
                Localized::new("settings_not_saved").with("detail", error.to_string())
            })?;
        }
        *current = changed;
        Ok(())
    }

    fn apply(&self, app: &tauri::AppHandle, mut config: EngineConfig) -> Result<(), Localized> {
        config.audio.default_names = settings::track_names(self.language());
        config.validate()?;
        let previous = self.config().storage;
        self.update(|settings| settings.engine = config.clone())?;
        let storage = config.storage.clone();
        self.handle.configure(config)?;
        if previous.clips_dir != storage.clips_dir {
            clips::start_library(app);
        } else if previous.quota_gb != storage.quota_gb {
            self.library_changed(app, &[]);
        }
        Ok(())
    }

    fn set_language(
        &self,
        app: &tauri::AppHandle,
        language: i18n::LanguageSetting,
    ) -> Result<Language, Localized> {
        let resolved = language.resolve();
        let mut config = self.config();
        config.audio.default_names = settings::track_names(resolved);
        self.update(|settings| {
            settings.language = language;
            settings.engine = config.clone();
        })?;
        self.handle.configure(config)?;
        refresh_tray(app, resolved, self.state() == EngineState::Recording);
        Ok(resolved)
    }

    fn hotkeys(&self) -> Hotkeys {
        self.settings
            .lock()
            .map(|settings| settings.hotkeys.clone())
            .unwrap_or_default()
    }

    fn run_hotkey(&self, shortcut: &Shortcut) {
        let action = self.shortcuts.lock().ok().and_then(|shortcuts| {
            shortcuts
                .iter()
                .find(|(_, registered)| registered == shortcut)
                .map(|(action, _)| *action)
        });
        let result = match action {
            Some(HotkeyAction::SaveReplay) => self.handle.save_replay(),
            Some(HotkeyAction::ToggleRecording) => {
                if self.state() == EngineState::Recording {
                    self.handle.stop_recording()
                } else {
                    self.handle.start_recording()
                }
            }
            Some(HotkeyAction::ToggleReplay) => {
                let armed = self
                    .status
                    .lock()
                    .map(|status| status.state != EngineState::Idle)
                    .unwrap_or(false);
                if armed {
                    self.handle.disarm()
                } else {
                    self.handle.arm()
                }
            }
            None => return,
        };
        if let Err(error) = result {
            tracing::warn!(%error, "action du raccourci impossible");
        }
    }

    fn register_hotkeys(
        &self,
        app: &tauri::AppHandle,
        bindings: &[(HotkeyAction, Shortcut)],
    ) -> Vec<(HotkeyAction, Shortcut)> {
        let mut failed = Vec::new();
        let mut registered = Vec::new();
        for (action, shortcut) in bindings {
            match app.global_shortcut().register(*shortcut) {
                Ok(()) => registered.push((*action, *shortcut)),
                Err(error) => {
                    tracing::warn!(%error, keys = %hotkeys::canonical(shortcut), "raccourci indisponible");
                    failed.push((*action, *shortcut));
                }
            }
        }
        if let Ok(mut shortcuts) = self.shortcuts.lock() {
            *shortcuts = registered;
        }
        if let Ok(mut unavailable) = self.unavailable.lock() {
            *unavailable = failed.iter().map(|(action, _)| *action).collect();
        }
        failed
    }

    fn unregister_hotkeys(&self, app: &tauri::AppHandle) {
        let shortcuts = self
            .shortcuts
            .lock()
            .map(|mut shortcuts| std::mem::take(&mut *shortcuts))
            .unwrap_or_default();
        for (_, shortcut) in shortcuts {
            app.global_shortcut().unregister(shortcut).ok();
        }
    }

    fn resume_hotkeys(&self, app: &tauri::AppHandle) {
        self.unregister_hotkeys(app);
        let bindings = hotkeys::validate(&self.hotkeys()).unwrap_or_default();
        self.register_hotkeys(app, &bindings);
    }

    fn set_hotkeys(&self, app: &tauri::AppHandle, wanted: Hotkeys) -> Result<(), Localized> {
        let bindings = hotkeys::validate(&wanted)?;
        let previous = hotkeys::validate(&self.hotkeys()).unwrap_or_default();
        self.unregister_hotkeys(app);
        let failed = self.register_hotkeys(app, &bindings);
        if !failed.is_empty() {
            self.unregister_hotkeys(app);
            self.register_hotkeys(app, &previous);
            let taken: Vec<String> = failed
                .iter()
                .map(|(_, shortcut)| hotkeys::canonical(shortcut))
                .collect();
            return Err(Localized::new("hotkeys_taken").with("keys", taken.join(", ")));
        }
        let canonical_for = |wanted_action| {
            bindings
                .iter()
                .find(|(action, _)| *action == wanted_action)
                .map(|(_, shortcut)| hotkeys::canonical(shortcut))
        };
        let saved = Hotkeys {
            save_replay: canonical_for(HotkeyAction::SaveReplay),
            toggle_replay: canonical_for(HotkeyAction::ToggleReplay),
            toggle_recording: canonical_for(HotkeyAction::ToggleRecording),
        };
        self.update(|settings| settings.hotkeys = saved)
    }

    fn theme(&self) -> settings::Theme {
        self.settings
            .lock()
            .map(|settings| settings.theme)
            .unwrap_or_default()
    }

    fn reset(&self, app: &tauri::AppHandle) -> Result<(), Localized> {
        let defaults = settings::Settings::default();
        self.set_hotkeys(app, defaults.hotkeys.clone())?;
        self.apply(app, defaults.engine.clone())?;
        self.update(|settings| *settings = defaults)?;
        refresh_tray(app, self.language(), self.state() == EngineState::Recording);
        tauri_plugin_autostart::ManagerExt::autolaunch(app)
            .disable()
            .map_err(|error| {
                Localized::new("autostart_not_disabled").with("detail", error.to_string())
            })
    }

    fn react(&self, app: &tauri::AppHandle, event: &EngineEvent) {
        match event {
            EngineEvent::StateChanged { state } => {
                refresh_tray_state(app, self.language(), *state == EngineState::Recording);
            }
            EngineEvent::GameNamed { exe_name, name } => {
                if let Err(error) = self.update(|settings| {
                    settings
                        .engine
                        .storage
                        .game_names
                        .insert(exe_name.clone(), name.clone());
                }) {
                    tracing::warn!(%error, "nom de jeu non enregistré");
                }
            }
            EngineEvent::ClipSaved {
                path,
                duration_secs,
                ..
            } => {
                self.library_changed(app, &[std::path::PathBuf::from(path)]);
                let feedback = self.feedback();
                if feedback.sound {
                    feedback::play_chime();
                }
                if feedback.toast {
                    let game = std::path::Path::new(path)
                        .parent()
                        .and_then(std::path::Path::file_name)
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let seconds = duration_secs.round() as u64;
                    feedback::toast(
                        app,
                        &i18n::text(self.language(), "toast.clipSaved"),
                        &format!("{game} · {}:{:02}", seconds / 60, seconds % 60),
                    );
                }
            }
            EngineEvent::Alert { text, .. }
                if text.code == "clip_save_failed" && self.feedback().toast =>
            {
                let language = self.language();
                feedback::toast(
                    app,
                    &i18n::text(language, "toast.clipNotSaved"),
                    &i18n::render(language, text),
                );
            }
            _ => {}
        }
    }

    fn add_preview(
        &self,
        preview: palaclips_engine::capture::PreviewCapture,
    ) -> Result<u32, Localized> {
        let mut previews = self.previews.lock().map_err(|_| internal_error())?;
        if previews.len() >= MAX_PREVIEWS {
            return Err(Localized::new("too_many_previews"));
        }
        let id = self.next_preview.fetch_add(1, Ordering::Relaxed);
        previews.insert(id, preview);
        Ok(id)
    }

    fn stop_preview(&self, id: u32) {
        let removed = self
            .previews
            .lock()
            .ok()
            .and_then(|mut previews| previews.remove(&id));
        drop(removed);
    }

    fn stop_previews(&self) {
        let removed: Vec<_> = self
            .previews
            .lock()
            .map(|mut previews| previews.drain().map(|(_, preview)| preview).collect())
            .unwrap_or_default();
        drop(removed);
    }

    fn clips_root(&self) -> palaclips_engine::Result<std::path::PathBuf> {
        match self.config().storage.clips_dir {
            Some(dir) => Ok(dir),
            None => palaclips_engine::paths::default_clips_dir(),
        }
    }

    fn publish(&self, app: &tauri::AppHandle, event: EngineEvent) {
        if let Ok(mut status) = self.status.lock() {
            status.apply(&event);
        }
        app.emit(ENGINE_EVENT, &event).ok();
    }

    fn shutdown(&self) {
        let engine = self.engine.lock().ok().and_then(|mut engine| engine.take());
        drop(engine);
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            clips::MEDIA_SCHEME,
            |context, request, responder| {
                clips::serve_media(context.app_handle(), request, responder);
            },
        )
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args([AUTOSTART_ARG])
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Err(error) = open_main_window(app) {
                tracing::warn!(%error, "ouverture de la fenêtre impossible");
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && let Some(slot) = app.try_state::<EngineSlot>()
                    {
                        slot.run_hotkey(shortcut);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::engine_version,
            commands::default_config,
            commands::get_state,
            commands::get_config,
            commands::set_config,
            commands::reset_settings,
            commands::get_theme,
            commands::set_theme,
            commands::get_language,
            commands::set_language,
            commands::get_hotkeys,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::set_hotkeys,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_feedback,
            commands::set_feedback,
            commands::replay_arm,
            commands::replay_disarm,
            commands::replay_save,
            commands::record_start,
            commands::record_stop,
            commands::list_clips,
            commands::clip_games,
            commands::set_clip_favorite,
            commands::rename_clip,
            commands::trash_clip,
            commands::reveal_clip,
            commands::quota_preview,
            commands::prepare_player,
            commands::copy_clip,
            commands::track_peaks,
            commands::start_export,
            commands::cancel_export,
            commands::list_recent_clips,
            commands::list_encoders,
            commands::memory_status,
            commands::estimate_replay,
            commands::storage_status,
            commands::pick_clips_dir,
            commands::open_clips_dir,
            commands::list_capture_targets,
            commands::list_audio_devices,
            commands::calibrate_audio,
            commands::list_app_sessions,
            commands::pick_app_exe,
            commands::app_icon,
            commands::start_preview,
            commands::stop_preview,
            commands::stop_previews,
            commands::set_performance_panel,
        ])
        .setup(|app| {
            match palaclips_engine::paths::data_dirs_to_migrate() {
                Ok(Some((old, new))) => {
                    if let Err(error) = palaclips_engine::migrate::migrate_data(&old, &new) {
                        tracing::warn!(%error, "déplacement des données impossible");
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "dossiers de données introuvables"),
            }
            let settings_path = palaclips_engine::paths::settings_path().ok();
            let loaded = settings_path
                .as_deref()
                .map(settings::load)
                .unwrap_or_else(|| settings::Loaded {
                    settings: settings::Settings::default(),
                    writable: false,
                    problem: Some(Localized::new("settings_dir_missing")),
                });
            let slot = EngineSlot::start(
                app.handle(),
                loaded.settings,
                settings_path,
                loaded.writable,
            )?;
            if let Some(problem) = loaded.problem {
                slot.publish(app.handle(), shell_alert(problem));
            }
            let bindings = hotkeys::validate(&slot.hotkeys())
                .or_else(|_| hotkeys::validate(&Hotkeys::default()))
                .unwrap_or_default();
            for (_, shortcut) in slot.register_hotkeys(app.handle(), &bindings) {
                slot.publish(
                    app.handle(),
                    hotkey_unavailable_alert(&hotkeys::canonical(&shortcut)),
                );
            }
            let language = slot.language();
            app.manage(slot);
            if let Ok(thumbnails) = palaclips_engine::paths::thumbnail_dir() {
                app.asset_protocol_scope()
                    .allow_directory(thumbnails, true)?;
            }
            clips::start_library(app.handle());
            build_tray(app.handle(), language)?;
            if !std::env::args().any(|arg| arg == AUTOSTART_ARG) {
                open_main_window(app.handle())?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("PalaClips n'a pas pu démarrer : {err}");
            std::process::exit(1);
        });
    app.run(|app, event| match event {
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(slot) = app.try_state::<EngineSlot>() {
                slot.shutdown();
            }
        }
        _ => {}
    });
}

const TRAY: &str = "main";
const TRAY_ITEMS: [&str; 6] = ["open", "save", "record", "arm", "disarm", "quit"];
const RECORDING_DOT: [u8; 4] = [0xe5, 0x2b, 0x2b, 0xff];

fn tray_menu(
    app: &tauri::AppHandle,
    language: Language,
    recording: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    let items = TRAY_ITEMS
        .iter()
        .map(|id| {
            let key = match *id {
                "record" if recording => "tray.recordStop".to_owned(),
                "record" => "tray.recordStart".to_owned(),
                id => format!("tray.{id}"),
            };
            MenuItem::with_id(app, *id, i18n::text(language, &key), true, None::<&str>)
        })
        .collect::<tauri::Result<Vec<_>>>()?;
    let items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    Menu::with_items(app, &items)
}

pub fn refresh_tray(app: &tauri::AppHandle, language: Language, recording: bool) {
    let Some(tray) = app.tray_by_id(TRAY) else {
        return;
    };
    if let Err(error) =
        tray_menu(app, language, recording).and_then(|menu| tray.set_menu(Some(menu)))
    {
        tracing::warn!(%error, "menu de la zone de notification non traduit");
    }
}

fn refresh_tray_state(app: &tauri::AppHandle, language: Language, recording: bool) {
    refresh_tray(app, language, recording);
    let (Some(tray), Some(icon)) = (app.tray_by_id(TRAY), app.default_window_icon()) else {
        return;
    };
    let icon = if recording {
        recording_icon(icon)
    } else {
        icon.clone()
    };
    if let Err(error) = tray.set_icon(Some(icon)) {
        tracing::warn!(%error, "icône de la zone de notification non changée");
    }
}

fn recording_icon(base: &tauri::image::Image<'_>) -> tauri::image::Image<'static> {
    let (width, height) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    let radius = width.min(height) as f32 * 0.24;
    let center = (width as f32 - radius - 1.0, height as f32 - radius - 1.0);
    for y in 0..height {
        for x in 0..width {
            let distance =
                ((x as f32 + 0.5 - center.0).powi(2) + (y as f32 + 0.5 - center.1).powi(2)).sqrt();
            if distance <= radius {
                let offset = ((y * width + x) * 4) as usize;
                rgba[offset..offset + 4].copy_from_slice(&RECORDING_DOT);
            }
        }
    }
    tauri::image::Image::new_owned(rgba, width, height)
}

fn build_tray(app: &tauri::AppHandle, language: Language) -> tauri::Result<()> {
    let menu = tray_menu(app, language, false)?;
    let mut tray = TrayIconBuilder::with_id(TRAY)
        .tooltip("PalaClips")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                open_main_window(app).ok();
            }
            "save" | "record" | "arm" | "disarm" => {
                if let Some(slot) = app.try_state::<EngineSlot>() {
                    let result = match event.id().as_ref() {
                        "save" => slot.handle.save_replay(),
                        "record" if slot.state() == EngineState::Recording => {
                            slot.handle.stop_recording()
                        }
                        "record" => slot.handle.start_recording(),
                        "arm" => slot.handle.arm(),
                        _ => slot.handle.disarm(),
                    };
                    if let Err(error) = result {
                        tracing::warn!(%error, "action du menu impossible");
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_main_window(tray.app_handle()).ok();
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn internal_error() -> Localized {
    Localized::new("internal_error")
}

fn shell_alert(text: Localized) -> EngineEvent {
    EngineEvent::Alert {
        severity: Severity::Warning,
        message: i18n::render(Language::Fr, &text),
        text,
    }
}

fn hotkey_unavailable_alert(keys: &str) -> EngineEvent {
    shell_alert(Localized::new("hotkey_unavailable").with("keys", keys))
}

const TEST_WINDOW_ENV: &str = "PALACLIPS_TEST_WINDOW";

fn show_without_focus(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};

    let handle = HWND(window.hwnd()?.0);
    // SAFETY: `handle` is the live top-level window Tauri just created on this thread.
    unsafe { ShowWindow(handle, SW_SHOWNOACTIVATE) }.ok().ok();
    Ok(())
}

fn test_window_position() -> Option<(i32, i32)> {
    let value = std::env::var(TEST_WINDOW_ENV).ok()?;
    let (x, y) = value.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

pub fn open_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let placed = test_window_position();
    let mut builder =
        WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
            .title("PalaClips")
            .inner_size(1180.0, 760.0)
            .min_inner_size(900.0, 600.0)
            .visible(placed.is_none())
            .focused(placed.is_none());
    if let Some(dir) = palaclips_engine::paths::isolated_webview_dir() {
        builder = builder.data_directory(dir);
    }
    let window = builder.build()?;
    if let Some((x, y)) = placed {
        window.set_position(tauri::PhysicalPosition::new(x, y))?;
        show_without_focus(&window)?;
    }
    set_level_rate(app, LevelRate::Focused);
    let watched = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed)
            && let Some(slot) = watched.app_handle().try_state::<EngineSlot>()
        {
            slot.stop_previews();
            slot.performance_panel.store(false, Ordering::Relaxed);
        }
        let rate = match event {
            WindowEvent::Focused(true) => LevelRate::Focused,
            WindowEvent::Focused(false) => LevelRate::Background,
            WindowEvent::Resized(_) if watched.is_minimized().unwrap_or(false) => LevelRate::Off,
            WindowEvent::Destroyed => LevelRate::Off,
            _ => return,
        };
        set_level_rate(watched.app_handle(), rate);
        if let Some(slot) = watched.app_handle().try_state::<EngineSlot>() {
            slot.apply_performance(rate != LevelRate::Off);
        }
    });
    Ok(())
}

impl EngineSlot {
    pub(crate) fn set_performance_panel(&self, open: bool) {
        self.performance_panel.store(open, Ordering::Relaxed);
        self.apply_performance(true);
    }

    fn apply_performance(&self, visible: bool) {
        let enabled = visible && self.performance_panel.load(Ordering::Relaxed);
        if let Err(error) = self.handle.set_performance(enabled) {
            tracing::warn!(%error, "mesures de performances non transmises");
        }
    }
}

fn set_level_rate(app: &tauri::AppHandle, rate: LevelRate) {
    if let Some(slot) = app.try_state::<EngineSlot>()
        && let Err(error) = slot.handle.set_level_rate(rate)
    {
        tracing::warn!(%error, "niveaux audio non transmis");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_replay_stats_and_clears_when_disarmed() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::ReplayArmed,
        });
        status.apply(&EngineEvent::ReplayStats {
            buffered_secs: 12.5,
            bytes: 64_000_000,
        });
        assert_eq!(status.state, EngineState::ReplayArmed);
        assert_eq!(status.buffer_bytes, 64_000_000);
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::Idle,
        });
        assert_eq!(status.buffered_secs, 0.0);
        assert_eq!(status.buffer_bytes, 0);
    }

    fn descriptor(title: &str, kind: TrackKind) -> TrackDescriptor {
        TrackDescriptor {
            title: title.into(),
            kind,
        }
    }

    #[test]
    fn track_states_follow_the_capture_and_reset_when_disarmed() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::CaptureStarted {
            source: Localized::new("source_primary_screen"),
            width: 2560,
            height: 1440,
            fps: 60,
            tracks: vec![
                descriptor("Mix complet", TrackKind::Mix),
                descriptor("Discord", TrackKind::Application),
            ],
        });
        status.apply(&EngineEvent::TrackChanged {
            track: 1,
            title: "Discord".into(),
            state: TrackState::Waiting,
            source: None,
        });
        assert_eq!(status.tracks[1].state, Some(TrackState::Waiting));
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::Idle,
        });
        assert!(status.capture.is_none());
        assert_eq!(status.tracks.len(), 2);
        assert_eq!(status.tracks[1].state, None);
    }

    #[test]
    fn tracks_with_the_same_title_keep_separate_states() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::CaptureStarted {
            source: Localized::new("source_primary_screen"),
            width: 1920,
            height: 1080,
            fps: 60,
            tracks: vec![
                descriptor("Jeu", TrackKind::Game),
                descriptor("Jeu", TrackKind::Game),
            ],
        });
        status.apply(&EngineEvent::TrackChanged {
            track: 1,
            title: "Jeu".into(),
            state: TrackState::Capturing,
            source: Some("game.exe".into()),
        });
        assert_eq!(status.tracks[0].state, None);
        assert_eq!(status.tracks[1].state, Some(TrackState::Capturing));
    }

    #[test]
    fn rearming_clears_the_previous_replay_failure() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::Alert {
            severity: Severity::Error,
            text: Localized::new("replay_stopped"),
            message: "Le replay s'est arrêté.".into(),
        });
        status.apply(&hotkey_unavailable_alert("Alt+F10"));
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::ReplayArmed,
        });
        assert_eq!(status.alerts.len(), 1);
        assert_eq!(status.alerts[0].text.code, "hotkey_unavailable");
    }

    #[test]
    fn a_saved_clip_clears_the_too_short_notice() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::Alert {
            severity: Severity::Info,
            text: Localized::new("clip_too_short"),
            message: "Rien de nouveau à sauvegarder.".into(),
        });
        status.apply(&EngineEvent::ClipSaved {
            path: "clip.mp4".into(),
            duration_secs: 12.0,
            save_latency_ms: 20,
            thumbnail: None,
        });
        assert!(status.alerts.is_empty());
    }

    #[test]
    fn an_alert_code_is_listed_once_with_its_latest_message() {
        let mut status = AppStatus::default();
        status.apply(&hotkey_unavailable_alert("Alt+F10"));
        status.apply(&hotkey_unavailable_alert("Alt+F9"));
        assert_eq!(status.alerts.len(), 1);
        assert_eq!(status.alerts[0].text.params["keys"], "Alt+F9");
        assert!(status.alerts[0].message.starts_with("Alt+F9 est déjà pris"));
    }

    #[test]
    fn recording_stats_reset_when_the_recording_stops() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::Recording,
        });
        status.apply(&EngineEvent::RecordingStats {
            elapsed_secs: 42.0,
            bytes: 9_000_000,
        });
        assert_eq!(status.recording_bytes, 9_000_000);
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::ReplayArmed,
        });
        assert_eq!(status.recording_secs, 0.0);
        assert_eq!(status.recording_bytes, 0);
    }

    #[test]
    fn the_recording_icon_keeps_the_size_and_adds_a_red_dot() {
        let base = tauri::image::Image::new_owned(vec![0; 32 * 32 * 4], 32, 32);
        let icon = recording_icon(&base);
        assert_eq!((icon.width(), icon.height()), (32, 32));
        let corner = ((26 * 32 + 26) * 4) as usize;
        assert_eq!(&icon.rgba()[corner..corner + 4], &RECORDING_DOT);
        assert_eq!(&icon.rgba()[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn an_alert_reaches_the_front_as_a_code_with_parameters() {
        let json = serde_json::to_value(hotkey_unavailable_alert("Alt+F10")).expect("json");
        assert_eq!(json["type"], "alert");
        assert_eq!(json["code"], "hotkey_unavailable");
        assert_eq!(json["params"]["keys"], "Alt+F10");
        assert!(
            json["message"]
                .as_str()
                .is_some_and(|text| text.contains("Alt+F10"))
        );
    }
}
