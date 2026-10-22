mod audio;
mod convert;
mod d3d11;
mod hdr;
mod log;
mod mf;
mod readback;
pub mod threads;
mod thumbnail;
mod video;

use std::ffi::CStr;

use rsmpeg::avcodec::AVCodec;
use rsmpeg::ffi;
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

pub use audio::AacEncoder;
pub use convert::{GpuConverter, Size, fit_rect, output_size};
pub use d3d11::{create_device, frame_texture, render_texture, staging_texture};
pub use hdr::{HdrToSdr, blob_bytes, compile_shader};
pub use log::route_ffmpeg_logs;
pub use readback::{FrameConverter, SystemConverter};
pub use thumbnail::{THUMBNAIL_WIDTH, encode_jpeg, encode_png_rgba, write_thumbnail};
pub use video::{
    SOFTWARE_ENCODERS, SystemFrameEncoder, VideoEncoder, VideoEncoderSettings, is_software_encoder,
    svt_preset, system_pixel_format,
};

use crate::config::{EncoderChoice, VideoCodec};
use crate::error::{EngineError, Result};

const PROBE_SIZE: Size = Size::new(1280, 720);
const PROBE_FPS: u32 = 60;
const PROBE_FRAMES: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EncoderInfo {
    pub name: String,
    pub description: String,
    pub codec: VideoCodec,
    pub hardware: bool,
    pub failure: Option<String>,
}

impl EncoderInfo {
    pub fn is_available(&self) -> bool {
        self.failure.is_none()
    }
}

pub fn ffmpeg_version() -> String {
    route_ffmpeg_logs();
    // SAFETY: av_version_info returns a static NUL-terminated string owned by libavutil.
    unsafe { CStr::from_ptr(ffi::av_version_info()) }
        .to_string_lossy()
        .into_owned()
}

pub fn video_encoders() -> Result<Vec<EncoderInfo>> {
    let device = create_device()?;
    let probe = |codec: &AVCodec, codec_kind: VideoCodec, hardware: bool| {
        let name = codec.name().to_string_lossy().into_owned();
        let (outcome, logs) =
            log::with_captured_errors(|| probe_encoder(&device, &name, PROBE_SIZE, PROBE_FPS));
        EncoderInfo {
            description: codec.long_name().to_string_lossy().into_owned(),
            codec: codec_kind,
            hardware,
            failure: outcome.err().map(|error| failure_reason(&error, &logs)),
            name,
        }
    };
    let hardware = AVCodec::iterate()
        .filter(|codec| accepts_d3d11_frames(codec))
        .filter_map(|codec| Some(probe(&codec, video_codec(&codec)?, true)));
    let software = SOFTWARE_ENCODERS.iter().filter_map(|(name, codec_kind)| {
        let codec = AVCodec::find_encoder_by_name(&std::ffi::CString::new(*name).ok()?)?;
        Some(probe(&codec, *codec_kind, false))
    });
    Ok(hardware.chain(software).collect())
}

pub fn pick_encoder(
    encoders: &[EncoderInfo],
    codec: VideoCodec,
    choice: EncoderChoice,
) -> Result<&EncoderInfo> {
    let available = |hardware: bool| {
        encoders
            .iter()
            .filter(move |encoder| encoder.hardware == hardware && encoder.is_available())
    };
    let hardware = match choice {
        EncoderChoice::Hardware => true,
        EncoderChoice::Cpu => false,
        EncoderChoice::Auto => available(true).next().is_some(),
    };
    if available(hardware).next().is_none() {
        return Err(if hardware {
            EngineError::NoHardwareEncoder
        } else {
            EngineError::NoVideoEncoder
        });
    }
    if let Some(encoder) = available(hardware).find(|encoder| encoder.codec == codec) {
        return Ok(encoder);
    }
    if hardware {
        return Err(EngineError::CodecUnavailable(codec));
    }
    let fallback = available(false)
        .next()
        .ok_or(EngineError::CodecUnavailable(codec))?;
    tracing::warn!(
        wanted = codec.label(),
        encoder = %fallback.name,
        "codec indisponible sur le processeur : autre encodeur CPU"
    );
    Ok(fallback)
}

pub fn max_output_side(codec: VideoCodec) -> u32 {
    match codec {
        VideoCodec::H264 => 4096,
        VideoCodec::Hevc | VideoCodec::Av1 => 8192,
    }
}

