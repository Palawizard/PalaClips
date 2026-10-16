use std::collections::{HashMap, HashSet};

use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction, SessionState};
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::error::{EngineError, Result};

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

pub fn root_pid(processes: &[Process], exe_name: &str) -> Option<u32> {
    let same_exe: HashSet<u32> = processes
        .iter()
        .filter(|p| p.exe_name.eq_ignore_ascii_case(exe_name))
        .map(|p| p.pid)
        .collect();
    processes
        .iter()
        .filter(|p| same_exe.contains(&p.pid) && !same_exe.contains(&p.parent_pid))
        .map(|p| p.pid)
        .min()
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

pub fn foreground_pid() -> Option<u32> {
    let mut pid = 0;
    // SAFETY: GetWindowThreadProcessId only writes the PID of the window passed in.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    (pid != 0).then_some(pid)
}

pub fn foreground_app(processes: &[Process]) -> Option<String> {
    let pid = foreground_pid()?;
    if pid == std::process::id() {
        return None;
    }
    let exe = exe_name_of(processes, pid)?;
    (!SHELL_EXES
        .iter()
        .any(|shell| exe.eq_ignore_ascii_case(shell)))
    .then(|| exe.to_owned())
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

pub fn session_peak(root: u32) -> Result<f32> {
    let loud: Vec<AudioSession> = audio_sessions(&[])?
        .into_iter()
        .filter(|session| session.peak > 0.0 && session.pid != std::process::id())
        .collect();
    if loud.is_empty() {
        return Ok(0.0);
    }
    let tree = process_tree(&processes()?, root);
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

    #[test]
    fn process_tree_survives_pid_cycles() {
        let list = vec![process(10, 20, "a.exe"), process(20, 10, "b.exe")];
        assert_eq!(process_tree(&list, 10), HashSet::from([10, 20]));
    }
}
