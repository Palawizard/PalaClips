use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::AVPacket;
use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::avutil::AVRational;

use crate::clock::{HNS_PER_SECOND, Hns};
use crate::config::RecordSplit;
use crate::encode::{route_ffmpeg_logs, write_thumbnail};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Localized, Severity};
use crate::library::{STOP_FREE_BYTES, WARN_FREE_BYTES, disk_space};
use crate::mux::{Mp4Writer, MuxStream};
use crate::paths;
use crate::replay::{PacketReader, ReplayPacket, packet_data, share_packet};

const PREVIOUS_SEGMENT_GRACE: Hns = Hns::from_millis(3_000);
const SPACE_CHECK_INTERVAL: Hns = Hns::from_millis(2_000);

static JOURNAL_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub struct Journal {
    path: PathBuf,
}

impl Journal {
    pub fn open_default() -> Result<Self> {
        Ok(Self::at(paths::recordings_journal()?))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn entries(&self) -> Vec<(u32, PathBuf)> {
        let _guard = JOURNAL_LOCK.lock();
        self.read()
    }

    fn read(&self) -> Vec<(u32, PathBuf)> {
        std::fs::read_to_string(&self.path)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (pid, path) = line.split_once('\t')?;
                Some((pid.parse().ok()?, PathBuf::from(path)))
            })
            .collect()
    }

    fn write(&self, entries: &[(u32, PathBuf)]) -> Result<()> {
        if entries.is_empty() {
            match std::fs::remove_file(&self.path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => return Ok(()),
            }
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text: String = entries
            .iter()
            .map(|(pid, path)| format!("{pid}\t{}\n", path.display()))
            .collect();
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn add(&self, file: &Path) -> Result<()> {
        let _guard = JOURNAL_LOCK.lock();
        let mut entries = self.read();
        entries.push((std::process::id(), file.to_path_buf()));
        self.write(&entries)
    }

    pub fn remove(&self, file: &Path) -> Result<()> {
        let _guard = JOURNAL_LOCK.lock();
        let mut entries = self.read();
        entries.retain(|(_, path)| path != file);
        self.write(&entries)
    }
}

fn process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    // SAFETY: SYNCHRONIZE only allows waiting on the process; the handle is closed below.
    let Ok(handle) = (unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }) else {
        return false;
    };
    // SAFETY: `handle` was just opened and is still valid.
    let alive = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
    // SAFETY: `handle` is owned here and closed once.
    unsafe { CloseHandle(handle) }.ok();
    alive
}

pub fn finalize(path: &Path) -> Result<()> {
    route_ffmpeg_logs();
    let url = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| EngineError::Mux("chemin de fichier invalide".into()))?;
    let mut input =
        AVFormatContextInput::open(&url).map_err(|error| EngineError::Mux(error.to_string()))?;
    let specs: Vec<MuxStream> = input
        .streams()
        .iter()
        .map(|stream| MuxStream {
            codec_parameters: stream.codecpar().clone(),
            time_base: stream.time_base,
            frame_rate: (stream.avg_frame_rate.num > 0 && stream.avg_frame_rate.den > 0)
                .then(|| (stream.avg_frame_rate.num / stream.avg_frame_rate.den) as u32),
            title: crate::library::stream_title(stream),
        })
        .collect();
    let finished = path.with_extension("recovering.mp4");
    let mut writer = Mp4Writer::create(&finished, &specs)?;
    let copied = (|| {
        while let Some(packet) = input
            .read_packet()
            .map_err(|error| EngineError::Mux(error.to_string()))?
        {
            let stream = packet.stream_index as usize;
            writer.write(stream, packet)?;
        }
        writer.finish()
    })();
    drop(input);
    if let Err(error) = copied {
        std::fs::remove_file(&finished).ok();
        return Err(error);
    }
    std::fs::rename(&finished, path)?;
    Ok(())
}

