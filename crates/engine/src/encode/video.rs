use std::ffi::{CStr, CString};
use std::ptr::NonNull;

use rsmpeg::avcodec::{AVCodec, AVCodecContext, AVCodecParameters, AVPacket};
use rsmpeg::avutil::{AVFrame, AVHWDeviceContext, AVHWFramesContext, AVRational, ra};
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;

use super::convert::Size;
use super::d3d11::{ffmpeg_error, hw_device_context, hw_frames_context};
use super::log::route_ffmpeg_logs;
use super::threads::lower_encoder_threads;
use crate::config::VideoCodec;
use crate::error::{EngineError, Result};

pub const SOFTWARE_ENCODERS: [(&str, VideoCodec); 2] = [
    ("h264_mf", VideoCodec::H264),
    ("libsvtav1", VideoCodec::Av1),
];

pub fn is_software_encoder(name: &str) -> bool {
    SOFTWARE_ENCODERS
        .iter()
        .any(|(software, _)| *software == name)
}

pub fn system_pixel_format(encoder: &str) -> ffi::AVPixelFormat {
    if encoder == "libsvtav1" {
        ffi::AV_PIX_FMT_YUV420P
    } else {
        ffi::AV_PIX_FMT_NV12
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoEncoderSettings {
    pub encoder: String,
    pub size: Size,
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub keyframe_interval_secs: f32,
}

pub struct VideoEncoder {
    context: AVCodecContext,
    frames: Option<AVHWFramesContext>,
    qsv_frames: Option<AVHWFramesContext>,
    size: Size,
    threads_to_lower: bool,
}

impl std::fmt::Debug for VideoEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoEncoder")
            .field("encoder", &self.context.codec().name())
            .finish_non_exhaustive()
    }
}

impl VideoEncoder {
    pub fn open(device: &ID3D11Device, settings: &VideoEncoderSettings) -> Result<Self> {
        route_ffmpeg_logs();
        let name = CString::new(settings.encoder.as_str())
            .map_err(|_| EngineError::Encoder("nom d'encodeur invalide".into()))?;
        let codec = AVCodec::find_encoder_by_name(&name).ok_or_else(|| {
            EngineError::Encoder(format!("encodeur {} absent de FFmpeg", settings.encoder))
        })?;
        let (frames, qsv_frames) = if is_software_encoder(&settings.encoder) {
            (None, None)
        } else {
            let hw_device = hw_device_context(device)?;
            let frames = hw_frames_context(&hw_device, settings.size.width, settings.size.height)?;
            let qsv_frames = settings
                .encoder
                .ends_with("_qsv")
                .then(|| qsv_frames_context(&hw_device, &frames))
                .transpose()?;
            (Some(frames), qsv_frames)
        };

        let bit_rate = settings.bitrate_kbps as i64 * 1_000;
        let mut context = AVCodecContext::new(&codec);
        context.set_width(settings.size.width as i32);
        context.set_height(settings.size.height as i32);
        context.set_pix_fmt(match (&frames, &qsv_frames) {
            (None, _) => system_pixel_format(&settings.encoder),
            (Some(_), Some(_)) => ffi::AV_PIX_FMT_QSV,
            (Some(_), None) => ffi::AV_PIX_FMT_D3D11,
        });
        context.set_time_base(ra(1, settings.fps as i32));
        context.set_framerate(ra(settings.fps as i32, 1));
        context.set_gop_size(
            ((settings.keyframe_interval_secs * settings.fps as f32).round() as i32).max(1),
        );
        context.set_max_b_frames(0);
        context.set_bit_rate(bit_rate);
        context.set_flags(context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        // SAFETY: plain field writes on an allocated, not yet opened codec context.
        unsafe {
            let raw = &mut *context.as_mut_ptr();
            raw.sw_pix_fmt = ffi::AV_PIX_FMT_NV12;
            raw.rc_max_rate = bit_rate;
            raw.rc_buffer_size = bit_rate.min(i32::MAX as i64) as i32;
            raw.color_range = ffi::AVCOL_RANGE_MPEG;
            raw.colorspace = ffi::AVCOL_SPC_BT709;
            raw.color_primaries = ffi::AVCOL_PRI_BT709;
            raw.color_trc = ffi::AVCOL_TRC_BT709;
        }
        if let Some(frames) = qsv_frames.as_ref().or(frames.as_ref()) {
            context.set_hw_frames_ctx(frames.clone());
        }
        set_private_options(&mut context, &settings.encoder, settings.size, settings.fps)?;
        context
            .open(None)
            .map_err(|error| EngineError::Encoder(format!("{} : {error}", settings.encoder)))?;
        let threads_to_lower = frames.is_none();
        if threads_to_lower {
            lower_encoder_threads();
        }
        Ok(Self {
            context,
            frames,
            qsv_frames,
            size: settings.size,
            threads_to_lower,
        })
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn time_base(&self) -> AVRational {
        self.context.time_base
    }

    pub fn codec_parameters(&self) -> AVCodecParameters {
        self.context.extract_codecpar()
    }

    pub fn is_hardware(&self) -> bool {
        self.frames.is_some()
    }

    pub fn pixel_format(&self) -> ffi::AVPixelFormat {
        self.context.pix_fmt
    }

    pub fn acquire_frame(&mut self) -> Result<AVFrame> {
        let mut frame = AVFrame::new();
        match &mut self.frames {
            Some(frames) => frames.get_buffer(&mut frame).map_err(ffmpeg_error)?,
            None => {
                frame.set_width(self.size.width as i32);
                frame.set_height(self.size.height as i32);
                frame.set_format(self.context.pix_fmt);
                frame.alloc_buffer().map_err(ffmpeg_error)?;
            }
        }
        Ok(frame)
    }

    pub fn encode(
        &mut self,
        mut frame: AVFrame,
        pts: i64,
        on_packet: &mut impl FnMut(AVPacket) -> Result<()>,
    ) -> Result<()> {
        frame.set_pts(pts);
        let frame = match &self.qsv_frames {
            Some(qsv_frames) => map_to_qsv(&frame, qsv_frames)?,
            None => frame,
        };
        self.context
            .send_frame(Some(&frame))
            .map_err(ffmpeg_error)?;
        if std::mem::take(&mut self.threads_to_lower) {
            lower_encoder_threads();
        }
        self.drain(on_packet)
    }

    pub fn finish(&mut self, on_packet: &mut impl FnMut(AVPacket) -> Result<()>) -> Result<()> {
        self.context.send_frame(None).map_err(ffmpeg_error)?;
        self.drain(on_packet)
    }

    fn drain(&mut self, on_packet: &mut impl FnMut(AVPacket) -> Result<()>) -> Result<()> {
        loop {
            match self.context.receive_packet() {
                Ok(packet) => on_packet(packet)?,
                Err(RsmpegError::EncoderDrainError | RsmpegError::EncoderFlushedError) => {
                    return Ok(());
                }
                Err(error) => return Err(ffmpeg_error(error)),
            }
        }
    }
}

pub struct SystemFrameEncoder {
    context: AVCodecContext,
    threads_to_lower: bool,
}

impl std::fmt::Debug for SystemFrameEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemFrameEncoder")
            .field("encoder", &self.context.codec().name())
            .finish_non_exhaustive()
    }
}

