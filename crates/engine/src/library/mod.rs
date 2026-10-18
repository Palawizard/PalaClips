use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::ffi;
use serde::Serialize;
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::core::HSTRING;

use crate::encode::route_ffmpeg_logs;
use crate::error::{EngineError, Result};
use crate::paths;

mod index;
mod watch;

pub use index::{Clip, ClipKind, ClipQuery, ClipSort, Library, SyncReport};
pub use watch::{LibraryWatch, watch};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipInfo {
    pub duration_secs: f64,
    pub tracks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipSummary {
    pub path: String,
    pub game: String,
    pub modified_ms: u64,
    pub size_bytes: u64,
    pub duration_secs: f64,
    pub tracks: Vec<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DiskSpace {
    pub free_bytes: u64,
    pub total_bytes: u64,
}

pub fn probe_clip(path: &Path) -> Result<ClipInfo> {
    route_ffmpeg_logs();
    let url = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
    let input =
        AVFormatContextInput::open(&url).map_err(|error| EngineError::Mux(error.to_string()))?;
    let tracks = input
        .streams()
        .iter()
        .filter(|stream| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_AUDIO)
        .enumerate()
        .map(|(index, stream)| {
            let metadata = stream.metadata();
            [c"title", c"handler_name"]
                .into_iter()
                .find_map(|key| metadata.as_ref()?.get(key, None, 0))
                .map(|entry| entry.value().to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("Piste {}", index + 1))
        })
        .collect();
    Ok(ClipInfo {
        duration_secs: input.duration.max(0) as f64 / f64::from(ffi::AV_TIME_BASE),
        tracks,
    })
}

pub fn move_to_recycle_bin(path: &Path) -> Result<()> {
    use windows::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
        SHFileOperationW,
    };

    use std::os::windows::ffi::OsStrExt;

    let mut from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: windows::core::PCWSTR(from.as_mut_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT).0 as u16,
        ..Default::default()
    };
    // SAFETY: `from` is a double-null-terminated wide path that outlives the call.
    let status = unsafe { SHFileOperationW(&mut operation) };
    if status != 0 || operation.fAnyOperationsAborted.as_bool() {
        return Err(EngineError::Io(std::io::Error::other(format!(
            "Corbeille : code {status}"
        ))));
    }
    Ok(())
}

pub fn recent_clips(root: &Path, limit: usize) -> Vec<ClipSummary> {
    let mut clips: Vec<(u64, PathBuf, u64)> = Vec::new();
    let dirs = std::iter::once(root.to_path_buf()).chain(
        std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path()),
    );
    for dir in dirs {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
            {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_millis() as u64);
            clips.push((modified, path, metadata.len()));
        }
    }
    clips.sort_by(|a, b| b.0.cmp(&a.0));
    clips
        .into_iter()
        .take(limit)
        .filter_map(|(modified_ms, path, size_bytes)| {
            let info = probe_clip(&path)
                .inspect_err(
                    |error| tracing::warn!(%error, path = %path.display(), "clip illisible"),
                )
                .ok()?;
            let thumbnail = paths::thumbnail_path(&path)
                .ok()
                .filter(|thumbnail| thumbnail.exists())
                .map(|thumbnail| thumbnail.to_string_lossy().into_owned());
            Some(ClipSummary {
                game: game_of(root, &path),
                path: path.to_string_lossy().into_owned(),
                modified_ms,
                size_bytes,
                duration_secs: info.duration_secs,
                tracks: info.tracks,
                thumbnail,
            })
        })
        .collect()
}

fn game_of(root: &Path, clip: &Path) -> String {
    clip.parent()
        .filter(|parent| *parent != root)
        .and_then(Path::file_name)
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

pub const STOP_FREE_BYTES: u64 = 300_000_000;
pub const WARN_FREE_BYTES: u64 = 2_000_000_000;

pub fn ensure_free_space(dir: &Path, needed: u64) -> Result<u64> {
    let free = disk_space(dir)?.free_bytes;
    if free < needed.saturating_add(STOP_FREE_BYTES) {
        return Err(EngineError::DiskFull);
    }
    Ok(free)
}

pub fn disk_space(path: &Path) -> Result<DiskSpace> {
    let existing = path
        .ancestors()
        .find(|ancestor| ancestor.exists())
        .unwrap_or(path);
    let mut free = 0u64;
    let mut total = 0u64;
    // SAFETY: the path string outlives the call and both out-pointers are valid.
    unsafe {
        GetDiskFreeSpaceExW(
            &HSTRING::from(existing.as_os_str()),
            Some(&mut free),
            Some(&mut total),
            None,
        )
    }
    .map_err(|error| EngineError::Io(std::io::Error::other(error.message())))?;
    Ok(DiskSpace {
        free_bytes: free,
        total_bytes: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_is_the_folder_under_the_clips_root() {
        let root = Path::new(r"C:\Videos\PalaClips");
        assert_eq!(
            game_of(root, &root.join("Fortnite").join("clip.mp4")),
            "Fortnite"
        );
        assert_eq!(game_of(root, &root.join("clip.mp4")), "");
    }

    #[test]
    fn disk_space_of_a_missing_folder_uses_its_drive() {
        let space = disk_space(&std::env::temp_dir().join("palaclips-missing").join("deep"))
            .expect("disk space");
        assert!(space.total_bytes >= space.free_bytes);
        assert!(space.total_bytes > 0);
    }
}
