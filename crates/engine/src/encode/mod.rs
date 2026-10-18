mod audio;
mod convert;
mod d3d11;
mod hdr;
mod log;
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
pub use hdr::HdrToSdr;
pub use log::route_ffmpeg_logs;
pub use thumbnail::{THUMBNAIL_WIDTH, encode_jpeg, encode_png_rgba, write_thumbnail};
pub use video::{SystemFrameEncoder, VideoEncoder, VideoEncoderSettings};

use crate::config::VideoCodec;
use crate::error::{EngineError, Result};

const PROBE_SIZE: Size = Size::new(1280, 720);
const PROBE_FPS: u32 = 60;
const PROBE_FRAMES: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HardwareEncoder {
    pub name: String,
    pub description: String,
    pub codec: VideoCodec,
    pub failure: Option<String>,
}

impl HardwareEncoder {
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

pub fn hardware_video_encoders() -> Result<Vec<HardwareEncoder>> {
    let device = create_device()?;
    Ok(AVCodec::iterate()
        .filter(|codec| accepts_d3d11_frames(codec))
        .filter_map(|codec| {
            let codec_kind = video_codec(&codec)?;
            let name = codec.name().to_string_lossy().into_owned();
            let (outcome, logs) =
                log::with_captured_errors(|| probe_encoder(&device, &name, PROBE_SIZE, PROBE_FPS));
            let failure = outcome.err().map(|error| failure_reason(&error, &logs));
            Some(HardwareEncoder {
                description: codec.long_name().to_string_lossy().into_owned(),
                codec: codec_kind,
                name,
                failure,
            })
        })
        .collect())
}

pub fn pick_encoder(encoders: &[HardwareEncoder], codec: VideoCodec) -> Result<&HardwareEncoder> {
    if !encoders.iter().any(HardwareEncoder::is_available) {
        return Err(EngineError::NoHardwareEncoder);
    }
    encoders
        .iter()
        .find(|encoder| encoder.codec == codec && encoder.is_available())
        .ok_or(EngineError::CodecUnavailable(codec))
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
    encoders: &[HardwareEncoder],
    codec: VideoCodec,
    size: Size,
    fps: u32,
) -> Result<()> {
    check_output_size(codec, size)?;
    probe_output(encoders, codec, size, fps)
}

pub fn probe_output(
    encoders: &[HardwareEncoder],
    codec: VideoCodec,
    size: Size,
    fps: u32,
) -> Result<()> {
    let encoder = pick_encoder(encoders, codec)?;
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
    let converter = GpuConverter::new(device, PROBE_SIZE, size, fps)?;
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
    let mut packets = 0;
    let mut count = |_| {
        packets += 1;
        Ok(())
    };
    for pts in 0..PROBE_FRAMES {
        let frame = encoder.acquire_frame()?;
        let (texture, index) = frame_texture(&frame)
            .ok_or_else(|| EngineError::Encoder("image matérielle sans texture".into()))?;
        converter.convert(&source, texture, index)?;
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
