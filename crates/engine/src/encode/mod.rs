mod convert;
mod d3d11;
mod log;
mod video;

use std::ffi::CStr;

use rsmpeg::avcodec::AVCodec;
use rsmpeg::ffi;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};

pub use convert::{GpuConverter, Size, fit_rect, output_size};
pub use d3d11::{create_device, frame_texture};
pub use log::route_ffmpeg_logs;
pub use video::{VideoEncoder, VideoEncoderSettings};

use crate::config::VideoCodec;
use crate::error::{EngineError, Result};

const PROBE_SIZE: Size = Size::new(1280, 720);
const PROBE_FPS: u32 = 60;
const PROBE_FRAMES: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
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
            let (outcome, logs) = log::with_captured_errors(|| probe_encoder(&device, &name));
            let failure = outcome.err().map(|error| {
                logs.first()
                    .map(|line| {
                        line.split_once("] ")
                            .map_or(line.as_str(), |(_, text)| text)
                    })
                    .map_or_else(|| error.to_string(), str::to_owned)
            });
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
    match codec.id {
        ffi::AV_CODEC_ID_H264 => Some(VideoCodec::H264),
        ffi::AV_CODEC_ID_HEVC => Some(VideoCodec::Hevc),
        ffi::AV_CODEC_ID_AV1 => Some(VideoCodec::Av1),
        _ => None,
    }
}

fn probe_encoder(device: &ID3D11Device, name: &str) -> Result<()> {
    let source = bgra_texture(device, PROBE_SIZE)?;
    let converter = GpuConverter::new(device, PROBE_SIZE, PROBE_SIZE, PROBE_FPS)?;
    let mut encoder = VideoEncoder::open(
        device,
        &VideoEncoderSettings {
            encoder: name.to_owned(),
            size: PROBE_SIZE,
            fps: PROBE_FPS,
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

pub fn bgra_texture(device: &ID3D11Device, size: Size) -> Result<ID3D11Texture2D> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: size.width,
        Height: size.height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    // SAFETY: `desc` describes a valid GPU-only texture and the out-pointer is valid.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(d3d11::gpu_error)?;
    texture.ok_or_else(|| EngineError::Encoder("texture BGRA non créée".into()))
}