impl SystemFrameEncoder {
    pub fn open(
        encoder: &str,
        size: Size,
        time_base: AVRational,
        frame_rate: AVRational,
        bitrate_kbps: u32,
    ) -> Result<Self> {
        route_ffmpeg_logs();
        let name = CString::new(encoder)
            .map_err(|_| EngineError::Encoder("nom d'encodeur invalide".into()))?;
        let codec = AVCodec::find_encoder_by_name(&name)
            .ok_or_else(|| EngineError::Encoder(format!("encodeur {encoder} absent de FFmpeg")))?;
        let fps = (f64::from(frame_rate.num) / f64::from(frame_rate.den.max(1))).round() as u32;
        let bit_rate = i64::from(bitrate_kbps) * 1_000;
        let mut context = AVCodecContext::new(&codec);
        context.set_width(size.width as i32);
        context.set_height(size.height as i32);
        context.set_pix_fmt(system_pixel_format(encoder));
        context.set_time_base(time_base);
        context.set_framerate(frame_rate);
        context.set_gop_size((fps * 2).max(1) as i32);
        context.set_max_b_frames(0);
        context.set_bit_rate(bit_rate);
        context.set_flags(context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        // SAFETY: plain field writes on an allocated, not yet opened codec context.
        unsafe {
            let raw = &mut *context.as_mut_ptr();
            raw.rc_max_rate = bit_rate;
            raw.rc_buffer_size = bit_rate.min(i32::MAX as i64) as i32;
        }
        set_private_options(&mut context, encoder, size, fps.max(1))?;
        context
            .open(None)
            .map_err(|error| EngineError::Encoder(format!("{encoder} : {error}")))?;
        let threads_to_lower = is_software_encoder(encoder);
        if threads_to_lower {
            lower_encoder_threads();
        }
        Ok(Self {
            context,
            threads_to_lower,
        })
    }

    pub fn codec_parameters(&self) -> AVCodecParameters {
        self.context.extract_codecpar()
    }

    pub fn pixel_format(&self) -> ffi::AVPixelFormat {
        self.context.pix_fmt
    }

    pub fn encode(
        &mut self,
        frame: Option<&AVFrame>,
        on_packet: &mut impl FnMut(AVPacket) -> Result<()>,
    ) -> Result<()> {
        self.context.send_frame(frame).map_err(ffmpeg_error)?;
        if std::mem::take(&mut self.threads_to_lower) {
            lower_encoder_threads();
        }
        loop {
            match self.context.receive_packet() {
                Ok(packet) => on_packet(packet)?,
                Err(RsmpegError::EncoderDrainError | RsmpegError::EncoderFlushedError) => {
                    return Ok(());
                }
                Err(error) => return Err(ffmpeg_error(error)),
            }
        }
    }
}

fn qsv_frames_context(
    hw_device: &AVHWDeviceContext,
    frames: &AVHWFramesContext,
) -> Result<AVHWFramesContext> {
    let mut qsv_device = hw_device
        .create_derived(ffi::AV_HWDEVICE_TYPE_QSV)
        .map_err(ffmpeg_error)?;
    let mut derived = std::ptr::null_mut();
    // SAFETY: both contexts are initialised; on success FFmpeg returns a new reference we take ownership of.
    let status = unsafe {
        ffi::av_hwframe_ctx_create_derived(
            &mut derived,
            ffi::AV_PIX_FMT_QSV,
            qsv_device.as_mut_ptr(),
            frames.as_ptr() as *mut _,
            0,
        )
    };
    match NonNull::new(derived) {
        // SAFETY: `derived` is a valid AVBufferRef owned by us from here on.
        Some(derived) if status >= 0 => Ok(unsafe { AVHWFramesContext::from_raw(derived) }),
        _ => Err(EngineError::Encoder(
            "impossible de partager les textures Direct3D 11 avec Quick Sync".into(),
        )),
    }
}

fn map_to_qsv(frame: &AVFrame, qsv_frames: &AVHWFramesContext) -> Result<AVFrame> {
    let mut mapped = AVFrame::new();
    // SAFETY: `mapped` is a fresh frame; it receives its own reference to the QSV frames context
    // before av_hwframe_map, which keeps `frame` alive through the mapping until `mapped` is freed.
    let status = unsafe {
        let raw = &mut *mapped.as_mut_ptr();
        raw.format = ffi::AV_PIX_FMT_QSV;
        raw.hw_frames_ctx = ffi::av_buffer_ref(qsv_frames.as_ptr());
        ffi::av_hwframe_map(raw, frame.as_ptr(), ffi::AV_HWFRAME_MAP_READ)
    };
    if status < 0 {
        return Err(EngineError::Encoder(
            "impossible de transmettre l'image à Quick Sync".into(),
        ));
    }
    mapped.set_pts(frame.pts);
    Ok(mapped)
}

fn nvenc_pipeline_depth(fps: u32) -> (&'static CStr, &'static CStr) {
    if fps <= 60 {
        (c"2", c"1")
    } else {
        (c"4", c"3")
    }
}

pub fn svt_preset(size: Size, fps: u32) -> &'static CStr {
    if u64::from(size.width) * u64::from(size.height) * u64::from(fps) <= 1280 * 720 * 30 {
        c"10"
    } else {
        c"12"
    }
}

