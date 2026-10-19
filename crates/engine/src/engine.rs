use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Select, Sender, bounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::AVPacket;
use rsmpeg::avutil::AVRational;
use rsmpeg::ffi::AV_PKT_FLAG_KEY;

use crate::audio::{AudioSource, AudioTrack, MixBus, Mixdown, PeakMeter, SAMPLE_RATE, mix_lag_ms};
use crate::calibrate::{
    Calibration, EMIT_SECONDS, TrackMeasurement, calibration_config, emit_flashes, measure_tracks,
    proposals, read_signals,
};
use crate::capture::{VideoCapture, VideoStream, target_size};
use crate::clock::{HNS_PER_SECOND, Hns};
use crate::config::{AudioSettings, CaptureTarget, EngineConfig, ReplayStorage};
use crate::encode::{
    HardwareEncoder, Size, hardware_video_encoders, output_size, pick_encoder, try_output,
    write_thumbnail,
};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, EngineState, Localized, Severity, TrackDescriptor, TrackKind};
use crate::library::ensure_free_space;
use crate::mux::{Mp4Writer, MuxStream};
use crate::record::{
    Journal, LivePacket, RecordLayout, RecordTarget, Recorder, recover_interrupted,
};
use crate::replay::{
    ClipPlan, PacketReader, PacketRing, ReplayPacket, Spool, TimedPacket, buffer_window,
    estimate_buffer_bytes, forget_saved, plan_clip, share_packet,
};
use crate::sources::{CurrentGame, ForegroundWatch};
use crate::{games, paths};

