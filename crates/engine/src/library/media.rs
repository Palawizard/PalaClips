use std::ffi::CString;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::ffi;
use serde::Serialize;

use crate::encode::{route_ffmpeg_logs, write_thumbnail};
use crate::error::{EngineError, Result};
use crate::mux::{Mp4Writer, MuxStream};

const TRACK_EXTENSION: &str = "m4a";
const PARTIAL_SUFFIX: &str = ".part";

pub(super) fn open_input(path: &Path) -> Result<AVFormatContextInput> {
    route_ffmpeg_logs();
    let url = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
    AVFormatContextInput::open(&url).map_err(|error| EngineError::Mux(error.to_string()))
}

pub fn thumbnail_from_file(clip: &Path, thumbnail: &Path) -> Result<()> {
    let mut input = open_input(clip)?;
    let video = input
        .streams()
        .iter()
        .position(|stream| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_VIDEO)
        .ok_or_else(|| EngineError::Mux("aucune piste vidéo".into()))?;
    let parameters = input.streams()[video].codecpar().clone();
    while let Some(packet) = input
        .read_packet()
        .map_err(|error| EngineError::Mux(error.to_string()))?
    {
        if packet.stream_index as usize == video && packet.flags & ffi::AV_PKT_FLAG_KEY as i32 != 0
        {
            return write_thumbnail(&parameters, &packet, thumbnail);
        }
    }
    Err(EngineError::Mux("aucune image clé".into()))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackFile {
    pub title: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct TrackCache {
    dir: PathBuf,
    limit_bytes: u64,
}

impl TrackCache {
    pub fn new(dir: PathBuf, limit_bytes: u64) -> Self {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().contains(PARTIAL_SUFFIX) {
                std::fs::remove_file(entry.path()).ok();
            }
        }
        Self { dir, limit_bytes }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn key(clip: &Path) -> Result<String> {
        let metadata = std::fs::metadata(clip)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        clip.hash(&mut hasher);
        metadata.len().hash(&mut hasher);
        metadata.modified().ok().hash(&mut hasher);
        Ok(format!("{:016x}", hasher.finish()))
    }

    pub fn tracks(&self, clip: &Path) -> Result<Vec<TrackFile>> {
        let key = Self::key(clip)?;
        let mut input = open_input(clip)?;
        let audio: Vec<(usize, MuxStream)> = input
            .streams()
            .iter()
            .enumerate()
            .filter(|(_, stream)| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_AUDIO)
            .map(|(index, stream)| {
                let title = super::stream_title(stream);
                (
                    index,
                    MuxStream {
                        codec_parameters: stream.codecpar().clone(),
                        time_base: stream.time_base,
                        frame_rate: None,
                        title,
                    },
                )
            })
            .collect();
        let files: Vec<PathBuf> = (0..audio.len())
            .map(|position| self.dir.join(format!("{key}_{position}.{TRACK_EXTENSION}")))
            .collect();
        if !files.iter().all(|file| file.exists()) {
            std::fs::create_dir_all(&self.dir)?;
            self.extract(&mut input, &audio, &files)?;
        }
        let now = std::time::SystemTime::now();
        for file in &files {
            std::fs::File::options()
                .write(true)
                .open(file)
                .and_then(|handle| handle.set_modified(now))
                .ok();
        }
        self.evict(&files);
        Ok(audio
            .into_iter()
            .zip(files)
            .map(|((_, stream), file)| TrackFile {
                title: stream.title.unwrap_or_default(),
                path: file.to_string_lossy().into_owned(),
            })
            .collect())
    }

    fn extract(
        &self,
        input: &mut AVFormatContextInput,
        audio: &[(usize, MuxStream)],
        files: &[PathBuf],
    ) -> Result<()> {
        let partials: Vec<PathBuf> = files
            .iter()
            .map(|file| file.with_extension(format!("part.{TRACK_EXTENSION}")))
            .collect();
        let written = (|| {
            let mut writers = audio
                .iter()
                .zip(&partials)
                .map(|((_, stream), partial)| {
                    Mp4Writer::create(partial, std::slice::from_ref(stream))
                })
                .collect::<Result<Vec<_>>>()?;
            while let Some(packet) = input
                .read_packet()
                .map_err(|error| EngineError::Mux(error.to_string()))?
            {
                let stream = packet.stream_index as usize;
                if let Some(position) = audio.iter().position(|(index, _)| *index == stream) {
                    writers[position].write(0, packet)?;
                }
            }
            for writer in writers {
                writer.finish()?;
            }
            Ok(())
        })();
        if let Err(error) = written {
            for partial in &partials {
                std::fs::remove_file(partial).ok();
            }
            return Err(error);
        }
        for (partial, file) in partials.iter().zip(files) {
            std::fs::rename(partial, file)?;
        }
        Ok(())
    }

    fn evict(&self, keep: &[PathBuf]) {
        let mut cached: Vec<(std::time::SystemTime, u64, PathBuf)> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let path = entry.path();
                let modified = metadata.modified().ok()?;
                path.extension()
                    .is_some_and(|extension| extension == TRACK_EXTENSION)
                    .then_some((modified, metadata.len(), path))
            })
            .collect();
        let mut total: u64 = cached.iter().map(|(_, size, _)| size).sum();
        cached.sort_by_key(|(modified, _, _)| *modified);
        for (_, size, path) in cached {
            if total <= self.limit_bytes {
                break;
            }
            if keep.contains(&path) {
                continue;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= size;
            }
        }
    }
}

