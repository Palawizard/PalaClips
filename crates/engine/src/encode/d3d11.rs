use std::ffi::c_void;

use rsmpeg::avutil::{AVFrame, AVHWDeviceContext, AVHWFramesContext};
use rsmpeg::ffi;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
    D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device, ID3D11Multithread, ID3D11Texture2D,
};
use windows::core::Interface;

use crate::error::{EngineError, Result};

#[repr(C)]
struct AVD3D11VADeviceContext {
    device: *mut c_void,
    device_context: *mut c_void,
    video_device: *mut c_void,
    video_context: *mut c_void,
    lock: Option<unsafe extern "C" fn(*mut c_void)>,
    unlock: Option<unsafe extern "C" fn(*mut c_void)>,
    lock_ctx: *mut c_void,
    bind_flags: u32,
    misc_flags: u32,
}

#[repr(C)]
struct AVD3D11VAFramesContext {
    texture: *mut c_void,
    bind_flags: u32,
    misc_flags: u32,
    texture_infos: *mut c_void,
}

pub fn create_device() -> Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: all out-pointers are valid for the duration of the call.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }
    .map_err(gpu_error)?;
    device.ok_or_else(|| EngineError::Encoder("aucun périphérique Direct3D 11".into()))
}

pub fn hw_device_context(device: &ID3D11Device) -> Result<AVHWDeviceContext> {
    let multithread: ID3D11Multithread = device.cast().map_err(gpu_error)?;
    // SAFETY: plain COM call on a valid interface.
    let _was_protected = unsafe { multithread.SetMultithreadProtected(true) };

    let mut context = AVHWDeviceContext::alloc(ffi::AV_HWDEVICE_TYPE_D3D11VA);
    // SAFETY: av_hwdevice_ctx_alloc(D3D11VA) allocates an AVHWDeviceContext whose hwctx is an
    // AVD3D11VADeviceContext; FFmpeg takes ownership of the AddRef'd device and releases it on free.
    unsafe {
        let hw = &mut *(*(context.data as *mut ffi::AVHWDeviceContext))
            .hwctx
            .cast::<AVD3D11VADeviceContext>();
        hw.device = device.clone().into_raw();
    }
    context.init().map_err(ffmpeg_error)?;
    Ok(context)
}

pub fn hw_frames_context(
    device: &AVHWDeviceContext,
    width: u32,
    height: u32,
) -> Result<AVHWFramesContext> {
    let mut frames = device.hwframe_ctx_alloc();
    let data = frames.data();
    data.format = ffi::AV_PIX_FMT_D3D11;
    data.sw_format = ffi::AV_PIX_FMT_NV12;
    data.width = width as i32;
    data.height = height as i32;
    data.initial_pool_size = 0;
    // SAFETY: for a D3D11VA device, hwctx points to an AVD3D11VAFramesContext allocated by FFmpeg.
    unsafe {
        (*data.hwctx.cast::<AVD3D11VAFramesContext>()).bind_flags =
            D3D11_BIND_RENDER_TARGET.0 as u32;
    }
    frames.init().map_err(ffmpeg_error)?;
    Ok(frames)
}

pub fn frame_texture(frame: &AVFrame) -> Option<(&ID3D11Texture2D, u32)> {
    // SAFETY: for AV_PIX_FMT_D3D11 frames, data[0] is an ID3D11Texture2D owned by the frame and
    // data[1] its array index; the borrow lives as long as the frame.
    let texture = unsafe {
        ID3D11Texture2D::from_raw_borrowed(&*(&raw const frame.data[0]).cast::<*mut c_void>())
    }?;
    Some((texture, frame.data[1] as usize as u32))
}

pub(crate) fn gpu_error(error: windows::core::Error) -> EngineError {
    EngineError::Encoder(format!("Direct3D 11 : {}", error.message()))
}

pub(crate) fn ffmpeg_error(error: rsmpeg::error::RsmpegError) -> EngineError {
    EngineError::Encoder(error.to_string())
}
