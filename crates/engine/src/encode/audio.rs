use rsmpeg::avcodec::{AVCodec, AVCodecContext, AVCodecParameters, AVPacket};
use rsmpeg::avutil::{AVChannelLayout, AVFrame, AVRational, ra};
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;

use super::log::route_ffmpeg_logs;
use crate::error::{EngineError, Result};

const AAC_BITRATE: i64 = 192_000;

pub struct AacEncoder {
    context: AVCodecContext,
    channels: usize,
    frame_size: usize,
    pending: Vec<f32>,
    next_pts: i64,
}

impl std::fmt::Debug for AacEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AacEncoder")
            .field("frame_size", &self.frame_size)
            .field("next_pts", &self.next_pts)
            .finish_non_exhaustive()
    }
}

impl AacEncoder {
    pub fn open(sample_rate: u32, channels: usize) -> Result<Self> {
        Self::open_with_bitrate(sample_rate, channels, AAC_BITRATE)
    }

    pub fn open_with_bitrate(sample_rate: u32, channels: usize, bit_rate: i64) -> Result<Self> {
        route_ffmpeg_logs();
        let codec = AVCodec::find_encoder(ffi::AV_CODEC_ID_AAC)
            .ok_or_else(|| EngineError::Encoder("encodeur AAC absent de FFmpeg".into()))?;
        let mut context = AVCodecContext::new(&codec);
        context.set_sample_fmt(ffi::AV_SAMPLE_FMT_FLTP);
        context.set_sample_rate(sample_rate as i32);
        context.set_ch_layout(AVChannelLayout::from_nb_channels(channels as i32).into_inner());
        context.set_bit_rate(bit_rate);
        context.set_time_base(ra(1, sample_rate as i32));
        context.set_flags(context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        context
            .open(None)
            .map_err(|error| EngineError::Encoder(format!("AAC : {error}")))?;
        let frame_size = context.frame_size.max(1) as usize;
        Ok(Self {
            context,
            channels,
            frame_size,
            pending: Vec::with_capacity(frame_size * channels * 2),
            next_pts: 0,
        })
    }

    pub fn codec_parameters(&self) -> AVCodecParameters {
        self.context.extract_codecpar()
    }

    pub fn time_base(&self) -> AVRational {
        self.context.time_base
    }

    pub fn priming_samples(&self) -> i64 {
        i64::from(self.context.initial_padding)
    }

    pub fn encode(
        &mut self,
        interleaved: &[f32],
        on_packet: &mut impl FnMut(AVPacket) -> Result<()>,
    ) -> Result<()> {
        self.pending.extend_from_slice(interleaved);
        let frame_len = self.frame_size * self.channels;
        let mut start = 0;
        while self.pending.len() - start >= frame_len {
            self.send(start, self.frame_size, on_packet)?;
            start += frame_len;
        }
        self.pending.drain(..start);
        Ok(())
    }

    pub fn finish(&mut self, on_packet: &mut impl FnMut(AVPacket) -> Result<()>) -> Result<()> {
        let remaining = self.pending.len() / self.channels;
        if remaining > 0 {
            self.send(0, remaining, on_packet)?;
            self.pending.clear();
        }
        self.context.send_frame(None).map_err(encoder_error)?;
        self.drain(on_packet)
    }

    fn send(
        &mut self,
        start: usize,
        samples: usize,
        on_packet: &mut impl FnMut(AVPacket) -> Result<()>,
    ) -> Result<()> {
        let mut frame = AVFrame::new();
        frame.set_format(ffi::AV_SAMPLE_FMT_FLTP);
        frame.set_sample_rate(self.context.sample_rate);
        frame.set_ch_layout(AVChannelLayout::from_nb_channels(self.channels as i32).into_inner());
        frame.set_nb_samples(samples as i32);
        frame.set_pts(self.next_pts);
        frame.alloc_buffer().map_err(encoder_error)?;
        let source = &self.pending[start..start + samples * self.channels];
        for channel in 0..self.channels {
            let plane = frame.data_mut()[channel].cast::<f32>();
            for (index, frame_samples) in source.chunks_exact(self.channels).enumerate() {
                // SAFETY: av_frame_get_buffer allocated `samples` f32 values per FLTP plane.
                unsafe { plane.add(index).write(frame_samples[channel]) };
            }
        }
        self.next_pts += samples as i64;
        self.context
            .send_frame(Some(&frame))
            .map_err(encoder_error)?;
        self.drain(on_packet)
    }

    fn drain(&mut self, on_packet: &mut impl FnMut(AVPacket) -> Result<()>) -> Result<()> {
        loop {
            match self.context.receive_packet() {
                Ok(packet) => on_packet(packet)?,
                Err(RsmpegError::EncoderDrainError | RsmpegError::EncoderFlushedError) => {
                    return Ok(());
                }
                Err(error) => return Err(encoder_error(error)),
            }
        }
    }
}

fn encoder_error(error: RsmpegError) -> EngineError {
    EngineError::Encoder(format!("AAC : {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aac_packets_cover_every_input_sample() -> Result<()> {
        let mut encoder = AacEncoder::open(48_000, 2)?;
        let mut packets = Vec::new();
        let tone: Vec<f32> = (0..48_000 * 2)
            .map(|index| ((index / 2) as f32 * 0.05).sin() * 0.5)
            .collect();
        for chunk in tone.chunks(960) {
            encoder.encode(chunk, &mut |packet| {
                packets.push((packet.pts, packet.duration));
                Ok(())
            })?;
        }
        encoder.finish(&mut |packet| {
            packets.push((packet.pts, packet.duration));
            Ok(())
        })?;
        let priming = encoder.priming_samples();
        assert_eq!(packets[0].0, -priming);
        let end = packets.last().map(|(pts, duration)| pts + duration);
        assert_eq!(end, Some(48_000));
        Ok(())
    }
}
