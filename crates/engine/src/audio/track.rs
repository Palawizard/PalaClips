use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, bounded};
use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avutil::AVRational;
use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, DeviceEventCallbacks, Direction, Handle,
    Role, SampleType, StreamMode, WasapiError, WaveFormat, initialize_mta,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::AUDCLNT_E_DEVICE_IN_USE;
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, PROCESS_SYNCHRONIZE, SetEvent, WaitForMultipleObjects,
};

use super::filler::{FillStats, GapFiller};
use super::mixdown::{MixBus, MixIn, Mixdown};
use super::{AudioSource, CHANNELS, PeakMeter, SAMPLE_RATE};
use crate::clock::Hns;
use crate::encode::AacEncoder;
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Localized, Severity, TrackState};
use crate::sources::{self, ForegroundWatch, ForegroundWindow};

const BUFFER_DURATION_HNS: i64 = 2_000_000;
const BYTES_PER_FRAME: usize = CHANNELS * size_of::<f32>();
const EXCLUSIVE_RETRY_MS: u32 = 1_000;
const END_GRACE_MS: i64 = 250;
const SILENCE_WAKE_MS: u32 = 50;
const SILENCE_LAG_MS: i64 = 100;
const APP_RETRY_MS: u32 = 1_000;
const FOREGROUND_QUEUE: usize = 16;
const SILENT_CHECK_MS: i64 = 1_000;
const SILENT_AFTER_MS: i64 = 3_000;
const SILENT_CHECKS_BEFORE_ALERT: u32 = 2;
const SESSION_SIGNAL_PEAK: f32 = 0.001;
const PACKET_QUEUE: usize = 256;
const ALERT_QUEUE: usize = 32;
const MIXDOWN_END_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct AudioStream {
    pub codec_parameters: AVCodecParameters,
    pub time_base: AVRational,
    pub title: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AudioTrackStats {
    pub packets: u64,
    pub fill: FillStats,
    pub peak: f32,
    pub device_switches: u64,
    pub silent_alerts: u64,
}

pub struct AudioTrack {
    stream: AudioStream,
    packets: Receiver<AVPacket>,
    alerts: Receiver<EngineEvent>,
    meter: PeakMeter,
    stop: Arc<Waitable>,
    end: Arc<AtomicI64>,
    thread: Option<JoinHandle<Result<AudioTrackStats>>>,
}

impl std::fmt::Debug for AudioTrack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioTrack")
            .field("title", &self.stream.title)
            .finish_non_exhaustive()
    }
}

impl AudioTrack {
    pub fn start(
        index: usize,
        source: AudioSource,
        title: impl Into<String>,
        origin: Hns,
        sync_offset: Hns,
        mixdown: Mixdown,
    ) -> Result<Self> {
        let encoder = AacEncoder::open(SAMPLE_RATE, CHANNELS)?;
        let title = title.into();
        let stream = AudioStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            title: title.clone(),
        };
        let (packet_tx, packets) = bounded(PACKET_QUEUE);
        let (alert_tx, alerts) = bounded(ALERT_QUEUE);
        let stop = Arc::new(Waitable::event()?);
        let end = Arc::new(AtomicI64::new(i64::MIN));
        let meter = PeakMeter::default();
        let worker = Worker {
            index,
            meter: meter.clone(),
            source,
            title,
            filler: GapFiller::new(origin),
            sync_offset,
            stats: AudioTrackStats::default(),
            filled: Vec::new(),
            mixing: match mixdown {
                Mixdown::None => Mixing::None,
                Mixdown::Feed(bus) => Mixing::Feed(bus),
                Mixdown::Mix(bus) => Mixing::Mix(MixIn::new(bus), Vec::new()),
            },
            encoder,
            packets: packet_tx,
            alerts: alert_tx,
            stop: stop.clone(),
            end: end.clone(),
            app: None,
            foreground: None,
            silence: SilenceWatch::default(),
            reported: None,
        };
        let thread = std::thread::Builder::new()
            .name(format!("palaclips-audio-{}", worker.title))
            .spawn(move || worker.run())?;
        Ok(Self {
            stream,
            packets,
            alerts,
            meter,
            stop,
            end,
            thread: Some(thread),
        })
    }

    pub fn stream(&self) -> &AudioStream {
        &self.stream
    }

    pub fn packets(&self) -> &Receiver<AVPacket> {
        &self.packets
    }

    pub fn alerts(&self) -> &Receiver<EngineEvent> {
        &self.alerts
    }

    pub fn meter(&self) -> &PeakMeter {
        &self.meter
    }

    pub fn finish_at(&self, until: Hns) {
        self.end.store(until.0, Ordering::Relaxed);
        self.stop.set();
    }

    pub fn stop(mut self, until: Hns) -> Result<AudioTrackStats> {
        self.shutdown(until)
    }

    fn shutdown(&mut self, until: Hns) -> Result<AudioTrackStats> {
        if self.end.load(Ordering::Relaxed) == i64::MIN {
            self.finish_at(until);
        }
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| EngineError::Audio("le fil audio a paniqué".into()))?,
            None => Ok(AudioTrackStats::default()),
        }
    }
}

