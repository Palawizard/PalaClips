use serde::Serialize;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::IsIconic;
use windows_capture::monitor::Monitor;
use windows_capture::window::Window;

use crate::config::CaptureTarget;
use crate::error::{EngineError, MissingTarget, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetInfo {
    Monitor {
        device_name: String,
        monitor_path: Option<String>,
        name: String,
        width: u32,
        height: u32,
        refresh_rate: u32,
        primary: bool,
    },
    Window {
        exe_name: String,
        app_name: String,
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

pub fn current_device_name(
    device_name: &str,
    monitor_path: Option<&str>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match monitor_path {
        Some(path) => lookup(path),
        None => Some(device_name.to_owned()),
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
            monitor_path: super::display::monitor_path(&device_name),
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
            let path = crate::games::exe_path(process_id)?;
            let exe_name = path.file_name()?.to_string_lossy().into_owned();
            Some(TargetInfo::Window {
                app_name: crate::games::app_name(&path, &exe_name),
                exe_name,
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
            .map_err(|_| EngineError::TargetNotFound(MissingTarget::PrimaryMonitor)),
        CaptureTarget::Monitor {
            device_name,
            monitor_path,
        } => Monitor::enumerate()
            .map_err(windows_error)?
            .into_iter()
            .find(|monitor| {
                monitor.device_name().is_ok_and(|name| {
                    current_device_name(device_name, monitor_path.as_deref(), |path| {
                        super::display::device_name_of(path)
                    })
                    .is_some_and(|wanted| name.eq_ignore_ascii_case(&wanted))
                })
            })
            .map(ResolvedTarget::Monitor)
            .ok_or_else(|| {
                EngineError::TargetNotFound(MissingTarget::Monitor(match monitor_path {
                    Some(path) if device_name.is_empty() => path.clone(),
                    _ => device_name.clone(),
                }))
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
                .ok_or_else(|| EngineError::TargetNotFound(MissingTarget::Window(wanted.clone())))
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
    super::icon::app_icon_png(&crate::games::exe_path(process_id)?)
}

fn exe_name(process_id: u32) -> Option<String> {
    Some(
        crate::games::exe_path(process_id)?
            .file_name()?
            .to_string_lossy()
            .into_owned(),
    )
}

fn windows_error(error: impl std::fmt::Display) -> EngineError {
    EngineError::Capture(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(path: &str) -> Option<String> {
        (path == r"\\?\DISPLAY#GSM5B7F#UID4352").then(|| r"\\.\DISPLAY1".to_owned())
    }

    #[test]
    fn a_known_monitor_is_found_under_its_new_gdi_name() {
        assert_eq!(
            current_device_name(
                r"\\.\DISPLAY2",
                Some(r"\\?\DISPLAY#GSM5B7F#UID4352"),
                lookup
            ),
            Some(r"\\.\DISPLAY1".to_owned())
        );
    }

    #[test]
    fn an_unplugged_monitor_is_not_replaced_by_another_one() {
        assert_eq!(
            current_device_name(r"\\.\DISPLAY2", Some(r"\\?\DISPLAY#OTHER"), lookup),
            None
        );
    }

    #[test]
    fn a_setting_saved_before_the_monitor_path_keeps_its_gdi_name() {
        assert_eq!(
            current_device_name(r"\\.\DISPLAY2", None, lookup),
            Some(r"\\.\DISPLAY2".to_owned())
        );
    }
}
