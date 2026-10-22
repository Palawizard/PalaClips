use std::collections::BTreeSet;
use std::ffi::c_void;
use std::path::Path;

use windows::Wdk::System::Threading::{NtQueryInformationThread, ThreadQuerySetWin32StartAddress};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, GetThreadPriority, OpenThread, SetThreadPriority,
    THREAD_PRIORITY_BELOW_NORMAL, THREAD_QUERY_INFORMATION, THREAD_SET_INFORMATION,
};
use windows::core::PCWSTR;

const ENCODER_MODULES: [&str; 2] = ["mfh264enc.dll", "avcodec-"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadInfo {
    pub id: u32,
    pub module: Option<String>,
    pub priority: i32,
}

pub fn thread_ids() -> BTreeSet<u32> {
    let mut ids = BTreeSet::new();
    // SAFETY: plain Win32 calls; the snapshot handle is closed before returning.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
            return ids;
        };
        let process = GetCurrentProcessId();
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut found = Thread32First(snapshot, &mut entry).is_ok();
        while found {
            if entry.th32OwnerProcessID == process {
                ids.insert(entry.th32ThreadID);
            }
            found = Thread32Next(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    ids
}

pub fn thread_info(id: u32) -> Option<ThreadInfo> {
    let handle = open(id, false)?;
    let mut start: *mut c_void = std::ptr::null_mut();
    // SAFETY: `handle` is a live thread handle opened with query rights; `start` receives one pointer.
    let (status, priority) = unsafe {
        let status = NtQueryInformationThread(
            handle,
            ThreadQuerySetWin32StartAddress,
            (&raw mut start).cast(),
            size_of::<*mut c_void>() as u32,
            std::ptr::null_mut(),
        );
        (status, GetThreadPriority(handle))
    };
    close(handle);
    Some(ThreadInfo {
        id,
        module: status.is_ok().then(|| module_of(start)).flatten(),
        priority,
    })
}

pub fn lower_encoder_threads() -> usize {
    let lowered = thread_ids()
        .into_iter()
        .filter_map(thread_info)
        .filter(|info| info.priority > THREAD_PRIORITY_BELOW_NORMAL.0)
        .filter(|info| info.module.as_deref().is_some_and(is_encoder_module))
        .filter(|info| lower_priority(info.id))
        .count();
    if lowered > 0 {
        tracing::debug!(lowered, "fils d'encodage passés en priorité basse");
    }
    lowered
}

fn is_encoder_module(module: &str) -> bool {
    ENCODER_MODULES
        .iter()
        .any(|prefix| module.starts_with(prefix))
}

pub fn lower_priority(id: u32) -> bool {
    let Some(handle) = open(id, true) else {
        return false;
    };
    // SAFETY: `handle` was opened with THREAD_SET_INFORMATION.
    let lowered = unsafe { SetThreadPriority(handle, THREAD_PRIORITY_BELOW_NORMAL) }.is_ok();
    close(handle);
    lowered
}

fn open(id: u32, write: bool) -> Option<HANDLE> {
    let access = if write {
        THREAD_QUERY_INFORMATION | THREAD_SET_INFORMATION
    } else {
        THREAD_QUERY_INFORMATION
    };
    // SAFETY: OpenThread has no preconditions; failure is returned as an error.
    unsafe { OpenThread(access, false, id) }.ok()
}

fn close(handle: HANDLE) {
    // SAFETY: the handle comes from OpenThread and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
}

pub(crate) fn module_of(address: *mut c_void) -> Option<String> {
    let mut module = HMODULE::default();
    let mut path = [0u16; 260];
    // SAFETY: with FROM_ADDRESS the "name" is any address inside a module; UNCHANGED_REFCOUNT
    // keeps the module's reference count untouched, and `path` is a valid output buffer.
    let length = unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(address.cast()),
            &mut module,
        )
        .ok()?;
        GetModuleFileNameW(Some(module), &mut path)
    };
    let path = String::from_utf16_lossy(&path[..length as usize]);
    Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_encoder_modules_are_lowered() {
        assert!(is_encoder_module("mfh264enc.dll"));
        assert!(is_encoder_module("avcodec-62.dll"));
        assert!(!is_encoder_module("palaclips.exe"));
        assert!(!is_encoder_module("ntdll.dll"));
        assert!(!is_encoder_module("audioses.dll"));
    }

    #[test]
    fn the_current_thread_is_listed_with_its_module() {
        // SAFETY: GetCurrentThreadId has no preconditions.
        let current = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
        assert!(thread_ids().contains(&current));
        let info = thread_info(current).expect("own thread is readable");
        assert!(info.module.is_some(), "{info:?}");
    }
}
