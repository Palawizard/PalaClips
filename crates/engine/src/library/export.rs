use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rsmpeg::avcodec::{AVCodec, AVCodecContext, AVCodecParameters, AVPacket};
use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::avutil::{AVChannelLayout, AVFrame, AVRational};
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swresample::SwrContext;
use rsmpeg::swscale::SwsContext;
use serde::Deserialize;

use super::media::open_input;
use super::stream_title;
use crate::config::EncoderChoice;
use crate::encode::{AacEncoder, EncoderInfo, Size, SystemFrameEncoder, codec_of_id, pick_encoder};
use crate::error::{EngineError, Result};
use crate::mux::{Mp4Writer, MuxStream};

const MIX_RATE: u32 = 48_000;
const MIX_CHANNELS: usize = 2;
const AUDIO_LEAD_SECS: f64 = 0.03;
const MIN_EXPORT_SECS: f64 = 0.5;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ExportRequest {
    pub source: PathBuf,
    pub output: PathBuf,
    pub start_secs: f64,
    pub end_secs: f64,
    pub precise: bool,
    pub tracks: Vec<usize>,
    #[serde(default)]
    pub gains: Vec<f32>,
    pub mix: bool,
    pub mix_title: String,
}

pub fn partial_path(output: &Path) -> PathBuf {
    output.with_extension("part.mp4")
}

pub fn export_clip(
    request: &ExportRequest,
    encoders: &[EncoderInfo],
    progress: &mut dyn FnMut(f64),
    cancel: &AtomicBool,
) -> Result<()> {
    let partial = partial_path(&request.output);
    if let Some(dir) = partial.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match Export::open(request, encoders, &partial).and_then(|export| export.run(progress, cancel))
    {
        Ok(()) => {
            std::fs::rename(&partial, &request.output)?;
            Ok(())
        }
        Err(error) => {
            std::fs::remove_file(&partial).ok();
            Err(error)
        }
    }
}

fn ffmpeg_error(error: RsmpegError) -> EngineError {
    EngineError::Mux(error.to_string())
}

fn seconds(ts: i64, base: AVRational) -> f64 {
    ts as f64 * f64::from(base.num) / f64::from(base.den.max(1))
}

fn timestamp(seconds: f64, base: AVRational) -> i64 {
    (seconds * f64::from(base.den) / f64::from(base.num.max(1))).round() as i64
}

fn decoder(parameters: &AVCodecParameters, time_base: AVRational) -> Result<AVCodecContext> {
    let codec = AVCodec::find_decoder(parameters.codec_id)
        .ok_or_else(|| EngineError::Mux("décodeur absent".into()))?;
    let mut context = AVCodecContext::new(&codec);
    context.apply_codecpar(parameters).map_err(ffmpeg_error)?;
    context.set_pkt_timebase(time_base);
    context.open(None).map_err(ffmpeg_error)?;
    Ok(context)
}

fn frames(context: &mut AVCodecContext, packet: Option<&AVPacket>) -> Result<Vec<AVFrame>> {
    context.send_packet(packet).map_err(ffmpeg_error)?;
    let mut decoded = Vec::new();
    loop {
        match context.receive_frame() {
            Ok(frame) => decoded.push(frame),
            Err(RsmpegError::DecoderDrainError | RsmpegError::DecoderFlushedError) => {
                return Ok(decoded);
            }
            Err(error) => return Err(ffmpeg_error(error)),
        }
    }
}

#[derive(Debug, Default)]
struct Mixer {
    mixed: Vec<f32>,
    origin: i64,
    reached: Vec<i64>,
}

impl Mixer {
    fn new(tracks: usize) -> Self {
        Self {
            mixed: Vec::new(),
            origin: 0,
            reached: vec![0; tracks],
        }
    }

    fn add(&mut self, track: usize, first: i64, samples: &[f32], gain: f32) {
        let (first, samples) = if first < self.origin {
            let skip = ((self.origin - first) as usize * MIX_CHANNELS).min(samples.len());
            (self.origin, &samples[skip..])
        } else {
            (first, samples)
        };
        let offset = (first - self.origin) as usize * MIX_CHANNELS;
        if self.mixed.len() < offset + samples.len() {
            self.mixed.resize(offset + samples.len(), 0.0);
        }
        for (slot, sample) in self.mixed[offset..].iter_mut().zip(samples) {
            *slot += sample * gain;
        }
        let end = first + (samples.len() / MIX_CHANNELS) as i64;
        if let Some(reached) = self.reached.get_mut(track) {
            *reached = (*reached).max(end);
        }
    }

