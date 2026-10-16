mod commands;

use std::sync::{Arc, Mutex};

use palaclips_engine::EngineConfig;
use palaclips_engine::engine::{Engine, EngineHandle, LevelRate};
use palaclips_engine::events::{EngineEvent, EngineState, Severity};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const MAIN_WINDOW: &str = "main";
pub const ENGINE_EVENT: &str = "engine-event";

#[derive(Debug, Clone, Serialize)]
pub struct ActiveAlert {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AppStatus {
    pub state: EngineState,
    pub buffered_secs: f64,
    pub buffer_bytes: u64,
    pub alerts: Vec<ActiveAlert>,
}

impl AppStatus {
    fn apply(&mut self, event: &EngineEvent) {
        match event {
            EngineEvent::StateChanged { state } => {
                self.state = *state;
                if *state == EngineState::Idle {
                    self.buffered_secs = 0.0;
                    self.buffer_bytes = 0;
                }
            }
            EngineEvent::ReplayStats {
                buffered_secs,
                bytes,
            } => {
                self.buffered_secs = *buffered_secs;
                self.buffer_bytes = *bytes;
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
    engine: Mutex<Option<Engine>>,
    handle: EngineHandle,
    status: Arc<Mutex<AppStatus>>,
}

impl EngineSlot {
    fn start(app: &tauri::AppHandle, config: EngineConfig) -> palaclips_engine::Result<Self> {
        let arm = config.replay.arm_on_startup;
        let engine = Engine::start(config)?;
        let handle = engine.handle();
        let status = Arc::new(Mutex::new(AppStatus::default()));
        let events = engine.events().clone();
        let relay_status = status.clone();
        let app = app.clone();
        std::thread::Builder::new()
            .name("palaclips-events".into())
            .spawn(move || {
                for event in events.iter() {
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
            engine: Mutex::new(Some(engine)),
            handle,
            status,
        })
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
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Err(error) = open_main_window(app) {
                tracing::warn!(%error, "ouverture de la fenêtre impossible");
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed
                        && *shortcut == save_replay_shortcut()
                        && let Some(slot) = app.try_state::<EngineSlot>()
                        && let Err(error) = slot.handle.save_replay()
                    {
                        tracing::warn!(%error, "sauvegarde par raccourci impossible");
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::engine_version,
            commands::default_config,
            commands::get_state,
            commands::replay_arm,
            commands::replay_disarm,
            commands::replay_save,
        ])
        .setup(|app| {
            let slot = EngineSlot::start(app.handle(), EngineConfig::default())?;
            if let Err(error) = app.global_shortcut().register(save_replay_shortcut()) {
                tracing::warn!(%error, "raccourci de sauvegarde indisponible");
                slot.publish(app.handle(), hotkey_unavailable_alert("Alt+F10"));
            }
            app.manage(slot);
            build_tray(app.handle())?;
            open_main_window(app.handle())?;
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
    let save = MenuItem::with_id(app, "save", "Sauvegarder le replay", true, Some("Alt+F10"))?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &save, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("PalaClips")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                open_main_window(app).ok();
            }
            "save" => {
                if let Some(slot) = app.try_state::<EngineSlot>() {
                    slot.handle.save_replay().ok();
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

fn save_replay_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::F10)
}

fn hotkey_unavailable_alert(keys: &str) -> EngineEvent {
    EngineEvent::Alert {
        severity: Severity::Warning,
        code: "hotkey_unavailable".into(),
        message: format!(
            "{keys} est déjà pris par une autre application, probablement l'overlay NVIDIA (il y enregistre son propre replay). Désactive ce raccourci dans l'overlay (Alt+Z → Paramètres → Raccourcis clavier) ou choisis-en un autre dans Réglages → Raccourcis."
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
    fn an_alert_code_is_listed_once_with_its_latest_message() {
        let mut status = AppStatus::default();
        status.apply(&hotkey_unavailable_alert("Alt+F10"));
        status.apply(&hotkey_unavailable_alert("Alt+F9"));
        assert_eq!(status.alerts.len(), 1);
        assert!(status.alerts[0].message.starts_with("Alt+F9"));
    }
}
