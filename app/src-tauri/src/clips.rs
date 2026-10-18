use std::path::{Path, PathBuf};

use palaclips_engine::events::{EngineEvent, Localized, Severity};
use palaclips_engine::library::{Clip, ClipQuery, Library, move_to_recycle_bin, watch};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::{EngineSlot, i18n, internal_error};

pub const LIBRARY_EVENT: &str = "library-changed";

#[derive(Debug, Serialize)]
pub struct QuotaPreview {
    pub total_bytes: u64,
    pub deleted: Vec<Clip>,
}

impl EngineSlot {
    fn with_library<T>(
        &self,
        action: impl FnOnce(&mut Library) -> palaclips_engine::Result<T>,
    ) -> Result<T, Localized> {
        let mut library = self.library.lock().map_err(|_| internal_error())?;
        let library = library
            .as_mut()
            .ok_or_else(|| Localized::new("library_loading"))?;
        Ok(action(library)?)
    }

    pub fn reload_library(&self, app: &tauri::AppHandle) {
        if let Err(error) = self.open_library(app) {
            tracing::warn!(%error, "bibliothèque des clips indisponible");
        }
    }

    fn open_library(&self, app: &tauri::AppHandle) -> Result<(), Localized> {
        let root = self.clips_root()?;
        {
            let mut library = self.library.lock().map_err(|_| internal_error())?;
            match library.as_mut() {
                Some(library) => library.set_root(root.clone()),
                None => {
                    *library = Some(Library::open(
                        &palaclips_engine::paths::library_db()?,
                        root.clone(),
                    )?)
                }
            }
        }
        let report = self.with_library(Library::sync)?;
        tracing::info!(?report, root = %root.display(), "bibliothèque des clips synchronisée");
        self.enforce_quota(app);
        app.emit(LIBRARY_EVENT, ()).ok();
        let watcher_app = app.clone();
        let watching = watch(&root, move |paths| {
            if let Some(slot) = watcher_app.try_state::<EngineSlot>() {
                slot.library_changed(&watcher_app, &paths);
            }
        })?;
        if let Ok(mut current) = self.library_watch.lock() {
            *current = Some(watching);
        }
        Ok(())
    }

    pub fn library_changed(&self, app: &tauri::AppHandle, paths: &[PathBuf]) {
        match self.with_library(|library| library.apply_changes(paths)) {
            Ok(_) => {
                self.enforce_quota(app);
                app.emit(LIBRARY_EVENT, ()).ok();
            }
            Err(error) => tracing::warn!(%error, "bibliothèque des clips non mise à jour"),
        }
    }

    fn enforce_quota(&self, app: &tauri::AppHandle) {
        let Some(quota) = self.config().storage.quota_bytes() else {
            return;
        };
        let deleted = match self.with_library(|library| library.apply_quota(quota)) {
            Ok(deleted) => deleted,
            Err(error) => {
                tracing::warn!(%error, "quota non appliqué");
                return;
            }
        };
        if deleted.is_empty() {
            return;
        }
        let size_mb = deleted.iter().map(|clip| clip.size_bytes).sum::<u64>() / 1_000_000;
        let text = Localized::new("quota_deleted")
            .with("count", deleted.len())
            .with("size_mb", size_mb);
        self.publish(
            app,
            EngineEvent::Alert {
                severity: Severity::Info,
                message: i18n::render(i18n::Language::Fr, &text),
                text,
            },
        );
    }

    fn known_clip(&self, path: &str) -> Result<PathBuf, Localized> {
        let path = PathBuf::from(path);
        let known = self.with_library(|library| library.contains(&path))?;
        if known {
            Ok(path)
        } else {
            Err(Localized::new("clip_unknown"))
        }
    }

    pub fn list_clips(&self, query: &ClipQuery) -> Result<Vec<Clip>, Localized> {
        self.with_library(|library| library.query(query))
    }

    pub fn clip_games(&self) -> Result<Vec<String>, Localized> {
        self.with_library(|library| library.games())
    }

    pub fn set_clip_favorite(&self, path: &str, favorite: bool) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        self.with_library(|library| library.set_favorite(&path, favorite))
    }

    pub fn rename_clip(
        &self,
        app: &tauri::AppHandle,
        path: &str,
        name: &str,
    ) -> Result<String, Localized> {
        let path = self.known_clip(path)?;
        let renamed = self.with_library(|library| library.rename(&path, name))?;
        app.emit(LIBRARY_EVENT, ()).ok();
        Ok(renamed.to_string_lossy().into_owned())
    }

    pub fn trash_clip(&self, app: &tauri::AppHandle, path: &str) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        move_to_recycle_bin(&path)?;
        self.with_library(|library| library.forget_deleted(&path))?;
        app.emit(LIBRARY_EVENT, ()).ok();
        Ok(())
    }

    pub fn reveal_clip(&self, path: &str) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        reveal(&path)
    }

    pub fn quota_preview(&self, quota_gb: u32) -> Result<QuotaPreview, Localized> {
        let quota = u64::from(quota_gb) * 1_000_000_000;
        self.with_library(|library| {
            Ok(QuotaPreview {
                total_bytes: library.total_bytes()?,
                deleted: library.over_quota(quota)?,
            })
        })
    }
}

fn reveal(path: &Path) -> Result<(), Localized> {
    let mut select = std::ffi::OsString::from("/select,");
    select.push(path.as_os_str());
    std::process::Command::new("explorer")
        .arg(select)
        .spawn()
        .map(|_| ())
        .map_err(|error| Localized::new("explorer_failed").with("detail", error.to_string()))
}

pub fn start_library(app: &tauri::AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("palaclips-library".into())
        .spawn(move || {
            if let Some(slot) = app.try_state::<EngineSlot>() {
                slot.reload_library(&app);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "bibliothèque des clips non démarrée");
    }
}
