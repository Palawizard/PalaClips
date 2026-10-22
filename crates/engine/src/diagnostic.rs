use std::ffi::c_void;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIDevice, IDXGIFactory1,
};
use windows::Win32::NetworkManagement::NetManagement::{
    NetApiBufferFree, NetUserGetInfo, USER_INFO_10,
};
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows::Win32::System::Time::{DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation};
use windows::core::{Interface, PCWSTR, w};

use crate::capture::{TargetInfo, display_color, list_targets};
use crate::error::{EngineError, Result};
use crate::sources::audio_devices;

pub const SYSTEM_ENTRY: &str = "system.txt";
pub const SETTINGS_ENTRY: &str = "settings.json";
const MASKED_TITLE: &str = "[titre masqué]";
const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
pub const MIN_WINDOWS_BUILD: u32 = 20_348;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub contents: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anonymizer {
    places: Vec<(String, &'static str)>,
    names: Vec<String>,
}

impl Anonymizer {
    pub fn new(profile: &str, computer_name: &str, user_names: &[&str]) -> Self {
        let mut places: Vec<(String, &'static str)> = Vec::new();
        if profile.chars().count() >= 4 {
            for variant in [
                profile.to_owned(),
                profile.replace('\\', "\\\\"),
                profile.replace('\\', "/"),
            ] {
                if !places.iter().any(|(word, _)| *word == variant) {
                    places.push((variant, "%USERPROFILE%"));
                }
            }
        }
        if computer_name.chars().count() >= 2 {
            places.push((computer_name.to_owned(), "[pc]"));
        }
        places.sort_by_key(|(word, _)| std::cmp::Reverse(word.len()));
        let mut names: Vec<String> = Vec::new();
        for word in user_names.iter().flat_map(|name| name.split_whitespace()) {
            if word.chars().count() >= 3
                && !names.iter().any(|name| name.eq_ignore_ascii_case(word))
            {
                names.push(word.to_owned());
            }
        }
        names.sort_by_key(|name| std::cmp::Reverse(name.len()));
        Self { places, names }
    }

    pub fn from_environment() -> Self {
        let profile = std::env::var("USERPROFILE").unwrap_or_default();
        let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
        let user = std::env::var("USERNAME").unwrap_or_default();
        let full_name = account_full_name(&user).unwrap_or_default();
        Self::new(
            profile.trim_end_matches('\\'),
            &computer,
            &[&user, &full_name],
        )
    }

    pub fn mask_places(&self, text: &str) -> String {
        self.places
            .iter()
            .fold(text.to_owned(), |text, (word, mask)| {
                replace_word(&text, word, mask)
            })
    }

    pub fn mask_names(&self, text: &str) -> String {
        self.names.iter().fold(text.to_owned(), |text, name| {
            replace_word(&text, name, "[user]")
        })
    }

    pub fn apply(&self, text: &str) -> String {
        self.mask_names(&self.mask_places(text))
    }

    pub fn settings(&self, text: &str) -> String {
        let masked = match serde_json::from_str::<serde_json::Value>(text) {
            Ok(mut value) => {
                if let Some(hint) = value.pointer_mut("/engine/capture/title_hint")
                    && hint.is_string()
                {
                    *hint = MASKED_TITLE.into();
                }
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_owned())
            }
            Err(_) => text.to_owned(),
        };
        self.apply(&masked)
    }
}

fn replace_word(text: &str, word: &str, mask: &str) -> String {
    let haystack = text.to_ascii_lowercase();
    let needle = word.to_ascii_lowercase();
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    let mut result = String::with_capacity(text.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(&needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char) {
            result.push_str(&text[copied..start]);
            result.push_str(mask);
            copied = end;
        }
        from = end;
    }
    result.push_str(&text[copied..]);
    result
}

pub fn entries(extra: &[(&str, String)]) -> Result<Vec<Entry>> {
    let anonymizer = Anonymizer::from_environment();
    let mut entries = vec![Entry {
        name: SYSTEM_ENTRY.to_owned(),
        contents: anonymizer.mask_places(&system_report(extra, &anonymizer)),
    }];
    if let Ok(text) = std::fs::read_to_string(crate::paths::settings_path()?) {
        entries.push(Entry {
            name: SETTINGS_ENTRY.to_owned(),
            contents: anonymizer.settings(&text),
        });
    }
    let mut logs: Vec<_> = match std::fs::read_dir(crate::paths::log_dir()?) {
        Ok(dir) => dir
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file())
            .collect(),
        Err(_) => Vec::new(),
    };
    logs.sort();
    for path in logs {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        entries.push(Entry {
            name: format!("logs/{name}"),
            contents: anonymizer.apply(&String::from_utf8_lossy(&bytes)),
        });
    }
    Ok(entries)
}

