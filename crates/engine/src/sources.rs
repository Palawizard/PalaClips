use std::collections::{HashMap, HashSet};

use serde::Serialize;
use wasapi::{DeviceEnumerator, Direction, SessionState};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
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
            });
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        CloseHandle(snapshot).ok();
        Ok(list)
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

pub fn session_peak(processes: &[Process], root: u32) -> Result<f32> {
    let tree = process_tree(processes, root);
    Ok(audio_sessions(processes)?
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
    fn process_tree_survives_pid_cycles() {
        let list = vec![process(10, 20, "a.exe"), process(20, 10, "b.exe")];
        assert_eq!(process_tree(&list, 10), HashSet::from([10, 20]));
    }
}
