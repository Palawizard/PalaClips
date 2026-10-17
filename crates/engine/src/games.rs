use std::ffi::c_void;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use crate::sources;

pub const DESKTOP_GAME: &str = "Bureau";

const GENERIC_NAMES: &[&str] = &[
    "unreal engine",
    "unreal engine 4",
    "unreal engine 5",
    "ue4",
    "ue5",
    "ue4game",
    "ue5game",
    "shipping",
    "unity",
    "unity player",
    "unityplayer",
    "godot engine",
    "cryengine",
    "source engine",
    "game",
    "client",
    "launcher",
    "bootstrapper",
    "application",
];

const BUILD_SUFFIXES: &[&str] = &[
    "_eac_eos",
    "_eac",
    "_eos",
    "_be",
    "-win64-shipping",
    "-wingdk-shipping",
    "-win32-shipping",
    "-win64-test",
    "-win64-development",
    "-win64-debuggame",
    "-shipping",
    "-win64",
];

const CLIENT_SUFFIX: &str = "client";

pub fn sanitize(text: &str) -> String {
    let decoded = text
        .replace("&apos;", "'")
        .replace("&quot;", "")
        .replace("&lt;", "")
        .replace("&gt;", "")
        .replace("&amp;", "&");
    let kept: String = decoded
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '™' | '®' | '©'
            ) && !c.is_control()
        })
        .collect();
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['.', ' '])
        .to_owned()
}

fn is_generic(name: &str) -> bool {
    GENERIC_NAMES.contains(&name.to_lowercase().as_str())
}

fn strip_suffix_ignore_case<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let split = text.len().checked_sub(suffix.len())?;
    (text.is_char_boundary(split) && text[split..].eq_ignore_ascii_case(suffix))
        .then(|| &text[..split])
}

pub fn exe_stem(exe_name: &str) -> &str {
    strip_suffix_ignore_case(exe_name, ".exe").unwrap_or(exe_name)
}

pub fn clean_exe_stem(exe_name: &str) -> String {
    let mut stem = exe_stem(exe_name);
    while let Some(shorter) = BUILD_SUFFIXES
        .iter()
        .find_map(|suffix| strip_suffix_ignore_case(stem, suffix))
    {
        stem = shorter;
    }
    if let Some(shorter) = strip_suffix_ignore_case(stem, CLIENT_SUFFIX)
        && !sanitize(shorter).is_empty()
    {
        stem = shorter;
    }
    sanitize(stem)
}

pub fn pick_name(product: Option<&str>, description: Option<&str>, exe_name: &str) -> String {
    [product, description]
        .into_iter()
        .flatten()
        .map(sanitize)
        .find(|name| !name.is_empty() && !is_generic(name))
        .or_else(|| Some(clean_exe_stem(exe_name)).filter(|name| !name.is_empty()))
        .or_else(|| Some(sanitize(exe_stem(exe_name))).filter(|name| !name.is_empty()))
        .unwrap_or_else(|| DESKTOP_GAME.to_owned())
}

#[derive(Debug)]
pub struct Resolved {
    pub name: String,
    pub from_metadata_lookup: bool,
}

pub fn resolve(exe_name: &str) -> Resolved {
    let path = sources::processes()
        .ok()
        .and_then(|processes| sources::root_pid(&processes, exe_name))
        .and_then(exe_path);
    let (product, description) = path.as_deref().map(version_strings).unwrap_or_default();
    Resolved {
        name: pick_name(product.as_deref(), description.as_deref(), exe_name),
        from_metadata_lookup: path.is_some(),
    }
}

fn exe_path(pid: u32) -> Option<PathBuf> {
    // SAFETY: the handle is checked and closed before returning; the buffer outlives the call.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut length = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        CloseHandle(process).ok();
        queried.ok()?;
        Some(PathBuf::from(String::from_utf16_lossy(
            &buffer[..length as usize],
        )))
    }
}

