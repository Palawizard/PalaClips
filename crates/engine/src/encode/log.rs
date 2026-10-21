use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Once;

use rsmpeg::ffi;

thread_local! {
    static CAPTURED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

pub fn route_ffmpeg_logs() {
    static ROUTED: Once = Once::new();
    // SAFETY: the callback is a plain function valid for the whole program and is thread-safe.
    ROUTED.call_once(|| unsafe { ffi::av_log_set_callback(Some(forward_to_tracing)) });
}

pub(crate) fn with_captured_errors<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
    route_ffmpeg_logs();
    CAPTURED.with_borrow_mut(|captured| *captured = Some(Vec::new()));
    let result = run();
    let lines = CAPTURED.with_borrow_mut(Option::take).unwrap_or_default();
    (result, lines)
}

unsafe extern "C" fn forward_to_tracing(
    class: *mut c_void,
    level: c_int,
    format: *const c_char,
    args: ffi::va_list,
) {
    if level > ffi::AV_LOG_VERBOSE as c_int {
        return;
    }
    let mut buffer = [0 as c_char; 1024];
    let mut print_prefix = 1;
    // SAFETY: arguments come straight from av_log; the buffer length is passed and FFmpeg NUL-terminates it.
    unsafe {
        ffi::av_log_format_line2(
            class,
            level,
            format,
            args,
            buffer.as_mut_ptr(),
            buffer.len() as c_int,
            &mut print_prefix,
        );
    }
    // SAFETY: av_log_format_line2 always writes a NUL-terminated string into `buffer`.
    let line = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_string_lossy();
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    let is_error = level <= ffi::AV_LOG_ERROR as c_int;
    let captured = CAPTURED.with_borrow_mut(|captured| {
        captured
            .as_mut()
            .map(|lines| {
                if is_error {
                    lines.push(line.to_owned());
                }
            })
            .is_some()
    });
    if captured {
        tracing::debug!(target: "ffmpeg", "{line}");
        return;
    }
    let level = level as u32;
    if level <= ffi::AV_LOG_ERROR {
        tracing::error!(target: "ffmpeg", "{line}");
    } else if level <= ffi::AV_LOG_WARNING {
        tracing::warn!(target: "ffmpeg", "{line}");
    } else if level <= ffi::AV_LOG_INFO {
        tracing::info!(target: "ffmpeg", "{line}");
    } else {
        tracing::debug!(target: "ffmpeg", "{line}");
    }
}