pub fn write_archive(out: &Path, entries: &[Entry]) -> Result<()> {
    let file = std::fs::File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for entry in entries {
        zip.start_file(entry.name.as_str(), options)
            .map_err(archive_error)?;
        zip.write_all(entry.contents.as_bytes())?;
    }
    zip.finish().map_err(archive_error)?;
    Ok(())
}

fn archive_error(error: zip::result::ZipError) -> EngineError {
    EngineError::Io(std::io::Error::other(error))
}

pub fn system_report(extra: &[(&str, String)], anonymizer: &Anonymizer) -> String {
    let mut report = String::new();
    let _ = writeln!(report, "PalaClips {}", crate::VERSION);
    let _ = writeln!(report, "Date : {}", crate::paths::local_timestamp());
    let _ = writeln!(report, "Fuseau horaire : {}", time_zone());
    let _ = writeln!(report, "Windows : {}", windows_version());
    if let Some(cpu) = registry_string(
        w!(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"),
        w!("ProcessorNameString"),
    ) {
        let _ = writeln!(report, "Processeur : {}", cpu.trim());
    }
    if let Some((total, available)) = crate::paths::physical_memory() {
        let _ = writeln!(
            report,
            "Mémoire : {:.1} Go, {:.1} Go libres",
            gigabytes(total),
            gigabytes(available)
        );
    }
    for adapter in adapters() {
        let _ = writeln!(report, "Carte graphique : {adapter}");
    }
    let hags = registry_dword(
        w!(r"SYSTEM\CurrentControlSet\Control\GraphicsDrivers"),
        w!("HwSchMode"),
    );
    let _ = writeln!(
        report,
        "Planification GPU matérielle : {}",
        match hags {
            Some(2) => "active",
            Some(_) => "inactive",
            None => "inconnue",
        }
    );
    let _ = writeln!(report, "Écrans :");
    for target in list_targets().unwrap_or_default() {
        if let TargetInfo::Monitor {
            device_name,
            monitor_path: _,
            name,
            width,
            height,
            refresh_rate,
            primary,
        } = target
        {
            let hdr = display_color(&device_name).is_some_and(|color| color.hdr);
            let _ = writeln!(
                report,
                "  {device_name} {name} {width}×{height} {refresh_rate} Hz{}{}",
                if hdr { " HDR" } else { "" },
                if primary { " principal" } else { "" }
            );
        }
    }
    let _ = writeln!(report, "Encodeurs :");
    match crate::encode::video_encoders() {
        Ok(encoders) => {
            for encoder in encoders {
                let _ = writeln!(
                    report,
                    "  {} ({}) {}",
                    encoder.name,
                    encoder.codec.label(),
                    encoder.failure.as_deref().unwrap_or("OK")
                );
            }
        }
        Err(error) => {
            let _ = writeln!(report, "  indisponibles : {error}");
        }
    }
    for (title, input) in [("Sorties audio :", false), ("Entrées audio :", true)] {
        let _ = writeln!(report, "{title}");
        for device in audio_devices(input).unwrap_or_default() {
            let _ = writeln!(
                report,
                "  {}{}",
                anonymizer.mask_names(&device.name),
                if device.default { " (par défaut)" } else { "" }
            );
        }
    }
    for (label, value) in extra {
        let _ = writeln!(report, "{label} : {value}");
    }
    report
}

fn time_zone() -> String {
    let mut info = DYNAMIC_TIME_ZONE_INFORMATION::default();
    // SAFETY: `info` is a valid out-pointer.
    let id = unsafe { GetDynamicTimeZoneInformation(&mut info) };
    let bias = info.Bias
        + if id == TIME_ZONE_ID_DAYLIGHT {
            info.DaylightBias
        } else {
            info.StandardBias
        };
    let offset = -bias;
    format!(
        "UTC{}{:02}:{:02} ({})",
        if offset < 0 { '-' } else { '+' },
        offset.abs() / 60,
        offset.abs() % 60,
        String::from_utf16_lossy(&info.TimeZoneKeyName).trim_end_matches('\0')
    )
}

fn account_full_name(user: &str) -> Option<String> {
    let user = windows::core::HSTRING::from(user);
    let mut buffer: *mut u8 = std::ptr::null_mut();
    // SAFETY: `buffer` receives a USER_INFO_10 allocated by the system, freed below.
    let status = unsafe { NetUserGetInfo(PCWSTR::null(), &user, 10, &mut buffer) };
    if status != 0 || buffer.is_null() {
        return None;
    }
    // SAFETY: level 10 fills `buffer` with a USER_INFO_10 whose strings live in the same block.
    let name = unsafe {
        (*buffer.cast::<USER_INFO_10>())
            .usri10_full_name
            .to_string()
    }
    .ok();
    // SAFETY: `buffer` was allocated by NetUserGetInfo and is freed once.
    unsafe { NetApiBufferFree(Some(buffer.cast::<c_void>().cast_const())) };
    name
}

fn gigabytes(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 30) as f64
}