fn set_private_options(
    context: &mut AVCodecContext,
    encoder: &str,
    size: Size,
    fps: u32,
) -> Result<()> {
    let options: &[(&CStr, &CStr)] = if encoder.ends_with("_nvenc") {
        let (surfaces, delay) = nvenc_pipeline_depth(fps);
        &[
            (c"preset", c"p4"),
            (c"rc", c"cbr"),
            (c"dpb_size", c"2"),
            (c"surfaces", surfaces),
            (c"delay", delay),
        ]
    } else if encoder.ends_with("_amf") {
        &[(c"rc", c"cbr"), (c"quality", c"balanced")]
    } else if encoder == "h264_mf" {
        &[(c"hw_encoding", c"0"), (c"rate_control", c"cbr")]
    } else if encoder == "libsvtav1" {
        &[
            (c"preset", svt_preset(size, fps)),
            (c"svtav1-params", c"rtc=1:rc=2:lp=2"),
        ]
    } else {
        &[]
    };
    for (key, value) in options {
        // SAFETY: priv_data is the encoder's AVOptions-enabled private context, allocated with the codec context.
        let status = unsafe {
            ffi::av_opt_set(
                (*context.as_mut_ptr()).priv_data,
                key.as_ptr(),
                value.as_ptr(),
                0,
            )
        };
        if status < 0 {
            return Err(EngineError::Encoder(format!(
                "{encoder} : option {} refusée",
                key.to_string_lossy()
            )));
        }
    }
    Ok(())
}
