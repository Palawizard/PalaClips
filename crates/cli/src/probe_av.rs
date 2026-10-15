use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow};
use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, EndPaint, FillRect, GetStockObject, HBRUSH, InvalidateRect,
    PAINTSTRUCT, UpdateWindow, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetSystemMetrics, MSG,
    MsgWaitForMultipleObjects, PM_REMOVE, PeekMessageW, QS_ALLINPUT, RegisterClassW, SM_CXSCREEN,
    SM_CYSCREEN, TranslateMessage, WM_ERASEBKGND, WM_PAINT, WNDCLASSW, WS_EX_TOPMOST, WS_POPUP,
    WS_VISIBLE,
};
use windows::core::w;

const SAMPLE_RATE: usize = 48_000;
const BEEP_FRAMES: usize = SAMPLE_RATE / 20;

static WHITE: AtomicBool = AtomicBool::new(false);

pub fn probe_av_emitter(seconds: u64) -> Result<()> {
    initialize_mta()
        .ok()
        .map_err(|error| anyhow!("COM : {error}"))?;
    let window = create_window()?;
    let device = DeviceEnumerator::new()?.get_default_device(&Direction::Render)?;
    let mut client = device.get_iaudioclient()?;
    let (default_period, _) = client.get_device_period()?;
    client.initialize_client(
        &WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE, 2, None),
        &Direction::Render,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: default_period,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let render = client.get_audiorenderclient()?;
    // SAFETY: GetStreamLatency only reads the state of an initialized audio client.
    let stream_latency = unsafe { client.as_iaudioclient().GetStreamLatency() }.unwrap_or(-1);
    println!(
        "émetteur : « {} », tampon {} images, latence du flux {:.1} ms, flash + bip de 50 ms chaque seconde pendant {seconds} s",
        device.get_friendlyname()?,
        client.get_buffer_size()?,
        stream_latency as f64 / 10_000.0
    );
    client.start_stream()?;

    let deadline = Hns::now() + Hns(seconds as i64 * HNS_PER_SECOND);
    let mut position = 0usize;
    let mut next_beep = SAMPLE_RATE;
    let mut beep_start: Option<usize> = None;
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
        let mut flash = None;
        if position >= next_beep {
            beep_start = Some(position);
            next_beep += SAMPLE_RATE;
            flash = Some(true);
        }
        bytes.clear();
        for frame in position..position + frames {
            let sample = match beep_start {
                Some(start) if frame - start < BEEP_FRAMES => {
                    0.5 * ((frame - start) as f32 * 2.0 * std::f32::consts::PI * 1_000.0
                        / SAMPLE_RATE as f32)
                        .sin()
                }
                _ => 0.0,
            };
            bytes.extend_from_slice(&sample.to_le_bytes());
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        if beep_start.is_some_and(|start| position + frames >= start + BEEP_FRAMES) {
            beep_start = None;
            flash = flash.or(Some(false));
        }
        render.write_to_device(frames, &bytes, None)?;
        position += frames;
        if let Some(white) = flash {
            WHITE.store(white, Ordering::Relaxed);
            // SAFETY: `window` is a live window created on this thread.
            unsafe {
                let _ = InvalidateRect(Some(window), None, false);
                let _ = UpdateWindow(window);
            }
        }
    }
    client.stop_stream()?;
    // SAFETY: the window was created on this thread and is destroyed once.
    unsafe { DestroyWindow(window)? };
    Ok(())
}

fn create_window() -> Result<HWND> {
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
        let window = CreateWindowExW(
            WS_EX_TOPMOST,
            w!("PalaClipsAvEmitter"),
            w!("PalaClips - émetteur A/V"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
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
