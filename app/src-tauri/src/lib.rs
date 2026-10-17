mod commands;
mod feedback;
mod hotkeys;
mod settings;

use std::sync::{Arc, Mutex};

use palaclips_engine::EngineConfig;
use palaclips_engine::engine::{Engine, EngineHandle, LevelRate, audio_sources};
use palaclips_engine::events::{EngineEvent, EngineState, Severity, TrackState};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::hotkeys::{HotkeyAction, Hotkeys};

pub const MAIN_WINDOW: &str = "main";
pub const ENGINE_EVENT: &str = "engine-event";
pub const AUTOSTART_ARG: &str = "--autostart";

#[derive(Debug, Clone, Serialize)]
pub struct ActiveAlert {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureInfo {
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub title: String,
    pub state: Option<TrackState>,
    pub source: Option<String>,
}

impl TrackInfo {
    fn named(title: String) -> Self {
        Self {
            title,
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
    pub capture: Option<CaptureInfo>,
    pub tracks: Vec<TrackInfo>,
    pub alerts: Vec<ActiveAlert>,
}

impl AppStatus {
    fn apply(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::StateChanged { state } => {
                self.state = *state;
                if *state != EngineState::Idle {
                    self.alerts.retain(|alert| {
                        alert.code != "replay_arm_failed" && alert.code != "replay_stopped"
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
            EngineEvent::ClipSaved { .. } => {
                self.alerts.retain(|alert| alert.code != "clip_too_short");
            }
            EngineEvent::Alert {
                severity,
                code,
                message,
            } => {
                self.alerts.retain(|alert| alert.code != *code);
                self.alerts.push(ActiveAlert {
                    severity: *severity,
                    code: code.clone(),
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
    unavailable: Mutex<Vec<HotkeyAction>>,
    engine: Mutex<Option<Engine>>,
    handle: EngineHandle,
    status: Arc<Mutex<AppStatus>>,
}

impl EngineSlot {
    fn start(
        app: &tauri::AppHandle,
        settings: settings::Settings,
        settings_path: Option<std::path::PathBuf>,
        settings_writable: bool,
    ) -> palaclips_engine::Result<Self> {
        let config = settings.engine.clone();
        let arm = config.replay.arm_on_startup;
        let engine = Engine::start(config.clone())?;
        let handle = engine.handle();
        let status = Arc::new(Mutex::new(AppStatus {
            tracks: audio_sources(&config)
                .into_iter()
                .map(|(_, title, _)| TrackInfo::named(title))
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
            unavailable: Mutex::new(Vec::new()),
            engine: Mutex::new(Some(engine)),
            handle,
            status,
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

    fn update(&self, change: impl FnOnce(&mut settings::Settings)) -> Result<(), String> {
        if !self.settings_writable {
            return Err(
                "Réglages en lecture seule : ils viennent d'une version plus récente de PalaClips."
                    .into(),
            );
        }
        let mut current = self
            .settings
            .lock()
            .map_err(|_| "réglages illisibles".to_owned())?;
        let mut changed = current.clone();
        change(&mut changed);
        if let Some(path) = &self.settings_path {
            settings::save(path, &changed)
                .map_err(|error| format!("Réglages non enregistrés : {error}."))?;
        }
        *current = changed;
        Ok(())
    }

    fn apply(&self, config: EngineConfig) -> Result<(), String> {
        config.validate().map_err(|error| error.to_string())?;
        self.update(|settings| settings.engine = config.clone())?;
        self.handle
            .configure(config)
            .map_err(|error| error.to_string())
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

    fn set_hotkeys(&self, app: &tauri::AppHandle, wanted: Hotkeys) -> Result<(), String> {
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
            return Err(format!(
                "{} déjà pris par une autre application : tes raccourcis précédents sont gardés.",
                taken.join(", ")
            ));
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
        };
        self.update(|settings| settings.hotkeys = saved)
    }

    fn react(&self, app: &tauri::AppHandle, event: &EngineEvent) {
        match event {
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
                        "Clip sauvegardé",
                        &format!("{game} · {}:{:02}", seconds / 60, seconds % 60),
                    );
                }
            }
            EngineEvent::Alert { code, message, .. }
                if code == "clip_save_failed" && self.feedback().toast =>
            {
                feedback::toast(app, "Clip non sauvegardé", message);
            }
            _ => {}
        }
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
            commands::reset_config,
            commands::get_hotkeys,
            commands::set_hotkeys,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_feedback,
            commands::set_feedback,
            commands::replay_arm,
            commands::replay_disarm,
            commands::replay_save,
            commands::list_recent_clips,
            commands::storage_status,
        ])
        .setup(|app| {
            let settings_path = palaclips_engine::paths::settings_path().ok();
            let loaded = settings_path
                .as_deref()
                .map(settings::load)
                .unwrap_or_else(|| settings::Loaded {
                    settings: settings::Settings::default(),
                    writable: false,
                    problem: Some(
                        "Dossier des réglages introuvable : ils ne seront pas enregistrés.".into(),
                    ),
                });
            let slot = EngineSlot::start(
                app.handle(),
                loaded.settings,
                settings_path,
                loaded.writable,
            )?;
            if let Some(problem) = loaded.problem {
                slot.publish(
                    app.handle(),
                    EngineEvent::Alert {
                        severity: Severity::Warning,
                        code: "settings_reset".into(),
                        message: problem,
                    },
                );
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
            app.manage(slot);
            build_tray(app.handle())?;
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

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir PalaClips", true, None::<&str>)?;
    let save = MenuItem::with_id(app, "save", "Sauvegarder le replay", true, None::<&str>)?;
    let arm = MenuItem::with_id(app, "arm", "Armer le replay", true, None::<&str>)?;
    let disarm = MenuItem::with_id(app, "disarm", "Désarmer le replay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &save, &arm, &disarm, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("PalaClips")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                open_main_window(app).ok();
            }
            "save" | "arm" | "disarm" => {
                if let Some(slot) = app.try_state::<EngineSlot>() {
                    let result = match event.id().as_ref() {
                        "save" => slot.handle.save_replay(),
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

fn hotkey_unavailable_alert(keys: &str) -> EngineEvent {
    EngineEvent::Alert {
        severity: Severity::Warning,
        code: "hotkey_unavailable".into(),
        message: format!(
            "{keys} est déjà pris par une autre application, souvent l'overlay NVIDIA (Alt+F10 et Alt+Shift+F10 y servent au replay). Désactive ce raccourci dans l'overlay (Alt+Z → Paramètres → Raccourcis clavier) ou choisis-en un autre dans Réglages → Raccourcis."
        ),
    }
}

pub fn open_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("PalaClips")
        .inner_size(1180.0, 760.0)
        .min_inner_size(900.0, 600.0)
        .build()?;
    set_level_rate(app, LevelRate::Focused);
    let watched = window.clone();
    window.on_window_event(move |event| {
        let rate = match event {
            WindowEvent::Focused(true) => LevelRate::Focused,
            WindowEvent::Focused(false) => LevelRate::Background,
            WindowEvent::Resized(_) if watched.is_minimized().unwrap_or(false) => LevelRate::Off,
            WindowEvent::Destroyed => LevelRate::Off,
            _ => return,
        };
        set_level_rate(watched.app_handle(), rate);
    });
    Ok(())
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

    #[test]
    fn track_states_follow_the_capture_and_reset_when_disarmed() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::CaptureStarted {
            source: "Écran principal".into(),
            width: 2560,
            height: 1440,
            fps: 60,
            tracks: vec!["Mix complet".into(), "Discord".into()],
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
            source: "Écran principal".into(),
            width: 1920,
            height: 1080,
            fps: 60,
            tracks: vec!["Jeu".into(), "Jeu".into()],
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
            code: "replay_stopped".into(),
            message: "Le replay s'est arrêté.".into(),
        });
        status.apply(&hotkey_unavailable_alert("Alt+F10"));
        status.apply(&EngineEvent::StateChanged {
            state: EngineState::ReplayArmed,
        });
        assert_eq!(status.alerts.len(), 1);
        assert_eq!(status.alerts[0].code, "hotkey_unavailable");
    }

    #[test]
    fn a_saved_clip_clears_the_too_short_notice() {
        let mut status = AppStatus::default();
        status.apply(&EngineEvent::Alert {
            severity: Severity::Info,
            code: "clip_too_short".into(),
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
        assert!(status.alerts[0].message.starts_with("Alt+F9"));
    }
}