pub fn recover_interrupted(journal: &Journal, events: &Sender<EngineEvent>) {
    for (pid, path) in journal.entries() {
        if process_alive(pid) {
            continue;
        }
        let result = match std::fs::metadata(&path) {
            Err(_) => Ok(false),
            Ok(meta) if meta.len() == 0 => std::fs::remove_file(&path)
                .map(|()| false)
                .map_err(Into::into),
            Ok(_) => finalize(&path).map(|()| true),
        };
        match result {
            Ok(recovered) => {
                journal.remove(&path).ok();
                if recovered {
                    tracing::info!(path = %path.display(), "enregistrement interrompu récupéré");
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    events
                        .try_send(EngineEvent::Alert {
                            severity: Severity::Info,
                            message: format!("Enregistrement interrompu récupéré : {name}."),
                            text: Localized::new("recording_recovered").with("name", name),
                        })
                        .ok();
                }
            }
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "enregistrement interrompu non récupéré");
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Current,
    Previous,
    Both,
}

#[derive(Debug)]
pub struct Splitter {
    split: RecordSplit,
    audio_packet: Hns,
    start: Hns,
    bytes: u64,
    reached: Option<Vec<bool>>,
}

impl Splitter {
    pub fn new(split: RecordSplit, audio_packet: Hns, start: Hns) -> Self {
        Self {
            split,
            audio_packet,
            start,
            bytes: 0,
            reached: None,
        }
    }

    pub fn splits_at(&self, stream: usize, pts: Hns, keyframe: bool) -> bool {
        if stream != 0 || !keyframe || pts <= self.start {
            return false;
        }
        match self.split {
            RecordSplit::Never => false,
            RecordSplit::Duration { minutes } => {
                pts - self.start >= Hns(i64::from(minutes) * 60 * HNS_PER_SECOND)
            }
            RecordSplit::Size { megabytes } => self.bytes >= u64::from(megabytes) * 1_000_000,
        }
    }

    pub fn begin(&mut self, start: Hns, audio_streams: usize) {
        self.start = start;
        self.bytes = 0;
        self.reached = Some(vec![false; audio_streams]);
    }

    pub fn route(&mut self, stream: usize, pts: Hns) -> Route {
        let Some(reached) = self.reached.as_mut() else {
            return Route::Current;
        };
        if stream == 0 {
            return Route::Current;
        }
        if pts < self.start - self.audio_packet {
            return Route::Previous;
        }
        if pts < self.start {
            return Route::Both;
        }
        if let Some(flag) = reached.get_mut(stream - 1) {
            *flag = true;
        }
        Route::Current
    }

    pub fn previous_done(&self, newest_video: Hns) -> bool {
        self.reached
            .as_ref()
            .is_some_and(|reached| reached.iter().all(|flag| *flag))
            || newest_video - self.start > PREVIOUS_SEGMENT_GRACE
    }

    pub fn close_previous(&mut self) {
        self.reached = None;
    }

    pub fn has_previous(&self) -> bool {
        self.reached.is_some()
    }

    pub fn written(&mut self, bytes: u64) {
        self.bytes += bytes;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Recording,
    Replay,
}

#[derive(Debug, Clone)]
pub struct RecordTarget {
    pub root: PathBuf,
    pub game: String,
    pub split: RecordSplit,
    pub kind: RecordKind,
}

#[derive(Debug, Clone)]
pub struct RecordLayout {
    pub specs: Vec<MuxStream>,
    pub time_bases: Vec<AVRational>,
    pub frame: Hns,
    pub audio_packet: Hns,
}

struct Segment {
    writer: Mp4Writer,
    path: PathBuf,
    start: Hns,
    newest_video: Hns,
    last_keyframe: Option<AVPacket>,
}

impl Segment {
    fn open(
        target: &RecordTarget,
        layout: &RecordLayout,
        journal: &Journal,
        start: Hns,
    ) -> Result<Self> {
        let stamp = paths::local_timestamp();
        let path = match target.kind {
            RecordKind::Recording => paths::recording_path(&target.root, &target.game, &stamp),
            RecordKind::Replay => paths::replay_clip_path(&target.root, &target.game, &stamp),
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        journal.add(&path)?;
        let writer = Mp4Writer::create_fragmented(&path, &layout.specs)?;
        tracing::info!(path = %path.display(), "enregistrement : nouveau fichier");
        Ok(Self {
            writer,
            path,
            start,
            newest_video: start,
            last_keyframe: None,
        })
    }

    fn write(
        &mut self,
        layout: &RecordLayout,
        stream: usize,
        pts: Hns,
        keyframe: bool,
        mut packet: AVPacket,
    ) -> Result<()> {
        if stream == 0 {
            self.newest_video = self.newest_video.max(pts);
            if keyframe {
                self.last_keyframe = Some(share_packet(&packet)?);
            }
        }
        let base = layout.time_bases[stream];
        let shift = self
            .start
            .to_timebase(i64::from(base.num), i64::from(base.den));
        packet.set_pts(packet.pts - shift);
        packet.set_dts(packet.dts - shift);
        self.writer.write(stream, packet)
    }

    fn finish(self, layout: &RecordLayout, journal: &Journal) -> Result<EngineEvent> {
        self.writer.finish()?;
        journal.remove(&self.path)?;
        let duration = self.newest_video + layout.frame - self.start;
        let thumbnail = self.last_keyframe.and_then(|keyframe| {
            let path = paths::thumbnail_path(&self.path).ok()?;
            match write_thumbnail(&layout.specs[0].codec_parameters, &keyframe, &path) {
                Ok(()) => Some(path.to_string_lossy().into_owned()),
                Err(error) => {
                    tracing::warn!(%error, "miniature de l'enregistrement impossible");
                    None
                }
            }
        });
        tracing::info!(
            path = %self.path.display(),
            duration_s = duration.as_secs_f64(),
            "enregistrement terminé"
        );
        Ok(EngineEvent::ClipSaved {
            path: self.path.to_string_lossy().into_owned(),
            duration_secs: duration.as_secs_f64(),
            save_latency_ms: 0,
            thumbnail,
        })
    }
}

struct Writer {
    target: RecordTarget,
    layout: RecordLayout,
    journal: Journal,
    splitter: Splitter,
    current: Segment,
    previous: Option<Segment>,
    bytes: Arc<AtomicU64>,
    events: Sender<EngineEvent>,
    next_space_check: Hns,
    warned_space: bool,
}

impl Writer {
    fn check_space(&mut self, pts: Hns) -> Result<()> {
        if pts < self.next_space_check {
            return Ok(());
        }
        self.next_space_check = pts + SPACE_CHECK_INTERVAL;
        let free = disk_space(&self.target.root)?.free_bytes;
        if free < STOP_FREE_BYTES {
            return Err(EngineError::DiskFull);
        }
        if free < WARN_FREE_BYTES && !self.warned_space {
            self.warned_space = true;
            let free_mb = free / 1_000_000;
            self.events
                .try_send(EngineEvent::Alert {
                    severity: Severity::Warning,
                    text: Localized::new("disk_almost_full").with("free_mb", free_mb),
                    message: format!("Disque presque plein : {free_mb} Mo libres."),
                })
                .ok();
        }
        Ok(())
    }

    fn push(&mut self, stream: usize, pts: Hns, keyframe: bool, packet: AVPacket) -> Result<()> {
        if stream == 0 && keyframe {
            self.check_space(pts)?;
        }
        if self.splitter.splits_at(stream, pts, keyframe) {
            self.close_previous()?;
            let next = Segment::open(&self.target, &self.layout, &self.journal, pts)?;
            self.previous = Some(std::mem::replace(&mut self.current, next));
            self.splitter.begin(pts, self.layout.specs.len() - 1);
        }
        let size = packet_data(&packet).len() as u64;
        match self.splitter.route(stream, pts) {
            Route::Current => self
                .current
                .write(&self.layout, stream, pts, keyframe, packet)?,
            Route::Previous => {
                if let Some(previous) = self.previous.as_mut() {
                    previous.write(&self.layout, stream, pts, keyframe, packet)?;
                }
            }
            Route::Both => {
                if let Some(previous) = self.previous.as_mut() {
                    previous.write(&self.layout, stream, pts, keyframe, share_packet(&packet)?)?;
                }
                self.current
                    .write(&self.layout, stream, pts, keyframe, packet)?;
            }
        }
        self.splitter.written(size);
        self.bytes.fetch_add(size, Ordering::Relaxed);
        if self.splitter.has_previous() && self.splitter.previous_done(self.current.newest_video) {
            self.close_previous()?;
        }
        Ok(())
    }

    fn close_previous(&mut self) -> Result<()> {
        self.splitter.close_previous();
        if let Some(previous) = self.previous.take() {
            let event = previous.finish(&self.layout, &self.journal)?;
            self.events.try_send(event).ok();
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        let previous = self.close_previous();
        let event = self.current.finish(&self.layout, &self.journal)?;
        self.events.try_send(event).ok();
        previous
    }
}

pub struct LivePacket {
    pub stream: usize,
    pub pts: Hns,
    pub keyframe: bool,
    pub packet: AVPacket,
}

impl std::fmt::Debug for LivePacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LivePacket")
            .field("stream", &self.stream)
            .field("pts", &self.pts)
            .finish_non_exhaustive()
    }
}

pub struct Recorder {
    packets: Option<Sender<LivePacket>>,
    thread: Option<JoinHandle<()>>,
    started: std::time::Instant,
    bytes: Arc<AtomicU64>,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

impl Recorder {
    pub fn start(
        target: RecordTarget,
        layout: RecordLayout,
        start: Hns,
        backlog: Vec<(Hns, usize, ReplayPacket)>,
        events: Sender<EngineEvent>,
    ) -> Result<Self> {
        let journal = Journal::open_default()?;
        let kind = target.kind;
        let current = Segment::open(&target, &layout, &journal, start)?;
        let bytes = Arc::new(AtomicU64::new(0));
        let writer = Writer {
            splitter: Splitter::new(target.split, layout.audio_packet, start),
            target,
            layout,
            journal,
            current,
            previous: None,
            bytes: bytes.clone(),
            events: events.clone(),
            next_space_check: start,
            warned_space: false,
        };
        let (packets, live) = unbounded();
        let thread = std::thread::Builder::new()
            .name("palaclips-record".into())
            .spawn(move || {
                if let Err(error) = write_all(writer, backlog, &live) {
                    tracing::error!(%error, ?kind, "écriture interrompue");
                    let (code, message) = match kind {
                        RecordKind::Recording => (
                            "recording_failed",
                            format!("Enregistrement arrêté : {error}."),
                        ),
                        RecordKind::Replay => (
                            "clip_cut_short",
                            format!("Clip arrêté avant la fin : {error}. Le début est gardé."),
                        ),
                    };
                    events
                        .try_send(EngineEvent::Alert {
                            severity: Severity::Error,
                            text: Localized::new(code).with("reason", error.localized()),
                            message,
                        })
                        .ok();
                }
            })?;
        Ok(Self {
            packets: Some(packets),
            thread: Some(thread),
            started: std::time::Instant::now(),
            bytes,
        })
    }

    pub fn send(&self, packet: LivePacket) {
        if let Some(packets) = &self.packets {
            packets.send(packet).ok();
        }
    }

    pub fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    pub fn stats(&self) -> EngineEvent {
        EngineEvent::RecordingStats {
            elapsed_secs: self.started.elapsed().as_secs_f64(),
            bytes: self.bytes.load(Ordering::Relaxed),
        }
    }

    pub fn stop(mut self) {
        self.packets.take();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn write_all(
    mut writer: Writer,
    backlog: Vec<(Hns, usize, ReplayPacket)>,
    live: &Receiver<LivePacket>,
) -> Result<()> {
    let written = (|| {
        let mut reader = PacketReader::default();
        for (pts, stream, packet) in backlog {
            let keyframe = stream == 0 && crate::replay::TimedPacket::is_keyframe(&packet);
            let packet = reader.read(packet)?;
            writer.push(stream, pts, keyframe, packet)?;
        }
        for item in live {
            writer.push(item.stream, item.pts, item.keyframe, item.packet)?;
        }
        Ok(())
    })();
    let finished = writer.finish();
    written.and(finished)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::AacEncoder;

    const AAC: Hns = Hns(213_333);
    const DEAD_PID: u32 = u32::MAX - 1;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-record-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn tone() -> Result<(MuxStream, Vec<AVPacket>)> {
        let mut encoder = AacEncoder::open(48_000, 2)?;
        let samples: Vec<f32> = (0..48_000 * 2 * 4)
            .map(|index| ((index / 2) as f32 * 0.05).sin() * 0.3)
            .collect();
        let mut packets = Vec::new();
        for chunk in samples.chunks(960) {
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

    fn packet_pts(packet: &AVPacket) -> Hns {
        Hns(packet.pts * HNS_PER_SECOND / 48_000)
    }

    fn writer(root: &Path, journal: Journal, stream: MuxStream) -> Result<Writer> {
        let (events, _) = unbounded();
        let target = RecordTarget {
            root: root.to_path_buf(),
            game: "Test".into(),
            split: RecordSplit::Never,
            kind: RecordKind::Recording,
        };
        let layout = RecordLayout {
            time_bases: vec![stream.time_base],
            specs: vec![stream],
            frame: AAC,
            audio_packet: AAC,
        };
        let current = Segment::open(&target, &layout, &journal, Hns::ZERO)?;
        Ok(Writer {
            splitter: Splitter::new(RecordSplit::Never, AAC, Hns::ZERO),
            target,
            layout,
            journal,
            current,
            previous: None,
            bytes: Arc::new(AtomicU64::new(0)),
            events,
            next_space_check: Hns::ZERO,
            warned_space: false,
        })
    }

    #[test]
    fn the_journal_lists_recordings_until_they_finish() {
        let dir = scratch("journal");
        let journal = Journal::at(dir.join("recordings.txt"));
        journal.add(&dir.join("a.mp4")).expect("add");
        journal.add(&dir.join("b.mp4")).expect("add");
        journal.remove(&dir.join("a.mp4")).expect("remove");
        assert_eq!(journal.entries(), [(std::process::id(), dir.join("b.mp4"))]);
        journal.remove(&dir.join("b.mp4")).expect("remove");
        assert!(!dir.join("recordings.txt").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_full_disk_stops_the_recording_and_the_file_is_recovered_at_startup() -> Result<()> {
        let dir = scratch("disk-full");
        let journal = Journal::at(dir.join("recordings.txt"));
        let (stream, packets) = tone()?;
        let mut writer = writer(&dir, journal.clone(), stream)?;
        writer.current.writer.fail_after(30_000);
        let path = writer.current.path.clone();
        let mut stopped = None;
        for packet in packets {
            let pts = packet_pts(&packet);
            if let Err(error) = writer.push(0, pts, true, packet) {
                stopped = Some(error);
                break;
            }
        }
        assert!(
            matches!(stopped, Some(EngineError::DiskFull)),
            "{stopped:?}"
        );
        assert!(matches!(writer.finish(), Err(EngineError::DiskFull)));
        assert_eq!(journal.entries().len(), 1);
        journal.write(&[(DEAD_PID, path.clone())])?;
        let (events, received) = unbounded();
        recover_interrupted(&journal, &events);
        assert!(journal.entries().is_empty());
        let info = crate::library::probe_clip(&path)?;
        assert!(info.duration_secs > 0.5, "{}", info.duration_secs);
        let event = received.try_recv();
        assert!(
            matches!(&event, Ok(EngineEvent::Alert { text, .. }) if text.code == "recording_recovered"),
            "{event:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
        Ok(())
    }

    #[test]
    fn a_recording_of_a_running_process_is_left_alone() {
        let dir = scratch("alive");
        let journal = Journal::at(dir.join("recordings.txt"));
        let file = dir.join("live.mp4");
        std::fs::write(&file, b"en cours").expect("write");
        journal.add(&file).expect("add");
        let (events, _) = unbounded();
        recover_interrupted(&journal, &events);
        assert_eq!(journal.entries().len(), 1);
        assert_eq!(std::fs::read(&file).expect("read"), b"en cours");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn seconds(value: i64) -> Hns {
        Hns(value * HNS_PER_SECOND)
    }

    #[test]
    fn a_recording_without_split_never_splits() {
        let splitter = Splitter::new(RecordSplit::Never, AAC, Hns::ZERO);
        assert!(!splitter.splits_at(0, seconds(36_000), true));
    }

    #[test]
    fn a_duration_split_waits_for_the_next_video_keyframe() {
        let splitter = Splitter::new(RecordSplit::Duration { minutes: 1 }, AAC, Hns::ZERO);
        assert!(!splitter.splits_at(0, seconds(59), true));
        assert!(!splitter.splits_at(0, seconds(61), false));
        assert!(!splitter.splits_at(1, seconds(61), true));
        assert!(splitter.splits_at(0, seconds(61), true));
    }

    #[test]
    fn a_size_split_counts_the_bytes_of_the_current_file() {
        let mut splitter = Splitter::new(RecordSplit::Size { megabytes: 100 }, AAC, Hns::ZERO);
        splitter.written(99_000_000);
        assert!(!splitter.splits_at(0, seconds(10), true));
        splitter.written(1_000_000);
        assert!(splitter.splits_at(0, seconds(10), true));
        splitter.begin(seconds(10), 2);
        assert!(!splitter.splits_at(0, seconds(11), true));
    }

    #[test]
    fn late_audio_goes_to_the_file_it_belongs_to() {
        let mut splitter = Splitter::new(RecordSplit::Duration { minutes: 1 }, AAC, Hns::ZERO);
        splitter.begin(seconds(60), 2);
        assert_eq!(splitter.route(0, seconds(60)), Route::Current);
        assert_eq!(splitter.route(1, seconds(59)), Route::Previous);
        assert_eq!(splitter.route(1, seconds(60) - Hns(100_000)), Route::Both);
        assert_eq!(splitter.route(1, seconds(60)), Route::Current);
        assert!(!splitter.previous_done(seconds(60)));
        assert_eq!(splitter.route(2, seconds(60) + AAC), Route::Current);
        assert!(splitter.previous_done(seconds(60)));
        splitter.close_previous();
        assert_eq!(splitter.route(1, seconds(59)), Route::Current);
    }

    #[test]
    fn a_silent_track_cannot_keep_the_previous_file_open() {
        let mut splitter = Splitter::new(RecordSplit::Duration { minutes: 1 }, AAC, Hns::ZERO);
        splitter.begin(seconds(60), 1);
        assert!(!splitter.previous_done(seconds(62)));
        assert!(splitter.previous_done(seconds(64)));
    }
}
