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

const FRAGMENT_MICROSECONDS: &std::ffi::CStr = c"1000000";
const NO_SPACE_LEFT: i32 = -28;

pub struct Mp4Writer {
    output: AVFormatContextOutput,
    time_bases: Vec<(AVRational, AVRational)>,
    #[cfg(test)]
    budget: Option<u64>,
}

impl std::fmt::Debug for Mp4Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mp4Writer").finish_non_exhaustive()
    }
}

impl Mp4Writer {
    pub fn create(path: &Path, streams: &[MuxStream]) -> Result<Self> {
        Self::open(path, streams, None)
    }

    pub fn create_fragmented(path: &Path, streams: &[MuxStream]) -> Result<Self> {
        let options = AVDictionary::new(c"movflags", c"+hybrid_fragmented+frag_keyframe", 0)
            .set(c"frag_duration", FRAGMENT_MICROSECONDS, 0)
            .set(c"fflags", c"+flush_packets", 0);
        Self::open(path, streams, Some(options))
    }

    fn open(path: &Path, streams: &[MuxStream], mut options: Option<AVDictionary>) -> Result<Self> {
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
        output.write_header(&mut options).map_err(mux_error)?;
        let time_bases = streams
            .iter()
            .zip(output.streams().iter())
            .map(|(spec, stream)| (spec.time_base, stream.time_base))
            .collect();
        Ok(Self {
            output,
            time_bases,
            #[cfg(test)]
            budget: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn fail_after(&mut self, bytes: u64) {
        self.budget = Some(bytes);
    }

    #[cfg(test)]
    fn spend(&mut self, bytes: u64) -> Result<()> {
        match self.budget.as_mut() {
            Some(left) if *left < bytes => {
                *left = 0;
                Err(EngineError::DiskFull)
            }
            Some(left) => {
                *left -= bytes;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn write(&mut self, stream: usize, mut packet: AVPacket) -> Result<()> {
        let (packet_time_base, stream_time_base) = *self
            .time_bases
            .get(stream)
            .ok_or_else(|| EngineError::Mux(format!("flux {stream} inconnu")))?;
        if packet.duration == 0 {
            packet.set_duration(1);
        }
        #[cfg(test)]
        self.spend(crate::replay::packet_data(&packet).len() as u64)?;
        packet.rescale_ts(packet_time_base, stream_time_base);
        packet.set_stream_index(stream as i32);
        self.output
            .interleaved_write_frame(&mut packet)
            .map_err(mux_error)
    }

    pub fn finish(mut self) -> Result<()> {
        #[cfg(test)]
        self.spend(1)?;
        self.output.write_trailer().map_err(mux_error)
    }
}

fn mux_error(error: rsmpeg::error::RsmpegError) -> EngineError {
    if error.raw_error() == Some(NO_SPACE_LEFT) {
        return EngineError::DiskFull;
    }
    EngineError::Mux(error.to_string())
}

#[cfg(test)]
mod tests {
    use rsmpeg::avformat::AVFormatContextInput;

    use super::*;
    use crate::encode::AacEncoder;

    fn tone_packets(seconds: usize) -> Result<(MuxStream, Vec<AVPacket>)> {
        let mut encoder = AacEncoder::open(48_000, 2)?;
        let tone: Vec<f32> = (0..48_000 * 2 * seconds)
            .map(|index| ((index / 2) as f32 * 0.05).sin() * 0.3)
            .collect();
        let mut packets = Vec::new();
        for chunk in tone.chunks(960) {
            encoder.encode(chunk, &mut |packet| {
                packets.push(packet);
                Ok(())
            })?;
        }
        let stream = MuxStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            frame_rate: None,
            title: Some("Mix".into()),
        };
        Ok((stream, packets))
    }

    fn readable_packets(path: &Path) -> Result<usize> {
        let url = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| EngineError::Mux("chemin".into()))?;
        let mut input = AVFormatContextInput::open(&url).map_err(mux_error)?;
        let mut count = 0;
        while input.read_packet().map_err(mux_error)?.is_some() {
            count += 1;
        }
        Ok(count)
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("palaclips-mux-tests");
        std::fs::create_dir_all(&dir).ok();
        dir.join(format!("{name}-{}.mp4", std::process::id()))
    }

    #[test]
    fn an_unfinished_recording_reads_up_to_its_last_fragment() -> Result<()> {
        let (stream, packets) = tone_packets(4)?;
        let total = packets.len();
        let path = scratch("crash");
        let mut writer = Mp4Writer::create_fragmented(&path, std::slice::from_ref(&stream))?;
        for packet in packets {
            writer.write(0, packet)?;
        }
        drop(writer);
        let readable = readable_packets(&path)?;
        std::fs::remove_file(&path).ok();
        assert!(readable * 4 >= total * 3, "{readable} / {total}");
        Ok(())
    }

    #[test]
    fn a_finished_recording_is_a_regular_mp4() -> Result<()> {
        let (stream, packets) = tone_packets(2)?;
        let total = packets.len();
        let path = scratch("finished");
        let mut writer = Mp4Writer::create_fragmented(&path, std::slice::from_ref(&stream))?;
        for packet in packets {
            writer.write(0, packet)?;
        }
        writer.finish()?;
        let readable = readable_packets(&path)?;
        let info = crate::library::probe_clip(&path)?;
        std::fs::remove_file(&path).ok();
        assert_eq!(readable, total);
        assert!(
            (info.duration_secs - 2.0).abs() < 0.1,
            "{}",
            info.duration_secs
        );
        assert_eq!(info.tracks, ["Mix"]);
        Ok(())
    }
}
