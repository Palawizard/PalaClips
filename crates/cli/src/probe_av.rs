use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, EndPaint, FillRect, GetStockObject, HBRUSH, PAINTSTRUCT, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetSystemMetrics, MSG,
    MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT, RegisterClassW, SM_CXSCREEN,
    SM_CYSCREEN, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ERASEBKGND, WM_PAINT,
    WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
};
use windows::core::w;

const SAMPLE_RATE: usize = 48_000;

static WHITE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ToneWindow {
    Borderless,
    Normal,
}

pub fn probe_av_emitter(seconds: u64, device: Option<&str>) -> Result<()> {
    let device_id = device
        .map(|name| -> Result<String> {
            Ok(crate::probe_audio::find_device(Direction::Render, name)?.get_id()?)
        })
        .transpose()?;
    println!("émetteur : flash + bip de 50 ms chaque seconde pendant {seconds} s");
    palaclips_engine::calibrate::emit_flashes(seconds as u32, device_id.as_deref())?;
    Ok(())
}

pub fn play_tone(
    hz: f64,
    amplitude: f64,
    seconds: u64,
    device: Option<&str>,
    window: Option<ToneWindow>,
) -> Result<()> {
    initialize_mta()
        .ok()
        .map_err(|error| anyhow!("COM : {error}"))?;
    let window = window
        .map(|kind| match kind {
            ToneWindow::Borderless => create_window(WINDOW_EX_STYLE(0), WS_POPUP, None),
            ToneWindow::Normal => create_window(
                WINDOW_EX_STYLE(0),
                WS_OVERLAPPEDWINDOW,
                Some((200, 200, 640, 360)),
            ),
        })
        .transpose()?;
    let device = match device {
        Some(name) => crate::probe_audio::find_device(Direction::Render, name)?,
        None => DeviceEnumerator::new()?.get_default_device(&Direction::Render)?,
    };
    let mut client = device.get_iaudioclient()?;
    client.initialize_client(
        &WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE, 2, None),
        &Direction::Render,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 200_000,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let render = client.get_audiorenderclient()?;
    println!(
        "sinusoïde {hz} Hz, amplitude {amplitude}, sur « {} » pendant {seconds} s (PID {})",
        device.get_friendlyname()?,
        std::process::id()
    );
    client.start_stream()?;
    let deadline = Hns::now() + Hns(seconds as i64 * HNS_PER_SECOND);
    let mut position = 0u64;
    let mut bytes = Vec::new();
    while Hns::now() < deadline {
        // SAFETY: the event handle is owned by `event`, alive for the whole loop.
        let woken = unsafe {
            MsgWaitForMultipleObjects(Some(&[event.as_handle()]), false, 1_000, QS_ALLINPUT)
        };
        if woken.0 == 1 {
            pump_messages();
            continue;
        }
        let frames = client.get_available_space_in_frames()? as usize;
        bytes.clear();
        for frame in position..position + frames as u64 {
            let phase = (frame as f64 * hz / SAMPLE_RATE as f64).fract();
            let sample = (amplitude * (phase * std::f64::consts::TAU).sin()) as f32;
            bytes.extend_from_slice(&sample.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        render.write_to_device(frames, &bytes, None)?;
        position += frames as u64;
    }
    client.stop_stream()?;
    if let Some(window) = window {
        // SAFETY: the window was created on this thread and is destroyed once.
        unsafe { DestroyWindow(window)? };
    }
    Ok(())
}

fn create_window(
    ex_style: WINDOW_EX_STYLE,
    style: WINDOW_STYLE,
    rect: Option<(i32, i32, i32, i32)>,
) -> Result<HWND> {
    // SAFETY: plain Win32 window creation on the current thread with a static class name.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: w!("PalaClipsAvEmitter"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let (x, y, width, height) = rect.unwrap_or_else(|| {
            (
                0,
                0,
                GetSystemMetrics(SM_CXSCREEN),
                GetSystemMetrics(SM_CYSCREEN),
            )
        });
        let window = CreateWindowExW(
            ex_style,
            w!("PalaClipsAvEmitter"),
            w!("PalaClips - émetteur A/V"),
            style | WS_VISIBLE,
            x,
            y,
            width,
            height,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        Ok(window)
    }
}

fn pump_messages() {
    let mut message = MSG::default();
    // SAFETY: standard message pump on the thread that owns the window.
    unsafe {
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let brush = if WHITE.load(Ordering::Relaxed) {
                WHITE_BRUSH
            } else {
                BLACK_BRUSH
            };
            // SAFETY: WM_PAINT handling on the window's own thread with a stock brush.
            unsafe {
                let dc = BeginPaint(window, &mut paint);
                FillRect(dc, &paint.rcPaint, HBRUSH(GetStockObject(brush).0));
                let _ = EndPaint(window, &paint);
            }
            LRESULT(0)
        }
        // SAFETY: default handling for every other message.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