pub fn check_output_size(codec: VideoCodec, size: Size) -> Result<()> {
    let limit = max_output_side(codec);
    if size.width > limit || size.height > limit {
        return Err(EngineError::OutputTooLarge {
            codec,
            limit,
            width: size.width,
            height: size.height,
        });
    }
    Ok(())
}

pub fn try_output(
    encoders: &[EncoderInfo],
    codec: VideoCodec,
    choice: EncoderChoice,
    size: Size,
    fps: u32,
) -> Result<()> {
    check_output_size(codec, size)?;
    probe_output(encoders, codec, choice, size, fps)
}

pub fn probe_output(
    encoders: &[EncoderInfo],
    codec: VideoCodec,
    choice: EncoderChoice,
    size: Size,
    fps: u32,
) -> Result<()> {
    let encoder = pick_encoder(encoders, codec, choice)?;
    let device = create_device()?;
    let (outcome, logs) =
        log::with_captured_errors(|| probe_encoder(&device, &encoder.name, size, fps));
    outcome.map_err(|error| {
        tracing::warn!(
            encoder = %encoder.name,
            width = size.width,
            height = size.height,
            fps,
            reason = %failure_reason(&error, &logs),
            "sortie vidéo refusée par l'encodeur"
        );
        EngineError::OutputUnsupported {
            codec,
            width: size.width,
            height: size.height,
            fps,
        }
    })
}

fn failure_reason(error: &EngineError, logs: &[String]) -> String {
    logs.first()
        .map(|line| {
            line.split_once("] ")
                .map_or(line.as_str(), |(_, text)| text)
        })
        .map_or_else(|| error.to_string(), str::to_owned)
}

fn accepts_d3d11_frames(codec: &AVCodec) -> bool {
    // SAFETY: the codec comes from av_codec_iterate and lives for the whole program.
    let is_encoder = unsafe { ffi::av_codec_is_encoder(codec.as_ptr()) } != 0;
    is_encoder
        && codec.type_ == ffi::AVMEDIA_TYPE_VIDEO
        && (0..)
            .map_while(|index| codec.hw_config(index))
            .any(|config| {
                matches!(config.pix_fmt, ffi::AV_PIX_FMT_D3D11 | ffi::AV_PIX_FMT_QSV)
                    && config.methods & ffi::AV_CODEC_HW_CONFIG_METHOD_HW_FRAMES_CTX != 0
            })
}

fn video_codec(codec: &AVCodec) -> Option<VideoCodec> {
    codec_of_id(codec.id)
}

pub fn codec_of_id(id: ffi::AVCodecID) -> Option<VideoCodec> {
    match id {
        ffi::AV_CODEC_ID_H264 => Some(VideoCodec::H264),
        ffi::AV_CODEC_ID_HEVC => Some(VideoCodec::Hevc),
        ffi::AV_CODEC_ID_AV1 => Some(VideoCodec::Av1),
        _ => None,
    }
}

