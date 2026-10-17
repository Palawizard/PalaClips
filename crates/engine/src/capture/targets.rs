use serde::Serialize;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::IsIconic;
use windows::core::PWSTR;
use windows_capture::monitor::Monitor;
use windows_capture::window::Window;

use crate::config::CaptureTarget;
use crate::error::{EngineError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetInfo {
    Monitor {
        device_name: String,
        name: String,
        width: u32,
        height: u32,
        refresh_rate: u32,
        primary: bool,
    },
    Window {
        exe_name: String,
        title: String,
        process_id: u32,
        width: i32,
        height: i32,
        minimized: bool,
    },
}

const MIN_WINDOW_WIDTH: i32 = 160;
const MIN_WINDOW_HEIGHT: i32 = 90;

#[derive(Debug, Clone, Copy)]
pub enum ResolvedTarget {
    Monitor(Monitor),
    Window(Window),
}

impl ResolvedTarget {
    pub fn monitor(&self) -> Option<Monitor> {
        match self {
            ResolvedTarget::Monitor(monitor) => Some(*monitor),
            ResolvedTarget::Window(window) => window.monitor(),
        }
    }
}

pub fn list_targets() -> Result<Vec<TargetInfo>> {
    let primary = Monitor::primary()
        .and_then(|monitor| monitor.device_name())
        .ok();
    let monitors = Monitor::enumerate().map_err(windows_error)?;
    let windows = Window::enumerate().map_err(windows_error)?;

    let monitor_infos = monitors.into_iter().filter_map(|monitor| {
        let device_name = monitor.device_name().ok()?;
        Some(TargetInfo::Monitor {
            name: monitor.name().unwrap_or_else(|_| device_name.clone()),
            width: monitor.width().ok()?,
            height: monitor.height().ok()?,
            refresh_rate: monitor.refresh_rate().unwrap_or_default(),
            primary: primary.as_deref() == Some(device_name.as_str()),
            device_name,
        })
    });
    let window_infos = windows
        .into_iter()
        .filter(|window| !is_cloaked(window))
        .filter_map(|window| {
            let title = window.title().ok().filter(|title| !title.is_empty())?;
            let process_id = window.process_id().ok()?;
            // SAFETY: IsIconic only reads the state of a window handle from EnumWindows.
            let minimized = unsafe { IsIconic(HWND(window.as_raw_hwnd())) }.as_bool();
            let width = window.width().ok()?;
            let height = window.height().ok()?;
            if !minimized && (width < MIN_WINDOW_WIDTH || height < MIN_WINDOW_HEIGHT) {
                return None;
            }
            Some(TargetInfo::Window {
                exe_name: exe_name(process_id)?,
                title,
                process_id,
                width,
                height,
                minimized,
            })
        })
        .filter(|info| !matches!(info, TargetInfo::Window { process_id, .. } if *process_id == std::process::id()));

    Ok(monitor_infos.chain(window_infos).collect())
}

pub fn target_size(target: &CaptureTarget) -> Result<(u32, u32)> {
    match resolve_target(target)? {
        ResolvedTarget::Monitor(monitor) => Ok((
            monitor.width().map_err(windows_error)?,
            monitor.height().map_err(windows_error)?,
        )),
        ResolvedTarget::Window(window) => Ok((
            window.width().map_err(windows_error)?.max(1) as u32,
            window.height().map_err(windows_error)?.max(1) as u32,
        )),
    }
}

pub fn resolve_target(target: &CaptureTarget) -> Result<ResolvedTarget> {
    match target {
        CaptureTarget::PrimaryMonitor => Monitor::primary()
            .map(ResolvedTarget::Monitor)
            .map_err(|_| EngineError::TargetNotFound("aucun écran principal".into())),
        CaptureTarget::Monitor { device_name } => Monitor::enumerate()
            .map_err(windows_error)?
            .into_iter()
            .find(|monitor| {
                monitor
                    .device_name()
                    .is_ok_and(|name| name.eq_ignore_ascii_case(device_name))
            })
            .map(ResolvedTarget::Monitor)
            .ok_or_else(|| {
                EngineError::TargetNotFound(format!("l'écran {device_name} n'est pas branché"))
            }),
        CaptureTarget::Window {
            exe_name: wanted,
            title_hint,
        } => {
            let candidates: Vec<(Window, String)> = Window::enumerate()
                .map_err(windows_error)?
                .into_iter()
                .filter(|window| !is_cloaked(window))
                .filter(|window| {
                    window
                        .process_id()
                        .ok()
                        .and_then(exe_name)
                        .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
                })
                .map(|window| (window, window.title().unwrap_or_default()))
                .collect();
            let hinted = title_hint
                .as_deref()
                .and_then(|hint| candidates.iter().find(|(_, title)| title.contains(hint)));
            hinted
                .or_else(|| candidates.iter().find(|(_, title)| !title.is_empty()))
                .or(candidates.first())
                .map(|(window, _)| ResolvedTarget::Window(*window))
                .ok_or_else(|| {
                    EngineError::TargetNotFound(format!("aucune fenêtre de {wanted} n'est ouverte"))
                })
        }
    }
}

fn is_cloaked(window: &Window) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: the HWND comes from EnumWindows and DWM writes exactly size_of::<u32>() bytes into `cloaked`.
    let result = unsafe {
        DwmGetWindowAttribute(
            HWND(window.as_raw_hwnd()),
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked != 0
}

pub fn app_icon(process_id: u32) -> Option<Vec<u8>> {
    super::icon::app_icon_png(&exe_path(process_id)?)
}

fn exe_name(process_id: u32) -> Option<String> {
    Some(
        exe_path(process_id)?
            .file_name()?
            .to_string_lossy()
            .into_owned(),
    )
}

fn exe_path(process_id: u32) -> Option<std::path::PathBuf> {
    // SAFETY: PROCESS_QUERY_LIMITED_INFORMATION is the minimal right for QueryFullProcessImageNameW and works on protected processes.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut buffer = [0u16; 1024];
    let mut len = buffer.len() as u32;
    // SAFETY: `buffer` holds `len` UTF-16 units and the handle is valid until CloseHandle below.
    let queried = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
    };
    // SAFETY: the handle was opened above and is closed exactly once.
    unsafe { CloseHandle(process) }.ok()?;
    queried.ok()?;
    Some(String::from_utf16_lossy(&buffer[..len as usize]).into())
}

fn windows_error(error: impl std::fmt::Display) -> EngineError {
    EngineError::Capture(error.to_string())
}