impl Drop for AudioTrack {
    fn drop(&mut self) {
        if self.thread.is_some()
            && let Err(error) = self.shutdown(Hns::now())
        {
            tracing::warn!(%error, "arrêt de la piste audio");
        }
    }
}

struct Worker {
    index: usize,
    meter: PeakMeter,
    source: AudioSource,
    title: String,
    filler: GapFiller,
    sync_offset: Hns,
    stats: AudioTrackStats,
    filled: Vec<f32>,
    mixing: Mixing,
    encoder: AacEncoder,
    packets: Sender<AVPacket>,
    alerts: Sender<EngineEvent>,
    stop: Arc<Waitable>,
    end: Arc<AtomicI64>,
    app: Option<AttachedApp>,
    foreground: Option<ForegroundSignal>,
    silence: SilenceWatch,
    reported: Option<(TrackState, Option<String>)>,
}

enum Mixing {
    None,
    Feed(MixBus),
    Mix(MixIn, Vec<f32>),
}

#[derive(Clone)]
struct AttachedApp {
    exe_name: String,
    pid: u32,
}

struct ForegroundSignal {
    _watch: ForegroundWatch,
    changed: Arc<Waitable>,
    windows: Receiver<ForegroundWindow>,
}

#[derive(Debug, Default)]
struct SilenceWatch {
    last_signal: Hns,
    next_check: Hns,
    signal_checks: u32,
    reported: bool,
}

impl SilenceWatch {
    fn new(now: Hns) -> Self {
        Self {
            last_signal: now,
            ..Self::default()
        }
    }

    fn on_signal(&mut self, now: Hns) {
        self.last_signal = now;
        self.signal_checks = 0;
        self.reported = false;
    }

    fn check_due(&mut self, now: Hns) -> bool {
        if now < self.next_check || now - self.last_signal < Hns::from_millis(SILENT_AFTER_MS) {
            return false;
        }
        self.next_check = now + Hns::from_millis(SILENT_CHECK_MS);
        true
    }

    fn should_alert(&mut self, session_peak: f32) -> bool {
        if session_peak <= SESSION_SIGNAL_PEAK {
            return false;
        }
        self.signal_checks += 1;
        if self.signal_checks < SILENT_CHECKS_BEFORE_ALERT || self.reported {
            return false;
        }
        self.reported = true;
        true
    }
}

enum Wake {
    Stop,
    Reopen,
}