fn version_strings(path: &Path) -> (Option<String>, Option<String>) {
    let file = HSTRING::from(path.as_os_str());
    // SAFETY: the size query only reads the path.
    let size = unsafe { GetFileVersionInfoSizeW(&file, None) };
    if size == 0 {
        return (None, None);
    }
    let mut block = vec![0u8; size as usize];
    // SAFETY: `block` holds `size` bytes as requested by GetFileVersionInfoSizeW.
    if unsafe { GetFileVersionInfoW(&file, None, size, block.as_mut_ptr().cast()) }.is_err() {
        return (None, None);
    }
    let mut languages = vec!["040904b0".to_owned(), "040904e4".to_owned()];
    if let Some(translations) = query(&block, "\\VarFileInfo\\Translation") {
        for pair in translations.chunks_exact(4) {
            let language = u16::from_le_bytes([pair[0], pair[1]]);
            let codepage = u16::from_le_bytes([pair[2], pair[3]]);
            languages.insert(0, format!("{language:04x}{codepage:04x}"));
        }
    }
    let string = |key: &str| {
        languages.iter().find_map(|language| {
            let bytes = query(&block, &format!("\\StringFileInfo\\{language}\\{key}"))?;
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                .take_while(|unit| *unit != 0)
                .collect();
            Some(String::from_utf16_lossy(&units)).filter(|text| !text.trim().is_empty())
        })
    };
    (string("ProductName"), string("FileDescription"))
}

fn query<'a>(block: &'a [u8], sub_block: &str) -> Option<&'a [u8]> {
    let sub_block = HSTRING::from(sub_block);
    let mut pointer: *mut c_void = std::ptr::null_mut();
    let mut length = 0u32;
    // SAFETY: `block` is a version resource filled by GetFileVersionInfoW; the returned pointer
    // points inside it and is only read while `block` is borrowed.
    let found = unsafe {
        VerQueryValueW(
            block.as_ptr().cast(),
            PCWSTR(sub_block.as_ptr()),
            &mut pointer,
            &mut length,
        )
    };
    if !found.as_bool() || pointer.is_null() {
        return None;
    }
    let start = pointer as usize - block.as_ptr() as usize;
    let bytes = if sub_block.to_string().starts_with("\\StringFileInfo") {
        length as usize * 2
    } else {
        length as usize
    };
    block.get(start..start + bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_name_comes_first() {
        assert_eq!(
            pick_name(
                Some("Rocket League®"),
                Some("Rocket League"),
                "RocketLeague.exe"
            ),
            "Rocket League"
        );
    }

    #[test]
    fn description_replaces_an_empty_or_generic_product_name() {
        assert_eq!(
            pick_name(Some("  "), Some("Terraria"), "Terraria.exe"),
            "Terraria"
        );
        assert_eq!(
            pick_name(Some("Unreal Engine"), Some("Some Game™"), "x.exe"),
            "Some Game"
        );
    }

    #[test]
    fn generic_metadata_falls_back_to_the_cleaned_exe_name() {
        assert_eq!(
            pick_name(Some("UE4"), Some("Shipping"), "Valorant-Win64-Shipping.exe"),
            "Valorant"
        );
        assert_eq!(
            pick_name(Some("Unity Player"), None, "Hollow Knight.exe"),
            "Hollow Knight"
        );
    }

    #[test]
    fn unreal_build_suffixes_and_client_are_stripped() {
        assert_eq!(
            pick_name(None, None, "FortniteClient-Win64-Shipping.exe"),
            "Fortnite"
        );
        assert_eq!(
            pick_name(None, None, "FortniteClient-Win64-Shipping_EAC_EOS.exe"),
            "Fortnite"
        );
        assert_eq!(pick_name(None, None, "cs2.exe"), "cs2");
    }

    #[test]
    fn a_name_that_is_only_client_is_kept() {
        assert_eq!(clean_exe_stem("Client.exe"), "Client");
    }

    #[test]
    fn names_are_cleaned_for_the_file_system() {
        assert_eq!(sanitize("Half-Life: Alyx™"), "Half-Life Alyx");
        assert_eq!(sanitize("  Space   Game?.  "), "Space Game");
        assert_eq!(pick_name(Some("<>:?*"), None, "a:b?.exe"), "ab");
        assert_eq!(
            sanitize("Assassin&apos;s Creed &amp; Co"),
            "Assassin's Creed & Co"
        );
    }

    #[test]
    fn nothing_usable_gives_the_desktop_name() {
        assert_eq!(pick_name(None, None, "...exe"), DESKTOP_GAME);
    }
}
