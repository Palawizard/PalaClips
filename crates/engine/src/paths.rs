use std::path::{Path, PathBuf};

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::SystemInformation::{
    GetLocalTime, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::Shell::{
    FOLDERID_LocalAppData, FOLDERID_RoamingAppData, FOLDERID_Videos, KF_FLAG_DEFAULT,
    SHGetKnownFolderPath,
};
use windows::core::GUID;

use crate::error::{EngineError, Result};
use crate::games::{DESKTOP_GAME, exe_stem, sanitize};

const APP_DIR: &str = "PalaClips";

pub fn default_clips_dir() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_Videos)?.join(APP_DIR))
}

pub const CONFIG_DIR_ENV: &str = "PALACLIPS_CONFIG_DIR";

pub fn settings_path() -> Result<PathBuf> {
    let dir = match std::env::var_os(CONFIG_DIR_ENV) {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => known_folder(&FOLDERID_RoamingAppData)?.join(APP_DIR),
    };
    Ok(dir.join("settings.json"))
}

pub fn replay_spool_dir() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_LocalAppData)?
        .join(APP_DIR)
        .join("replay"))
}

pub fn calibration_clip_path() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_LocalAppData)?
        .join(APP_DIR)
        .join("cache")
        .join("calibration.mp4"))
}

pub fn thumbnail_path(clip: &Path) -> Result<PathBuf> {
    let name = clip.file_stem().unwrap_or_default();
    Ok(known_folder(&FOLDERID_LocalAppData)?
        .join(APP_DIR)
        .join("cache")
        .join("thumbnails")
        .join(name)
        .with_extension("jpg"))
}

pub fn game_label(exe_name: Option<&str>) -> String {
    exe_name
        .map(|exe_name| sanitize(exe_stem(exe_name)))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| DESKTOP_GAME.to_owned())
}

pub fn replay_clip_path(root: &Path, game: &str, stamp: &str) -> PathBuf {
    let dir = root.join(game);
    let mut path = dir.join(format!("{game}_{stamp}_replay.mp4"));
    let mut index = 2;
    while path.exists() {
        path = dir.join(format!("{game}_{stamp}_replay_{index}.mp4"));
        index += 1;
    }
    path
}

pub fn recording_path(root: &Path, game: &str, stamp: &str) -> PathBuf {
    let dir = root.join(game);
    let mut path = dir.join(format!("{game}_{stamp}.mp4"));
    let mut index = 2;
    while path.exists() {
        path = dir.join(format!("{game}_{stamp}_{index}.mp4"));
        index += 1;
    }
    path
}

pub fn local_timestamp() -> String {
    // SAFETY: GetLocalTime only returns the current local time.
    let now = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    )
}

pub fn physical_memory() -> Option<(u64, u64)> {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is sized by `dwLength` as the call requires.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((status.ullTotalPhys, status.ullAvailPhys))
}

pub fn private_bytes() -> Option<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // SAFETY: the pseudo-handle of the current process is always valid and `counters` is sized by `cb`.
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            counters.cb,
        )
    }
    .ok()?;
    Some(counters.PrivateUsage as u64)
}

fn known_folder(id: &GUID) -> Result<PathBuf> {
    // SAFETY: the returned string is owned by us and freed with CoTaskMemFree right after copying.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)
            .map_err(|error| EngineError::Io(std::io::Error::other(error.message())))?;
        let path = raw.to_string();
        CoTaskMemFree(Some(raw.0.cast()));
        path.map(PathBuf::from)
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_label_strips_the_extension_and_forbidden_characters() {
        assert_eq!(
            game_label(Some("FortniteClient-Win64-Shipping.exe")),
            "FortniteClient-Win64-Shipping"
        );
        assert_eq!(game_label(Some("cs2.EXE")), "cs2");
        assert_eq!(game_label(Some("a:b?.exe")), "ab");
        assert_eq!(game_label(None), "Bureau");
        assert_eq!(game_label(Some("...exe")), "Bureau");
    }

    #[test]
    fn replay_clips_land_in_the_game_folder() {
        let path = replay_clip_path(Path::new("C:/clips"), "cs2", "2026-10-16_15-30-00");
        assert_eq!(
            path,
            Path::new("C:/clips/cs2/cs2_2026-10-16_15-30-00_replay.mp4")
        );
    }

    #[test]
    fn a_second_save_in_the_same_second_gets_a_new_name() {
        let root = std::env::temp_dir().join(format!("palaclips-names-{}", std::process::id()));
        let first = replay_clip_path(&root, "cs2", "2026-10-16_15-30-00");
        std::fs::create_dir_all(first.parent().expect("dir")).expect("dir");
        std::fs::write(&first, b"").expect("write");
        let second = replay_clip_path(&root, "cs2", "2026-10-16_15-30-00");
        assert_eq!(
            second.file_name().and_then(|name| name.to_str()),
            Some("cs2_2026-10-16_15-30-00_replay_2.mp4")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn known_folders_resolve() {
        assert!(default_clips_dir().expect("videos").ends_with("PalaClips"));
        assert!(
            replay_spool_dir()
                .expect("local app data")
                .ends_with("replay")
        );
    }
}