impl Worker {
    fn run(mut self) -> Result<AudioTrackStats> {
        initialize_mta()
            .ok()
            .map_err(|error| EngineError::Audio(error.message()))?;
        let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
        let default_changed = Arc::new(Waitable::event()?);
        let devices_changed = Arc::new(Waitable::event()?);
        let direction = self.direction();
        let follows_default = !self.source.is_application() && self.device_id().is_none();
        let mut callbacks = DeviceEventCallbacks::new();
        callbacks.set_default_device_callback({
            let default_changed = default_changed.clone();
            move |changed, role, _| {
                if changed == direction && role == Role::Console {
                    default_changed.set();
                }
            }
        });
        callbacks.set_device_state_callback({
            let devices_changed = devices_changed.clone();
            move |_, _| devices_changed.set()
        });
        callbacks.set_device_added_callback({
            let devices_changed = devices_changed.clone();
            move |_| devices_changed.set()
        });
        let _registration = enumerator
            .register_notification_callback(callbacks)
            .map_err(audio_error)?;

        let mut buffer = Vec::new();
        let mut samples = Vec::new();
        let mut current_device: Option<String> = None;
        let mut unavailable_reported = false;
        self.foreground = self.watch_foreground()?;
        let mut waits_for_device = vec![
            self.stop.handle(),
            default_changed.handle(),
            devices_changed.handle(),
        ];
        if let Some(foreground) = &self.foreground {
            waits_for_device.push(foreground.changed.handle());
        }

        loop {
            let opened = if self.source.is_application() {
                match self.attach() {
                    Some(app) => open_app_stream(&app),
                    None => {
                        self.report(TrackState::Waiting, None);
                        if self.idle(&waits_for_device, Some(APP_RETRY_MS))? {
                            break;
                        }
                        continue;
                    }
                }
            } else {
                open_stream(&enumerator, &self.source)
            };
            let stream = match opened {
                Ok(stream) => stream,
                Err(error) => {
                    let in_use = is_device_in_use(&error);
                    if !unavailable_reported {
                        unavailable_reported = true;
                        self.alert_unavailable(&error, in_use);
                        self.report(TrackState::Unavailable, None);
                    }
                    if self.idle(&waits_for_device, in_use.then_some(EXCLUSIVE_RETRY_MS))? {
                        break;
                    }
                    continue;
                }
            };
            unavailable_reported = false;
            self.report(TrackState::Capturing, Some(stream.name.clone()));
            if self.source.is_application() {
                tracing::info!(track = %self.title, app = %stream.name, "application rattachée");
                if current_device.is_some() {
                    self.stats.device_switches += 1;
                }
                current_device = Some(stream.name.clone());
                self.silence = SilenceWatch::new(Hns::now());
            } else if current_device.as_ref() != Some(&stream.name) {
                tracing::info!(track = %self.title, device = %stream.name, "périphérique audio ouvert");
                if current_device.is_some() {
                    self.stats.device_switches += 1;
                    self.alert(
                        Severity::Info,
                        Localized::new("audio_device_changed")
                            .with("track", &self.title)
                            .with("device", &stream.name),
                        format!(
                            "Piste « {} » : la capture suit maintenant « {} ».",
                            self.title, stream.name
                        ),
                    );
                }
                current_device = Some(stream.name.clone());
            }

            let mut handles = vec![self.stop.handle(), stream.event.as_handle()];
            if let Some(process) = &stream.process {
                handles.push(process.handle());
            } else if follows_default {
                handles.push(default_changed.handle());
            }
            let foreground_index = self.foreground.as_ref().map(|foreground| {
                handles.push(foreground.changed.handle());
                handles.len() - 1
            });
            let outcome = loop {
                match wait(&handles, SILENCE_WAKE_MS) {
                    Waited::Handle(0) => break Ok(Wake::Stop),
                    Waited::Handle(1) => {
                        if let Err(error) = stream.drain(&mut buffer, &mut samples, |qpc, data| {
                            self.on_packet(qpc, data)
                        }) {
                            break Err(error);
                        }
                    }
                    Waited::Handle(index) if Some(index) == foreground_index => {
                        if self.update_game() {
                            break Ok(Wake::Reopen);
                        }
                    }
                    Waited::Handle(_) => break Ok(Wake::Reopen),
                    Waited::Timeout => {
                        if let Err(error) = self.fill_silence() {
                            break Err(error);
                        }
                    }
                    Waited::Failed => {
                        break Err(EngineError::Audio("attente WASAPI impossible".into()));
                    }
                }
                if self.source.is_application() {
                    self.check_app_silence();
                }
            };
            match outcome {
                Ok(Wake::Stop) => {
                    self.read_until_end(&stream, &mut buffer, &mut samples)?;
                    break;
                }
                Ok(Wake::Reopen) => {}
                Err(error) => {
                    drop(stream);
                    tracing::warn!(track = %self.title, %error, "flux audio interrompu");
                    if self.idle(&waits_for_device, Some(EXCLUSIVE_RETRY_MS))? {
                        break;
                    }
                }
            }
        }

        let end = Hns(self.end.load(Ordering::Relaxed));
        self.filler.set_end(end);
        self.filler.fill_until(end, &mut self.filled);
        self.encode_filled(true)?;
        if let Mixing::Feed(bus) = &self.mixing {
            bus.close();
        }
        let packets = &self.packets;
        self.encoder.finish(&mut |packet| send(packets, packet))?;
        let mut stats = self.stats;
        stats.fill = self.filler.stats();
        tracing::info!(
            track = %self.title,
            packets = stats.packets,
            silence_ms = stats.fill.silence_frames * 1_000 / u64::from(SAMPLE_RATE),
            dropped_ms = stats.fill.dropped_frames * 1_000 / u64::from(SAMPLE_RATE),
            drift_corrections = stats.fill.drift_corrections,
            device_switches = stats.device_switches,
            peak = stats.peak,
            "piste audio arrêtée"
        );
        Ok(stats)
    }

