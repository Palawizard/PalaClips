use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Select, Sender, bounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::AVPacket;
use rsmpeg::avutil::AVRational;
use rsmpeg::ffi::AV_PKT_FLAG_KEY;

use crate::audio::{AudioSource, AudioTrack, SAMPLE_RATE};
use crate::capture::{VideoCapture, VideoStream};
use crate::clock::{HNS_PER_SECOND, Hns};
use crate::config::{EngineConfig, ReplayStorage};
use crate::encode::{hardware_video_encoders, pick_encoder};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, EngineState, Severity};
use crate::mux::{Mp4Writer, MuxStream};
use crate::paths;
use crate::replay::{PacketReader, PacketRing, ReplayPacket, Spool, TimedPacket, plan_clip};
use crate::sources::{CurrentGame, ForegroundWatch};

const COMMAND_QUEUE: usize = 16;
const EVENT_QUEUE: usize = 256;
const STATS_INTERVAL: Hns = Hns::from_millis(1_000);
const AAC_PACKET: Hns = Hns(1_024 * HNS_PER_SECOND / SAMPLE_RATE as i64);
const AUDIO_LAG_MARGIN: Hns = Hns::from_millis(2_000);

enum Command {
    Arm,
    Disarm,
    SaveReplay,
    Configure(Box<EngineConfig>),
    Shutdown,
}

pub struct Engine {
    commands: Sender<Command>,
    events: Receiver<EngineEvent>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct EngineHandle {
    commands: Sender<Command>,
}

impl std::fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineHandle").finish_non_exhaustive()
    }
}

impl Engine {
    pub fn start(config: EngineConfig) -> Result<Self> {
        config.validate()?;
        let (commands, command_rx) = bounded(COMMAND_QUEUE);
        let (event_tx, events) = bounded(EVENT_QUEUE);
        let thread = std::thread::Builder::new()
            .name("palaclips-engine".into())
            .spawn(move || {
                Core {
                    config,
                    events: event_tx,
                    pipeline: None,
                }
                .run(&command_rx)
            })?;
        Ok(Self {
            commands,
            events,
            thread: Some(thread),
        })
    }

    pub fn handle(&self) -> EngineHandle {
        EngineHandle {
            commands: self.commands.clone(),
        }
    }

    pub fn events(&self) -> &Receiver<EngineEvent> {
        &self.events
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.commands.send(Command::Shutdown).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

impl EngineHandle {
    pub fn arm(&self) -> Result<()> {
        self.send(Command::Arm)
    }

    pub fn disarm(&self) -> Result<()> {
        self.send(Command::Disarm)
    }

    pub fn save_replay(&self) -> Result<()> {
        self.send(Command::SaveReplay)
    }

    pub fn configure(&self, config: EngineConfig) -> Result<()> {
        config.validate()?;
        self.send(Command::Configure(Box::new(config)))
    }

    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| EngineError::Stopped)
    }
}

pub fn audio_sources(config: &EngineConfig) -> Vec<(AudioSource, String, i32)> {
    let audio = &config.audio;
    let mut sources = Vec::new();
    if audio.record_full_mix {
        sources.push((
            AudioSource::SystemMix {
                device_id: audio.output_device_id.clone(),
            },
            "Mix complet".to_owned(),
            audio.full_mix_sync_offset_ms,
        ));
    }
    if audio.record_microphone {
        sources.push((
            AudioSource::Microphone {
                device_id: audio.microphone_device_id.clone(),
                gain_db: audio.microphone_gain_db,
            },
            "Micro".to_owned(),
            audio.microphone_sync_offset_ms,
        ));
    }
    for track in audio.app_tracks.iter().filter(|track| track.enabled) {
        sources.push((
            AudioSource::Application {
                exe_name: track.exe_name.clone(),
            },
            track.label.clone(),
            track.sync_offset_ms,
        ));
    }
    if audio.auto_game_track {
        sources.push((AudioSource::Game(current_game(config)), "Jeu".to_owned(), 0));
    }
    sources
}

