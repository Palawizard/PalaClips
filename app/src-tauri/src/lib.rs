mod commands;

use std::sync::{Arc, Mutex};

use palaclips_engine::EngineConfig;
use palaclips_engine::engine::{Engine, EngineHandle};
use palaclips_engine::events::{EngineEvent, EngineState};
use serde::Serialize;
use tauri::{Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";
pub const ENGINE_EVENT: &str = "engine-event";

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub state: EngineState,
    pub buffered_secs: f64,
    pub buffer_bytes: u64,
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
        let status = Arc::new(Mutex::new(AppStatus {
            state: EngineState::Idle,
            buffered_secs: 0.0,
            buffer_bytes: 0,
        }));
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

    fn shutdown(&self) {
        let engine = self.engine.lock().ok().and_then(|mut engine| engine.take());
        drop(engine);
    }
}

pub fn run() {
    let app = tauri::Builder::default()
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
            app.manage(slot);
            open_main_window(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|err| {
            eprintln!("PalaClips n'a pas pu démarrer : {err}");
            std::process::exit(1);
        });
    app.run(|app, event| {
        if let RunEvent::Exit = event
            && let Some(slot) = app.try_state::<EngineSlot>()
        {
            slot.shutdown();
        }
    });
}

pub fn open_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::App("index.html".into()))
        .title("PalaClips")
        .inner_size(1180.0, 760.0)
        .min_inner_size(900.0, 600.0)
        .build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_replay_stats_and_clears_when_disarmed() {
        let mut status = AppStatus {
            state: EngineState::Idle,
            buffered_secs: 0.0,
            buffer_bytes: 0,
        };
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
}