    fn read_until_end(
        &mut self,
        stream: &CaptureStream,
        buffer: &mut Vec<u8>,
        samples: &mut Vec<f32>,
    ) -> Result<()> {
        let end = Hns(self.end.load(Ordering::Relaxed));
        self.filler.set_end(end);
        let deadline = end.max(Hns::now())
            + Hns::from_millis(END_GRACE_MS)
            + Hns((-self.sync_offset.0).max(0));
        let handles = [stream.event.as_handle()];
        while !self.filler.reached_end() {
            let remaining_ms = (deadline - Hns::now()).0 / 10_000;
            if remaining_ms <= 0 {
                break;
            }
            match wait(&handles, (remaining_ms as u32).min(SILENCE_WAKE_MS)) {
                Waited::Handle(_) => {
                    stream.drain(buffer, samples, |qpc, data| self.on_packet(qpc, data))?
                }
                Waited::Timeout => self.fill_silence()?,
                Waited::Failed => break,
            }
        }
        Ok(())
    }

    fn idle(&mut self, handles: &[HANDLE], timeout_ms: Option<u32>) -> Result<bool> {
        let deadline = timeout_ms.map(|ms| Hns::now() + Hns::from_millis(ms.into()));
        loop {
            let remaining_ms = deadline.map_or(SILENCE_WAKE_MS, |deadline| {
                ((deadline - Hns::now()).0 / 10_000).clamp(0, SILENCE_WAKE_MS.into()) as u32
            });
            match wait(handles, remaining_ms) {
                Waited::Handle(index) => return Ok(index == 0),
                Waited::Failed => return Ok(false),
                Waited::Timeout => self.fill_silence()?,
            }
            if deadline.is_some_and(|deadline| Hns::now() >= deadline) {
                return Ok(false);
            }
        }
    }

    fn fill_silence(&mut self) -> Result<()> {
        self.filler.fill_until(
            silence_horizon(Hns::now(), self.sync_offset),
            &mut self.filled,
        );
        self.flush()
    }

    fn attach(&mut self) -> Option<AttachedApp> {
        self.update_game();
        let exe_name = match &self.source {
            AudioSource::Application { exe_name } => exe_name.clone(),
            AudioSource::Game(game) => game.exe_name()?.to_owned(),
            _ => return None,
        };
        let pid = sources::root_pid(&sources::processes().ok()?, &exe_name)?;
        self.app = Some(AttachedApp { exe_name, pid });
        self.app.clone()
    }

    fn watch_foreground(&self) -> Result<Option<ForegroundSignal>> {
        let AudioSource::Game(game) = &self.source else {
            return Ok(None);
        };
        if !game.follows_foreground() {
            return Ok(None);
        }
        let changed = Arc::new(Waitable::event()?);
        let (sender, windows) = bounded(FOREGROUND_QUEUE);
        let watch = ForegroundWatch::start({
            let changed = changed.clone();
            move |window| {
                sender.try_send(window).ok();
                changed.set();
            }
        })?;
        Ok(Some(ForegroundSignal {
            _watch: watch,
            changed,
            windows,
        }))
    }

    fn update_game(&mut self) -> bool {
        let (Some(foreground), AudioSource::Game(game)) = (&self.foreground, &mut self.source)
        else {
            return false;
        };
        let mut changed = false;
        for window in foreground.windows.try_iter() {
            let from = game.exe_name().map(str::to_owned);
            if game.on_foreground(&window) {
                tracing::info!(track = %self.title, ?from, to = %window.exe_name, "la piste Jeu suit le nouveau jeu");
                changed = true;
            }
        }
        changed
    }

    fn check_app_silence(&mut self) {
        if self.silence.check_due(Hns::now()) {
            self.check_silent_track();
        }
    }