fn current_game(config: &EngineConfig) -> CurrentGame {
    CurrentGame::new(config.audio.game_exe.clone(), &config.capture)
}

fn ring_window(config: &EngineConfig) -> Hns {
    Hns(i64::from(config.replay.duration_secs) * HNS_PER_SECOND)
        + Hns::from_secs_f64(f64::from(config.video.keyframe_interval_secs))
        + AUDIO_LAG_MARGIN
}

fn needs_restart(old: &EngineConfig, new: &EngineConfig) -> bool {
    old.capture != new.capture
        || old.video != new.video
        || old.audio != new.audio
        || old.replay.storage != new.replay.storage
}

struct Core {
    config: EngineConfig,
    events: Sender<EngineEvent>,
    pipeline: Option<Pipeline>,
}

impl Core {
    fn run(mut self, commands: &Receiver<Command>) {
        loop {
            let command = match self.pipeline.as_mut() {
                None => commands.recv().unwrap_or(Command::Shutdown),
                Some(pipeline) => match pipeline.pump(commands, &self.events) {
                    Ok(command) => command.unwrap_or(Command::Shutdown),
                    Err(error) => {
                        tracing::error!(%error, "replay interrompu");
                        self.alert(Severity::Error, "replay_stopped", format!(
                            "Le replay s'est arrêté : {error}. Réarme-le quand la source est de nouveau disponible."
                        ));
                        self.disarm();
                        continue;
                    }
                },
            };
            match command {
                Command::Arm => self.arm(),
                Command::Disarm => self.disarm(),
                Command::SaveReplay => self.save_replay(),
                Command::Configure(config) => self.configure(*config),
                Command::Shutdown => {
                    self.disarm();
                    break;
                }
            }
        }
    }

    fn arm(&mut self) {
        if self.pipeline.is_some() {
            return;
        }
        match Pipeline::start(&self.config, &self.events) {
            Ok(pipeline) => {
                self.pipeline = Some(pipeline);
                self.emit(EngineEvent::StateChanged {
                    state: EngineState::ReplayArmed,
                });
            }
            Err(error) => {
                tracing::error!(%error, "armement du replay impossible");
                self.alert(
                    Severity::Error,
                    "replay_arm_failed",
                    format!("Impossible d'armer le replay : {error}."),
                );
            }
        }
    }

    fn disarm(&mut self) {
        let Some(pipeline) = self.pipeline.take() else {
            return;
        };
        pipeline.stop();
        self.emit(EngineEvent::StateChanged {
            state: EngineState::Idle,
        });
    }

    fn save_replay(&mut self) {
        let requested = Hns::now();
        let result = match self.pipeline.as_mut() {
            Some(pipeline) => pipeline.save(requested, &self.config, &self.events),
            None => Err(EngineError::NothingToSave("le replay n'est pas armé")),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "sauvegarde du replay impossible");
            self.alert(
                Severity::Warning,
                "clip_save_failed",
                format!("Clip non sauvegardé : {error}."),
            );
        }
    }

    fn configure(&mut self, config: EngineConfig) {
        let restart = self.pipeline.is_some() && needs_restart(&self.config, &config);
        if let Some(pipeline) = self.pipeline.as_mut() {
            pipeline.set_window(ring_window(&config));
        }
        self.config = config;
        if restart {
            tracing::info!("sources ou zone de capture changées : redémarrage du replay");
            self.disarm();
            self.arm();
        }
    }

    fn alert(&self, severity: Severity, code: &str, message: String) {
        self.emit(EngineEvent::Alert {
            severity,
            code: code.into(),
            message,
        });
    }

    fn emit(&self, event: EngineEvent) {
        self.events.try_send(event).ok();
    }
}