const WINDOWS_VERSION_KEY: PCWSTR = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");

pub fn windows_too_old(build: Option<u32>) -> bool {
    build.is_some_and(|build| build < MIN_WINDOWS_BUILD)
}

pub fn windows_build() -> Option<u32> {
    registry_string(WINDOWS_VERSION_KEY, w!("CurrentBuild"))?
        .parse()
        .ok()
}

fn windows_version() -> String {
    let key = WINDOWS_VERSION_KEY;
    let build = registry_string(key, w!("CurrentBuild")).unwrap_or_default();
    let major = if build.parse::<u32>().is_ok_and(|build| build >= 22_000) {
        11
    } else {
        10
    };
    format!(
        "Windows {major} {} {}, build {build}.{}",
        registry_string(key, w!("EditionID")).unwrap_or_default(),
        registry_string(key, w!("DisplayVersion")).unwrap_or_default(),
        registry_dword(key, w!("UBR")).unwrap_or_default()
    )
}

fn adapters() -> Vec<String> {
    // SAFETY: CreateDXGIFactory1 has no preconditions.
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return Vec::new();
    };
    let mut adapters = Vec::new();
    let mut index = 0;
    // SAFETY: EnumAdapters1 fails once `index` passes the last adapter.
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
        index += 1;
        // SAFETY: the adapter comes from the factory above.
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
            continue;
        };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let name = String::from_utf16_lossy(&desc.Description);
        // SAFETY: the interface id points to a static GUID.
        let driver = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID) }
            .map(|version| {
                format!(
                    "{}.{}.{}.{}",
                    (version >> 48) & 0xffff,
                    (version >> 32) & 0xffff,
                    (version >> 16) & 0xffff,
                    version & 0xffff
                )
            })
            .unwrap_or_else(|_| "?".to_owned());
        let line = format!(
            "{}, {:.1} Go, pilote {driver}",
            name.trim_end_matches('\0'),
            gigabytes(desc.DedicatedVideoMemory as u64)
        );
        if !adapters.contains(&line) {
            adapters.push(line);
        }
    }
    adapters
}