    fn check_silent_track(&mut self) {
        let Some(app) = &self.app else { return };
        let session_peak = sources::session_peak(app.pid).unwrap_or(0.0);
        if !self.silence.should_alert(session_peak) {
            return;
        }
        self.stats.silent_alerts += 1;
        tracing::warn!(track = %self.title, app = %app.exe_name, pid = app.pid, session_peak, "piste d'application muette alors que l'appli émet du son");
        let message = format!(
            "Piste « {} » : {} émet du son mais la piste reste muette. Vérifie que l'appli n'utilise pas une sortie en mode exclusif, puis relance-la.",
            self.title, app.exe_name
        );
        self.alert(
            Severity::Warning,
            Localized::new("app_track_silent")
                .with("track", &self.title)
                .with("app", &app.exe_name),
            message,
        );
        let source = self.reported.clone().and_then(|(_, source)| source);
        self.report(TrackState::Silent, source);
    }

    fn report(&mut self, state: TrackState, source: Option<String>) {
        let reported = Some((state, source.clone()));
        if self.reported == reported {
            return;
        }
        self.reported = reported;
        self.alerts
            .try_send(EngineEvent::TrackChanged {
                track: self.index,
                title: self.title.clone(),
                state,
                source,
            })
            .ok();
    }

    fn on_packet(&mut self, qpc: Hns, data: &mut [f32]) -> Result<()> {
        self.stats.packets += 1;
        if data.iter().any(|sample| *sample != 0.0) {
            self.silence.on_signal(Hns::now());
            if let Some((TrackState::Silent, source)) = self.reported.clone() {
                self.report(TrackState::Capturing, source);
            }
        }
        let gain = self.gain();
        let mut peak = 0.0f32;
        for sample in data.iter_mut() {
            *sample *= gain;
            peak = peak.max(sample.abs());
        }
        self.stats.peak = self.stats.peak.max(peak);
        self.meter.record(peak);
        self.filler
            .push(qpc + self.sync_offset, data, &mut self.filled);
        self.flush()
    }

    fn flush(&mut self) -> Result<()> {
        self.encode_filled(false)
    }

    fn encode_filled(&mut self, last: bool) -> Result<()> {
        let written = self.filler.written_frames();
        match &mut self.mixing {
            Mixing::None => {}
            Mixing::Feed(bus) => bus.push(
                written - (self.filled.len() / CHANNELS) as i64,
                &self.filled,
            ),
            Mixing::Mix(mix, ready) => {
                if last {
                    mix.take_all(&self.filled, written, ready, MIXDOWN_END_WAIT);
                } else {
                    mix.take_ready(&self.filled, written, ready);
                }
                std::mem::swap(&mut self.filled, ready);
                ready.clear();
            }
        }
        let packets = &self.packets;
        let result = self
            .encoder
            .encode(&self.filled, &mut |packet| send(packets, packet));
        self.filled.clear();
        result
    }

    fn direction(&self) -> Direction {
        match self.source {
            AudioSource::Microphone { .. } => Direction::Capture,
            _ => Direction::Render,
        }
    }

    fn device_id(&self) -> Option<&str> {
        match &self.source {
            AudioSource::SystemMix { device_id } | AudioSource::Microphone { device_id, .. } => {
                device_id.as_deref()
            }
            _ => None,
        }
    }

    fn gain(&self) -> f32 {
        match self.source {
            AudioSource::Microphone { gain_db, .. } => 10f32.powf(gain_db / 20.0),
            _ => 1.0,
        }
    }

    fn alert_unavailable(&self, error: &WasapiError, in_use: bool) {
        let reason = if in_use {
            Localized::new("device_in_exclusive_use")
        } else {
            Localized::new("no_audio_device").with("detail", error.to_string())
        };
        let message = if in_use {
            format!(
                "Piste « {} » : le périphérique est utilisé en mode exclusif par une autre application. PalaClips réessaie chaque seconde ; désactive le mode exclusif de cette application pour l'enregistrer.",
                self.title
            )
        } else {
            format!(
                "Piste « {} » : aucun périphérique audio disponible ({error}). La piste reste silencieuse jusqu'à son retour.",
                self.title
            )
        };
        tracing::warn!(track = %self.title, %error, "périphérique audio indisponible");
        self.alert(
            Severity::Warning,
            Localized::new("audio_device_unavailable")
                .with("track", &self.title)
                .with("reason", reason),
            message,
        );
    }