struct Pipeline {
    capture: VideoCapture,
    tracks: Vec<AudioTrack>,
    video: VideoStream,
    specs: Vec<MuxStream>,
    time_bases: Vec<AVRational>,
    receivers: Vec<Receiver<AVPacket>>,
    alerts: Vec<Receiver<EngineEvent>>,
    rings: Vec<PacketRing<ReplayPacket>>,
    spool: Option<Spool>,
    game: Arc<Mutex<CurrentGame>>,
    _foreground: Option<ForegroundWatch>,
    next_stats: Hns,
}

impl Pipeline {
    fn start(config: &EngineConfig, events: &Sender<EngineEvent>) -> Result<Self> {
        let encoders = hardware_video_encoders()?;
        let encoder = pick_encoder(&encoders, config.video.codec)?;
        let capture = VideoCapture::start(&config.capture, &config.video, &encoder.name)?;
        if let Some(alert) = capture.alert() {
            events.try_send(alert.clone()).ok();
        }
        let video = capture.stream().clone();
        let tracks = audio_sources(config)
            .into_iter()
            .map(|(source, title, offset_ms)| {
                AudioTrack::start(
                    source,
                    title,
                    video.origin,
                    Hns::from_millis(offset_ms.into()),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let mut specs = vec![MuxStream {
            codec_parameters: video.codec_parameters.clone(),
            time_base: video.time_base,
            frame_rate: Some(video.fps),
            title: None,
        }];
        let mut receivers = vec![capture.packets().clone()];
        for track in &tracks {
            specs.push(MuxStream {
                codec_parameters: track.stream().codec_parameters.clone(),
                time_base: track.stream().time_base,
                frame_rate: None,
                title: Some(track.stream().title.clone()),
            });
            receivers.push(track.packets().clone());
        }
        let window = ring_window(config);
        let spool = match config.replay.storage {
            ReplayStorage::Ram => None,
            ReplayStorage::Disk => Some(Spool::new(paths::replay_spool_dir()?)?),
        };
        let game = Arc::new(Mutex::new(current_game(config)));
        let foreground = if game.lock().follows_foreground() {
            let game = game.clone();
            Some(ForegroundWatch::start(move |window| {
                let mut game = game.lock();
                if game.on_foreground(&window) {
                    tracing::info!(game = ?game.exe_name(), "jeu en cours");
                }
            })?)
        } else {
            None
        };
        tracing::info!(
            encoder = %encoder.name,
            width = video.size.width,
            height = video.size.height,
            fps = video.fps,
            tracks = tracks.len(),
            duration_s = config.replay.duration_secs,
            storage = ?config.replay.storage,
            "replay armé"
        );
        Ok(Self {
            time_bases: specs.iter().map(|spec| spec.time_base).collect(),
            rings: specs.iter().map(|_| PacketRing::new(window)).collect(),
            alerts: tracks.iter().map(|track| track.alerts().clone()).collect(),
            capture,
            tracks,
            video,
            specs,
            receivers,
            spool,
            game,
            _foreground: foreground,
            next_stats: Hns::now() + STATS_INTERVAL,
        })
    }

    fn pump(
        &mut self,
        commands: &Receiver<Command>,
        events: &Sender<EngineEvent>,
    ) -> Result<Option<Command>> {
        let receivers = self.receivers.clone();
        let alerts = self.alerts.clone();
        let mut select = Select::new();
        select.recv(commands);
        for receiver in &receivers {
            select.recv(receiver);
        }
        for alert in &alerts {
            select.recv(alert);
        }
        loop {
            let wait = (self.next_stats - Hns::now()).0.max(0) as u64;
            let Ok(operation) = select.select_timeout(Duration::from_nanos(wait * 100)) else {
                self.next_stats = Hns::now() + STATS_INTERVAL;
                events.try_send(self.stats()).ok();
                continue;
            };
            let index = operation.index();
            if index == 0 {
                return Ok(operation.recv(commands).ok());
            }
            let stream = index - 1;
            if let Some(receiver) = receivers.get(stream) {
                match operation.recv(receiver) {
                    Ok(packet) => self.store(stream, packet)?,
                    Err(_) => {
                        return Err(if stream == 0 {
                            EngineError::CaptureSourceLost("la capture vidéo s'est arrêtée".into())
                        } else {
                            EngineError::Audio(format!(
                                "la piste « {} » s'est arrêtée",
                                self.specs[stream].title.as_deref().unwrap_or("?")
                            ))
                        });
                    }
                }
            } else {
                match operation.recv(&alerts[stream - receivers.len()]) {
                    Ok(alert) => {
                        events.try_send(alert).ok();
                    }
                    Err(_) => select.remove(index),
                }
            }
        }
    }

    fn store(&mut self, stream: usize, packet: AVPacket) -> Result<()> {
        let time_base = self.time_bases[stream];
        let pts = Hns((i128::from(packet.pts)
            * i128::from(HNS_PER_SECOND)
            * i128::from(time_base.num)
            / i128::from(time_base.den)) as i64);
        let keyframe = stream > 0 || packet.flags & AV_PKT_FLAG_KEY as i32 != 0;
        let stored = match self.spool.as_mut() {
            Some(spool) => spool.store(packet, pts, keyframe)?,
            None => ReplayPacket::in_ram(packet, pts, keyframe),
        };
        self.rings[stream].push(stored);
        Ok(())
    }

    fn set_window(&mut self, window: Hns) {
        for ring in &mut self.rings {
            ring.set_window(window);
        }
    }

    fn stats(&self) -> EngineEvent {
        EngineEvent::ReplayStats {
            buffered_secs: self.rings[0].buffered().as_secs_f64(),
            bytes: self.rings.iter().map(|ring| ring.bytes() as u64).sum(),
        }
    }

    fn save(
        &mut self,
        requested: Hns,
        config: &EngineConfig,
        events: &Sender<EngineEvent>,
    ) -> Result<()> {
        if let Some(spool) = self.spool.as_mut() {
            spool.flush()?;
        }
        let duration = Hns(i64::from(config.replay.duration_secs) * HNS_PER_SECOND);
        let frame = Hns(HNS_PER_SECOND / i64::from(self.video.fps));
        let (video, audio) = self
            .rings
            .split_first()
            .ok_or(EngineError::NothingToSave("aucun flux"))?;
        let plan = plan_clip(video, audio, duration, frame, AAC_PACKET)
            .ok_or(EngineError::NothingToSave("le tampon est encore vide"))?;
        let mut packets = Vec::new();
        for (stream, ring) in self.rings.iter().enumerate() {
            let from = if stream == 0 {
                plan.video_from
            } else {
                plan.audio_from[stream - 1]
            };
            for packet in ring.iter_from(from).take_while(|p| p.pts() < plan.end) {
                packets.push((packet.pts(), stream, packet.share()?));
            }
        }
        packets.sort_by_key(|(pts, stream, _)| (*pts, *stream));
        let root = match &config.storage.clips_dir {
            Some(dir) => dir.clone(),
            None => paths::default_clips_dir()?,
        };
        let game = paths::game_label(self.game.lock().exe_name());
        let path = paths::replay_clip_path(&root, &game, &paths::local_timestamp());
        let shifts: Vec<i64> = self
            .time_bases
            .iter()
            .map(|base| {
                plan.start
                    .to_timebase(i64::from(base.num), i64::from(base.den))
            })
            .collect();
        let job = SaveJob {
            path,
            specs: self.specs.clone(),
            shifts,
            packets,
            duration: plan.end - plan.start,
            requested,
        };
        let events = events.clone();
        std::thread::Builder::new()
            .name("palaclips-replay-save".into())
            .spawn(move || {
                let path = job.path.clone();
                let event = match job.write() {
                    Ok(event) => event,
                    Err(error) => {
                        tracing::warn!(%error, path = %path.display(), "écriture du clip impossible");
                        std::fs::remove_file(&path).ok();
                        EngineEvent::Alert {
                            severity: Severity::Warning,
                            code: "clip_save_failed".into(),
                            message: format!("Clip non sauvegardé : {error}."),
                        }
                    }
                };
                events.try_send(event).ok();
            })?;
        Ok(())
    }

    fn stop(self) {
        let Self {
            capture,
            tracks,
            receivers,
            ..
        } = self;
        let drain = std::thread::spawn(move || drain_until_closed(&receivers));
        if let Err(error) = capture.stop() {
            tracing::warn!(%error, "arrêt de la capture vidéo");
        }
        let now = Hns::now();
        for track in tracks {
            if let Err(error) = track.stop(now) {
                tracing::warn!(%error, "arrêt d'une piste audio");
            }
        }
        drain.join().ok();
        tracing::info!("replay désarmé");
    }
}

struct SaveJob {
    path: PathBuf,
    specs: Vec<MuxStream>,
    shifts: Vec<i64>,
    packets: Vec<(Hns, usize, ReplayPacket)>,
    duration: Hns,
    requested: Hns,
}

impl SaveJob {
    fn write(self) -> Result<EngineEvent> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut writer = Mp4Writer::create(&self.path, &self.specs)?;
        let mut reader = PacketReader::default();
        for (_, stream, packet) in self.packets {
            let mut packet = reader.read(packet)?;
            let shift = self.shifts[stream];
            packet.set_pts(packet.pts - shift);
            packet.set_dts(packet.dts - shift);
            writer.write(stream, packet)?;
        }
        writer.finish()?;
        let latency = Hns::now() - self.requested;
        tracing::info!(
            path = %self.path.display(),
            duration_s = self.duration.as_secs_f64(),
            latency_ms = latency.0 / 10_000,
            "clip sauvegardé"
        );
        Ok(EngineEvent::ClipSaved {
            path: self.path.to_string_lossy().into_owned(),
            duration_secs: self.duration.as_secs_f64(),
            save_latency_ms: (latency.0 / 10_000) as u64,
        })
    }
}

fn drain_until_closed(receivers: &[Receiver<AVPacket>]) {
    let mut select = Select::new();
    for receiver in receivers {
        select.recv(receiver);
    }
    let mut open = receivers.len();
    while open > 0 {
        let operation = select.select();
        let index = operation.index();
        if operation.recv(&receivers[index]).is_err() {
            select.remove(index);
            open -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppTrack;

    #[test]
    fn tracks_follow_the_mix_mic_apps_game_order() {
        let mut config = EngineConfig::default();
        config.audio.app_tracks = vec![
            AppTrack {
                exe_name: "Discord.exe".into(),
                label: "Discord".into(),
                enabled: true,
                sync_offset_ms: 0,
            },
            AppTrack {
                exe_name: "chrome.exe".into(),
                label: "Navigateur".into(),
                enabled: false,
                sync_offset_ms: 0,
            },
        ];
        config.audio.full_mix_sync_offset_ms = -120;
        let titles: Vec<(String, i32)> = audio_sources(&config)
            .into_iter()
            .map(|(_, title, offset)| (title, offset))
            .collect();
        assert_eq!(
            titles,
            [
                ("Mix complet".to_owned(), -120),
                ("Micro".to_owned(), 0),
                ("Discord".to_owned(), 0),
                ("Jeu".to_owned(), 0),
            ]
        );
    }

    #[test]
    fn only_capture_or_source_changes_restart_the_replay() {
        let old = EngineConfig::default();
        let mut longer = old.clone();
        longer.replay.duration_secs = 120;
        assert!(!needs_restart(&old, &longer));
        let mut other_fps = old.clone();
        other_fps.video.fps = 120;
        assert!(needs_restart(&old, &other_fps));
        let mut no_mic = old.clone();
        no_mic.audio.record_microphone = false;
        assert!(needs_restart(&old, &no_mic));
        let mut on_disk = old.clone();
        on_disk.replay.storage = ReplayStorage::Disk;
        assert!(needs_restart(&old, &on_disk));
    }
}
