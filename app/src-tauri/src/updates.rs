use std::sync::Mutex;
use std::time::Duration;

use palaclips_engine::events::{EngineEvent, EngineState, Localized, Severity};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::i18n::{self, Language};
use crate::{EngineSlot, internal_error};

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const ENDPOINT_ENV: &str = "PALACLIPS_UPDATE_URL";
pub const UPDATE_ALERT: &str = "update_available";

#[derive(Default)]
pub struct Updates(Mutex<Option<Update>>);

impl std::fmt::Debug for Updates {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Updates").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub available: Option<AvailableUpdate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
}

fn update_failed(error: impl std::fmt::Display) -> Localized {
    Localized::new("update_failed").with("detail", error.to_string())
}

async fn find(app: &tauri::AppHandle) -> Result<Option<Update>, Localized> {
    let mut builder = app.updater_builder();
    if let Some(url) = std::env::var(ENDPOINT_ENV)
        .ok()
        .filter(|url| !url.is_empty())
    {
        let url = url.parse().map_err(update_failed)?;
        builder = builder.endpoints(vec![url]).map_err(update_failed)?;
    }
    builder
        .build()
        .map_err(update_failed)?
        .check()
        .await
        .map_err(update_failed)
}

fn info(app: &tauri::AppHandle, update: Option<&Update>) -> UpdateInfo {
    UpdateInfo {
        current: app.package_info().version.to_string(),
        available: update.map(|update| AvailableUpdate {
            version: update.version.clone(),
            notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
        }),
    }
}

pub async fn check(app: &tauri::AppHandle) -> Result<UpdateInfo, Localized> {
    let found = find(app).await?;
    let state = app.state::<Updates>();
    let mut current = state.0.lock().map_err(|_| internal_error())?;
    let announce = match (&found, current.as_ref()) {
        (Some(new), Some(old)) => new.version != old.version,
        (Some(_), None) => true,
        _ => false,
    };
    *current = found;
    let view = info(app, current.as_ref());
    drop(current);
    if let (true, Some(available)) = (announce, &view.available)
        && let Some(slot) = app.try_state::<EngineSlot>()
    {
        let text = Localized::new(UPDATE_ALERT).with("version", available.version.clone());
        slot.publish(
            app,
            EngineEvent::Alert {
                severity: Severity::Info,
                message: i18n::render(Language::Fr, &text),
                text,
            },
        );
    }
    Ok(view)
}

pub fn cached(app: &tauri::AppHandle) -> Result<UpdateInfo, Localized> {
    let state = app.state::<Updates>();
    let current = state.0.lock().map_err(|_| internal_error())?;
    Ok(info(app, current.as_ref()))
}

pub async fn install(app: &tauri::AppHandle) -> Result<(), Localized> {
    if let Some(slot) = app.try_state::<EngineSlot>()
        && slot.state() == EngineState::Recording
    {
        return Err(Localized::new("update_while_recording"));
    }
    let update = app
        .state::<Updates>()
        .0
        .lock()
        .map_err(|_| internal_error())?
        .clone()
        .ok_or_else(|| Localized::new("update_none"))?;
    tracing::info!(version = %update.version, "téléchargement de la mise à jour");
    let package = update
        .download(|_, _| {}, || {})
        .await
        .map_err(update_failed)?;
    if let Some(slot) = app.try_state::<EngineSlot>() {
        slot.shutdown();
    }
    tracing::info!(version = %update.version, "installation de la mise à jour");
    update.install(package).map_err(update_failed)?;
    app.restart();
}

pub fn start_checks(app: &tauri::AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("palaclips-updates".into())
        .spawn(move || {
            loop {
                if let Err(error) = tauri::async_runtime::block_on(check(&app)) {
                    tracing::warn!(error = %error.code, "recherche de mise à jour impossible");
                }
                std::thread::sleep(CHECK_EVERY);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "recherche de mise à jour non démarrée");
    }
}