    fn alert(&self, severity: Severity, text: Localized, message: String) {
        self.alerts
            .try_send(EngineEvent::Alert {
                severity,
                text,
                message,
            })
            .ok();
    }
}

struct CaptureStream {
    client: AudioClient,
    capture: AudioCaptureClient,
    event: Handle,
    name: String,
    process: Option<Waitable>,
}

impl CaptureStream {
    fn drain(
        &self,
        buffer: &mut Vec<u8>,
        samples: &mut Vec<f32>,
        mut on_packet: impl FnMut(Hns, &mut [f32]) -> Result<()>,
    ) -> Result<()> {
        while let Some(frames) = self
            .capture
            .get_next_packet_size()
            .map_err(audio_error)?
            .filter(|frames| *frames > 0)
        {
            buffer.resize(frames as usize * BYTES_PER_FRAME, 0);
            let (read, info) = self.capture.read_from_device(buffer).map_err(audio_error)?;
            let bytes = &buffer[..read as usize * BYTES_PER_FRAME];
            samples.clear();
            if info.flags.silent {
                samples.resize(bytes.len() / size_of::<f32>(), 0.0);
            } else {
                samples.extend(
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| f32::from_le_bytes(*b)),
                );
            }
            on_packet(Hns(info.timestamp as i64), samples)?;
        }
        Ok(())
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        self.client.stop_stream().ok();
    }
}

fn open_stream(
    enumerator: &DeviceEnumerator,
    source: &AudioSource,
) -> std::result::Result<CaptureStream, WasapiError> {
    let (direction, device_id) = match source {
        AudioSource::Microphone { device_id, .. } => (Direction::Capture, device_id),
        AudioSource::SystemMix { device_id } => (Direction::Render, device_id),
        AudioSource::Application { .. } | AudioSource::Game(_) => (Direction::Render, &None),
    };
    let device = match device_id {
        Some(id) => enumerator.get_device(id)?,
        None => enumerator.get_default_device(&direction)?,
    };
    let name = device.get_friendlyname()?;
    start_capture(device.get_iaudioclient()?, name, None)
}

fn open_app_stream(app: &AttachedApp) -> std::result::Result<CaptureStream, WasapiError> {
    let process = Waitable::process(app.pid);
    let client = AudioClient::new_application_loopback_client(app.pid, true)?;
    start_capture(
        client,
        format!("{} (PID {})", app.exe_name, app.pid),
        process,
    )
}

fn start_capture(
    mut client: AudioClient,
    name: String,
    process: Option<Waitable>,
) -> std::result::Result<CaptureStream, WasapiError> {
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNELS,
        None,
    );
    client.initialize_client(
        &format,
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: BUFFER_DURATION_HNS,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let capture = client.get_audiocaptureclient()?;
    client.start_stream()?;
    Ok(CaptureStream {
        client,
        capture,
        event,
        name,
        process,
    })
}

fn is_device_in_use(error: &WasapiError) -> bool {
    matches!(error, WasapiError::Windows(error) if error.code() == AUDCLNT_E_DEVICE_IN_USE)
}

fn send(packets: &Sender<AVPacket>, packet: AVPacket) -> Result<()> {
    packets
        .send(packet)
        .map_err(|_| EngineError::Audio("plus personne ne lit les paquets audio".into()))
}

fn silence_horizon(now: Hns, sync_offset: Hns) -> Hns {
    now + sync_offset - Hns::from_millis(SILENCE_LAG_MS)
}

enum Waited {
    Handle(usize),
    Timeout,
    Failed,
}

fn wait(handles: &[HANDLE], timeout_ms: u32) -> Waited {
    // SAFETY: every handle is a live event owned by the caller for the duration of the wait.
    let result = unsafe { WaitForMultipleObjects(handles, false, timeout_ms) };
    let index = result.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
    if index < handles.len() {
        Waited::Handle(index)
    } else if result == WAIT_TIMEOUT {
        Waited::Timeout
    } else {
        Waited::Failed
    }
}

fn audio_error(error: WasapiError) -> EngineError {
    EngineError::Audio(error.to_string())
}

struct Waitable(HANDLE);

// SAFETY: Win32 event and process handles may be signaled and waited on from any thread.
unsafe impl Send for Waitable {}

// SAFETY: SetEvent and waits are thread-safe kernel operations on the same handle.
unsafe impl Sync for Waitable {}

