use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction, SessionState};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentThreadId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, DispatchMessageW, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND,
    GWL_STYLE, GetForegroundWindow, GetMessageW, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, MSG, OBJID_WINDOW, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    WINEVENT_OUTOFCONTEXT, WM_QUIT, WS_CAPTION,
};

use crate::config::CaptureTarget;
use crate::error::{EngineError, Result};

const SNAPSHOT_MAX_AGE: Duration = Duration::from_millis(500);

const SHELL_EXES: [&str; 7] = [
    "explorer.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "searchhost.exe",
    "searchapp.exe",
    "lockapp.exe",
    "applicationframehost.exe",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Process {
    pub pid: u32,
    pub parent_pid: u32,
    pub exe_name: String,
    pub created: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioSession {
    pub pid: u32,
    pub exe_name: Option<String>,
    pub display_name: String,
    pub device: String,
    pub active: bool,
    pub peak: f32,
}

pub fn processes() -> Result<Vec<Process>> {
    // SAFETY: the snapshot handle is closed below and the entry size is set before use.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|error| EngineError::Audio(error.message()))?;
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut list = Vec::new();
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            list.push(Process {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                exe_name: String::from_utf16_lossy(&entry.szExeFile[..length]),
                created: creation_time(entry.th32ProcessID),
            });
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        CloseHandle(snapshot).ok();
        drop_reused_parents(&mut list);
        Ok(list)
    }
}

pub fn recent_processes() -> Result<Arc<Vec<Process>>> {
    static LATEST: Mutex<Option<(Instant, Arc<Vec<Process>>)>> = Mutex::new(None);
    let mut latest = LATEST.lock();
    if let Some((taken, list)) = latest.as_ref()
        && taken.elapsed() < SNAPSHOT_MAX_AGE
    {
        return Ok(list.clone());
    }
    let list = Arc::new(processes()?);
    *latest = Some((Instant::now(), list.clone()));
    Ok(list)
}

fn creation_time(pid: u32) -> Option<u64> {
    // SAFETY: the handle only allows querying limited information and is closed before returning.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let result = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user);
        CloseHandle(process).ok();
        result
            .ok()
            .map(|()| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}

fn drop_reused_parents(processes: &mut [Process]) {
    let created: HashMap<u32, Option<u64>> = processes.iter().map(|p| (p.pid, p.created)).collect();
    for process in processes.iter_mut() {
        if let (Some(Some(parent)), Some(child)) =
            (created.get(&process.parent_pid), process.created)
            && *parent > child
        {
            process.parent_pid = 0;
        }
    }
}

pub fn root_pids(processes: &[Process], exe_name: &str) -> Vec<u32> {
    let same_exe: HashSet<u32> = processes
        .iter()
        .filter(|p| p.exe_name.eq_ignore_ascii_case(exe_name))
        .map(|p| p.pid)
        .collect();
    let mut roots: Vec<u32> = processes
        .iter()
        .filter(|p| same_exe.contains(&p.pid) && !same_exe.contains(&p.parent_pid))
        .map(|p| p.pid)
        .collect();
    roots.sort_unstable();
    roots
}

pub fn root_pid(processes: &[Process], exe_name: &str) -> Option<u32> {
    root_pids(processes, exe_name).first().copied()
}

pub fn process_tree(processes: &[Process], root: u32) -> HashSet<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for process in processes.iter().filter(|p| p.pid != p.parent_pid) {
        children
            .entry(process.parent_pid)
            .or_default()
            .push(process.pid);
    }
    let mut tree = HashSet::from([root]);
    let mut pending = vec![root];
    while let Some(pid) = pending.pop() {
        for child in children.get(&pid).into_iter().flatten() {
            if tree.insert(*child) {
                pending.push(*child);
            }
        }
    }
    tree
}

pub fn exe_name_of(processes: &[Process], pid: u32) -> Option<&str> {
    processes
        .iter()
        .find(|p| p.pid == pid)
        .map(|p| p.exe_name.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundWindow {
    pub pid: u32,
    pub exe_name: String,
    pub covers_monitor: bool,
    pub has_caption: bool,
}

impl ForegroundWindow {
    pub fn is_game(&self) -> bool {
        self.covers_monitor
            && !self.has_caption
            && self.pid != std::process::id()
            && !SHELL_EXES
                .iter()
                .any(|shell| self.exe_name.eq_ignore_ascii_case(shell))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CurrentGame {
    fixed: Option<String>,
    last: Option<String>,
}

impl CurrentGame {
    pub fn new(pinned: Option<String>, target: &CaptureTarget) -> Self {
        let fixed = pinned.or_else(|| match target {
            CaptureTarget::Window { exe_name, .. } => Some(exe_name.clone()),
            _ => None,
        });
        Self { fixed, last: None }
    }

    pub fn follows_foreground(&self) -> bool {
        self.fixed.is_none()
    }

    pub fn on_foreground(&mut self, window: &ForegroundWindow) -> bool {
        let unchanged = self
            .last
            .as_ref()
            .is_some_and(|last| last.eq_ignore_ascii_case(&window.exe_name));
        if !self.follows_foreground() || !window.is_game() || unchanged {
            return false;
        }
        self.last = Some(window.exe_name.clone());
        true
    }

    pub fn exe_name(&self) -> Option<&str> {
        self.fixed.as_deref().or(self.last.as_deref())
    }
}

pub fn foreground_window() -> Option<ForegroundWindow> {
    let mut window = foreground_geometry()?;
    window.exe_name = exe_name_of(&processes().ok()?, window.pid)?.to_owned();
    Some(window)
}

fn foreground_geometry() -> Option<ForegroundWindow> {
    let mut pid = 0;
    let mut rect = RECT::default();
    let mut monitor = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: read-only queries on the current foreground window; every out pointer is a local.
    let style = unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        GetWindowThreadProcessId(window, Some(&mut pid));
        GetWindowRect(window, &mut rect).ok()?;
        let screen = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(screen, &mut monitor).as_bool() {
            return None;
        }
        GetWindowLongW(window, GWL_STYLE) as u32
    };
    if pid == 0 {
        return None;
    }
    let screen = monitor.rcMonitor;
    Some(ForegroundWindow {
        pid,
        exe_name: String::new(),
        covers_monitor: rect.left <= screen.left
            && rect.top <= screen.top
            && rect.right >= screen.right
            && rect.bottom >= screen.bottom,
        has_caption: style & WS_CAPTION.0 == WS_CAPTION.0,
    })
}

pub struct ForegroundWatch {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for ForegroundWatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForegroundWatch")
            .field("thread_id", &self.thread_id)
            .finish_non_exhaustive()
    }
}

struct WatchState {
    on_change: Box<dyn FnMut(ForegroundWindow)>,
    last: Option<ForegroundWindow>,
}

thread_local! {
    static WATCH: RefCell<Option<WatchState>> = const { RefCell::new(None) };
}

impl ForegroundWatch {
    pub fn start(on_change: impl FnMut(ForegroundWindow) + Send + 'static) -> Result<Self> {
        let (ready_tx, ready) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("palaclips-foreground".into())
            .spawn(move || {
                let mut message = MSG::default();
                // SAFETY: per-thread DPI awareness and message queue, for this watcher thread only.
                let thread_id = unsafe {
                    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                    GetCurrentThreadId()
                };
                WATCH.with_borrow_mut(|state| {
                    *state = Some(WatchState {
                        on_change: Box::new(on_change),
                        last: None,
                    })
                });
                // SAFETY: out-of-context hooks delivered to this thread's message loop, removed below.
                let hooks =
                    [EVENT_SYSTEM_FOREGROUND, EVENT_OBJECT_LOCATIONCHANGE].map(|event| unsafe {
                        SetWinEventHook(
                            event,
                            event,
                            None,
                            Some(on_win_event),
                            0,
                            0,
                            WINEVENT_OUTOFCONTEXT,
                        )
                    });
                report_foreground();
                ready_tx.send(thread_id).ok();
                // SAFETY: standard message loop on the thread that owns the hooks.
                unsafe {
                    while GetMessageW(&mut message, None, 0, 0).as_bool() {
                        DispatchMessageW(&message);
                    }
                    for hook in hooks {
                        let _ = UnhookWinEvent(hook);
                    }
                }
                WATCH.with_borrow_mut(Option::take);
            })?;
        let thread_id = ready
            .recv()
            .map_err(|_| EngineError::Audio("suivi du premier plan impossible".into()))?;
        Ok(Self {
            thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for ForegroundWatch {
    fn drop(&mut self) {
        // SAFETY: the watcher thread created its message queue before publishing its id.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    if event == EVENT_OBJECT_LOCATIONCHANGE
        && (object != OBJID_WINDOW.0 || child != CHILDID_SELF as i32 || !is_foreground(window))
    {
        return;
    }
    report_foreground();
}

fn is_foreground(window: HWND) -> bool {
    // SAFETY: GetForegroundWindow has no preconditions.
    window == unsafe { GetForegroundWindow() }
}

fn report_foreground() {
    let Some(mut window) = foreground_geometry() else {
        return;
    };
    WATCH.with_borrow_mut(|state| {
        let Some(state) = state else { return };
        if let Some(last) = &state.last {
            if (last.pid, last.covers_monitor, last.has_caption)
                == (window.pid, window.covers_monitor, window.has_caption)
            {
                return;
            }
            if last.pid == window.pid {
                window.exe_name = last.exe_name.clone();
            }
        }
        if window.exe_name.is_empty() {
            let Some(exe_name) = processes()
                .ok()
                .and_then(|list| exe_name_of(&list, window.pid).map(str::to_owned))
            else {
                return;
            };
            window.exe_name = exe_name;
        }
        state.last = Some(window.clone());
        (state.on_change)(window);
    });
}

pub fn audio_sessions(processes: &[Process]) -> Result<Vec<AudioSession>> {
    let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
    let mut sessions = Vec::new();
    for device in &enumerator
        .get_device_collection(&Direction::Render)
        .map_err(audio_error)?
    {
        let Ok(device) = device else { continue };
        let Ok(name) = device.get_friendlyname() else {
            continue;
        };
        let Ok(manager) = device.get_iaudiosessionmanager() else {
            continue;
        };
        let Ok(list) = manager.get_audiosessionenumerator() else {
            continue;
        };
        for index in 0..list.get_count().unwrap_or(0) {
            let Ok(control) = list.get_session(index) else {
                continue;
            };
            let Ok(pid) = control.get_process_id() else {
                continue;
            };
            if pid == 0 {
                continue;
            }
            let peak = control
                .get_audiometerinformation()
                .and_then(|meter| meter.get_peak_value())
                .unwrap_or(0.0);
            sessions.push(AudioSession {
                pid,
                exe_name: exe_name_of(processes, pid).map(str::to_owned),
                display_name: control.get_display_name().unwrap_or_default(),
                device: name.clone(),
                active: matches!(control.get_state(), Ok(SessionState::Active)),
                peak,
            });
        }
    }
    Ok(sessions)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

pub fn audio_devices(input: bool) -> Result<Vec<AudioDevice>> {
    let _ = wasapi::initialize_mta();
    let direction = if input {
        Direction::Capture
    } else {
        Direction::Render
    };
    let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
    let default = enumerator
        .get_default_device(&direction)
        .ok()
        .and_then(|device| device.get_id().ok());
    let mut devices = Vec::new();
    for device in &enumerator
        .get_device_collection(&direction)
        .map_err(audio_error)?
    {
        let Ok(device) = device else { continue };
        let (Ok(id), Ok(name)) = (device.get_id(), device.get_friendlyname()) else {
            continue;
        };
        devices.push(AudioDevice {
            default: default.as_deref() == Some(id.as_str()),
            id,
            name,
        });
    }
    Ok(devices)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppSession {
    pub exe_name: String,
    pub app_name: String,
    pub process_id: u32,
    pub peak: f32,
}

pub fn app_sessions() -> Result<Vec<AppSession>> {
    static APP_NAMES: parking_lot::Mutex<Option<HashMap<String, String>>> =
        parking_lot::Mutex::new(None);
    let _ = wasapi::initialize_mta();
    let processes = processes()?;
    let own = std::process::id();
    let mut by_exe: HashMap<String, AppSession> = HashMap::new();
    for session in audio_sessions(&processes)? {
        let Some(exe_name) = session.exe_name.filter(|_| session.pid != own) else {
            continue;
        };
        let key = exe_name.to_lowercase();
        let entry = by_exe.entry(key.clone()).or_insert_with(|| {
            let process_id = root_pid(&processes, &exe_name).unwrap_or(session.pid);
            let app_name = APP_NAMES
                .lock()
                .get_or_insert_with(HashMap::new)
                .entry(key)
                .or_insert_with(|| {
                    crate::games::exe_path(process_id).map_or_else(
                        || crate::games::clean_exe_stem(&exe_name),
                        |path| crate::games::app_name(&path, &exe_name),
                    )
                })
                .clone();
            AppSession {
                exe_name,
                app_name,
                process_id,
                peak: 0.0,
            }
        });
        entry.peak = entry.peak.max(session.peak);
    }
    let mut sessions: Vec<AppSession> = by_exe.into_values().collect();
    sessions.sort_by(|a, b| a.app_name.cmp(&b.app_name));
    Ok(sessions)
}

pub fn session_peak(roots: &[u32]) -> Result<f32> {
    let loud: Vec<AudioSession> = audio_sessions(&[])?
        .into_iter()
        .filter(|session| session.peak > 0.0 && session.pid != std::process::id())
        .collect();
    if loud.is_empty() {
        return Ok(0.0);
    }
    let processes = processes()?;
    let tree: HashSet<u32> = roots
        .iter()
        .flat_map(|root| process_tree(&processes, *root))
        .collect();
    Ok(loud
        .iter()
        .filter(|session| tree.contains(&session.pid))
        .map(|session| session.peak)
        .fold(0.0, f32::max))
}

fn audio_error(error: wasapi::WasapiError) -> EngineError {
    EngineError::Audio(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent_pid: u32, exe_name: &str) -> Process {
        Process {
            pid,
            parent_pid,
            exe_name: exe_name.into(),
            created: Some(u64::from(pid)),
        }
    }

    fn browser_and_friends() -> Vec<Process> {
        vec![
            process(4, 0, "System"),
            process(900, 4, "explorer.exe"),
            process(1200, 900, "chrome.exe"),
            process(1210, 1200, "chrome.exe"),
            process(1220, 1200, "chrome.exe"),
            process(1230, 1220, "audiodg.exe"),
            process(2000, 900, "Update.exe"),
            process(2100, 31337, "Discord.exe"),
            process(2110, 2100, "Discord.exe"),
        ]
    }

    #[test]
    fn root_pid_is_the_process_whose_parent_has_another_exe() {
        let list = browser_and_friends();
        assert_eq!(root_pid(&list, "chrome.exe"), Some(1200));
        assert_eq!(root_pid(&list, "CHROME.EXE"), Some(1200));
        assert_eq!(root_pid(&list, "discord.exe"), Some(2100));
        assert_eq!(root_pid(&list, "vlc.exe"), None);
    }

    #[test]
    fn every_independent_instance_of_an_exe_is_a_root() {
        let list = vec![
            process(900, 4, "explorer.exe"),
            process(12900, 16772, "Vesktop.exe"),
            process(12424, 16772, "Vesktop.exe"),
            process(27692, 12424, "Vesktop.exe"),
            process(32216, 12900, "Vesktop.exe"),
        ];
        assert_eq!(root_pids(&list, "vesktop.exe"), vec![12424, 12900]);
        assert_eq!(root_pids(&list, "vlc.exe"), Vec::<u32>::new());
    }

    #[test]
    fn process_tree_includes_every_descendant() {
        let list = browser_and_friends();
        assert_eq!(
            process_tree(&list, 1200),
            HashSet::from([1200, 1210, 1220, 1230])
        );
        assert_eq!(process_tree(&list, 2110), HashSet::from([2110]));
    }

    #[test]
    fn a_reused_parent_pid_does_not_adopt_an_older_process() {
        let mut list = vec![
            process(17652, 900, "WindowsTerminal.exe"),
            process(22376, 17652, "python.exe"),
            Process {
                created: Some(100),
                ..process(12752, 22376, "zen.exe")
            },
        ];
        drop_reused_parents(&mut list);
        assert_eq!(list[2].parent_pid, 0);
        assert_eq!(list[1].parent_pid, 17652);
        assert_eq!(process_tree(&list, 17652), HashSet::from([17652, 22376]));
    }

    #[test]
    fn unknown_creation_times_keep_the_parent_link() {
        let mut list = vec![
            Process {
                created: None,
                ..process(500, 4, "a.exe")
            },
            process(10, 500, "b.exe"),
        ];
        drop_reused_parents(&mut list);
        assert_eq!(list[1].parent_pid, 500);
    }

    fn window(exe_name: &str, covers_monitor: bool, has_caption: bool) -> ForegroundWindow {
        ForegroundWindow {
            pid: 4242,
            exe_name: exe_name.into(),
            covers_monitor,
            has_caption,
        }
    }

    fn monitor_capture() -> CurrentGame {
        CurrentGame::new(None, &CaptureTarget::PrimaryMonitor)
    }

    fn window_capture(exe_name: &str) -> CaptureTarget {
        CaptureTarget::Window {
            exe_name: exe_name.into(),
            title_hint: None,
        }
    }

    #[test]
    fn fullscreen_and_borderless_windows_become_the_game() {
        let mut game = monitor_capture();
        assert!(game.on_foreground(&window("FortniteClient-Win64-Shipping.exe", true, false)));
        assert_eq!(game.exe_name(), Some("FortniteClient-Win64-Shipping.exe"));
        assert!(game.on_foreground(&window("cs2.exe", true, false)));
        assert_eq!(game.exe_name(), Some("cs2.exe"));
    }

    #[test]
    fn alt_tab_to_a_windowed_or_maximized_app_keeps_the_game() {
        let mut game = monitor_capture();
        game.on_foreground(&window("cs2.exe", true, false));
        assert!(!game.on_foreground(&window("Discord.exe", false, true)));
        assert!(!game.on_foreground(&window("chrome.exe", true, true)));
        assert!(!game.on_foreground(&window("WindowsTerminal.exe", false, true)));
        assert_eq!(game.exe_name(), Some("cs2.exe"));
    }

    #[test]
    fn a_windowed_app_is_never_the_game() {
        let mut game = monitor_capture();
        assert!(!game.on_foreground(&window("Discord.exe", false, true)));
        assert!(!game.on_foreground(&window("popup.exe", false, false)));
        assert_eq!(game.exe_name(), None);
    }

    #[test]
    fn desktop_shell_and_palaclips_never_become_the_game() {
        let mut game = monitor_capture();
        assert!(!game.on_foreground(&window("explorer.exe", true, false)));
        assert!(!game.on_foreground(&window("SearchHost.exe", true, false)));
        let own = ForegroundWindow {
            pid: std::process::id(),
            ..window("palaclips.exe", true, false)
        };
        assert!(!game.on_foreground(&own));
        assert_eq!(game.exe_name(), None);
    }

    #[test]
    fn the_same_game_in_another_case_is_not_a_change() {
        let mut game = monitor_capture();
        game.on_foreground(&window("cs2.exe", true, false));
        assert!(!game.on_foreground(&window("CS2.EXE", true, false)));
    }

    #[test]
    fn a_window_capture_follows_the_captured_app() {
        let mut game = CurrentGame::new(None, &window_capture("Minecraft.Windows.exe"));
        assert!(!game.follows_foreground());
        assert!(!game.on_foreground(&window("cs2.exe", true, false)));
        assert_eq!(game.exe_name(), Some("Minecraft.Windows.exe"));
    }

    #[test]
    fn a_pinned_process_wins_over_everything() {
        let target = window_capture("Minecraft.Windows.exe");
        let mut game = CurrentGame::new(Some("javaw.exe".into()), &target);
        assert!(!game.on_foreground(&window("cs2.exe", true, false)));
        assert_eq!(game.exe_name(), Some("javaw.exe"));
        let pinned = CurrentGame::new(Some("javaw.exe".into()), &CaptureTarget::PrimaryMonitor);
        assert_eq!(pinned.exe_name(), Some("javaw.exe"));
    }

    #[test]
    fn process_tree_survives_pid_cycles() {
        let list = vec![process(10, 20, "a.exe"), process(20, 10, "b.exe")];
        assert_eq!(process_tree(&list, 10), HashSet::from([10, 20]));
    }
}
