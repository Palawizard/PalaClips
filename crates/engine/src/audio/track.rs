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
use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects};

use super::filler::{FillStats, GapFiller};
use super::{AudioSource, CHANNELS, SAMPLE_RATE};
use crate::clock::Hns;
use crate::encode::AacEncoder;
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Severity};

const BUFFER_DURATION_HNS: i64 = 2_000_000;
const BYTES_PER_FRAME: usize = CHANNELS * size_of::<f32>();
const EXCLUSIVE_RETRY_MS: u32 = 1_000;
const END_GRACE_MS: i64 = 250;
const SILENCE_WAKE_MS: u32 = 50;
const SILENCE_LAG_MS: i64 = 100;
const PACKET_QUEUE: usize = 256;
const ALERT_QUEUE: usize = 32;

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
}

pub struct AudioTrack {
    stream: AudioStream,
    packets: Receiver<AVPacket>,
    alerts: Receiver<EngineEvent>,
    stop: Arc<Event>,
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
        source: AudioSource,
        title: impl Into<String>,
        origin: Hns,
        sync_offset: Hns,
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
        let stop = Arc::new(Event::new()?);
        let end = Arc::new(AtomicI64::new(i64::MIN));
        let worker = Worker {
            source,
            title,
            filler: GapFiller::new(origin),
            sync_offset,
            stats: AudioTrackStats::default(),
            filled: Vec::new(),
            encoder,
            packets: packet_tx,
            alerts: alert_tx,
            stop: stop.clone(),
            end: end.clone(),
        };
        let thread = std::thread::Builder::new()
            .name(format!("palaclips-audio-{}", worker.title))
            .spawn(move || worker.run())?;
        Ok(Self {
            stream,
            packets,
            alerts,
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
    source: AudioSource,
    title: String,
    filler: GapFiller,
    sync_offset: Hns,
    stats: AudioTrackStats,
    filled: Vec<f32>,
    encoder: AacEncoder,
    packets: Sender<AVPacket>,
    alerts: Sender<EngineEvent>,
    stop: Arc<Event>,
    end: Arc<AtomicI64>,
}

enum Wake {
    Stop,
    DefaultChanged,
}

impl Worker {
    fn run(mut self) -> Result<AudioTrackStats> {
        initialize_mta()
            .ok()
            .map_err(|error| EngineError::Audio(error.message()))?;
        let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
        let default_changed = Arc::new(Event::new()?);
        let devices_changed = Arc::new(Event::new()?);
        let direction = self.direction();
        let follows_default = self.device_id().is_none();
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
        let waits_for_device = [
            self.stop.handle(),
            default_changed.handle(),
            devices_changed.handle(),
        ];

        loop {
            let stream = match open_stream(&enumerator, &self.source) {
                Ok(stream) => stream,
                Err(error) => {
                    let in_use = is_device_in_use(&error);
                    if !unavailable_reported {
                        unavailable_reported = true;
                        self.alert_unavailable(&error, in_use);
                    }
                    if self.idle(&waits_for_device, in_use.then_some(EXCLUSIVE_RETRY_MS))? {
                        break;
                    }
                    continue;
                }
            };
            unavailable_reported = false;
            if current_device.as_ref() != Some(&stream.name) {
                tracing::info!(track = %self.title, device = %stream.name, "périphérique audio ouvert");
                if current_device.is_some() {
                    self.stats.device_switches += 1;
                    self.alert(
                        Severity::Info,
                        "audio_device_changed",
                        format!(
                            "Piste « {} » : la capture suit maintenant « {} ».",
                            self.title, stream.name
                        ),
                    );
                }
                current_device = Some(stream.name.clone());
            }

            let mut handles = vec![self.stop.handle(), stream.event.as_handle()];
            if follows_default {
                handles.push(default_changed.handle());
            }
            let outcome = loop {
                match wait(&handles, SILENCE_WAKE_MS) {
                    Waited::Handle(0) => break Ok(Wake::Stop),
                    Waited::Handle(2) => break Ok(Wake::DefaultChanged),
                    Waited::Handle(_) => {}
                    Waited::Timeout => {
                        if let Err(error) = self.fill_silence() {
                            break Err(error);
                        }
                        continue;
                    }
                    Waited::Failed => {
                        break Err(EngineError::Audio("attente WASAPI impossible".into()));
                    }
                }
                if let Err(error) = stream.drain(&mut buffer, &mut samples, |qpc, data| {
                    self.on_packet(qpc, data)
                }) {
                    break Err(error);
                }
            };
            match outcome {
                Ok(Wake::Stop) => {
                    self.read_until_end(&stream, &mut buffer, &mut samples)?;
                    break;
                }
                Ok(Wake::DefaultChanged) => {}
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
        self.flush()?;
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

    fn on_packet(&mut self, qpc: Hns, data: &mut [f32]) -> Result<()> {
        self.stats.packets += 1;
        let gain = self.gain();
        for sample in data.iter_mut() {
            *sample *= gain;
            self.stats.peak = self.stats.peak.max(sample.abs());
        }
        self.filler
            .push(qpc + self.sync_offset, data, &mut self.filled);
        self.flush()
    }

    fn flush(&mut self) -> Result<()> {
        let packets = &self.packets;
        let result = self
            .encoder
            .encode(&self.filled, &mut |packet| send(packets, packet));
        self.filled.clear();
        result
    }

    fn direction(&self) -> Direction {
        match self.source {
            AudioSource::SystemMix { .. } => Direction::Render,
            AudioSource::Microphone { .. } => Direction::Capture,
        }
    }

    fn device_id(&self) -> Option<&str> {
        match &self.source {
            AudioSource::SystemMix { device_id } | AudioSource::Microphone { device_id, .. } => {
                device_id.as_deref()
            }
        }
    }

    fn gain(&self) -> f32 {
        match self.source {
            AudioSource::SystemMix { .. } => 1.0,
            AudioSource::Microphone { gain_db, .. } => 10f32.powf(gain_db / 20.0),
        }
    }

    fn alert_unavailable(&self, error: &WasapiError, in_use: bool) {
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
        self.alert(Severity::Warning, "audio_device_unavailable", message);
    }

    fn alert(&self, severity: Severity, code: &str, message: String) {
        self.alerts
            .try_send(EngineEvent::Alert {
                severity,
                code: code.into(),
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
        AudioSource::SystemMix { device_id } => (Direction::Render, device_id),
        AudioSource::Microphone { device_id, .. } => (Direction::Capture, device_id),
    };
    let device = match device_id {
        Some(id) => enumerator.get_device(id)?,
        None => enumerator.get_default_device(&direction)?,
    };
    let name = device.get_friendlyname()?;
    let mut client = device.get_iaudioclient()?;
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

struct Event(HANDLE);

// SAFETY: Win32 event handles may be signaled and waited on from any thread.
unsafe impl Send for Event {}

// SAFETY: SetEvent and waits are thread-safe kernel operations on the same handle.
unsafe impl Sync for Event {}

impl Event {
    fn new() -> Result<Self> {
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

impl Drop for Event {
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
