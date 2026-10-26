use std::path::{Path, PathBuf};

use palaclips_engine::events::{EngineEvent, Localized, Severity};
use palaclips_engine::library::{
    Clip, ClipQuery, Library, TrackCache, TrackFile, copy_file_to_clipboard, move_to_recycle_bin,
    track_peaks, watch,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::{EngineSlot, i18n, internal_error};

pub const LIBRARY_EVENT: &str = "library-changed";
pub const EXPORT_PROGRESS_EVENT: &str = "export-progress";
pub const EXPORT_FINISHED_EVENT: &str = "export-finished";
const PROGRESS_STEP: f64 = 0.01;
pub const MEDIA_SCHEME: &str = "palaclips";
const TRACK_CACHE_BYTES: u64 = 2_000_000_000;
const RANGE_CHUNK: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExportInput {
    pub path: String,
    pub name: String,
    pub start_secs: f64,
    pub end_secs: f64,
    pub precise: bool,
    pub tracks: Vec<usize>,
    #[serde(default)]
    pub gains: Vec<f32>,
    pub mix: bool,
    pub mix_title: String,
}

#[derive(Debug, Clone, Serialize)]
struct ExportProgress {
    id: u32,
    progress: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ExportFinished {
    id: u32,
    path: Option<String>,
    error: Option<Localized>,
}

#[derive(Debug, Clone, Serialize)]
struct LibraryChange {
    removed: Vec<String>,
}

fn emit_library(app: &tauri::AppHandle, removed: &[PathBuf]) {
    let removed = removed
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    app.emit(LIBRARY_EVENT, LibraryChange { removed }).ok();
}

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
        let deleted = self.enforce_quota(app);
        emit_library(app, &deleted);
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
                let mut removed: Vec<PathBuf> = paths
                    .iter()
                    .filter(|path| !path.exists())
                    .cloned()
                    .collect();
                removed.extend(self.enforce_quota(app));
                emit_library(app, &removed);
            }
            Err(error) => tracing::warn!(%error, "bibliothèque des clips non mise à jour"),
        }
    }

    fn enforce_quota(&self, app: &tauri::AppHandle) -> Vec<PathBuf> {
        let Some(quota) = self.config().storage.quota_bytes() else {
            return Vec::new();
        };
        let deleted = match self.with_library(|library| library.apply_quota(quota)) {
            Ok(deleted) => deleted,
            Err(error) => {
                tracing::warn!(%error, "quota non appliqué");
                return Vec::new();
            }
        };
        if deleted.is_empty() {
            return Vec::new();
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
        deleted
            .into_iter()
            .map(|clip| PathBuf::from(clip.path))
            .collect()
    }

    fn known_clip(&self, path: &str) -> Result<PathBuf, Localized> {
        let path = PathBuf::from(path);
        let known = self.with_library(|library| library.contains(&path))?;
        if !known {
            return Err(Localized::new("clip_unknown"));
        }
        if !path.is_file() {
            self.library_changed(&self.app, std::slice::from_ref(&path));
            return Err(Localized::new("file_missing"));
        }
        Ok(path)
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
        emit_library(app, &[]);
        Ok(renamed.to_string_lossy().into_owned())
    }

    pub fn trash_clip(&self, app: &tauri::AppHandle, path: &str) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        move_to_recycle_bin(&path)?;
        self.with_library(|library| library.forget_deleted(&path))?;
        emit_library(app, std::slice::from_ref(&path));
        Ok(())
    }

    pub fn reveal_clip(&self, path: &str) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        reveal(&path)
    }

    pub fn prepare_player(&self, path: &str) -> Result<Vec<TrackFile>, Localized> {
        let path = self.known_clip(path)?;
        let cache = self.track_cache()?;
        let _extracting = self.extracting.lock().map_err(|_| internal_error())?;
        Ok(cache.tracks(&path)?)
    }

    pub fn track_peaks(&self, path: &str) -> Result<Vec<u8>, Localized> {
        let path = PathBuf::from(path);
        let cache = self.track_cache()?;
        let inside = path.starts_with(cache.dir())
            && !path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir));
        if !inside {
            return Err(Localized::new("clip_unknown"));
        }
        Ok(track_peaks(&path)?)
    }

    pub fn copy_clip(&self, path: &str) -> Result<(), Localized> {
        let path = self.known_clip(path)?;
        Ok(copy_file_to_clipboard(&path)?)
    }

    fn track_cache(&self) -> Result<TrackCache, Localized> {
        let mut cache = self.track_cache.lock().map_err(|_| internal_error())?;
        if cache.is_none() {
            *cache = Some(TrackCache::new(
                palaclips_engine::paths::track_cache_dir()?,
                TRACK_CACHE_BYTES,
            ));
        }
        cache.clone().ok_or_else(internal_error)
    }

    fn may_serve(&self, path: &Path) -> bool {
        if path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return false;
        }
        let cached = self
            .track_cache()
            .is_ok_and(|cache| path.starts_with(cache.dir()));
        cached
            || self
                .with_library(|library| library.contains(path))
                .unwrap_or(false)
    }

    pub fn start_export(
        &self,
        app: &tauri::AppHandle,
        input: ExportInput,
    ) -> Result<u32, Localized> {
        let source = self.known_clip(&input.path)?;
        let name = input.name.trim();
        palaclips_engine::config::check_file_name(name)?;
        let output = unique_path(&source.with_file_name(format!("{name}.mp4")));
        let encoders = if input.precise {
            crate::commands::cached_encoders(self)?
        } else {
            Vec::new()
        };
        let request = palaclips_engine::library::ExportRequest {
            source,
            output,
            start_secs: input.start_secs,
            end_secs: input.end_secs,
            precise: input.precise,
            tracks: input.tracks,
            gains: input.gains,
            mix: input.mix,
            mix_title: input.mix_title,
        };
        let id = self
            .next_export
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.exports
            .lock()
            .map_err(|_| internal_error())?
            .insert(id, cancel.clone());
        let app = app.clone();
        std::thread::Builder::new()
            .name("palaclips-export".into())
            .spawn(move || {
                let mut reported = -1.0;
                let mut report = |progress: f64| {
                    if progress - reported >= PROGRESS_STEP || progress >= 1.0 {
                        reported = progress;
                        app.emit(EXPORT_PROGRESS_EVENT, ExportProgress { id, progress })
                            .ok();
                    }
                };
                let result = palaclips_engine::library::export_clip(
                    &request,
                    &encoders,
                    &mut report,
                    &cancel,
                );
                if let Err(error) = &result {
                    tracing::warn!(%error, source = %request.source.display(), "export impossible");
                }
                if let Some(slot) = app.try_state::<EngineSlot>() {
                    if let Ok(mut exports) = slot.exports.lock() {
                        exports.remove(&id);
                    }
                    if result.is_ok() {
                        slot.library_changed(&app, std::slice::from_ref(&request.output));
                    }
                }
                app.emit(
                    EXPORT_FINISHED_EVENT,
                    ExportFinished {
                        id,
                        path: result
                            .as_ref()
                            .ok()
                            .map(|()| request.output.to_string_lossy().into_owned()),
                        error: result.err().map(|error| error.localized()),
                    },
                )
                .ok();
            })
            .map_err(|_| internal_error())?;
        Ok(id)
    }

    pub fn cancel_export(&self, id: u32) {
        if let Ok(exports) = self.exports.lock()
            && let Some(cancel) = exports.get(&id)
        {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
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

fn unique_path(wanted: &Path) -> PathBuf {
    let stem = wanted
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut path = wanted.to_path_buf();
    let mut index = 2;
    while path.exists() {
        path = wanted.with_file_name(format!("{stem}_{index}.mp4"));
        index += 1;
    }
    path
}

pub(crate) fn reveal(path: &Path) -> Result<(), Localized> {
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

pub fn serve_media(
    app: &tauri::AppHandle,
    request: tauri::http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = app
            .try_state::<EngineSlot>()
            .map(|slot| media_response(&slot, &request))
            .unwrap_or_else(|| status(tauri::http::StatusCode::SERVICE_UNAVAILABLE));
        responder.respond(response);
    });
}

fn status(code: tauri::http::StatusCode) -> tauri::http::Response<Vec<u8>> {
    tauri::http::Response::builder()
        .status(code)
        .body(Vec::new())
        .unwrap_or_default()
}

fn media_response(
    slot: &EngineSlot,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    use tauri::http::{StatusCode, header};

    let Some(path) = percent_decode(request.uri().path().trim_start_matches('/')) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let path = PathBuf::from(path);
    if !slot.may_serve(&path) {
        return status(StatusCode::FORBIDDEN);
    }
    let Ok(mut file) = std::fs::File::open(&path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let Some((start, end)) = byte_range(range, length, RANGE_CHUNK) else {
        return tauri::http::Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{length}"))
            .body(Vec::new())
            .unwrap_or_default();
    };
    let mut body = vec![0; (end - start + 1) as usize];
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_exact(&mut body).is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mime = if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("m4a"))
    {
        "audio/mp4"
    } else {
        "video/mp4"
    };
    tauri::http::Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{length}"),
        )
        .header(header::CONTENT_LENGTH, body.len())
        .body(body)
        .unwrap_or_default()
}

