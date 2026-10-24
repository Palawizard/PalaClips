use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rsmpeg::avcodec::{AVCodecContext, AVPacket};
use rsmpeg::avfilter::{AVFilter, AVFilterGraph, AVFilterInOut};
use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::avutil::{AVFrame, AVRational, ra};
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swresample::SwrContext;
use serde::{Deserialize, Serialize};

use super::export::{
    MIX_CHANNELS, MIX_RATE, Mixer, decoder, ffmpeg_error, frames, interleaved, seconds, timestamp,
};
use super::media::open_input;
use super::stream_title;
use crate::config::{EncoderChoice, VideoCodec};
use crate::encode::{AacEncoder, EncoderInfo, Size, SystemFrameEncoder, pick_encoder};
use crate::error::{EngineError, Result};
use crate::mux::{Mp4Writer, MuxStream};

const SIZE_MARGIN: f64 = 0.97;
const CONTAINER_SHARE: f64 = 0.01;
const CONTAINER_FIXED_BYTES: f64 = 65_536.0;
const MAX_ATTEMPTS: u32 = 3;
const RETRY_EXTRA: f64 = 1.1;
pub const MIN_VIDEO_KBPS: u32 = 100;
const MAX_FPS: u32 = 60;
const LOW_FPS: u32 = 30;
const HEIGHTS: [u32; 4] = [1440, 1080, 720, 480];
const SHARP_HEIGHT: u32 = 720;
const GOOD_BPP: f64 = 0.10;
const FAIR_BPP: f64 = 0.07;
const AV1_BPP_SCALE: f64 = 0.7;
const AUDIO_TIERS: [(f64, u32); 2] = [(3_000.0, 128), (1_000.0, 96)];
const AUDIO_FLOOR_KBPS: u32 = 64;
const WHOLE_FILE_SLACK_SECS: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressCodec {
    H264,
    Av1,
}

impl CompressCodec {
    fn video_codec(self) -> VideoCodec {
        match self {
            Self::H264 => VideoCodec::H264,
            Self::Av1 => VideoCodec::Av1,
        }
    }