fn probe_encoder(device: &ID3D11Device, name: &str, size: Size, fps: u32) -> Result<()> {
    let source = render_texture(device, PROBE_SIZE, DXGI_FORMAT_B8G8R8A8_UNORM)?;
    let mut encoder = VideoEncoder::open(
        device,
        &VideoEncoderSettings {
            encoder: name.to_owned(),
            size,
            fps,
            bitrate_kbps: 5_000,
            keyframe_interval_secs: 1.0,
        },
    )?;
    let mut converter = FrameConverter::new(device, PROBE_SIZE, size, fps, &encoder)?;
    let mut packets = 0;
    let mut count = |_| {
        packets += 1;
        Ok(())
    };
    for pts in 0..PROBE_FRAMES {
        let mut frame = encoder.acquire_frame()?;
        converter.convert(&source, &mut frame)?;
        encoder.encode(frame, pts, &mut count)?;
    }
    encoder.finish(&mut count)?;
    if packets == 0 {
        return Err(EngineError::Encoder(format!(
            "{name} n'a produit aucune image"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoder(name: &str, codec: VideoCodec, hardware: bool, works: bool) -> EncoderInfo {
        EncoderInfo {
            name: name.into(),
            description: String::new(),
            codec,
            hardware,
            failure: (!works).then(|| "échec".into()),
        }
    }

    fn picked(encoders: &[EncoderInfo], codec: VideoCodec, choice: EncoderChoice) -> String {
        match pick_encoder(encoders, codec, choice) {
            Ok(encoder) => encoder.name.clone(),
            Err(error) => error.localized().code.to_owned(),
        }
    }

    #[test]
    fn auto_prefers_the_graphics_card_and_falls_back_to_the_processor() {
        let gpu = [
            encoder("h264_nvenc", VideoCodec::H264, true, true),
            encoder("h264_mf", VideoCodec::H264, false, true),
            encoder("libsvtav1", VideoCodec::Av1, false, true),
        ];
        assert_eq!(
            picked(&gpu, VideoCodec::H264, EncoderChoice::Auto),
            "h264_nvenc"
        );
        assert_eq!(
            picked(&gpu, VideoCodec::H264, EncoderChoice::Cpu),
            "h264_mf"
        );
        assert_eq!(
            picked(&gpu, VideoCodec::Av1, EncoderChoice::Auto),
            "codec_unavailable"
        );
        let no_gpu = [
            encoder("h264_nvenc", VideoCodec::H264, true, false),
            encoder("h264_mf", VideoCodec::H264, false, true),
            encoder("libsvtav1", VideoCodec::Av1, false, true),
        ];
        assert_eq!(
            picked(&no_gpu, VideoCodec::H264, EncoderChoice::Auto),
            "h264_mf"
        );
        assert_eq!(
            picked(&no_gpu, VideoCodec::Av1, EncoderChoice::Auto),
            "libsvtav1"
        );
        assert_eq!(
            picked(&no_gpu, VideoCodec::H264, EncoderChoice::Hardware),
            "no_hardware_encoder"
        );
        assert_eq!(
            picked(&no_gpu, VideoCodec::Hevc, EncoderChoice::Auto),
            "h264_mf"
        );
    }

    #[test]
    fn without_media_foundation_h264_falls_back_to_svt_av1() {
        let n_edition = [
            encoder("h264_nvenc", VideoCodec::H264, true, false),
            encoder("h264_mf", VideoCodec::H264, false, false),
            encoder("libsvtav1", VideoCodec::Av1, false, true),
        ];
        assert_eq!(
            picked(&n_edition, VideoCodec::H264, EncoderChoice::Auto),
            "libsvtav1"
        );
        assert_eq!(
            picked(&n_edition, VideoCodec::H264, EncoderChoice::Cpu),
            "libsvtav1"
        );
        let mf_missing = [encoder("libsvtav1", VideoCodec::Av1, false, true)];
        assert_eq!(
            picked(&mf_missing, VideoCodec::H264, EncoderChoice::Auto),
            "libsvtav1"
        );
        assert_eq!(
            picked(&n_edition, VideoCodec::H264, EncoderChoice::Hardware),
            "no_hardware_encoder"
        );
    }

    #[test]
    fn nothing_that_works_is_reported_as_no_encoder_at_all() {
        let none = [
            encoder("h264_nvenc", VideoCodec::H264, true, false),
            encoder("h264_mf", VideoCodec::H264, false, false),
        ];
        assert_eq!(
            picked(&none, VideoCodec::H264, EncoderChoice::Auto),
            "no_video_encoder"
        );
        assert_eq!(
            picked(&none, VideoCodec::H264, EncoderChoice::Cpu),
            "no_video_encoder"
        );
    }

    #[test]
    fn h264_accepts_exactly_4096_and_refuses_wider() {
        assert!(check_output_size(VideoCodec::H264, Size::new(4096, 2160)).is_ok());
        assert!(check_output_size(VideoCodec::H264, Size::new(4098, 1152)).is_err());
        assert!(check_output_size(VideoCodec::H264, Size::new(1080, 4098)).is_err());
    }

    #[test]
    fn super_ultrawide_native_needs_hevc_or_av1() {
        let size = Size::new(5120, 1440);
        assert!(check_output_size(VideoCodec::H264, size).is_err());
        assert!(check_output_size(VideoCodec::Hevc, size).is_ok());
        assert!(check_output_size(VideoCodec::Av1, size).is_ok());
        assert!(check_output_size(VideoCodec::Hevc, Size::new(8194, 2160)).is_err());
    }

    #[test]
    fn refusal_names_the_codec_and_the_size() {
        let message = check_output_size(VideoCodec::H264, Size::new(5120, 1440))
            .expect_err("5120 px exceeds the H.264 limit")
            .to_string();
        assert!(message.contains("H.264"), "{message}");
        assert!(message.contains("5120×1440"), "{message}");
    }
}
