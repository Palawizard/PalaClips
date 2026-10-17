use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, unbounded};
use rsmpeg::avcodec::AVPacket;
use rsmpeg::avutil::AVRational;

use crate::clock::{HNS_PER_SECOND, Hns};
use crate::config::RecordSplit;
use crate::encode::write_thumbnail;
use crate::error::Result;
use crate::events::{EngineEvent, Localized, Severity};
use crate::mux::{Mp4Writer, MuxStream};
use crate::paths;
use crate::replay::{PacketReader, ReplayPacket, packet_data, share_packet};

const PREVIOUS_SEGMENT_GRACE: Hns = Hns::from_millis(3_000);

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

#[derive(Debug, Clone)]
pub struct RecordTarget {
    pub root: PathBuf,
    pub game: String,
    pub split: RecordSplit,
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
    fn open(target: &RecordTarget, layout: &RecordLayout, start: Hns) -> Result<Self> {
        let path = paths::recording_path(&target.root, &target.game, &paths::local_timestamp());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
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

    fn finish(self, layout: &RecordLayout) -> Result<EngineEvent> {
        self.writer.finish()?;
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
    splitter: Splitter,
    current: Segment,
    previous: Option<Segment>,
    bytes: Arc<AtomicU64>,
    events: Sender<EngineEvent>,
}

impl Writer {
    fn push(&mut self, stream: usize, pts: Hns, keyframe: bool, packet: AVPacket) -> Result<()> {
        if self.splitter.splits_at(stream, pts, keyframe) {
            self.close_previous()?;
            let next = Segment::open(&self.target, &self.layout, pts)?;
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
            let event = previous.finish(&self.layout)?;
            self.events.try_send(event).ok();
        }
        Ok(())
    }

    fn finish(mut self) -> Result<()> {
        self.close_previous()?;
        let event = self.current.finish(&self.layout)?;
        self.events.try_send(event).ok();
        Ok(())
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
        let current = Segment::open(&target, &layout, start)?;
        let bytes = Arc::new(AtomicU64::new(0));
        let writer = Writer {
            splitter: Splitter::new(target.split, layout.audio_packet, start),
            target,
            layout,
            current,
            previous: None,
            bytes: bytes.clone(),
            events: events.clone(),
        };
        let (packets, live) = unbounded();
        let thread = std::thread::Builder::new()
            .name("palaclips-record".into())
            .spawn(move || {
                if let Err(error) = write_all(writer, backlog, &live) {
                    tracing::error!(%error, "enregistrement interrompu");
                    events
                        .try_send(EngineEvent::Alert {
                            severity: Severity::Error,
                            text: Localized::new("recording_failed")
                                .with("reason", error.localized()),
                            message: format!("Enregistrement arrêté : {error}."),
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
    let mut reader = PacketReader::default();
    for (pts, stream, packet) in backlog {
        let keyframe = stream == 0 && crate::replay::TimedPacket::is_keyframe(&packet);
        let packet = reader.read(packet)?;
        writer.push(stream, pts, keyframe, packet)?;
    }
    for item in live {
        writer.push(item.stream, item.pts, item.keyframe, item.packet)?;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AAC: Hns = Hns(213_333);

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