    fn bpp_scale(self) -> f64 {
        match self {
            Self::H264 => 1.0,
            Self::Av1 => AV1_BPP_SCALE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub hdr: bool,
    pub audio_tracks: Vec<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Good,
    Fair,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompressPlan {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_kbps: u32,
    pub audio_kbps: u32,
    pub audio_tracks: usize,
    #[serde(default)]
    pub copy_bytes: Option<u64>,
}

impl CompressPlan {
    pub fn quality(&self, codec: CompressCodec) -> Quality {
        if self.copy_bytes.is_some() {
            return Quality::Good;
        }
        let pixels = f64::from(self.width) * f64::from(self.height) * f64::from(self.fps.max(1));
        let bpp = f64::from(self.video_kbps) * 1_000.0 / pixels / codec.bpp_scale();
        if bpp >= GOOD_BPP {
            Quality::Good
        } else if bpp >= FAIR_BPP {
            Quality::Fair
        } else {
            Quality::Low
        }
    }

    pub fn estimated_bytes(&self, duration_secs: f64) -> u64 {
        if let Some(bytes) = self.copy_bytes {
            return bytes;
        }
        let kbps =
            f64::from(self.video_kbps) + f64::from(self.audio_kbps) * self.audio_tracks as f64;
        let payload = kbps * 1_000.0 / 8.0 * duration_secs;
        (payload * (1.0 + CONTAINER_SHARE) + CONTAINER_FIXED_BYTES) as u64
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CompressRequest {
    pub source: PathBuf,
    pub output: PathBuf,
    pub target_bytes: u64,
    pub codec: CompressCodec,
    pub all_tracks: bool,
    #[serde(default)]
    pub start_secs: f64,
    #[serde(default)]
    pub end_secs: Option<f64>,
    pub plan: CompressPlan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompressResult {
    pub path: String,
    pub bytes: u64,
    pub attempts: u32,
    pub plan: CompressPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CompressProgress {
    pub attempt: u32,
    pub fraction: f64,
}

fn budget_kbps(target_bytes: u64, duration_secs: f64) -> f64 {
    let target = target_bytes as f64;
    let usable = target * SIZE_MARGIN - target * CONTAINER_SHARE - CONTAINER_FIXED_BYTES;
    usable.max(0.0) * 8.0 / duration_secs.max(0.1) / 1_000.0
}

fn audio_kbps_for(total_kbps: f64) -> u32 {
    AUDIO_TIERS
        .iter()
        .find(|(floor, _)| total_kbps >= *floor)
        .map_or(AUDIO_FLOOR_KBPS, |(_, kbps)| *kbps)
}

fn even(value: f64) -> u32 {
    ((value / 2.0).round() as u32 * 2).max(2)
}

fn width_for(source: &SourceInfo, height: u32) -> u32 {
    even(f64::from(height) * f64::from(source.width) / f64::from(source.height.max(1)))
}

fn bpp(source: &SourceInfo, height: u32, fps: u32, video_kbps: u32) -> f64 {
    let pixels = f64::from(width_for(source, height)) * f64::from(height) * f64::from(fps.max(1));
    f64::from(video_kbps) * 1_000.0 / pixels
}

fn size_and_fps(source: &SourceInfo, video_kbps: u32, codec: CompressCodec) -> (u32, u32) {
    let scale = codec.bpp_scale();
    let source_fps = (source.fps.round() as u32).clamp(1, MAX_FPS);
    let mut heights: Vec<u32> = std::iter::once(source.height)
        .chain(HEIGHTS)
        .filter(|height| *height <= source.height)
        .collect();
    heights.dedup();
    let sharp: Vec<u32> = heights
        .iter()
        .copied()
        .filter(|h| *h >= SHARP_HEIGHT)
        .collect();
    let rates: Vec<u32> = if source_fps > LOW_FPS {
        vec![source_fps, LOW_FPS]
    } else {
        vec![source_fps]
    };
    for fps in &rates {
        if let Some(height) = sharp
            .iter()
            .find(|height| bpp(source, **height, *fps, video_kbps) >= GOOD_BPP * scale)
        {
            return (*height, *fps);
        }
        if let Some(lowest) = sharp.last()
            && bpp(source, *lowest, *fps, video_kbps) >= FAIR_BPP * scale
        {
            return (*lowest, *fps);
        }
    }
    let fps = rates.last().copied().unwrap_or(source_fps);
    (heights.last().copied().unwrap_or(source.height), fps)
}

pub fn plan(
    source: &SourceInfo,
    target_bytes: u64,
    codec: CompressCodec,
    all_tracks: bool,
    duration_secs: f64,
) -> Result<CompressPlan> {
    let tracks = if all_tracks {
        source.audio_tracks.len()
    } else {
        source.audio_tracks.len().min(1)
    };
    let total = budget_kbps(target_bytes, duration_secs);
    let audio_kbps = if tracks == 0 {
        0
    } else {
        audio_kbps_for(total)
    };
    let video = total - f64::from(audio_kbps) * tracks as f64;
    if video < f64::from(MIN_VIDEO_KBPS) {
        return Err(EngineError::TargetTooSmall {
            min_mb: minimum_mb(duration_secs, tracks),
        });
    }
    let source_kbps = source.size_bytes as f64 * 8.0 / source.duration_secs.max(0.1) / 1_000.0;
    let video_kbps = video.min(source_kbps.max(f64::from(MIN_VIDEO_KBPS))) as u32;
    let whole = duration_secs >= source.duration_secs - WHOLE_FILE_SLACK_SECS;
    if whole && source.size_bytes <= target_bytes {
        return Ok(CompressPlan {
            width: source.width,
            height: source.height,
            fps: (source.fps.round() as u32).max(1),
            video_kbps: source_kbps as u32,
            audio_kbps: 0,
            audio_tracks: tracks,
            copy_bytes: Some(source.size_bytes),
        });
    }
    let (height, fps) = size_and_fps(source, video_kbps, codec);
    Ok(CompressPlan {
        width: width_for(source, height),
        height,
        fps,
        video_kbps,
        audio_kbps,
        audio_tracks: tracks,
        copy_bytes: None,
    })
}

fn minimum_mb(duration_secs: f64, tracks: usize) -> u64 {
    let kbps = f64::from(MIN_VIDEO_KBPS) + f64::from(AUDIO_FLOOR_KBPS) * tracks as f64;
    let bytes = (kbps * 1_000.0 / 8.0 * duration_secs + CONTAINER_FIXED_BYTES)
        / (SIZE_MARGIN - CONTAINER_SHARE);
    (bytes / 1_000_000.0).ceil() as u64
}

fn retried_kbps(plan: &CompressPlan, written: u64, target: u64, duration_secs: f64) -> u32 {
    let excess_bytes = written as f64 - target as f64 * SIZE_MARGIN;
    let cut = excess_bytes.max(0.0) * 8.0 / duration_secs.max(0.1) / 1_000.0 * RETRY_EXTRA;
    (f64::from(plan.video_kbps) - cut).max(0.0) as u32
}

pub fn compressed_name(source: &Path, target_bytes: u64) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = source.parent().unwrap_or(Path::new(""));
    let label = format!("{}MB", target_bytes / 1_000_000);
    let mut path = dir.join(format!("{stem}_{label}.mp4"));
    let mut index = 2;
    while path.exists() {
        path = dir.join(format!("{stem}_{label}_{index}.mp4"));
        index += 1;
    }
    path
}

fn is_hdr(transfer: ffi::AVColorTransferCharacteristic) -> bool {
    transfer == ffi::AVCOL_TRC_SMPTE2084 || transfer == ffi::AVCOL_TRC_ARIB_STD_B67
}

pub fn probe_source(path: &Path) -> Result<SourceInfo> {
    let input = open_input(path).map_err(|_| EngineError::NotAVideo)?;
    let video = input
        .streams()
        .iter()
        .find(|stream| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_VIDEO)
        .ok_or(EngineError::NotAVideo)?;
    let parameters = video.codecpar();
    if parameters.width <= 0
        || parameters.height <= 0
        || rsmpeg::avcodec::AVCodec::find_decoder(parameters.codec_id).is_none()
    {
        return Err(EngineError::NotAVideo);
    }
    let rate = if video.avg_frame_rate.num > 0 && video.avg_frame_rate.den > 0 {
        video.avg_frame_rate
    } else {
        video.r_frame_rate
    };
    let fps = if rate.den > 0 {
        f64::from(rate.num) / f64::from(rate.den)
    } else {
        30.0
    };
    let duration_secs = if input.duration > 0 {
        input.duration as f64 / f64::from(ffi::AV_TIME_BASE)
    } else {
        seconds(video.duration, video.time_base)
    };
    if duration_secs.is_nan() || duration_secs <= 0.0 {
        return Err(EngineError::NotAVideo);
    }
    Ok(SourceInfo {
        duration_secs,
        width: parameters.width as u32,
        height: parameters.height as u32,
        fps: if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            30.0
        },
        hdr: is_hdr(parameters.color_trc),
        audio_tracks: input
            .streams()
            .iter()
            .filter(|stream| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_AUDIO)
            .map(|stream| stream_title(stream).unwrap_or_default())
            .collect(),
        size_bytes: std::fs::metadata(path)?.len(),
    })
}

pub fn compress(
    request: &CompressRequest,
    encoders: &[EncoderInfo],
    progress: &mut dyn FnMut(CompressProgress),
    cancel: &AtomicBool,
) -> Result<CompressResult> {
    lower_thread_priority();
    let source = probe_source(&request.source)?;
    let end = request
        .end_secs
        .unwrap_or(source.duration_secs)
        .min(source.duration_secs);
    let start = request.start_secs.clamp(0.0, end);
    let duration = end - start;
    if duration < 0.5 {
        return Err(EngineError::ExportTooShort);
    }
    let partial = super::partial_path(&request.output);
    if let Some(dir) = partial.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut plan = request.plan.clone();
    if plan.copy_bytes.is_some() {
        let copied = remux(
            request,
            &source,
            &partial,
            &mut |fraction| {
                progress(CompressProgress {
                    attempt: 1,
                    fraction,
                })
            },
            cancel,
        );
        match copied {
            Err(EngineError::CompressCancelled) => {
                std::fs::remove_file(&partial).ok();
                return Err(EngineError::CompressCancelled);
            }
            Ok(()) if std::fs::metadata(&partial)?.len() <= request.target_bytes => {
                let written = std::fs::metadata(&partial)?.len();
                std::fs::rename(&partial, &request.output)?;
                tracing::info!(written, "compression : copie sans réencodage");
                return Ok(CompressResult {
                    path: request.output.to_string_lossy().into_owned(),
                    bytes: written,
                    attempts: 0,
                    plan,
                });
            }
            result => {
                if let Err(error) = result {
                    tracing::warn!(%error, "copie sans réencodage impossible, réencodage");
                }
                std::fs::remove_file(&partial).ok();
                let encoded_size = source.size_bytes.saturating_add(1);
                let reencoded = SourceInfo {
                    size_bytes: encoded_size,
                    ..source.clone()
                };
                plan = self::plan(
                    &reencoded,
                    request.target_bytes,
                    request.codec,
                    request.all_tracks,
                    duration,
                )?;
            }
        }
    }
    for attempt in 1..=MAX_ATTEMPTS {
        let encoded = Encode::open(request, &source, &plan, encoders, start, end, &partial)
            .and_then(|encode| {
                encode.run(
                    &mut |fraction| progress(CompressProgress { attempt, fraction }),
                    cancel,
                )
            });
        if let Err(error) = encoded {
            std::fs::remove_file(&partial).ok();
            return Err(error);
        }
        let written = std::fs::metadata(&partial)?.len();
        tracing::info!(
            attempt,
            written,
            target = request.target_bytes,
            video_kbps = plan.video_kbps,
            width = plan.width,
            height = plan.height,
            fps = plan.fps,
            "compression : essai terminé"
        );
        if written <= request.target_bytes {
            std::fs::rename(&partial, &request.output)?;
            return Ok(CompressResult {
                path: request.output.to_string_lossy().into_owned(),
                bytes: written,
                attempts: attempt,
                plan,
            });
        }
        std::fs::remove_file(&partial).ok();
        plan.video_kbps = retried_kbps(&plan, written, request.target_bytes, duration);
        if plan.video_kbps < MIN_VIDEO_KBPS {
            break;
        }
    }
    Err(EngineError::OverTarget)
}

fn remux(
    request: &CompressRequest,
    source: &SourceInfo,
    partial: &Path,
    progress: &mut dyn FnMut(f64),
    cancel: &AtomicBool,
) -> Result<()> {
    let mut input = open_input(&request.source)?;
    let mut specs = Vec::new();
    let mut outputs = std::collections::HashMap::new();
    let mut bases = Vec::new();
    let mut audio = 0;
    for (index, stream) in input.streams().iter().enumerate() {
        let kind = stream.codecpar().codec_type;
        let keep = if kind == ffi::AVMEDIA_TYPE_VIDEO {
            !specs
                .iter()
                .any(|spec: &MuxStream| spec.frame_rate.is_some())
        } else if kind == ffi::AVMEDIA_TYPE_AUDIO {
            audio += 1;
            request.all_tracks || audio == 1
        } else {
            false
        };
        if !keep {
            continue;
        }
        let rate = stream.avg_frame_rate;
        outputs.insert(index, specs.len());
        bases.push(stream.time_base);
        specs.push(MuxStream {
            codec_parameters: stream.codecpar().clone(),
            time_base: stream.time_base,
            frame_rate: (kind == ffi::AVMEDIA_TYPE_VIDEO).then(|| {
                if rate.num > 0 && rate.den > 0 {
                    (rate.num / rate.den).max(1) as u32
                } else {
                    (source.fps.round() as u32).max(1)
                }
            }),
            title: stream_title(stream),
        });
    }
    let mut writer = Mp4Writer::create(partial, &specs)?;
    while let Some(packet) = input.read_packet().map_err(ffmpeg_error)? {
        if cancel.load(Ordering::Relaxed) {
            return Err(EngineError::CompressCancelled);
        }
        let Some(output) = outputs.get(&(packet.stream_index as usize)).copied() else {
            continue;
        };
        progress(
            (seconds(packet.dts, bases[output]) / source.duration_secs.max(0.1)).clamp(0.0, 1.0),
        );
        writer.write(output, packet)?;
    }
    progress(1.0);
    writer.finish()
}

fn lower_thread_priority() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: GetCurrentThread returns a pseudo handle valid for the calling thread.
    let lowered = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };
    if let Err(error) = lowered {
        tracing::warn!(%error, "priorité basse de la compression non appliquée");
    }
}

fn pixel_format_name(format: ffi::AVPixelFormat) -> Result<String> {
    // SAFETY: av_get_pix_fmt_name returns a static string or null for an unknown format.
    let name = unsafe { ffi::av_get_pix_fmt_name(format) };
    if name.is_null() {
        return Err(EngineError::Encoder("format d'image inconnu".into()));
    }
    // SAFETY: checked non-null above; the string is static and nul-terminated.
    Ok(unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned())
}

const TONEMAP: &str = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,";

struct VideoChain {
    graph: AVFilterGraph,
}

impl VideoChain {
    fn open(
        decoder: &AVCodecContext,
        time_base: AVRational,
        hdr: bool,
        plan: &CompressPlan,
        format: ffi::AVPixelFormat,
    ) -> Result<Self> {
        let graph = AVFilterGraph::new();
        let args = CString::new(format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect=1/1",
            decoder.width, decoder.height, decoder.pix_fmt, time_base.num, time_base.den
        ))
        .map_err(|_| EngineError::Encoder("paramètres de filtre invalides".into()))?;
        let buffer = AVFilter::get_by_name(c"buffer")
            .ok_or_else(|| EngineError::Encoder("filtre buffer absent".into()))?;
        let sink = AVFilter::get_by_name(c"buffersink")
            .ok_or_else(|| EngineError::Encoder("filtre buffersink absent".into()))?;
        {
            let mut source_context = graph
                .create_filter_context(&buffer, c"in", Some(&args))
                .map_err(ffmpeg_error)?;
            let mut sink_context = graph
                .create_filter_context(&sink, c"out", None)
                .map_err(ffmpeg_error)?;
            let spec = CString::new(format!(
                "{}fps={},scale={}:{}:flags=bicubic,format={}",
                if hdr { TONEMAP } else { "" },
                plan.fps,
                plan.width,
                plan.height,
                pixel_format_name(format)?
            ))
            .map_err(|_| EngineError::Encoder("filtre invalide".into()))?;
            let outputs = AVFilterInOut::new(c"in", &mut source_context, 0);
            let inputs = AVFilterInOut::new(c"out", &mut sink_context, 0);
            graph
                .parse_ptr(&spec, Some(inputs), Some(outputs))
                .map_err(ffmpeg_error)?;
            graph.config().map_err(ffmpeg_error)?;
        }
        Ok(Self { graph })
    }

    fn push(&mut self, frame: Option<AVFrame>) -> Result<Vec<AVFrame>> {
        let mut source = self
            .graph
            .get_filter(c"in")
            .ok_or_else(|| EngineError::Encoder("filtre d'entrée perdu".into()))?;
        source
            .buffersrc_add_frame(frame, None)
            .map_err(ffmpeg_error)?;
        let mut sink = self
            .graph
            .get_filter(c"out")
            .ok_or_else(|| EngineError::Encoder("filtre de sortie perdu".into()))?;
        let mut out = Vec::new();
        loop {
            match sink.buffersink_get_frame(None) {
                Ok(frame) => out.push(frame),
                Err(RsmpegError::BufferSinkDrainError | RsmpegError::BufferSinkEofError) => {
                    return Ok(out);
                }
                Err(error) => return Err(ffmpeg_error(error)),
            }
        }
    }
}

struct AudioOut {
    stream: usize,
    time_base: AVRational,
    decoder: AVCodecContext,
    resampler: Option<SwrContext>,
    mixer: Mixer,
    encoder: AacEncoder,
    done: bool,
}

struct Encode {
    input: AVFormatContextInput,
    writer: Mp4Writer,
    video: usize,
    video_base: AVRational,
    decoder: AVCodecContext,
    chain: VideoChain,
    encoder: SystemFrameEncoder,
    audio: Vec<AudioOut>,
    start: f64,
    end: f64,
    threads_lowered: bool,
}

impl Encode {
    fn open(
        request: &CompressRequest,
        source: &SourceInfo,
        plan: &CompressPlan,
        encoders: &[EncoderInfo],
        start: f64,
        end: f64,
        partial: &Path,
    ) -> Result<Self> {
        let mut input = open_input(&request.source)?;
        let streams: Vec<(
            ffi::AVMediaType,
            rsmpeg::avcodec::AVCodecParameters,
            AVRational,
            Option<String>,
        )> = input
            .streams()
            .iter()
            .map(|stream| {
                (
                    stream.codecpar().codec_type,
                    stream.codecpar().clone(),
                    stream.time_base,
                    stream_title(stream),
                )
            })
            .collect();
        let video = streams
            .iter()
            .position(|(kind, ..)| *kind == ffi::AVMEDIA_TYPE_VIDEO)
            .ok_or(EngineError::NotAVideo)?;
        let (_, video_parameters, video_base, _) = &streams[video];
        let decoder = decoder(video_parameters, *video_base)?;
        let codec = request.codec.video_codec();
        let chosen = pick_encoder(encoders, codec, EncoderChoice::Auto)
            .or_else(|_| pick_encoder(encoders, codec, EncoderChoice::Cpu))?;
        let fps = plan.fps.max(1) as i32;
        let encoder = SystemFrameEncoder::open(
            &chosen.name,
            Size::new(plan.width, plan.height),
            ra(1, fps),
            ra(fps, 1),
            plan.video_kbps,
        )?;
        let chain = VideoChain::open(
            &decoder,
            *video_base,
            source.hdr,
            plan,
            encoder.pixel_format(),
        )?;
        tracing::info!(encoder = %chosen.name, hdr = source.hdr, "compression : encodeur choisi");
        let mut video_parameters = encoder.codec_parameters();
        // SAFETY: plain field writes on an owned AVCodecParameters copy before the muxer reads it.
        unsafe {
            let raw = &mut *video_parameters.as_mut_ptr();
            raw.color_primaries = ffi::AVCOL_PRI_BT709;
            raw.color_trc = ffi::AVCOL_TRC_BT709;
            raw.color_space = ffi::AVCOL_SPC_BT709;
            raw.color_range = ffi::AVCOL_RANGE_MPEG;
        }
        let mut specs = vec![MuxStream {
            codec_parameters: video_parameters,
            time_base: ra(1, fps),
            frame_rate: Some(plan.fps),
            title: None,
        }];
        let audio_streams: Vec<usize> = streams
            .iter()
            .enumerate()
            .filter(|(_, (kind, ..))| *kind == ffi::AVMEDIA_TYPE_AUDIO)
            .map(|(index, _)| index)
            .take(plan.audio_tracks)
            .collect();
        let mut audio = Vec::new();
        for stream in audio_streams {
            let (_, parameters, base, title) = &streams[stream];
            let encoder = AacEncoder::open_with_bitrate(
                MIX_RATE,
                MIX_CHANNELS,
                i64::from(plan.audio_kbps) * 1_000,
            )?;
            specs.push(MuxStream {
                codec_parameters: encoder.codec_parameters(),
                time_base: encoder.time_base(),
                frame_rate: None,
                title: title.clone(),
            });
            audio.push(AudioOut {
                stream,
                time_base: *base,
                decoder: super::export::decoder(parameters, *base)?,
                resampler: None,
                mixer: Mixer::new(1),
                encoder,
                done: false,
            });
        }
        input
            .seek(
                video as i32,
                timestamp(start, *video_base),
                ffi::AVSEEK_FLAG_BACKWARD as i32,
            )
            .map_err(ffmpeg_error)?;
        Ok(Self {
            writer: Mp4Writer::create(partial, &specs)?,
            input,
            video,
            video_base: *video_base,
            decoder,
            chain,
            encoder,
            audio,
            start,
            end,
            threads_lowered: false,
        })
    }

    fn run(mut self, progress: &mut dyn FnMut(f64), cancel: &AtomicBool) -> Result<()> {
        let mut video_done = false;
        while let Some(packet) = self.input.read_packet().map_err(ffmpeg_error)? {
            if cancel.load(Ordering::Relaxed) {
                return Err(EngineError::CompressCancelled);
            }
            let stream = packet.stream_index as usize;
            if stream == self.video {
                if !video_done {
                    video_done = self.video_packet(Some(&packet), progress)?;
                }
            } else if let Some(position) =
                self.audio.iter().position(|audio| audio.stream == stream)
            {
                self.audio_packet(position, Some(&packet))?;
            }
            if video_done && self.audio.iter().all(|audio| audio.done) {
                break;
            }
        }
        if !video_done {
            self.video_packet(None, progress)?;
        }
        let filtered = self.chain.push(None)?;
        self.encode_frames(filtered)?;
        let writer = &mut self.writer;
        self.encoder
            .encode(None, &mut |packet| writer.write(0, packet))?;
        for position in 0..self.audio.len() {
            self.audio_packet(position, None)?;
            let output = &mut self.audio[position];
            let ready = output.mixer.take(true);
            let index = position + 1;
            let writer = &mut self.writer;
            output
                .encoder
                .encode(&ready, &mut |packet| writer.write(index, packet))?;
            output
                .encoder
                .finish(&mut |packet| writer.write(index, packet))?;
        }
        progress(1.0);
        self.writer.finish()
    }

    fn video_packet(
        &mut self,
        packet: Option<&AVPacket>,
        progress: &mut dyn FnMut(f64),
    ) -> Result<bool> {
        let mut finished = false;
        let mut keep = Vec::new();
        for mut frame in frames(&mut self.decoder, packet)? {
            let at = seconds(frame.best_effort_timestamp, self.video_base);
            if at < self.start {
                continue;
            }
            if at >= self.end {
                finished = true;
                continue;
            }
            progress(((at - self.start) / (self.end - self.start)).clamp(0.0, 1.0));
            frame.set_pts(timestamp(at - self.start, self.video_base));
            keep.push(frame);
        }
        if !keep.is_empty() && !std::mem::replace(&mut self.threads_lowered, true) {
            crate::encode::threads::lower_encoder_threads();
        }
        for frame in keep {
            let filtered = self.chain.push(Some(frame))?;
            self.encode_frames(filtered)?;
        }
        Ok(finished)
    }

    fn encode_frames(&mut self, filtered: Vec<AVFrame>) -> Result<()> {
        let writer = &mut self.writer;
        for frame in filtered {
            self.encoder
                .encode(Some(&frame), &mut |packet| writer.write(0, packet))?;
        }
        Ok(())
    }

    fn audio_packet(&mut self, position: usize, packet: Option<&AVPacket>) -> Result<()> {
        let start = self.start;
        let end = self.end;
        let output = &mut self.audio[position];
        if output.done && packet.is_some() {
            return Ok(());
        }
        if let Some(packet) = packet
            && seconds(packet.pts, output.time_base) >= end
        {
            output.done = true;
            return Ok(());
        }
        for frame in frames(&mut output.decoder, packet)? {
            let first = ((seconds(frame.best_effort_timestamp, output.time_base) - start)
                * f64::from(MIX_RATE))
            .round() as i64;
            let samples = interleaved(&frame, &mut output.resampler)?;
            output.mixer.add(0, first, &samples, 1.0);
        }
        let ready = output.mixer.take(false);
        let index = position + 1;
        let writer = &mut self.writer;
        output
            .encoder
            .encode(&ready, &mut |packet| writer.write(index, packet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(width: u32, height: u32, fps: f64, seconds: f64, megabytes: u64) -> SourceInfo {
        SourceInfo {
            duration_secs: seconds,
            width,
            height,
            fps,
            hdr: false,
            audio_tracks: vec!["Mix complet".into(), "Micro".into(), "Discord".into()],
            size_bytes: megabytes * 1_000_000,
        }
    }

    #[test]
    fn the_plan_fits_the_target_with_its_margin() -> Result<()> {
        let clip = source(2560, 1440, 60.0, 60.0, 300);
        let plan = plan(&clip, 20_000_000, CompressCodec::H264, false, 60.0)?;
        assert_eq!(plan.audio_tracks, 1);
        assert!(plan.estimated_bytes(60.0) <= 20_000_000, "{plan:?}");
        assert!(plan.estimated_bytes(60.0) >= 19_000_000, "{plan:?}");
        Ok(())
    }

    #[test]
    fn a_tight_target_lowers_resolution_then_frame_rate() -> Result<()> {
        let clip = source(2560, 1440, 60.0, 60.0, 300);
        let generous = plan(&clip, 500_000_000, CompressCodec::H264, false, 60.0)?;
        assert_eq!((generous.height, generous.fps), (1440, 60));
        let discord = plan(&clip, 20_000_000, CompressCodec::H264, false, 60.0)?;
        assert_eq!(discord.height, 720, "{discord:?}");
        let long = plan(&clip, 20_000_000, CompressCodec::H264, false, 300.0)?;
        assert!(long.fps == 30 && long.height <= 720, "{long:?}");
        let tiny = plan(&clip, 20_000_000, CompressCodec::H264, false, 900.0)?;
        assert_eq!((tiny.height, tiny.fps), (480, 30), "{tiny:?}");
        Ok(())
    }

    #[test]
    fn av1_keeps_more_pixels_for_the_same_size() -> Result<()> {
        let clip = source(2560, 1440, 60.0, 60.0, 300);
        let h264 = plan(&clip, 50_000_000, CompressCodec::H264, false, 60.0)?;
        let av1 = plan(&clip, 50_000_000, CompressCodec::Av1, false, 60.0)?;
        assert!(
            av1.height * av1.fps >= h264.height * h264.fps,
            "{h264:?} {av1:?}"
        );
        Ok(())
    }

    #[test]
    fn keeping_every_track_spends_audio_on_each() -> Result<()> {
        let clip = source(1920, 1080, 60.0, 60.0, 300);
        let one = plan(&clip, 50_000_000, CompressCodec::H264, false, 60.0)?;
        let all = plan(&clip, 50_000_000, CompressCodec::H264, true, 60.0)?;
        assert_eq!(all.audio_tracks, 3);
        assert!(all.video_kbps < one.video_kbps);
        Ok(())
    }

    #[test]
    fn a_target_too_small_for_the_length_says_the_minimum() {
        let clip = source(1920, 1080, 60.0, 3_600.0, 3_000);
        let error = plan(&clip, 20_000_000, CompressCodec::H264, false, 3_600.0);
        assert!(
            matches!(error, Err(EngineError::TargetTooSmall { min_mb }) if min_mb > 20),
            "{error:?}"
        );
    }

    #[test]
    fn a_source_already_under_the_target_is_copied() -> Result<()> {
        let clip = source(1280, 720, 30.0, 10.0, 5);
        let whole = plan(&clip, 500_000_000, CompressCodec::H264, false, 10.0)?;
        assert_eq!(whole.copy_bytes, Some(5_000_000));
        assert_eq!(whole.estimated_bytes(10.0), 5_000_000);
        let part = plan(&clip, 500_000_000, CompressCodec::H264, false, 5.0)?;
        assert_eq!(part.copy_bytes, None);
        assert!(part.video_kbps <= 4_000, "{part:?}");
        Ok(())
    }

    #[test]
    fn a_copy_keeps_the_first_track_and_stays_under_the_target() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("palaclips-compress-copy-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir)?;
        let input = dir.join("source.mp4");
        super::super::export::file_tests::write_source(&input, 3);
        let info = probe_source(&input)?;
        let target = info.size_bytes + 1_000_000;
        let plan = plan(
            &info,
            target,
            CompressCodec::H264,
            false,
            info.duration_secs,
        )?;
        assert!(plan.copy_bytes.is_some());
        let request = CompressRequest {
            output: compressed_name(&input, target),
            source: input,
            target_bytes: target,
            codec: CompressCodec::H264,
            all_tracks: false,
            start_secs: 0.0,
            end_secs: None,
            plan,
        };
        let result = compress(&request, &[], &mut |_| {}, &AtomicBool::new(false))?;
        assert_eq!(result.attempts, 0);
        assert!(result.bytes <= target);
        let out = probe_source(&request.output)?;
        assert_eq!(out.audio_tracks.len(), 1);
        assert!((out.duration_secs - info.duration_secs).abs() < 0.1);
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_retry_lowers_the_bitrate_by_the_overshoot() {
        let plan = CompressPlan {
            width: 1280,
            height: 720,
            fps: 60,
            video_kbps: 2_500,
            audio_kbps: 128,
            audio_tracks: 1,
            copy_bytes: None,
        };
        let lower = retried_kbps(&plan, 21_000_000, 20_000_000, 60.0);
        assert!(lower < 2_500 && lower > 2_000, "{lower}");
        let estimated = CompressPlan {
            video_kbps: lower,
            ..plan
        };
        assert!(estimated.estimated_bytes(60.0) < 20_000_000);
    }

    #[test]
    fn the_output_sits_next_to_the_source_with_the_target_size() {
        let dir =
            std::env::temp_dir().join(format!("palaclips-compress-name-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let source = dir.join("Fortnite_2026-10-23_12-00-00_replay.mp4");
        let first = compressed_name(&source, 20_000_000);
        assert_eq!(
            first,
            dir.join("Fortnite_2026-10-23_12-00-00_replay_20MB.mp4")
        );
        std::fs::write(&first, b"x").expect("write");
        assert_eq!(
            compressed_name(&source, 20_000_000),
            dir.join("Fortnite_2026-10-23_12-00-00_replay_20MB_2.mp4")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn quality_follows_the_bits_per_pixel() {
        let plan = |kbps| CompressPlan {
            width: 1280,
            height: 720,
            fps: 60,
            video_kbps: kbps,
            audio_kbps: 128,
            audio_tracks: 1,
            copy_bytes: None,
        };
        assert_eq!(plan(6_000).quality(CompressCodec::H264), Quality::Good);
        assert_eq!(plan(4_000).quality(CompressCodec::H264), Quality::Fair);
        assert_eq!(plan(2_000).quality(CompressCodec::H264), Quality::Low);
        assert_eq!(plan(4_000).quality(CompressCodec::Av1), Quality::Good);
    }

    fn svt() -> [EncoderInfo; 1] {
        [EncoderInfo {
            name: "libsvtav1".into(),
            description: String::new(),
            codec: VideoCodec::Av1,
            hardware: false,
            failure: None,
        }]
    }

    #[test]
    fn a_compressed_clip_stays_under_its_target() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("palaclips-compress-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir)?;
        let input = dir.join("source.mp4");
        super::super::export::file_tests::write_source(&input, 4);
        let info = probe_source(&input)?;
        assert_eq!(info.audio_tracks.len(), 2);
        let target = 200_000;
        let plan = plan(&info, target, CompressCodec::Av1, false, info.duration_secs)?;
        let request = CompressRequest {
            output: compressed_name(&input, target),
            source: input.clone(),
            target_bytes: target,
            codec: CompressCodec::Av1,
            all_tracks: false,
            start_secs: 0.0,
            end_secs: None,
            plan,
        };
        let mut seen = Vec::new();
        let result = compress(
            &request,
            &svt(),
            &mut |step| seen.push(step),
            &AtomicBool::new(false),
        )?;
        assert!(result.bytes <= target, "{result:?}");
        assert_eq!(std::fs::metadata(&request.output)?.len(), result.bytes);
        let out = probe_source(&request.output)?;
        assert_eq!(out.audio_tracks.len(), 1);
        assert!(
            (out.duration_secs - info.duration_secs).abs() < 0.2,
            "{} {}",
            out.duration_secs,
            info.duration_secs
        );
        assert!(seen.last().is_some_and(|step| step.fraction == 1.0));
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_cancelled_compression_leaves_no_file() -> Result<()> {
        let dir =
            std::env::temp_dir().join(format!("palaclips-compress-cancel-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir)?;
        let input = dir.join("source.mp4");
        super::super::export::file_tests::write_source(&input, 3);
        let info = probe_source(&input)?;
        let request = CompressRequest {
            output: compressed_name(&input, 500_000),
            source: input,
            target_bytes: 500_000,
            codec: CompressCodec::Av1,
            all_tracks: true,
            start_secs: 0.0,
            end_secs: None,
            plan: plan(&info, 500_000, CompressCodec::Av1, true, info.duration_secs)?,
        };
        let result = compress(&request, &svt(), &mut |_| {}, &AtomicBool::new(true));
        assert!(
            matches!(result, Err(EngineError::CompressCancelled)),
            "{result:?}"
        );
        assert!(!request.output.exists());
        assert!(!super::super::partial_path(&request.output).exists());
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_file_without_video_is_refused_clearly() {
        let dir =
            std::env::temp_dir().join(format!("palaclips-compress-text-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let text = dir.join("notes.mp4");
        std::fs::write(&text, b"pas une video").expect("write");
        assert!(matches!(probe_source(&text), Err(EngineError::NotAVideo)));
        std::fs::remove_dir_all(&dir).ok();
    }
}
