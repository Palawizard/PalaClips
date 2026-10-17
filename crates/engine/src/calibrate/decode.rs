use std::ffi::CString;
use std::path::Path;

use rsmpeg::avcodec::{AVCodec, AVCodecContext};
use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::avutil::AVFrame;
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swscale::SwsContext;

use super::{AudioSignal, ClipSignals};
use crate::encode::route_ffmpeg_logs;
use crate::error::{EngineError, Result};

const LUMA_WIDTH: i32 = 64;
const LUMA_HEIGHT: i32 = 36;

enum Decoder {
    Video {
        context: AVCodecContext,
        time_base: f64,
        scaler: Option<SwsContext>,
    },
    Audio {
        context: AVCodecContext,
        time_base: f64,
        track: usize,
    },
    Skip,
}

pub fn read_signals(path: &Path) -> Result<ClipSignals> {
    route_ffmpeg_logs();
    let url = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
    let mut input = AVFormatContextInput::open(&url).map_err(decode_error)?;
    let mut signals = ClipSignals::default();
    let mut decoders = Vec::new();
    for stream in input.streams() {
        let parameters = stream.codecpar();
        let time_base = f64::from(stream.time_base.num) / f64::from(stream.time_base.den.max(1));
        let kind = parameters.codec_type;
        if kind != ffi::AVMEDIA_TYPE_VIDEO && kind != ffi::AVMEDIA_TYPE_AUDIO {
            decoders.push(Decoder::Skip);
            continue;
        }
        let codec = AVCodec::find_decoder(parameters.codec_id)
            .ok_or_else(|| EngineError::Mux("décodeur absent pour la mesure".into()))?;
        let mut context = AVCodecContext::new(&codec);
        context.apply_codecpar(&parameters).map_err(decode_error)?;
        context.open(None).map_err(decode_error)?;
        decoders.push(if kind == ffi::AVMEDIA_TYPE_VIDEO {
            Decoder::Video {
                context,
                time_base,
                scaler: None,
            }
        } else {
            signals.tracks.push(AudioSignal::default());
            Decoder::Audio {
                context,
                time_base,
                track: signals.tracks.len() - 1,
            }
        });
    }
    while let Some(packet) = input.read_packet().map_err(decode_error)? {
        if let Some(decoder) = decoders.get_mut(packet.stream_index as usize) {
            feed(decoder, Some(&packet), &mut signals)?;
        }
    }
    for decoder in &mut decoders {
        feed(decoder, None, &mut signals)?;
    }
    Ok(signals)
}

fn feed(
    decoder: &mut Decoder,
    packet: Option<&rsmpeg::avcodec::AVPacket>,
    signals: &mut ClipSignals,
) -> Result<()> {
    match decoder {
        Decoder::Skip => {}
        Decoder::Video {
            context,
            time_base,
            scaler,
        } => {
            context.send_packet(packet).map_err(decode_error)?;
            while let Some(frame) = next_frame(context)? {
                let at = frame.best_effort_timestamp as f64 * *time_base;
                signals.brightness.push((at, mean_luma(&frame, scaler)?));
            }
        }
        Decoder::Audio {
            context,
            time_base,
            track,
        } => {
            context.send_packet(packet).map_err(decode_error)?;
            while let Some(frame) = next_frame(context)? {
                append_mono(&frame, *time_base, &mut signals.tracks[*track]);
            }
        }
    }
    Ok(())
}

fn next_frame(context: &mut AVCodecContext) -> Result<Option<AVFrame>> {
    match context.receive_frame() {
        Ok(frame) => Ok(Some(frame)),
        Err(RsmpegError::DecoderDrainError | RsmpegError::DecoderFlushedError) => Ok(None),
        Err(error) => Err(decode_error(error)),
    }
}

fn mean_luma(frame: &AVFrame, scaler: &mut Option<SwsContext>) -> Result<f32> {
    if scaler.is_none() {
        *scaler = SwsContext::get_context(
            frame.width,
            frame.height,
            frame.format,
            LUMA_WIDTH,
            LUMA_HEIGHT,
            ffi::AV_PIX_FMT_GRAY8,
            ffi::SWS_AREA,
            None,
            None,
            None,
        );
    }
    let scaler = scaler
        .as_mut()
        .ok_or_else(|| EngineError::Mux("mise à l'échelle de la mesure impossible".into()))?;
    let mut gray = AVFrame::new();
    gray.set_width(LUMA_WIDTH);
    gray.set_height(LUMA_HEIGHT);
    gray.set_format(ffi::AV_PIX_FMT_GRAY8);
    gray.alloc_buffer().map_err(decode_error)?;
    scaler
        .scale_frame(frame, 0, frame.height, &mut gray)
        .map_err(decode_error)?;
    let stride = gray.linesize[0] as usize;
    let mut total = 0u64;
    for row in 0..LUMA_HEIGHT as usize {
        // SAFETY: the GRAY8 frame holds LUMA_HEIGHT rows of `stride` bytes, LUMA_WIDTH of them used.
        let line = unsafe {
            std::slice::from_raw_parts(gray.data[0].add(row * stride), LUMA_WIDTH as usize)
        };
        total += line.iter().map(|value| u64::from(*value)).sum::<u64>();
    }
    Ok(total as f32 / (LUMA_WIDTH * LUMA_HEIGHT) as f32)
}

fn append_mono(frame: &AVFrame, time_base: f64, signal: &mut AudioSignal) {
    if signal.samples.is_empty() {
        signal.start_s = frame.best_effort_timestamp as f64 * time_base;
        signal.rate = frame.sample_rate.max(1) as u32;
    }
    let channels = frame.ch_layout.nb_channels.max(1) as usize;
    let count = frame.nb_samples.max(0) as usize;
    if frame.format != ffi::AV_SAMPLE_FMT_FLTP {
        return;
    }
    for index in 0..count {
        let mut sum = 0.0f32;
        for channel in 0..channels.min(frame.data.len()) {
            // SAFETY: a planar float frame stores `nb_samples` f32 values per channel plane.
            sum += unsafe { *frame.data[channel].cast::<f32>().add(index) };
        }
        signal.samples.push(sum / channels as f32);
    }
}

fn decode_error(error: RsmpegError) -> EngineError {
    EngineError::Mux(format!("lecture du clip de mesure impossible : {error}"))
}