pub const PEAK_BINS: usize = 1_600;
const PEAKS_EXTENSION: &str = "peaks";

pub fn track_peaks(track: &Path) -> Result<Vec<u8>> {
    let cached = track.with_extension(PEAKS_EXTENSION);
    if let Ok(peaks) = std::fs::read(&cached)
        && peaks.len() == PEAK_BINS
    {
        return Ok(peaks);
    }
    let peaks = compute_peaks(track)?;
    std::fs::write(&cached, &peaks).ok();
    Ok(peaks)
}

fn compute_peaks(track: &Path) -> Result<Vec<u8>> {
    use rsmpeg::avcodec::{AVCodec, AVCodecContext};
    use rsmpeg::error::RsmpegError;

    let decode_error = |error: RsmpegError| EngineError::Mux(error.to_string());
    let mut input = open_input(track)?;
    let (index, parameters, time_base) = input
        .streams()
        .iter()
        .enumerate()
        .find(|(_, stream)| stream.codecpar().codec_type == ffi::AVMEDIA_TYPE_AUDIO)
        .map(|(index, stream)| (index, stream.codecpar().clone(), stream.time_base))
        .ok_or_else(|| EngineError::Mux("aucune piste audio".into()))?;
    let duration = input.duration.max(1) as f64 / f64::from(ffi::AV_TIME_BASE);
    let codec = AVCodec::find_decoder(parameters.codec_id)
        .ok_or_else(|| EngineError::Mux("décodeur absent".into()))?;
    let mut decoder = AVCodecContext::new(&codec);
    decoder.apply_codecpar(&parameters).map_err(decode_error)?;
    decoder.set_pkt_timebase(time_base);
    decoder.open(None).map_err(decode_error)?;
    let mut peaks = vec![0f32; PEAK_BINS];
    let drain = |decoder: &mut AVCodecContext, peaks: &mut [f32]| -> Result<()> {
        loop {
            let frame = match decoder.receive_frame() {
                Ok(frame) => frame,
                Err(RsmpegError::DecoderDrainError | RsmpegError::DecoderFlushedError) => {
                    return Ok(());
                }
                Err(error) => return Err(decode_error(error)),
            };
            if frame.format != ffi::AV_SAMPLE_FMT_FLTP {
                continue;
            }
            let rate = f64::from(frame.sample_rate.max(1));
            let start = frame.best_effort_timestamp as f64 * f64::from(time_base.num)
                / f64::from(time_base.den.max(1));
            let samples = frame.nb_samples.max(0) as usize;
            for channel in 0..frame.ch_layout.nb_channels.max(0) as usize {
                // SAFETY: an FLTP frame holds `nb_samples` f32 values in each channel plane.
                let plane = unsafe {
                    std::slice::from_raw_parts(
                        frame.extended_data.add(channel).read().cast::<f32>(),
                        samples,
                    )
                };
                for (offset, value) in plane.iter().enumerate() {
                    let at = start + offset as f64 / rate;
                    let bin = ((at / duration) * PEAK_BINS as f64) as usize;
                    if let Some(peak) = peaks.get_mut(bin.min(PEAK_BINS - 1)) {
                        *peak = peak.max(value.abs());
                    }
                }
            }
        }
    };
    while let Some(packet) = input.read_packet().map_err(decode_error)? {
        if packet.stream_index as usize == index {
            decoder.send_packet(Some(&packet)).map_err(decode_error)?;
            drain(&mut decoder, &mut peaks)?;
        }
    }
    decoder.send_packet(None).map_err(decode_error)?;
    drain(&mut decoder, &mut peaks)?;
    Ok(peaks
        .into_iter()
        .map(|peak| (peak.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect())
}

pub fn copy_file_to_clipboard(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_HDROP;
    use windows::Win32::UI::Shell::DROPFILES;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let header = size_of::<DROPFILES>();
    let clipboard_error = |error: windows::core::Error| {
        EngineError::Io(std::io::Error::other(format!(
            "presse-papiers : {}",
            error.message()
        )))
    };
    // SAFETY: allocates a movable global block owned by this function until the clipboard takes it.
    let memory: HGLOBAL =
        unsafe { GlobalAlloc(GMEM_MOVEABLE, header + wide.len() * 2) }.map_err(clipboard_error)?;
    // SAFETY: `memory` is a fresh block of `header + wide.len() * 2` bytes; the DROPFILES header and
    // the double-null-terminated wide path are written inside it before unlocking.
    unsafe {
        let base = GlobalLock(memory).cast::<u8>();
        base.cast::<DROPFILES>().write(DROPFILES {
            pFiles: header as u32,
            fWide: true.into(),
            ..Default::default()
        });
        std::ptr::copy_nonoverlapping(wide.as_ptr(), base.add(header).cast::<u16>(), wide.len());
        GlobalUnlock(memory).ok();
    }
    // SAFETY: the clipboard is opened for this thread and closed before returning; on success the
    // system owns `memory`, otherwise it is freed here.
    unsafe {
        OpenClipboard(None).map_err(clipboard_error)?;
        let placed = EmptyClipboard()
            .and_then(|()| SetClipboardData(u32::from(CF_HDROP.0), Some(HANDLE(memory.0))))
            .map_err(clipboard_error);
        CloseClipboard().ok();
        if placed.is_err() {
            GlobalFree(Some(memory)).ok();
        }
        placed.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use rsmpeg::avcodec::AVPacket;

    use super::*;
    use crate::encode::AacEncoder;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-media-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn two_track_clip(path: &Path, seconds: usize) {
        let mut streams = Vec::new();
        let mut packets: Vec<(usize, AVPacket)> = Vec::new();
        for (index, title) in ["Mix", "Discord"].into_iter().enumerate() {
            let mut encoder = AacEncoder::open(48_000, 2).expect("aac");
            let samples = vec![0.05f32 * (index + 1) as f32; 48_000 * 2 * seconds];
            for chunk in samples.chunks(960) {
                encoder
                    .encode(chunk, &mut |packet| {
                        packets.push((index, packet));
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
        packets.sort_by_key(|(index, packet)| (packet.pts, *index));
        let mut writer = Mp4Writer::create(path, &streams).expect("writer");
        for (index, packet) in packets {
            writer.write(index, packet).expect("write");
        }
        writer.finish().expect("finish");
    }

    #[test]
    fn each_audio_track_is_copied_once_into_its_own_file() -> Result<()> {
        let dir = scratch("tracks");
        let clip = dir.join("clip.mp4");
        two_track_clip(&clip, 2);
        let cache = TrackCache::new(dir.join("tracks"), 100_000_000);
        let tracks = cache.tracks(&clip)?;
        assert_eq!(
            tracks
                .iter()
                .map(|track| track.title.as_str())
                .collect::<Vec<_>>(),
            ["Mix", "Discord"]
        );
        for track in &tracks {
            let info = crate::library::probe_clip(Path::new(&track.path))?;
            assert!(
                (info.duration_secs - 2.0).abs() < 0.1,
                "{}",
                info.duration_secs
            );
        }
        let first = std::fs::metadata(&tracks[0].path)?.len();
        let again = cache.tracks(&clip)?;
        assert_eq!(again, tracks);
        assert_eq!(std::fs::metadata(&again[0].path)?.len(), first);
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn peaks_follow_the_loudness_of_the_track_and_are_cached() -> Result<()> {
        let dir = scratch("peaks");
        let clip = dir.join("clip.mp4");
        two_track_clip(&clip, 2);
        let cache = TrackCache::new(dir.join("tracks"), u64::MAX);
        let tracks = cache.tracks(&clip)?;
        let quiet = track_peaks(Path::new(&tracks[0].path))?;
        let loud = track_peaks(Path::new(&tracks[1].path))?;
        assert_eq!(quiet.len(), PEAK_BINS);
        let middle = PEAK_BINS / 2;
        assert!(
            loud[middle] > quiet[middle],
            "{} {}",
            loud[middle],
            quiet[middle]
        );
        assert!(
            Path::new(&tracks[0].path)
                .with_extension(PEAKS_EXTENSION)
                .exists()
        );
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn the_cache_drops_the_least_recently_used_tracks_beyond_its_limit() -> Result<()> {
        let dir = scratch("evict");
        let old = dir.join("old.mp4");
        let new = dir.join("new.mp4");
        two_track_clip(&old, 2);
        two_track_clip(&new, 2);
        let unlimited = TrackCache::new(dir.join("tracks"), u64::MAX);
        let old_tracks = unlimited.tracks(&old)?;
        let size: u64 = old_tracks
            .iter()
            .map(|track| {
                std::fs::metadata(&track.path)
                    .map(|meta| meta.len())
                    .unwrap_or(0)
            })
            .sum();
        let limited = TrackCache::new(dir.join("tracks"), size + size / 2);
        let new_tracks = limited.tracks(&new)?;
        assert!(
            new_tracks
                .iter()
                .all(|track| Path::new(&track.path).exists())
        );
        assert!(
            old_tracks
                .iter()
                .any(|track| !Path::new(&track.path).exists())
        );
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }
}