impl Waitable {
    fn process(pid: u32) -> Option<Self> {
        // SAFETY: SYNCHRONIZE only allows waiting for the process to exit; the handle is closed on Drop.
        unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }
            .ok()
            .map(Self)
    }

    fn event() -> Result<Self> {
        // SAFETY: creates an unnamed auto-reset event with default security.
        unsafe { CreateEventW(None, false, false, None) }
            .map(Self)
            .map_err(|error| EngineError::Audio(error.message()))
    }

    fn set(&self) {
        // SAFETY: the handle stays valid until Drop.
        unsafe { SetEvent(self.0) }.ok();
    }

    fn handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Waitable {
    fn drop(&mut self) {
        // SAFETY: the handle was created by CreateEventW and is closed exactly once.
        unsafe { CloseHandle(self.0) }.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Hns = Hns(5_000_000_000);
    const PACKET_FRAMES: i64 = 480;

    fn push_packet(filler: &mut GapFiller, qpc: Hns, sync_offset: Hns, out: &mut Vec<f32>) {
        filler.push(
            qpc + sync_offset,
            &[0.5; PACKET_FRAMES as usize * CHANNELS],
            out,
        );
    }

    fn frame_at(qpc: Hns) -> i64 {
        (qpc - ORIGIN).to_samples(SAMPLE_RATE)
    }

    fn alerts_over(watch: &mut SilenceWatch, start: Hns, seconds: i64, session_peak: f32) -> u32 {
        let mut alerts = 0;
        for tick in 0..seconds * 20 {
            let now = start + Hns::from_millis(tick * i64::from(SILENCE_WAKE_MS));
            if watch.check_due(now) && watch.should_alert(session_peak) {
                alerts += 1;
            }
        }
        alerts
    }

    #[test]
    fn zero_track_with_a_loud_session_alerts_once() {
        let mut watch = SilenceWatch::new(ORIGIN);
        assert_eq!(alerts_over(&mut watch, ORIGIN, 2, 0.5), 0);
        assert_eq!(
            alerts_over(&mut watch, ORIGIN + Hns::from_millis(2_000), 60, 0.5),
            1
        );
    }

    #[test]
    fn zero_track_with_a_quiet_session_never_alerts() {
        let mut watch = SilenceWatch::new(ORIGIN);
        assert_eq!(alerts_over(&mut watch, ORIGIN, 60, SESSION_SIGNAL_PEAK), 0);
    }

    #[test]
    fn signal_on_the_track_rearms_the_alert() {
        let mut watch = SilenceWatch::new(ORIGIN);
        assert_eq!(alerts_over(&mut watch, ORIGIN, 10, 0.5), 1);
        let later = ORIGIN + Hns::from_millis(20_000);
        watch.on_signal(later);
        assert_eq!(alerts_over(&mut watch, later, 2, 0.5), 0);
        assert_eq!(
            alerts_over(&mut watch, later + Hns::from_millis(2_000), 10, 0.5),
            1
        );
    }

    #[test]
    fn silent_device_keeps_the_track_growing_and_resumes_without_trimming() {
        for sync_offset in [Hns::from_millis(-120), Hns(0), Hns::from_millis(450)] {
            let mut filler = GapFiller::new(ORIGIN);
            let mut out = Vec::new();
            let mut now = ORIGIN + Hns::from_millis(1_000);
            push_packet(
                &mut filler,
                now - Hns::from_millis(10),
                sync_offset,
                &mut out,
            );
            for _ in 0..200 {
                now = now + Hns::from_millis(SILENCE_WAKE_MS.into());
                filler.fill_until(silence_horizon(now, sync_offset), &mut out);
            }
            assert_eq!(
                filler.written_frames(),
                frame_at(now + sync_offset - Hns::from_millis(SILENCE_LAG_MS)),
                "{sync_offset:?}"
            );
            let resumed = now - Hns::from_millis(10);
            push_packet(&mut filler, resumed, sync_offset, &mut out);
            assert_eq!(filler.stats().dropped_frames, 0, "{sync_offset:?}");
            assert_eq!(
                filler.written_frames(),
                frame_at(resumed + sync_offset) + PACKET_FRAMES,
                "{sync_offset:?}"
            );
            assert_eq!(out.len() as i64, filler.written_frames() * CHANNELS as i64);
        }
    }
}