fn byte_range(header: Option<&str>, length: u64, chunk: u64) -> Option<(u64, u64)> {
    if length == 0 {
        return None;
    }
    let last = length - 1;
    let (start, end) = match header.and_then(|value| value.strip_prefix("bytes=")) {
        None => (0, last),
        Some(spec) => {
            let (from, to) = spec.split(',').next()?.trim().split_once('-')?;
            match (from.parse::<u64>().ok(), to.parse::<u64>().ok()) {
                (Some(from), Some(to)) => (from, to.min(last)),
                (Some(from), None) => (from, last),
                (None, Some(suffix)) if suffix > 0 => (length.saturating_sub(suffix), last),
                _ => return None,
            }
        }
    };
    (start <= end && start < length).then(|| (start, end.min(start + chunk - 1)))
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_clamped_to_the_file_and_to_one_chunk() {
        assert_eq!(
            byte_range(Some("bytes=0-"), 10_000, 4_096),
            Some((0, 4_095))
        );
        assert_eq!(
            byte_range(Some("bytes=100-199"), 10_000, 4_096),
            Some((100, 199))
        );
        assert_eq!(
            byte_range(Some("bytes=9000-20000"), 10_000, 4_096),
            Some((9_000, 9_999))
        );
        assert_eq!(
            byte_range(Some("bytes=-500"), 10_000, 4_096),
            Some((9_500, 9_999))
        );
        assert_eq!(byte_range(None, 100, 4_096), Some((0, 99)));
        assert_eq!(byte_range(Some("bytes=10000-"), 10_000, 4_096), None);
        assert_eq!(byte_range(Some("bytes=abc"), 10_000, 4_096), None);
        assert_eq!(byte_range(Some("bytes=0-"), 0, 4_096), None);
    }

    #[test]
    fn encoded_windows_paths_are_decoded() {
        assert_eq!(
            percent_decode("C%3A%5CVid%C3%A9os%5CPalaClips%5Cclip%20un.mp4").as_deref(),
            Some("C:\\Vidéos\\PalaClips\\clip un.mp4")
        );
        assert_eq!(percent_decode("%ZZ"), None);
    }
}
