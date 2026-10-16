use std::ffi::CString;
use std::path::Path;

use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avformat::AVFormatContextOutput;
use rsmpeg::avutil::{AVDictionary, AVRational, ra};

use crate::encode::route_ffmpeg_logs;
use crate::error::{EngineError, Result};

#[derive(Debug, Clone)]
pub struct MuxStream {
    pub codec_parameters: AVCodecParameters,
    pub time_base: AVRational,
    pub frame_rate: Option<u32>,
    pub title: Option<String>,
}

pub struct Mp4Writer {
    output: AVFormatContextOutput,
    time_bases: Vec<(AVRational, AVRational)>,
}

impl std::fmt::Debug for Mp4Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mp4Writer").finish_non_exhaustive()
    }
}

impl Mp4Writer {
    pub fn create(path: &Path, streams: &[MuxStream]) -> Result<Self> {
        route_ffmpeg_logs();
        let filename = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
        let mut output = AVFormatContextOutput::create(&filename).map_err(mux_error)?;
        for spec in streams {
            let mut stream = output.new_stream();
            stream.set_codecpar(spec.codec_parameters.clone());
            stream.set_time_base(spec.time_base);
            if let Some(fps) = spec.frame_rate {
                stream.set_avg_frame_rate(ra(fps as i32, 1));
            }
            if let Some(title) = &spec.title {
                let title = CString::new(title.as_str())
                    .map_err(|_| EngineError::Mux("titre de piste invalide".into()))?;
                stream.set_metadata(Some(AVDictionary::new(c"title", &title, 0).set(
                    c"handler_name",
                    &title,
                    0,
                )));
            }
        }
        output.write_header(&mut None).map_err(mux_error)?;
        let time_bases = streams
            .iter()
            .zip(output.streams().iter())
            .map(|(spec, stream)| (spec.time_base, stream.time_base))
            .collect();
        Ok(Self { output, time_bases })
    }

    pub fn write(&mut self, stream: usize, mut packet: AVPacket) -> Result<()> {
        let (packet_time_base, stream_time_base) = *self
            .time_bases
            .get(stream)
            .ok_or_else(|| EngineError::Mux(format!("flux {stream} inconnu")))?;
        if packet.duration == 0 {
            packet.set_duration(1);
        }
        packet.rescale_ts(packet_time_base, stream_time_base);
        packet.set_stream_index(stream as i32);
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