const COMMAND_QUEUE: usize = 16;
const EVENT_QUEUE: usize = 256;
const STATS_INTERVAL: Hns = Hns::from_millis(1_000);
const LEVELS_FOCUSED: Hns = Hns::from_millis(50);
const LEVELS_BACKGROUND: Hns = Hns::from_millis(100);
pub(crate) const AAC_PACKET: Hns = Hns(1_024 * HNS_PER_SECOND / SAMPLE_RATE as i64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LevelRate {
    #[default]
    Off,
    Focused,
    Background,
}

impl LevelRate {
    fn interval(self) -> Option<Hns> {
        match self {
            Self::Off => None,
            Self::Focused => Some(LEVELS_FOCUSED),
            Self::Background => Some(LEVELS_BACKGROUND),
        }
    }
}

const CALIBRATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
const CALIBRATION_WARMUP: std::time::Duration = std::time::Duration::from_millis(800);

enum Command {
    Arm,
    Disarm,
    SaveReplay,
    StartRecording,
    StopRecording,
    RecordingEnded,
    Configure(Box<EngineConfig>),
    SetLevelRate(LevelRate),
    Calibrate(Sender<Result<Calibration>>),
    FinishCalibration(Result<()>),
    Shutdown,
}

struct PendingCalibration {
    reply: Sender<Result<Calibration>>,
    config: EngineConfig,
    was_armed: bool,
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
        let own_commands = commands.clone();
        if let Ok(journal) = Journal::open_default()
            && !journal.entries().is_empty()
        {
            let recovered = event_tx.clone();
            std::thread::Builder::new()
                .name("palaclips-recover".into())
                .spawn(move || recover_interrupted(&journal, &recovered))?;
        }
        let thread = std::thread::Builder::new()
            .name("palaclips-engine".into())
            .spawn(move || {
                Core {
                    config,
                    events: event_tx,
                    pipeline: None,
                    level_rate: LevelRate::Off,
                    commands: own_commands,
                    calibration: None,
                    recording: None,
                    restart_pending: false,
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

    pub fn start_recording(&self) -> Result<()> {
        self.send(Command::StartRecording)
    }

    pub fn stop_recording(&self) -> Result<()> {
        self.send(Command::StopRecording)
    }

    pub fn set_level_rate(&self, rate: LevelRate) -> Result<()> {
        self.send(Command::SetLevelRate(rate))
    }

    pub fn configure(&self, config: EngineConfig) -> Result<()> {
        config.validate()?;
        self.send(Command::Configure(Box::new(config)))
    }

    pub fn calibrate(&self) -> Result<Calibration> {
        let (reply, result) = bounded(1);
        self.send(Command::Calibrate(reply))?;
        result
            .recv_timeout(CALIBRATION_TIMEOUT)
            .map_err(|_| EngineError::Stopped)?
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
    if audio.record_full_mix && audio.desktop_in_full_mix {
        sources.push((
            AudioSource::SystemMix {
                device_id: audio.output_device_id.clone(),
            },
            audio.full_mix_title().to_owned(),
            audio.full_mix_sync_offset_ms,
        ));
    } else if audio.record_full_mix {
        sources.push((AudioSource::TracksMix, audio.full_mix_title().to_owned(), 0));
    }
    if audio.record_microphone {
        sources.push((
            AudioSource::Microphone {
                device_id: audio.microphone_device_id.clone(),
                gain_db: audio.microphone_gain_db,
            },
            audio.microphone_title().to_owned(),
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
        sources.push((
            AudioSource::Game(current_game(config)),
            audio.game_title().to_owned(),
            audio.game_sync_offset_ms,
        ));
    }
    sources
}

pub fn mixdowns(audio: &AudioSettings, sources: &[(AudioSource, String, i32)]) -> Vec<Mixdown> {
    let desktop = sources
        .iter()
        .any(|(source, ..)| matches!(source, AudioSource::SystemMix { .. }));
    let feeds = |source: &AudioSource| match source.kind() {
        TrackKind::Microphone => audio.microphone_in_full_mix,
        TrackKind::Application | TrackKind::Game => !desktop,
        TrackKind::Mix => false,
    };
    let fed_apps: Vec<String> = sources
        .iter()
        .filter_map(|(source, ..)| match source {
            AudioSource::Application { exe_name } if feeds(source) => Some(exe_name.clone()),
            _ => None,
        })
        .collect();
    let buses: Vec<Option<MixBus>> = sources
        .iter()
        .map(|(source, ..)| feeds(source).then(MixBus::default))
        .collect();
    let mix: Vec<MixBus> = buses.iter().flatten().cloned().collect();
    let lag_ms = mix_lag_ms(
        sources
            .iter()
            .zip(&buses)
            .filter(|(_, bus)| bus.is_some())
            .map(|((.., offset_ms), _)| *offset_ms),
    );
    sources
        .iter()
        .zip(buses)
        .map(|((source, ..), bus)| match (source.kind(), bus) {
            (TrackKind::Game, Some(bus)) => Mixdown::Feed {
                bus,
                duplicates: fed_apps.clone(),
            },
            (_, Some(bus)) => Mixdown::Feed {
                bus,
                duplicates: Vec::new(),
            },
            (TrackKind::Mix, None) if !mix.is_empty() => Mixdown::Mix {
                buses: mix.clone(),
                lag_ms,
            },
            _ => Mixdown::None,
        })
        .collect()
}

pub fn track_descriptors(config: &EngineConfig) -> Vec<TrackDescriptor> {
    audio_sources(config)
        .into_iter()
        .map(|(source, title, _)| TrackDescriptor {
            kind: source.kind(),
            title,
        })
        .collect()
}

fn capture_label(target: &CaptureTarget, fell_back: bool) -> Localized {
    match target {
        CaptureTarget::PrimaryMonitor => Localized::new("source_primary_screen"),
        CaptureTarget::Monitor { device_name } => {
            Localized::new("source_screen").with("name", device_name)
        }
        CaptureTarget::Window { exe_name, .. } if fell_back => {
            Localized::new("source_window_fallback").with("exe", exe_name)
        }
        CaptureTarget::Window { exe_name, .. } => {
            Localized::new("source_window").with("exe", exe_name)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ReplayEstimate {
    pub source_width: u32,
    pub source_height: u32,
    pub width: u32,
    pub height: u32,
    pub video_kbps: u32,
    pub audio_tracks: usize,
    pub bytes: u64,
}

pub fn estimate_replay(config: &EngineConfig) -> Result<ReplayEstimate> {
    let (width, height) = target_size(&config.capture)?;
    let size = output_size(Size::new(width, height), config.video.resolution);
    let video_kbps = config.video.bitrate_kbps_for(size.width, size.height);
    let audio_tracks = audio_sources(config).len();
    Ok(ReplayEstimate {
        source_width: width,
        source_height: height,
        width: size.width,
        height: size.height,
        video_kbps,
        audio_tracks,
        bytes: estimate_buffer_bytes(video_kbps, audio_tracks, ring_window(config)),
    })
}

pub fn try_video_output(config: &EngineConfig, encoders: &[HardwareEncoder]) -> Result<()> {
    let (width, height) = target_size(&config.capture)?;
    let size = output_size(Size::new(width, height), config.video.resolution);
    try_output(encoders, config.video.codec, size, config.video.fps)
}

fn current_game(config: &EngineConfig) -> CurrentGame {
    CurrentGame::new(config.audio.game_exe.clone(), &config.capture)
}

fn ring_window(config: &EngineConfig) -> Hns {
    buffer_window(
        config.replay.duration_secs,
        config.video.keyframe_interval_secs,
    )
}

fn needs_restart(old: &EngineConfig, new: &EngineConfig) -> bool {
    old.capture != new.capture
        || old.video != new.video
        || !old.audio.same_sources(&new.audio)
        || old.replay.storage != new.replay.storage
}

fn titled(specs: &[MuxStream], config: &EngineConfig) -> Vec<MuxStream> {
    let titles = track_descriptors(config);
    let mut specs = specs.to_vec();
    if titles.len() + 1 == specs.len() {
        for (spec, track) in specs.iter_mut().skip(1).zip(titles) {
            spec.title = Some(track.title);
        }
    }
    specs
}

struct Core {
    config: EngineConfig,
    events: Sender<EngineEvent>,
    pipeline: Option<Pipeline>,
    level_rate: LevelRate,
    commands: Sender<Command>,
    calibration: Option<PendingCalibration>,
    recording: Option<Recording>,
    restart_pending: bool,
}

struct Recording {
    replay_was_armed: bool,
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
                        self.alert(Severity::Error, Localized::new("replay_stopped").with("reason", error.localized()), format!(
                            "Le replay s'est arrêté : {error}. Réarme-le quand la source est de nouveau disponible."
                        ));
                        self.disarm();
                        continue;
                    }
                },
            };
            if self.calibration.is_some() {
                match command {
                    Command::FinishCalibration(emitted) => self.finish_calibration(emitted),
                    Command::Configure(config) => self.config = *config,
                    Command::Calibrate(reply) => {
                        reply.send(Err(EngineError::CalibrationBusy)).ok();
                    }
                    Command::Arm | Command::Disarm => {
                        if let Some(pending) = self.calibration.as_mut() {
                            pending.was_armed = matches!(command, Command::Arm);
                        }
                    }
                    Command::SaveReplay | Command::StartRecording => self.alert(
                        Severity::Info,
                        Localized::new("calibration_running"),
                        "Calibrage en cours : rien n'est sauvegardé.".into(),
                    ),
                    Command::StopRecording | Command::RecordingEnded => {}
                    Command::SetLevelRate(rate) => self.level_rate = rate,
                    Command::Shutdown => {
                        self.disarm();
                        break;
                    }
                }
                continue;
            }
            match command {
                Command::Arm => self.arm(),
                Command::Disarm => self.disarm(),
                Command::SaveReplay => self.save_replay(),
                Command::StartRecording => self.start_recording(),
                Command::StopRecording => self.stop_recording(),
                Command::RecordingEnded => self.recording_ended(),
                Command::Configure(config) => self.configure(*config),
                Command::Calibrate(reply) => self.start_calibration(reply),
                Command::FinishCalibration(_) => {}
                Command::SetLevelRate(rate) => {
                    self.level_rate = rate;
                    if let Some(pipeline) = self.pipeline.as_mut() {
                        pipeline.set_level_interval(rate.interval());
                    }
                }
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
            Ok(mut pipeline) => {
                pipeline.set_level_interval(self.level_rate.interval());
                self.pipeline = Some(pipeline);
                self.emit(EngineEvent::StateChanged {
                    state: EngineState::ReplayArmed,
                });
            }
            Err(error) => {
                tracing::error!(%error, "armement du replay impossible");
                self.alert(
                    Severity::Error,
                    Localized::new("replay_arm_failed").with("reason", error.localized()),
                    format!("Impossible d'armer le replay : {error}."),
                );
            }
        }
    }

    fn start_recording(&mut self) {
        if self.recording.is_some() {
            return;
        }
        let replay_was_armed = self.pipeline.is_some();
        self.arm();
        let exe_name = self.pipeline.as_ref().and_then(Pipeline::game_exe);
        let game = self.game_name(exe_name.as_deref());
        let root = match &self.config.storage.clips_dir {
            Some(dir) => Ok(dir.clone()),
            None => paths::default_clips_dir(),
        }
        .and_then(|root| ensure_free_space(&root, 0).map(|_| root));
        let duration = (self.config.record.include_replay && replay_was_armed)
            .then(|| Hns(i64::from(self.config.replay.duration_secs) * HNS_PER_SECOND));
        let result = match (self.pipeline.as_mut(), root) {
            (None, _) => return,
            (Some(_), Err(error)) => Err(error),
            (Some(pipeline), Ok(root)) => pipeline.request_recording(
                RecordTarget {
                    root,
                    game,
                    split: self.config.record.split,
                },
                &self.config,
                duration,
                &self.events,
            ),
        };
        if let Err(error) = result {
            tracing::error!(%error, "enregistrement impossible");
            self.alert(
                Severity::Error,
                Localized::new("recording_failed").with("reason", error.localized()),
                format!("Enregistrement arrêté : {error}."),
            );
            if !replay_was_armed {
                self.disarm();
            }
            return;
        }
        self.recording = Some(Recording { replay_was_armed });
        self.emit(EngineEvent::StateChanged {
            state: EngineState::Recording,
        });
    }

    fn stop_recording(&mut self) {
        if let Some(pipeline) = self.pipeline.as_mut() {
            pipeline.stop_recording();
        }
        self.recording_ended();
    }

    fn recording_ended(&mut self) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        if !recording.replay_was_armed {
            self.disarm();
            return;
        }
        if std::mem::take(&mut self.restart_pending) {
            self.disarm();
            self.arm();
            return;
        }
        self.emit(EngineEvent::StateChanged {
            state: EngineState::ReplayArmed,
        });
    }

    fn start_calibration(&mut self, reply: Sender<Result<Calibration>>) {
        if self.recording.is_some() {
            reply.send(Err(EngineError::RecordingRunning)).ok();
            return;
        }
        let own_exe = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "palaclips-app.exe".into());
        let config = calibration_config(&self.config, &own_exe);
        let was_armed = self.pipeline.is_some();
        self.disarm();
        match Pipeline::start(&config, &self.events) {
            Ok(pipeline) => self.pipeline = Some(pipeline),
            Err(error) => {
                if was_armed {
                    self.arm();
                }
                reply.send(Err(error)).ok();
                return;
            }
        }
        let commands = self.commands.clone();
        let device = config.audio.output_device_id.clone();
        let spawned = std::thread::Builder::new()
            .name("palaclips-calibration".into())
            .spawn(move || {
                std::thread::sleep(CALIBRATION_WARMUP);
                let emitted = emit_flashes(EMIT_SECONDS, device.as_deref());
                std::thread::sleep(CALIBRATION_WARMUP);
                commands.send(Command::FinishCalibration(emitted)).ok();
            });
        if let Err(error) = spawned {
            self.disarm();
            if was_armed {
                self.arm();
            }
            reply.send(Err(error.into())).ok();
            return;
        }
        self.calibration = Some(PendingCalibration {
            reply,
            config,
            was_armed,
        });
    }

    fn finish_calibration(&mut self, emitted: Result<()>) {
        let Some(pending) = self.calibration.take() else {
            return;
        };
        let result = emitted.and_then(|()| self.measure_calibration(&pending.config));
        if let Some(pipeline) = self.pipeline.take() {
            pipeline.stop();
        }
        if pending.was_armed {
            self.arm();
        }
        if let Ok(calibration) = &result {
            tracing::info!(?calibration, "calibrage terminé");
        }
        pending.reply.send(result).ok();
    }

    fn measure_calibration(&mut self, config: &EngineConfig) -> Result<Calibration> {
        let pipeline = self
            .pipeline
            .as_mut()
            .ok_or(EngineError::CalibrationStopped)?;
        let path = paths::calibration_clip_path()?;
        let job = pipeline.clip_job(Hns::now(), path.clone(), config)?;
        let signals = job.write().and_then(|_| read_signals(&path));
        std::fs::remove_file(&path).ok();
        let tracks = track_descriptors(config)
            .into_iter()
            .zip(measure_tracks(&signals?))
            .map(|(track, measurement)| TrackMeasurement {
                kind: track.kind,
                title: track.title,
                measurement,
            })
            .collect();
        Ok(proposals(&self.config.audio, tracks))
    }

    fn disarm(&mut self) {
        self.recording = None;
        self.restart_pending = false;
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
        let exe_name = self.pipeline.as_ref().and_then(Pipeline::game_exe);
        let game = self.game_name(exe_name.as_deref());
        let result = match self.pipeline.as_mut() {
            Some(pipeline) => pipeline.save(requested, &game, &self.config, &self.events),
            None => Err(EngineError::ReplayNotArmed),
        };
        match result {
            Ok(()) => {}
            Err(EngineError::ClipTooShort) => {
                tracing::info!("sauvegarde ignorée : moins de 3 s depuis le dernier clip");
                self.alert(
                    Severity::Info,
                    EngineError::ClipTooShort.localized(),
                    "Rien de nouveau à sauvegarder : moins de 3 s depuis le dernier clip.".into(),
                );
            }
            Err(error) => {
                tracing::warn!(%error, "sauvegarde du replay impossible");
                self.alert(
                    Severity::Warning,
                    Localized::new("clip_save_failed").with("reason", error.localized()),
                    format!("Clip non sauvegardé : {error}."),
                );
            }
        }
    }

    fn game_name(&mut self, exe_name: Option<&str>) -> String {
        let Some(exe_name) = exe_name else {
            return paths::game_label(None);
        };
        let key = exe_name.to_lowercase();
        if let Some(name) = self.config.storage.game_names.get(&key) {
            return name.clone();
        }
        let resolved = games::resolve(exe_name);
        if resolved.from_metadata_lookup {
            tracing::info!(exe = %exe_name, name = %resolved.name, "nom de jeu choisi");
            self.config
                .storage
                .game_names
                .insert(key.clone(), resolved.name.clone());
            self.emit(EngineEvent::GameNamed {
                exe_name: key,
                name: resolved.name.clone(),
            });
        }
        resolved.name
    }

    fn configure(&mut self, config: EngineConfig) {
        let restart = self.pipeline.is_some() && needs_restart(&self.config, &config);
        let renamed = track_descriptors(&self.config) != track_descriptors(&config);
        if let Some(pipeline) = self.pipeline.as_mut() {
            pipeline.set_window(ring_window(&config));
        }
        self.config = config;
        if restart && self.recording.is_some() {
            tracing::info!("sources changées pendant l'enregistrement : redémarrage à la fin");
            self.restart_pending = true;
            return;
        }
        if renamed && !restart && self.pipeline.is_some() {
            self.emit(EngineEvent::TracksRenamed {
                tracks: track_descriptors(&self.config),
            });
        }
        if restart {
            tracing::info!("sources ou zone de capture changées : redémarrage du replay");
            self.disarm();
            self.arm();
        }
    }

    fn alert(&self, severity: Severity, text: Localized, message: String) {
        self.emit(EngineEvent::Alert {
            severity,
            text,
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
    meters: Vec<PeakMeter>,
    level_interval: Option<Hns>,
    next_levels: Hns,
    rings: Vec<PacketRing<ReplayPacket>>,
    spool: Option<Spool>,
    game: Arc<Mutex<CurrentGame>>,
    _foreground: Option<ForegroundWatch>,
    next_stats: Hns,
    recorder: Option<Recorder>,
    pending_record: Option<PendingRecord>,
}

struct PendingRecord {
    target: RecordTarget,
    layout: RecordLayout,
    events: Sender<EngineEvent>,
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
        let sources = audio_sources(config);
        let plan = mixdowns(&config.audio, &sources);
        let tracks = sources
            .into_iter()
            .zip(plan)
            .enumerate()
            .map(|(index, ((source, title, offset_ms), mixdown))| {
                AudioTrack::start(
                    index,
                    source,
                    title,
                    video.origin,
                    Hns::from_millis(offset_ms.into()),
                    mixdown,
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
        events
            .try_send(EngineEvent::CaptureStarted {
                source: capture_label(&config.capture, capture.alert().is_some()),
                width: video.size.width,
                height: video.size.height,
                fps: video.fps,
                tracks: track_descriptors(config),
            })
            .ok();
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
            alerts: std::iter::once(capture.alerts().clone())
                .chain(tracks.iter().map(|track| track.alerts().clone()))
                .collect(),
            meters: tracks.iter().map(|track| track.meter().clone()).collect(),
            level_interval: None,
            next_levels: Hns::ZERO,
            capture,
            tracks,
            video,
            specs,
            receivers,
            spool,
            game,
            _foreground: foreground,
            next_stats: Hns::now() + STATS_INTERVAL,
            recorder: None,
            pending_record: None,
        })
    }

    fn request_recording(
        &mut self,
        target: RecordTarget,
        config: &EngineConfig,
        replay: Option<Hns>,
        events: &Sender<EngineEvent>,
    ) -> Result<()> {
        let pending = PendingRecord {
            target,
            layout: RecordLayout {
                specs: titled(&self.specs, config),
                time_bases: self.time_bases.clone(),
                frame: Hns(HNS_PER_SECOND / i64::from(self.video.fps)),
                audio_packet: AAC_PACKET,
            },
            events: events.clone(),
        };
        match self.recording_start(replay) {
            Some(video_from) => self.begin_recording(pending, video_from),
            None => {
                self.pending_record = Some(pending);
                Ok(())
            }
        }
    }

    fn recording_start(&self, replay: Option<Hns>) -> Option<usize> {
        let (video, audio) = self.rings.split_first()?;
        match replay {
            Some(duration) => {
                let frame = Hns(HNS_PER_SECOND / i64::from(self.video.fps));
                plan_clip(video, audio, duration, frame, AAC_PACKET).map(|plan| plan.video_from)
            }
            None => video.start_index(video.newest()?),
        }
    }

    fn begin_recording(&mut self, pending: PendingRecord, video_from: usize) -> Result<()> {
        if let Some(spool) = self.spool.as_mut() {
            spool.flush()?;
        }
        let (video, audio) = self.rings.split_first().ok_or(EngineError::NoStream)?;
        let start = video.get(video_from).ok_or(EngineError::NoStream)?.pts();
        let mut backlog = Vec::new();
        for packet in video.iter_from(video_from) {
            backlog.push((packet.pts(), 0, packet.share()?));
        }
        for (index, ring) in audio.iter().enumerate() {
            for packet in ring.iter_from(ring.index_at_or_before(start)) {
                backlog.push((packet.pts(), index + 1, packet.share()?));
            }
        }
        backlog.sort_by_key(|(pts, stream, _)| (*pts, *stream));
        self.recorder = Some(Recorder::start(
            pending.target,
            pending.layout,
            start,
            backlog,
            pending.events,
        )?);
        Ok(())
    }

    fn stop_recording(&mut self) {
        self.pending_record = None;
        if let Some(recorder) = self.recorder.take() {
            let stopped = std::thread::Builder::new()
                .name("palaclips-record-stop".into())
                .spawn(move || recorder.stop());
            if let Err(error) = stopped {
                tracing::warn!(%error, "fin de l'enregistrement sur le fil du moteur");
            }
        }
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
            if self
                .recorder
                .as_ref()
                .is_some_and(|recorder| !recorder.is_running())
            {
                if let Some(recorder) = self.recorder.take() {
                    recorder.stop();
                }
                return Ok(Some(Command::RecordingEnded));
            }
            let now = Hns::now();
            self.emit_due(now, events);
            let wait = (self.next_deadline() - now).0.max(0) as u64;
            let Ok(operation) = select.select_timeout(Duration::from_nanos(wait * 100)) else {
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
                            EngineError::VideoCaptureStopped
                        } else {
                            EngineError::TrackStopped(
                                self.specs[stream].title.clone().unwrap_or_default(),
                            )
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
        let live = match &self.recorder {
            Some(_) => Some(share_packet(&packet)?),
            None => None,
        };
        let stored = match self.spool.as_mut() {
            Some(spool) => spool.store(packet, pts, keyframe)?,
            None => ReplayPacket::in_ram(packet, pts, keyframe),
        };
        self.rings[stream].push(stored);
        if let (Some(recorder), Some(packet)) = (&self.recorder, live) {
            recorder.send(LivePacket {
                stream,
                pts,
                keyframe,
                packet,
            });
        } else if stream == 0
            && keyframe
            && let Some(pending) = self.pending_record.take()
        {
            let video_from = self.rings[0].len() - 1;
            self.begin_recording(pending, video_from)?;
        }
        Ok(())
    }

    fn emit_due(&mut self, now: Hns, events: &Sender<EngineEvent>) {
        if now >= self.next_stats {
            self.next_stats = now + STATS_INTERVAL;
            events.try_send(self.stats()).ok();
            if let Some(recorder) = &self.recorder {
                events.try_send(recorder.stats()).ok();
            }
        }
        if let Some(interval) = self.level_interval
            && now >= self.next_levels
        {
            self.next_levels = now + interval;
            events.try_send(self.levels()).ok();
        }
    }

    fn next_deadline(&self) -> Hns {
        match self.level_interval {
            Some(_) => self.next_stats.min(self.next_levels),
            None => self.next_stats,
        }
    }

    fn set_level_interval(&mut self, interval: Option<Hns>) {
        if self.level_interval.is_none() {
            self.next_levels = Hns::now();
        }
        self.level_interval = interval;
    }

    fn levels(&self) -> EngineEvent {
        EngineEvent::AudioLevels {
            levels: self.meters.iter().map(PeakMeter::take).collect(),
        }
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

    fn game_exe(&self) -> Option<String> {
        self.game.lock().exe_name().map(str::to_owned)
    }

    fn save(
        &mut self,
        requested: Hns,
        game: &str,
        config: &EngineConfig,
        events: &Sender<EngineEvent>,
    ) -> Result<()> {
        let root = match &config.storage.clips_dir {
            Some(dir) => dir.clone(),
            None => paths::default_clips_dir()?,
        };
        let buffered: u64 = self.rings.iter().map(|ring| ring.bytes() as u64).sum();
        ensure_free_space(&root, buffered)?;
        let path = paths::replay_clip_path(&root, game, &paths::local_timestamp());
        let job = self.clip_job(requested, path, config)?;
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
                            text: Localized::new("clip_save_failed")
                                .with("reason", error.localized()),
                            message: format!("Clip non sauvegardé : {error}."),
                        }
                    }
                };
                events.try_send(event).ok();
            })?;
        Ok(())
    }

    fn clip_job(
        &mut self,
        requested: Hns,
        path: std::path::PathBuf,
        config: &EngineConfig,
    ) -> Result<SaveJob> {
        if let Some(spool) = self.spool.as_mut() {
            spool.flush()?;
        }
        let duration = Hns(i64::from(config.replay.duration_secs) * HNS_PER_SECOND);
        let frame = Hns(HNS_PER_SECOND / i64::from(self.video.fps));
        let (video, audio) = self.rings.split_first().ok_or(EngineError::NoStream)?;
        let plan = plan_clip(video, audio, duration, frame, AAC_PACKET)
            .filter(ClipPlan::worth_saving)
            .ok_or(EngineError::ClipTooShort)?;
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
        if let Some((video, audio)) = self.rings.split_first_mut() {
            forget_saved(video, audio, plan.end);
        }
        let shifts: Vec<i64> = self
            .time_bases
            .iter()
            .map(|base| {
                plan.start
                    .to_timebase(i64::from(base.num), i64::from(base.den))
            })
            .collect();
        Ok(SaveJob {
            path,
            specs: titled(&self.specs, config),
            shifts,
            packets,
            duration: plan.end - plan.start,
            requested,
        })
    }

    fn stop(self) {
        let Self {
            capture,
            tracks,
            receivers,
            recorder,
            ..
        } = self;
        if let Some(recorder) = recorder {
            recorder.stop();
        }
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
        let mut last_keyframe = None;
        for (_, stream, packet) in self.packets {
            let keyframe = stream == 0 && packet.is_keyframe();
            let mut packet = reader.read(packet)?;
            if keyframe {
                last_keyframe = Some(share_packet(&packet)?);
            }
            let shift = self.shifts[stream];
            packet.set_pts(packet.pts - shift);
            packet.set_dts(packet.dts - shift);
            writer.write(stream, packet)?;
        }
        writer.finish()?;
        let latency = Hns::now() - self.requested;
        let thumbnail = last_keyframe.and_then(|keyframe| {
            let path = paths::thumbnail_path(&self.path).ok()?;
            match write_thumbnail(&self.specs[0].codec_parameters, &keyframe, &path) {
                Ok(()) => Some(path.to_string_lossy().into_owned()),
                Err(error) => {
                    tracing::warn!(%error, "miniature du clip impossible");
                    None
                }
            }
        });
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
            thumbnail,
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
        config.audio.desktop_in_full_mix = true;
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

    fn plan(config: &EngineConfig) -> Vec<&'static str> {
        mixdowns(&config.audio, &audio_sources(config))
            .iter()
            .map(|mixdown| match mixdown {
                Mixdown::None => "seule",
                Mixdown::Feed { .. } => "dans la piste 1",
                Mixdown::Mix { buses, .. } if buses.len() == 2 => "somme de 2",
                Mixdown::Mix { .. } => "somme",
            })
            .collect()
    }

    #[test]
    fn the_full_mix_waits_for_the_latest_track_it_adds() {
        let mut config = EngineConfig::default();
        config.audio.microphone_sync_offset_ms = -450;
        config.audio.game_sync_offset_ms = 80;
        let plan = mixdowns(&config.audio, &audio_sources(&config));
        assert!(matches!(plan[0], Mixdown::Mix { lag_ms: 600, .. }));
        config.audio.microphone_in_full_mix = false;
        let plan = mixdowns(&config.audio, &audio_sources(&config));
        assert!(matches!(plan[0], Mixdown::Mix { lag_ms: 200, .. }));
    }

    #[test]
    fn a_game_also_chosen_as_an_app_track_is_added_once() {
        let mut config = EngineConfig::default();
        config.audio.game_exe = Some("FortniteClient-Win64-Shipping.exe".into());
        config.audio.app_tracks = vec![AppTrack {
            exe_name: "fortniteclient-win64-shipping.exe".into(),
            label: "Fortnite".into(),
            enabled: true,
            sync_offset_ms: 0,
        }];
        let sources = audio_sources(&config);
        let plan = mixdowns(&config.audio, &sources);
        let Mixdown::Feed { duplicates, .. } = &plan[3] else {
            panic!("la piste Jeu doit alimenter la piste 1");
        };
        assert!(sources[3].0.is_one_of(duplicates));
        let Mixdown::Feed { duplicates, .. } = &plan[2] else {
            panic!("la piste d'appli doit alimenter la piste 1");
        };
        assert!(!sources[2].0.is_one_of(duplicates));
        config.audio.game_exe = Some("Discord.exe".into());
        let sources = audio_sources(&config);
        let Mixdown::Feed { duplicates, .. } = &mixdowns(&config.audio, &sources)[3] else {
            panic!("la piste Jeu doit alimenter la piste 1");
        };
        assert!(!sources[3].0.is_one_of(duplicates));
    }

    #[test]
    fn the_full_mix_adds_up_the_chosen_tracks_unless_it_takes_the_desktop() {
        let mut config = EngineConfig::default();
        assert!(matches!(
            audio_sources(&config)[0].0,
            AudioSource::TracksMix
        ));
        assert_eq!(
            plan(&config),
            ["somme de 2", "dans la piste 1", "dans la piste 1"]
        );
        config.audio.desktop_in_full_mix = true;
        assert!(matches!(
            audio_sources(&config)[0].0,
            AudioSource::SystemMix { .. }
        ));
        assert_eq!(plan(&config), ["somme", "dans la piste 1", "seule"]);
        config.audio.microphone_in_full_mix = false;
        assert_eq!(plan(&config), ["seule", "seule", "seule"]);
    }

    #[test]
    fn a_translated_default_name_renames_the_track_without_restarting() {
        let french = EngineConfig::default();
        let mut english = french.clone();
        english.audio.default_names.mix = "Full mix".into();
        assert!(!needs_restart(&french, &english));
        assert_eq!(track_descriptors(&english)[0].title, "Full mix");
        let mut renamed = french.clone();
        renamed.audio.game_label = Some("Partie".into());
        assert!(!needs_restart(&french, &renamed));
    }

    #[test]
    fn renamed_tracks_keep_their_order_and_offsets() {
        let mut config = EngineConfig::default();
        config.audio.full_mix_label = Some("Tout".into());
        config.audio.microphone_label = Some("Voix".into());
        config.audio.game_label = Some("Partie".into());
        config.audio.game_sync_offset_ms = 40;
        let titles: Vec<(String, i32)> = audio_sources(&config)
            .into_iter()
            .map(|(_, title, offset)| (title, offset))
            .collect();
        assert_eq!(
            titles,
            [
                ("Tout".to_owned(), 0),
                ("Voix".to_owned(), 0),
                ("Partie".to_owned(), 40),
            ]
        );
    }

    #[test]
    fn capture_label_names_the_window_or_its_fallback_screen() {
        let window = CaptureTarget::Window {
            exe_name: "FortniteClient-Win64-Shipping.exe".into(),
            title_hint: None,
        };
        let direct = capture_label(&window, false);
        assert_eq!(direct.code, "source_window");
        assert_eq!(direct.params["exe"], "FortniteClient-Win64-Shipping.exe");
        assert_eq!(capture_label(&window, true).code, "source_window_fallback");
        assert_eq!(
            capture_label(&CaptureTarget::PrimaryMonitor, false).code,
            "source_primary_screen"
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