    fn take(&mut self, all: bool) -> Vec<f32> {
        let ready = if all {
            self.mixed.len() / MIX_CHANNELS
        } else {
            let slowest = self.reached.iter().copied().min().unwrap_or(self.origin);
            ((slowest - self.origin).max(0) as usize).min(self.mixed.len() / MIX_CHANNELS)
        };
        self.origin += ready as i64;
        self.mixed
            .drain(..ready * MIX_CHANNELS)
            .map(|sample| sample.clamp(-1.0, 1.0))
            .collect()
    }
}

struct AudioIn {
    stream: usize,
    time_base: AVRational,
    decoder: Option<(AVCodecContext, Option<SwrContext>)>,
    done: bool,
}

struct PreciseVideo {
    decoder: AVCodecContext,
    encoder: SystemFrameEncoder,
    scaler: Option<SwsContext>,
    size: Size,
}

struct Export<'a> {
    request: &'a ExportRequest,
    input: AVFormatContextInput,
    writer: Mp4Writer,
    video: usize,
    video_base: AVRational,
    precise: Option<PreciseVideo>,
    audio: Vec<AudioIn>,
    mix: Option<(Mixer, AacEncoder)>,
}

impl<'a> Export<'a> {
    fn open(request: &'a ExportRequest, encoders: &[EncoderInfo], partial: &Path) -> Result<Self> {
        if request.end_secs - request.start_secs.max(0.0) < MIN_EXPORT_SECS {
            return Err(EngineError::ExportTooShort);
        }
        let mut input = open_input(&request.source)?;
        let streams: Vec<(AVCodecParameters, AVRational, AVRational, Option<String>)> = input
            .streams()
            .iter()
            .map(|stream| {
                (
                    stream.codecpar().clone(),
                    stream.time_base,
                    stream.avg_frame_rate,
                    stream_title(stream),
                )
            })
            .collect();
        let video = streams
            .iter()
            .position(|(parameters, ..)| parameters.codec_type == ffi::AVMEDIA_TYPE_VIDEO)
            .ok_or_else(|| EngineError::Mux("aucune piste vidéo".into()))?;
        let audio_streams: Vec<usize> = streams
            .iter()
            .enumerate()
            .filter(|(_, (parameters, ..))| parameters.codec_type == ffi::AVMEDIA_TYPE_AUDIO)
            .map(|(index, _)| index)
            .collect();
        let selected = request
            .tracks
            .iter()
            .map(|position| {
                audio_streams
                    .get(*position)
                    .copied()
                    .ok_or(EngineError::NoStream)
            })
            .collect::<Result<Vec<_>>>()?;
        let (video_parameters, video_base, frame_rate, _) = &streams[video];
        let precise = if request.precise {
            let codec = codec_of_id(video_parameters.codec_id).ok_or_else(|| {
                EngineError::Encoder("codec vidéo de ce clip non réencodable".into())
            })?;
            let encoder = pick_encoder(encoders, codec, EncoderChoice::Auto)
                .or_else(|_| pick_encoder(encoders, codec, EncoderChoice::Cpu))?;
            let size = Size::new(
                video_parameters.width as u32,
                video_parameters.height as u32,
            );
            let duration = input.duration.max(1) as f64 / f64::from(ffi::AV_TIME_BASE);
            let file_kbps = std::fs::metadata(&request.source)
                .map(|meta| (meta.len() as f64 * 8.0 / duration / 1_000.0) as u32)
                .unwrap_or(20_000);
            let source_kbps = (video_parameters.bit_rate / 1_000) as u32;
            let bitrate = if source_kbps > 0 {
                source_kbps
            } else {
                file_kbps
            }
            .clamp(2_000, 200_000);
            Some(PreciseVideo {
                decoder: decoder(video_parameters, *video_base)?,
                encoder: SystemFrameEncoder::open(
                    &encoder.name,
                    size,
                    *video_base,
                    *frame_rate,
                    bitrate,
                )?,
                scaler: None,
                size,
            })
        } else {
            None
        };
        let mut specs = vec![MuxStream {
            codec_parameters: precise.as_ref().map_or_else(
                || video_parameters.clone(),
                |video| video.encoder.codec_parameters(),
            ),
            time_base: *video_base,
            frame_rate: (frame_rate.num > 0 && frame_rate.den > 0)
                .then(|| (frame_rate.num / frame_rate.den) as u32),
            title: None,
        }];
        let mix = if request.mix && !selected.is_empty() {
            let encoder = AacEncoder::open(MIX_RATE, MIX_CHANNELS)?;
            specs.push(MuxStream {
                codec_parameters: encoder.codec_parameters(),
                time_base: encoder.time_base(),
                frame_rate: None,
                title: Some(request.mix_title.clone()),
            });
            Some((Mixer::new(selected.len()), encoder))
        } else {
            for stream in &selected {
                let (parameters, base, _, title) = &streams[*stream];
                specs.push(MuxStream {
                    codec_parameters: parameters.clone(),
                    time_base: *base,
                    frame_rate: None,
                    title: title.clone(),
                });
            }
            None
        };
        let audio = selected
            .iter()
            .map(|stream| {
                let (parameters, base, ..) = &streams[*stream];
                Ok(AudioIn {
                    stream: *stream,
                    time_base: *base,
                    decoder: if mix.is_some() {
                        Some((decoder(parameters, *base)?, None))
                    } else {
                        None
                    },
                    done: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let start = timestamp(request.start_secs.max(0.0), *video_base);
        input
            .seek(video as i32, start, ffi::AVSEEK_FLAG_BACKWARD as i32)
            .map_err(ffmpeg_error)?;
        Ok(Self {
            writer: Mp4Writer::create(partial, &specs)?,
            video_base: *video_base,
            request,
            input,
            video,
            precise,
            audio,
            mix,
        })
    }

    fn run(mut self, progress: &mut dyn FnMut(f64), cancel: &AtomicBool) -> Result<()> {
        let start = self.request.start_secs.max(0.0);
        let end = self.request.end_secs;
        let mut cut = self.precise.as_ref().map(|_| start);
        let mut video_done = false;
        while let Some(mut packet) = self.input.read_packet().map_err(ffmpeg_error)? {
            if cancel.load(Ordering::Relaxed) {
                return Err(EngineError::ExportCancelled);
            }
            let stream = packet.stream_index as usize;
            if stream == self.video {
                if video_done {
                    continue;
                }
                let at = seconds(
                    if packet.pts == ffi::AV_NOPTS_VALUE {
                        packet.dts
                    } else {
                        packet.pts
                    },
                    self.video_base,
                );
                let begin = *cut.get_or_insert(at);
                progress(((at - begin) / (end - begin)).clamp(0.0, 1.0));
                video_done = self.video_packet(packet, begin, start, end)?;
            } else if let Some(position) =
                self.audio.iter().position(|audio| audio.stream == stream)
            {
                let Some(begin) = cut else { continue };
                let base = self.audio[position].time_base;
                let at = seconds(packet.pts, base);
                if at >= end {
                    self.audio[position].done = true;
                    continue;
                }
                if at + AUDIO_LEAD_SECS < begin || self.audio[position].done {
                    continue;
                }
                if self.mix.is_some() {
                    self.mix_packet(position, Some(&packet), begin)?;
                } else {
                    let shift = timestamp(begin, base);
                    packet.set_pts(packet.pts - shift);
                    packet.set_dts(packet.dts - shift);
                    self.writer.write(position + 1, packet)?;
                }
            }
            if video_done && self.audio.iter().all(|audio| audio.done) {
                break;
            }
        }
        let begin = cut.unwrap_or(start);
        if let Some(video) = self.precise.as_mut() {
            let writer = &mut self.writer;
            video
                .encoder
                .encode(None, &mut |packet| writer.write(0, packet))?;
        }
        if self.mix.is_some() {
            for position in 0..self.audio.len() {
                self.mix_packet(position, None, begin)?;
            }
            self.flush_mix(true)?;
        }
        progress(1.0);
        self.writer.finish()
    }

    fn video_packet(
        &mut self,
        mut packet: AVPacket,
        begin: f64,
        start: f64,
        end: f64,
    ) -> Result<bool> {
        let base = self.video_base;
        let Some(video) = self.precise.as_mut() else {
            if seconds(packet.dts, base) >= end {
                return Ok(true);
            }
            let shift = timestamp(begin, base);
            packet.set_pts(packet.pts - shift);
            packet.set_dts(packet.dts - shift);
            self.writer.write(0, packet)?;
            return Ok(false);
        };
        let mut finished = false;
        for frame in frames(&mut video.decoder, Some(&packet))? {
            let at = seconds(frame.best_effort_timestamp, base);
            if at < start {
                continue;
            }
            if at >= end {
                finished = true;
                continue;
            }
            let mut converted = to_encoder_format(&frame, video)?;
            converted.set_pts(timestamp(at - start, base));
            let writer = &mut self.writer;
            video
                .encoder
                .encode(Some(&converted), &mut |packet| writer.write(0, packet))?;
        }
        Ok(finished)
    }

    fn mix_packet(&mut self, position: usize, packet: Option<&AVPacket>, begin: f64) -> Result<()> {
        let base = self.audio[position].time_base;
        let Some((decoder, resampler)) = self.audio[position].decoder.as_mut() else {
            return Ok(());
        };
        let Some((mixer, _)) = self.mix.as_mut() else {
            return Ok(());
        };
        for frame in frames(decoder, packet)? {
            let first = ((seconds(frame.best_effort_timestamp, base) - begin) * f64::from(MIX_RATE))
                .round() as i64;
            let samples = interleaved(&frame, resampler)?;
            let gain = self.request.gains.get(position).copied().unwrap_or(1.0);
            mixer.add(position, first, &samples, gain);
        }
        self.flush_mix(false)
    }

    fn flush_mix(&mut self, all: bool) -> Result<()> {
        let Some((mixer, encoder)) = self.mix.as_mut() else {
            return Ok(());
        };
        let ready = mixer.take(all);
        let writer = &mut self.writer;
        let stream = 1;
        encoder.encode(&ready, &mut |packet| writer.write(stream, packet))?;
        if all {
            encoder.finish(&mut |packet| writer.write(stream, packet))?;
        }
        Ok(())
    }
}

fn to_encoder_format(frame: &AVFrame, video: &mut PreciseVideo) -> Result<AVFrame> {
    let (width, height) = (video.size.width as i32, video.size.height as i32);
    let format = video.encoder.pixel_format();
    if video.scaler.is_none() {
        video.scaler = SwsContext::get_context(
            frame.width,
            frame.height,
            frame.format,
            width,
            height,
            format,
            ffi::SWS_BILINEAR,
            None,
            None,
            None,
        );
    }
    let scaler = video
        .scaler
        .as_mut()
        .ok_or_else(|| EngineError::Encoder("conversion d'image impossible".into()))?;
    let mut converted = AVFrame::new();
    converted.set_width(width);
    converted.set_height(height);
    converted.set_format(format);
    converted.alloc_buffer().map_err(ffmpeg_error)?;
    scaler
        .scale_frame(frame, 0, frame.height, &mut converted)
        .map_err(ffmpeg_error)?;
    Ok(converted)
}

fn interleaved(frame: &AVFrame, resampler: &mut Option<SwrContext>) -> Result<Vec<f32>> {
    let stereo = AVChannelLayout::from_nb_channels(MIX_CHANNELS as i32);
    if resampler.is_none() {
        let mut context = SwrContext::new(
            &stereo,
            ffi::AV_SAMPLE_FMT_FLT,
            MIX_RATE as i32,
            &frame.ch_layout,
            frame.format,
            frame.sample_rate,
        )
        .map_err(ffmpeg_error)?;
        context.init().map_err(ffmpeg_error)?;
        *resampler = Some(context);
    }
    let resampler = resampler
        .as_mut()
        .ok_or_else(|| EngineError::Mux("rééchantillonnage impossible".into()))?;
    let mut output = AVFrame::new();
    output.set_ch_layout(stereo.into_inner());
    output.set_format(ffi::AV_SAMPLE_FMT_FLT);
    output.set_sample_rate(MIX_RATE as i32);
    resampler
        .convert_frame(Some(frame), &mut output)
        .map_err(ffmpeg_error)?;
    let count = output.nb_samples.max(0) as usize * MIX_CHANNELS;
    if count == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: swr_convert_frame filled `nb_samples` interleaved stereo f32 samples in data[0].
    Ok(unsafe { std::slice::from_raw_parts(output.data[0].cast::<f32>(), count) }.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_tracks_add_up_and_wait_for_the_slowest_track() {
        let mut mixer = Mixer::new(2);
        mixer.add(0, 0, &[0.25; 8], 1.0);
        assert!(mixer.take(false).is_empty());
        mixer.add(1, 0, &[1.0; 4], 0.5);
        assert_eq!(mixer.take(false), [0.75; 4]);
        mixer.add(1, 2, &[0.5; 4], 1.0);
        assert_eq!(mixer.take(false), [0.75; 4]);
        assert!(mixer.take(true).is_empty());
    }

    #[test]
    fn a_loud_mix_is_clamped_and_samples_before_the_cut_are_dropped() {
        let mut mixer = Mixer::new(1);
        mixer.add(0, -2, &[0.9; 8], 1.0);
        mixer.add(0, 0, &[0.9; 4], 1.0);
        assert_eq!(mixer.take(true), [1.0; 4]);
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use crate::library::probe_clip;

    const FPS: i32 = 30;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-export-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn video_packets(seconds: i32) -> (MuxStream, Vec<AVPacket>) {
        let codec = AVCodec::find_encoder(ffi::AV_CODEC_ID_MPEG4).expect("mpeg4");
        let mut context = AVCodecContext::new(&codec);
        context.set_width(320);
        context.set_height(180);
        context.set_pix_fmt(ffi::AV_PIX_FMT_YUV420P);
        context.set_time_base(rsmpeg::avutil::ra(1, FPS));
        context.set_framerate(rsmpeg::avutil::ra(FPS, 1));
        context.set_gop_size(FPS);
        context.set_max_b_frames(0);
        context.set_flags(context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        context
            .open(Some(rsmpeg::avutil::AVDictionary::new_int(
                c"sc_threshold",
                1_000_000_000,
                0,
            )))
            .expect("open");
        let mut packets = Vec::new();
        for index in 0..seconds * FPS {
            let mut frame = AVFrame::new();
            frame.set_width(320);
            frame.set_height(180);
            frame.set_format(ffi::AV_PIX_FMT_YUV420P);
            frame.alloc_buffer().expect("buffer");
            frame.set_pts(i64::from(index));
            context.send_frame(Some(&frame)).expect("send");
            while let Ok(packet) = context.receive_packet() {
                packets.push(packet);
            }
        }
        context.send_frame(None).expect("flush");
        while let Ok(packet) = context.receive_packet() {
            packets.push(packet);
        }
        let stream = MuxStream {
            codec_parameters: context.extract_codecpar(),
            time_base: context.time_base,
            frame_rate: Some(FPS as u32),
            title: None,
        };
        (stream, packets)
    }

    fn write_source(path: &Path, seconds: i32) {
        let (video, video_packets) = video_packets(seconds);
        let mut streams = vec![video];
        let mut packets: Vec<(f64, usize, AVPacket)> = video_packets
            .into_iter()
            .map(|packet| (packet.pts as f64 / f64::from(FPS), 0, packet))
            .collect();
        for (track, title) in ["Mix", "Discord"].into_iter().enumerate() {
            let mut encoder = AacEncoder::open(MIX_RATE, MIX_CHANNELS).expect("aac");
            let samples = vec![0.2f32; MIX_RATE as usize * MIX_CHANNELS * seconds as usize];
            for chunk in samples.chunks(960) {
                encoder
                    .encode(chunk, &mut |packet| {
                        packets.push((packet.pts as f64 / f64::from(MIX_RATE), track + 1, packet));
                        Ok(())
                    })
                    .expect("encode");
            }
            streams.push(MuxStream {
                codec_parameters: encoder.codec_parameters(),
                time_base: encoder.time_base(),
                frame_rate: None,
                title: Some(title.into()),
            });
        }
        packets.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut writer = Mp4Writer::create(path, &streams).expect("writer");
        for (_, stream, packet) in packets {
            writer.write(stream, packet).expect("write");
        }
        writer.finish().expect("finish");
    }

    fn request(dir: &Path, tracks: Vec<usize>, mix: bool) -> ExportRequest {
        ExportRequest {
            source: dir.join("source.mp4"),
            output: dir.join("cut.mp4"),
            start_secs: 1.2,
            end_secs: 3.0,
            precise: false,
            tracks,
            gains: Vec::new(),
            mix,
            mix_title: "Mix".into(),
        }
    }

    fn run(request: &ExportRequest) -> Result<Vec<f64>> {
        let mut seen = Vec::new();
        export_clip(
            request,
            &[],
            &mut |value| seen.push(value),
            &AtomicBool::new(false),
        )?;
        Ok(seen)
    }

    #[test]
    fn a_fast_cut_starts_on_the_keyframe_and_keeps_the_chosen_tracks() -> Result<()> {
        let dir = scratch("fast");
        write_source(&dir.join("source.mp4"), 5);
        let progress = run(&request(&dir, vec![1], false))?;
        let info = probe_clip(&dir.join("cut.mp4"))?;
        assert!(
            (info.duration_secs - 2.0).abs() < 0.15,
            "{}",
            info.duration_secs
        );
        assert_eq!(info.tracks, ["Discord"]);
        assert_eq!(progress.last(), Some(&1.0));
        assert!(!partial_path(&dir.join("cut.mp4")).exists());
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn mixed_tracks_become_one_titled_track() -> Result<()> {
        let dir = scratch("mix");
        write_source(&dir.join("source.mp4"), 5);
        run(&request(&dir, vec![0, 1], true))?;
        let info = probe_clip(&dir.join("cut.mp4"))?;
        assert_eq!(info.tracks, ["Mix"]);
        assert!(
            (info.duration_secs - 2.0).abs() < 0.15,
            "{}",
            info.duration_secs
        );
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_cancelled_or_too_short_export_leaves_no_file() {
        let dir = scratch("cancel");
        write_source(&dir.join("source.mp4"), 3);
        let cancelled = export_clip(
            &request(&dir, vec![0], false),
            &[],
            &mut |_| {},
            &AtomicBool::new(true),
        );
        assert!(matches!(cancelled, Err(EngineError::ExportCancelled)));
        let mut short = request(&dir, vec![0], false);
        short.end_secs = 1.3;
        assert!(matches!(run(&short), Err(EngineError::ExportTooShort)));
        assert!(!dir.join("cut.mp4").exists());
        assert!(!partial_path(&dir.join("cut.mp4")).exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[cfg(test)]
mod encoder_tests {
    use super::*;
    use crate::config::VideoCodec;
    use crate::encode::video_encoders;
    use crate::library::probe_clip;

    fn precise_cut_duration(
        encoders: &[EncoderInfo],
        codec: VideoCodec,
        name: &str,
    ) -> Result<f64> {
        let dir = std::env::temp_dir()
            .join("palaclips-export-tests")
            .join(format!("precise-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir)?;
        let encoder = pick_encoder(encoders, codec, EncoderChoice::Auto)?;
        let base = rsmpeg::avutil::ra(1, 30);
        let size = Size::new(320, 180);
        let mut source_encoder =
            SystemFrameEncoder::open(&encoder.name, size, base, rsmpeg::avutil::ra(30, 1), 2_000)?;
        let stream = MuxStream {
            codec_parameters: source_encoder.codec_parameters(),
            time_base: base,
            frame_rate: Some(30),
            title: None,
        };
        let source = dir.join("source.mp4");
        let mut writer = Mp4Writer::create(&source, std::slice::from_ref(&stream))?;
        for index in 0..150 {
            let mut frame = AVFrame::new();
            frame.set_width(320);
            frame.set_height(180);
            frame.set_format(source_encoder.pixel_format());
            frame.alloc_buffer().map_err(ffmpeg_error)?;
            frame.set_pts(index);
            source_encoder.encode(Some(&frame), &mut |packet| writer.write(0, packet))?;
        }
        source_encoder.encode(None, &mut |packet| writer.write(0, packet))?;
        writer.finish()?;
        let request = ExportRequest {
            source,
            output: dir.join("cut.mp4"),
            start_secs: 1.5,
            end_secs: 3.0,
            precise: true,
            tracks: Vec::new(),
            gains: Vec::new(),
            mix: false,
            mix_title: String::new(),
        };
        export_clip(&request, encoders, &mut |_| {}, &AtomicBool::new(false))?;
        let duration = probe_clip(&request.output)?.duration_secs;
        std::fs::remove_dir_all(&dir).ok();
        Ok(duration)
    }

    #[test]
    #[ignore = "needs a hardware video encoder"]
    fn a_precise_cut_starts_on_the_exact_frame() -> Result<()> {
        let duration = precise_cut_duration(&video_encoders()?, VideoCodec::H264, "h264")?;
        assert!((duration - 1.5).abs() < 0.07, "{duration}");
        Ok(())
    }

    #[test]
    fn a_precise_cut_on_the_processor_starts_on_the_exact_frame() -> Result<()> {
        let svt = [EncoderInfo {
            name: "libsvtav1".into(),
            description: String::new(),
            codec: VideoCodec::Av1,
            hardware: false,
            failure: None,
        }];
        let duration = precise_cut_duration(&svt, VideoCodec::Av1, "av1")?;
        assert!((duration - 1.5).abs() < 0.07, "{duration}");
        Ok(())
    }
}