fn registry_string(key: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut size = 0u32;
    // SAFETY: a null data pointer asks only for the size in bytes.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buffer` holds `size` bytes as returned by the first call.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(
        String::from_utf16_lossy(&buffer)
            .trim_end_matches('\0')
            .to_owned(),
    )
}

fn registry_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `data` and `size` are valid out-pointers sized for a REG_DWORD.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut data).cast::<c_void>()),
            Some(&mut size),
        )
    }
    .ok()
    .ok()?;
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymizer_masks_the_profile_folder_and_the_computer_name() {
        let anonymizer = Anonymizer::new(r"C:\Users\super", "DESKTOP-42", &[]);
        assert_eq!(
            anonymizer.apply(
                r"c:\users\SUPER\Videos et C:/Users/super/x, RTX 4070 SUPER sur desktop-42."
            ),
            r"%USERPROFILE%\Videos et %USERPROFILE%/x, RTX 4070 SUPER sur [pc]."
        );
    }

    #[test]
    fn only_a_known_build_below_20348_is_too_old() {
        assert!(windows_too_old(Some(19045)));
        assert!(!windows_too_old(Some(20348)));
        assert!(!windows_too_old(Some(26200)));
        assert!(!windows_too_old(None));
    }

    #[test]
    fn anonymizer_masks_the_account_names_in_device_names() {
        let anonymizer = Anonymizer::new("", "", &["bapco", "Baptiste de La Tour"]);
        assert_eq!(
            anonymizer.mask_names("AirPods de Baptiste, Casque de BAPCO, La Tour"),
            "AirPods de [user], Casque de [user], La [user]"
        );
    }

    #[test]
    #[ignore = "needs a local account with a full name"]
    fn account_full_name_reads_the_current_account() {
        let user = std::env::var("USERNAME").expect("USERNAME");
        assert!(account_full_name(&user).is_some_and(|name| !name.is_empty()));
    }

    #[test]
    fn anonymizer_keeps_a_longer_folder_that_starts_like_the_profile() {
        let anonymizer = Anonymizer::new(r"C:\Users\super", "", &[]);
        assert_eq!(
            anonymizer.apply(r"C:\Users\superman\Videos"),
            r"C:\Users\superman\Videos"
        );
    }

    #[test]
    fn settings_hide_the_window_title_and_the_user_folder() {
        let anonymizer = Anonymizer::new(r"C:\Users\Palawi", "PC-1", &[]);
        let settings = r#"{"engine":{"capture":{"kind":"window","exe_name":"game.exe","title_hint":"Mon document"},"storage":{"clips_dir":"C:\\Users\\Palawi\\Videos"}}}"#;
        let masked = anonymizer.settings(settings);
        assert!(masked.contains(MASKED_TITLE));
        assert!(!masked.contains("Mon document"));
        assert!(masked.contains(r"%USERPROFILE%\\Videos"));
        assert!(masked.contains("game.exe"));
    }

    #[test]
    fn archive_holds_every_entry() {
        let dir = std::env::temp_dir().join(format!("palaclips-diag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier temporaire");
        let out = dir.join("diagnostic.zip");
        let entries = vec![
            Entry {
                name: SYSTEM_ENTRY.to_owned(),
                contents: "PalaClips".to_owned(),
            },
            Entry {
                name: "logs/palaclips.log".to_owned(),
                contents: "ligne".to_owned(),
            },
        ];
        write_archive(&out, &entries).expect("archive écrite");
        let bytes = std::fs::read(&out).expect("archive relue");
        std::fs::remove_dir_all(&dir).expect("dossier supprimé");
        let names = entries.iter().filter(|entry| {
            bytes
                .windows(entry.name.len())
                .any(|window| window == entry.name.as_bytes())
        });
        assert_eq!(names.count(), 2);
        assert_eq!(&bytes[..4], b"PK\x03\x04");
    }
}
