use std::ffi::CString;
use std::path::Path;

use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avformat::AVFormatContextOutput;
use rsmpeg::avutil::{AVRational, ra};

use crate::encode::route_ffmpeg_logs;
use crate::error::{EngineError, Result};

pub struct Mp4Writer {
    output: AVFormatContextOutput,
    packet_time_base: AVRational,
    stream_time_base: AVRational,
}

impl std::fmt::Debug for Mp4Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mp4Writer").finish_non_exhaustive()
    }
}

impl Mp4Writer {
    pub fn create(
        path: &Path,
        codec_parameters: &AVCodecParameters,
        packet_time_base: AVRational,
        fps: u32,
    ) -> Result<Self> {
        route_ffmpeg_logs();
        let filename = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
        let mut output = AVFormatContextOutput::create(&filename).map_err(mux_error)?;
        {
            let mut stream = output.new_stream();
            stream.set_codecpar(codec_parameters.clone());
            stream.set_time_base(packet_time_base);
            stream.set_avg_frame_rate(ra(fps as i32, 1));
        }
        output.write_header(&mut None).map_err(mux_error)?;
        let stream_time_base = output.streams()[0].time_base;
        Ok(Self {
            output,
            packet_time_base,
            stream_time_base,
        })
    }

    pub fn write(&mut self, mut packet: AVPacket) -> Result<()> {
        if packet.duration == 0 {
            packet.set_duration(1);
        }
        packet.rescale_ts(self.packet_time_base, self.stream_time_base);
        packet.set_stream_index(0);
        self.output
            .interleaved_write_frame(&mut packet)
            .map_err(mux_error)
    }

    pub fn finish(mut self) -> Result<()> {
        self.output.write_trailer().map_err(mux_error)
    }
}

fn mux_error(error: rsmpeg::error::RsmpegError) -> EngineError {
    EngineError::Mux(error.to_string())
}
