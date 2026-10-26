use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, SendTimeoutError, Sender, bounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avutil::{AVFrame, AVRational};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Multithread, ID3D11Texture2D};
use windows::Win32::System::Com::CoIncrementMTAUsage;
use windows::core::Interface;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    GraphicsCaptureItemType, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use super::display::display_color;
use super::pacer::{CfrClock, bounded_due, emit_due};
use super::rate::{
    FreezeWatch, RateWatch, SaturationWatch, composition_rate, exclusive_fullscreen,
};
use super::targets::{ResolvedTarget, resolve_target};
use crate::clock::{HNS_PER_SECOND, Hns};
use crate::config::{CaptureTarget, VideoSettings};
use crate::encode::{
    FrameConverter, HdrToSdr, Size, VideoEncoder, VideoEncoderSettings, check_output_size,
    output_size, render_texture,
};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Localized, Severity};
use crate::perf::CaptureCounters;

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const SEND_RETRY: Duration = Duration::from_millis(100);
const RATE_CHECK: Hns = Hns::from_millis(1_000);
const ALERT_QUEUE: usize = 8;

#[derive(Debug, Clone)]
pub struct VideoStream {
    pub codec_parameters: AVCodecParameters,
    pub time_base: AVRational,
    pub fps: u32,
    pub size: Size,
    pub origin: Hns,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VideoCaptureStats {
    pub frames: u64,
    pub captured: u64,
    pub duplicated: u64,
    pub late: u64,
    pub dropped: u64,
    pub resolution_changes: u64,
    pub max_lateness: Hns,
}

pub struct VideoCapture {
    stream: VideoStream,
    packets: Receiver<AVPacket>,
    alert: Option<EngineEvent>,
    alerts: Receiver<EngineEvent>,
    shared: Arc<Shared>,
    hdr_white: Option<f32>,
    encoder_thread: Option<JoinHandle<Result<VideoCaptureStats>>>,
    session: Option<CaptureControl<FrameSink, EngineError>>,
}

impl VideoCapture {
    pub fn start(target: &CaptureTarget, settings: &VideoSettings, encoder: &str) -> Result<Self> {
        settings.validate()?;
        keep_mta_alive();
        let resolved = resolve_target(target)?;
        let display = resolved
            .monitor()
            .and_then(|monitor| monitor.device_name().ok());
        let hdr_white = display
            .as_deref()
            .and_then(display_color)
            .filter(|color| color.hdr)
            .map(|color| color.sdr_white_scrgb());
        let hdr_watch = display.map(|name| (name, hdr_white.is_some()));
        let color_format = if hdr_white.is_some() {
            ColorFormat::Rgba16F
        } else {
            ColorFormat::Bgra8
        };
        tracing::info!(hdr = hdr_white.is_some(), "format de capture choisi");
        let ((shared, session), alert) = match start_session(resolved, settings, color_format) {
            Ok(session) => (session, None),
            Err(error) => {
                let (ResolvedTarget::Window(window), Some(monitor)) =
                    (resolved, resolved.monitor())
                else {
                    return Err(error);
                };
                tracing::warn!(%error, "capture de fenêtre impossible, repli sur son écran");
                let session =
                    start_session(ResolvedTarget::Monitor(monitor), settings, color_format)
                        .map_err(|_| error)?;
                (
                    session,
                    Some(fallback_alert(&window.title().unwrap_or_default())),
                )
            }
        };
        let first = shared.latest().ok_or_else(|| {
            EngineError::Capture("aucune image reçue de Windows Graphics Capture".into())
        })?;

        // SAFETY: GetDevice has no preconditions on a live texture.
        let device = unsafe { first.texture.0.GetDevice() }
            .map_err(|error| EngineError::Capture(error.message()))?;
        let size = output_size(first.size, settings.resolution);
        check_output_size(settings.codec, size)?;
        let encoder = VideoEncoder::open(
            &device,
            &VideoEncoderSettings {
                encoder: encoder.to_owned(),
                size,
                fps: settings.fps,
                bitrate_kbps: settings.bitrate_kbps_for(size.width, size.height),
                keyframe_interval_secs: settings.keyframe_interval_secs,
            },
        )?;
        let stream = VideoStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            fps: settings.fps,
            size,
            origin: Hns::ZERO,
        };
        let (packet_tx, packets) = bounded(settings.fps as usize * 2);
        let (origin_tx, origin_rx) = bounded(1);
        let (alert_tx, alerts) = bounded(ALERT_QUEUE);
        let encoder_thread = std::thread::Builder::new()
            .name("palaclips-video".into())
            .spawn({
                let shared = shared.clone();
                let device = SendCom(device);
                let fps = settings.fps;
                move || {
                    let hdr = hdr_white
                        .map(|white| HdrToSdr::new(&device.0, white))
                        .transpose()?;
                    let outputs = EncoderOutputs {
                        origin: origin_tx,
                        packets: packet_tx,
                        alerts: alert_tx,
                        hdr_watch,
                    };
                    run_encoder(shared, device, hdr, encoder, fps, outputs, Role::Primary)
                }
            })?;
        let mut capture = Self {
            stream,
            packets,
            alert,
            alerts,
            shared,
            hdr_white,
            encoder_thread: Some(encoder_thread),
            session: Some(session),
        };
        match origin_rx.recv() {
            Ok(origin) => {
                capture.stream.origin = origin;
                Ok(capture)
            }
            Err(_) => Err(capture.shutdown().err().unwrap_or_else(|| {
                EngineError::Encoder("le fil d'encodage s'est arrêté au démarrage".into())
            })),
        }
    }

    pub fn stream(&self) -> &VideoStream {
        &self.stream
    }

    pub fn packets(&self) -> &Receiver<AVPacket> {
        &self.packets
    }

    pub fn alert(&self) -> Option<&EngineEvent> {
        self.alert.as_ref()
    }

    pub fn alerts(&self) -> &Receiver<EngineEvent> {
        &self.alerts
    }

    pub fn counters(&self) -> CaptureCounters {
        *self.shared.counters.lock()
    }

    pub fn add_output(&self, settings: &VideoSettings, encoder: &str) -> Result<VideoOutput> {
        settings.validate()?;
        let latest = self.shared.latest().ok_or_else(|| {
            EngineError::Capture("aucune image reçue de Windows Graphics Capture".into())
        })?;
        // SAFETY: GetDevice has no preconditions on a live texture.
        let device = unsafe { latest.texture.0.GetDevice() }
            .map_err(|error| EngineError::Capture(error.message()))?;
        let size = output_size(latest.size, settings.resolution);
        check_output_size(settings.codec, size)?;
        let encoder = VideoEncoder::open(
            &device,
            &VideoEncoderSettings {
                encoder: encoder.to_owned(),
                size,
                fps: settings.fps,
                bitrate_kbps: settings.bitrate_kbps_for(size.width, size.height),
                keyframe_interval_secs: settings.keyframe_interval_secs,
            },
        )?;
        let stream = VideoStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            fps: settings.fps,
            size,
            origin: Hns::ZERO,
        };
        let (packet_tx, packets) = bounded(settings.fps as usize * 2);
        let (origin_tx, origin_rx) = bounded(1);
        let (alert_tx, alerts) = bounded(ALERT_QUEUE);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("palaclips-video-2".into())
            .spawn({
                let shared = self.shared.clone();
                let device = SendCom(device);
                let hdr_white = self.hdr_white;
                let fps = settings.fps;
                let stop = stop.clone();
                let align = self.stream.origin;
                move || {
                    let hdr = hdr_white
                        .map(|white| HdrToSdr::new(&device.0, white))
                        .transpose()?;
                    let outputs = EncoderOutputs {
                        origin: origin_tx,
                        packets: packet_tx,
                        alerts: alert_tx,
                        hdr_watch: None,
                    };
                    run_encoder(
                        shared,
                        device,
                        hdr,
                        encoder,
                        fps,
                        outputs,
                        Role::Extra { stop, align },
                    )
                }
            })?;
        let mut output = VideoOutput {
            stream,
            packets,
            alerts,
            stop,
            thread: Some(thread),
        };
        match origin_rx.recv() {
            Ok(origin) => {
                output.stream.origin = origin;
                Ok(output)
            }
            Err(_) => Err(output.shutdown().err().unwrap_or_else(|| {
                EngineError::Encoder("le 2e fil d'encodage s'est arrêté au démarrage".into())
            })),
        }
    }

    pub fn stop(mut self) -> Result<VideoCaptureStats> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<VideoCaptureStats> {
        self.shared.stop.store(true, Ordering::Relaxed);
        let stats = match self.encoder_thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| EngineError::Encoder("le fil d'encodage a paniqué".into()))?,
            None => Ok(VideoCaptureStats::default()),
        };
        if let Some(session) = self.session.take() {
            session
                .stop()
                .map_err(|error| EngineError::Capture(error.to_string()))?;
        }
        stats
    }
}

impl std::fmt::Debug for VideoCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoCapture")
            .field("stream", &self.stream)
            .field("alert", &self.alert)
            .finish_non_exhaustive()
    }
}

impl Drop for VideoCapture {
    fn drop(&mut self) {
        if self.encoder_thread.is_some()
            && let Err(error) = self.shutdown()
        {
            tracing::warn!(%error, "arrêt de la capture vidéo");
        }
    }
}

pub struct VideoOutput {
    stream: VideoStream,
    packets: Receiver<AVPacket>,
    alerts: Receiver<EngineEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<VideoCaptureStats>>>,
}

impl VideoOutput {
    pub fn stream(&self) -> &VideoStream {
        &self.stream
    }

    pub fn packets(&self) -> &Receiver<AVPacket> {
        &self.packets
    }

    pub fn alerts(&self) -> &Receiver<EngineEvent> {
        &self.alerts
    }

    pub fn stop(mut self) -> Result<VideoCaptureStats> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> Result<VideoCaptureStats> {
        self.stop.store(true, Ordering::Relaxed);
        match self.thread.take() {
            Some(thread) => thread
                .join()
                .map_err(|_| EngineError::Encoder("le 2e fil d'encodage a paniqué".into()))?,
            None => Ok(VideoCaptureStats::default()),
        }
    }
}

impl std::fmt::Debug for VideoOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoOutput")
            .field("stream", &self.stream)
            .finish_non_exhaustive()
    }
}

impl Drop for VideoOutput {
    fn drop(&mut self) {
        if self.thread.is_some()
            && let Err(error) = self.shutdown()
        {
            tracing::warn!(%error, "arrêt de la 2e sortie vidéo");
        }
    }
}

enum Role {
    Primary,
    Extra { stop: Arc<AtomicBool>, align: Hns },
}

impl Role {
    fn stopped(&self, shared: &Shared) -> bool {
        shared.stop.load(Ordering::Relaxed)
            || matches!(self, Self::Extra { stop, .. } if stop.load(Ordering::Relaxed))
    }
}

pub(super) fn keep_mta_alive() {
    static MTA: std::sync::Once = std::sync::Once::new();
    MTA.call_once(|| {
        // windows-rs caches WinRT factories for the whole process; once the MTA is torn down
        // between two sessions, WinTypes.dll unloads under them (https://learn.microsoft.com/windows/win32/api/combaseapi/nf-combaseapi-coincrementmtausage).
        // SAFETY: the usage cookie is deliberately never released so the MTA lives as long as the process.
        if let Err(error) = unsafe { CoIncrementMTAUsage() } {
            tracing::warn!(%error, "MTA COM non maintenu");
        }
    });
}

struct SendCom<T>(T);

// SAFETY: D3D11 devices are free-threaded; the immediate context is used only with
// ID3D11Multithread protection enabled before any frame is processed.
unsafe impl Send for SendCom<ID3D11Device> {}

// SAFETY: D3D11 textures are free-threaded resources, only read or copied through the protected
// immediate context.
unsafe impl Send for SendCom<ID3D11Texture2D> {}

#[derive(Clone)]
struct LatestFrame {
    texture: Arc<SendCom<ID3D11Texture2D>>,
    size: Size,
    count: u64,
}

#[derive(Default)]
struct Shared {
    latest: Mutex<Option<LatestFrame>>,
    counters: Mutex<CaptureCounters>,
    closed: AtomicBool,
    stop: AtomicBool,
}

impl Shared {
    fn latest(&self) -> Option<LatestFrame> {
        self.latest.lock().clone()
    }
}

struct FrameSink {
    shared: Arc<Shared>,
    first_frame: Sender<()>,
}

impl GraphicsCaptureApiHandler for FrameSink {
    type Flags = (Arc<Shared>, Sender<()>);
    type Error = EngineError;

    fn new(context: Context<Self::Flags>) -> Result<Self> {
        let multithread: ID3D11Multithread = context
            .device
            .cast()
            .map_err(|error| EngineError::Capture(error.message()))?;
        // SAFETY: plain COM call on a valid interface, before any concurrent use of the context.
        let _was_protected = unsafe { multithread.SetMultithreadProtected(true) };
        let (shared, first_frame) = context.flags;
        Ok(Self {
            shared,
            first_frame,
        })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<()> {
        let size = Size::new(frame.width(), frame.height());
        let mut latest = self.shared.latest.lock();
        let (texture, count) = match latest.as_ref() {
            Some(previous) if previous.size == size => (previous.texture.clone(), previous.count),
            previous => (
                Arc::new(SendCom(render_texture(
                    frame.device(),
                    size,
                    frame.desc().Format,
                )?)),
                previous.map_or(0, |previous| previous.count),
            ),
        };
        // SAFETY: both textures live on the capture device and have the same size and format.
        unsafe {
            frame
                .device_context()
                .CopyResource(&texture.0, frame.as_raw_texture())
        };
        *latest = Some(LatestFrame {
            texture,
            size,
            count: count + 1,
        });
        let _ = self.first_frame.try_send(());
        Ok(())
    }

    fn on_closed(&mut self) -> Result<()> {
        self.shared.closed.store(true, Ordering::Relaxed);
        Ok(())
    }
}

fn start_session(
    target: ResolvedTarget,
    settings: &VideoSettings,
    color_format: ColorFormat,
) -> Result<(Arc<Shared>, CaptureControl<FrameSink, EngineError>)> {
    let shared = Arc::new(Shared::default());
    let (first_tx, first_rx) = bounded(1);
    let flags = (shared.clone(), first_tx);
    let control = match target {
        ResolvedTarget::Monitor(monitor) => {
            FrameSink::start_free_threaded(session_settings(monitor, settings, color_format, flags))
        }
        ResolvedTarget::Window(window) => {
            FrameSink::start_free_threaded(session_settings(window, settings, color_format, flags))
        }
    }
    .map_err(|error| EngineError::Capture(error.to_string()))?;
    if first_rx.recv_timeout(FIRST_FRAME_TIMEOUT).is_err() {
        let _ = control.stop();
        return Err(EngineError::Capture(
            "aucune image reçue en 3 s (fenêtre réduite ou protégée ?)".into(),
        ));
    }
    Ok((shared, control))
}

fn session_settings<T: TryInto<GraphicsCaptureItemType>>(
    item: T,
    settings: &VideoSettings,
    color_format: ColorFormat,
    flags: (Arc<Shared>, Sender<()>),
) -> Settings<(Arc<Shared>, Sender<()>), T> {
    let half_period = Duration::from_secs_f64(0.5 / settings.fps as f64);
    Settings::new(
        item,
        if settings.capture_cursor {
            CursorCaptureSettings::WithCursor
        } else {
            CursorCaptureSettings::WithoutCursor
        },
        if settings.draw_border {
            DrawBorderSettings::WithBorder
        } else {
            DrawBorderSettings::WithoutBorder
        },
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Custom(half_period),
        DirtyRegionSettings::Default,
        color_format,
        flags,
    )
}

struct EncoderOutputs {
    origin: Sender<Hns>,
    packets: Sender<AVPacket>,
    alerts: Sender<EngineEvent>,
    hdr_watch: Option<(String, bool)>,
}

fn run_encoder(
    shared: Arc<Shared>,
    device: SendCom<ID3D11Device>,
    mut hdr: Option<HdrToSdr>,
    mut encoder: VideoEncoder,
    fps: u32,
    outputs: EncoderOutputs,
    role: Role,
) -> Result<VideoCaptureStats> {
    let primary = matches!(role, Role::Primary);
    let EncoderOutputs {
        origin,
        packets,
        alerts,
        hdr_watch,
    } = outputs;
    let device = device.0;
    let output = encoder.size();
    let mut converter: Option<FrameConverter> = None;
    let mut previous: Option<AVFrame> = None;
    let mut stats = VideoCaptureStats::default();
    let mut last_count = 0;
    let mut send = |mut packet: AVPacket| loop {
        match packets.send_timeout(packet, SEND_RETRY) {
            Ok(()) => return Ok(()),
            Err(SendTimeoutError::Timeout(_)) if role.stopped(&shared) => {
                return Ok(());
            }
            Err(SendTimeoutError::Timeout(unsent)) => packet = unsent,
            Err(SendTimeoutError::Disconnected(_)) => {
                return Err(EngineError::Encoder(
                    "plus personne ne lit les paquets vidéo".into(),
                ));
            }
        }
    };

    let mut clock = match &role {
        Role::Primary => CfrClock::new(Hns::now(), fps),
        Role::Extra { align, .. } => {
            let mut aligned = CfrClock::new(*align, fps);
            let _ = aligned.take_due(Hns::now());
            aligned
        }
    };
    let mut origin = Some(origin);
    let mut rate_watch = RateWatch::new(fps);
    let mut saturation_watch = if encoder.is_hardware() {
        SaturationWatch::default()
    } else {
        SaturationWatch::cpu_encoder()
    };
    let backlog = if encoder.is_hardware() {
        usize::MAX
    } else {
        (fps as usize / 2).max(2)
    };
    let mut freeze_watch = FreezeWatch::default();
    let mut next_rate_check = Hns::now() + RATE_CHECK;
    let mut frames_at_check = 0;
    let mut emitted_at_check = (0, 0);
    while !role.stopped(&shared) {
        if shared.closed.load(Ordering::Relaxed) {
            encoder.finish(&mut send)?;
            return Err(EngineError::CapturedWindowClosed);
        }
        let wait = clock.next_deadline() - Hns::now();
        if wait > Hns::ZERO {
            std::thread::sleep(Duration::from_nanos(wait.0 as u64 * 100));
            if role.stopped(&shared) {
                break;
            }
        }
        let now = Hns::now();
        if now >= next_rate_check {
            next_rate_check = now + RATE_CHECK;
            if let Some((screen, hdr)) = &hdr_watch
                && display_color(screen).is_some_and(|color| color.hdr != *hdr)
            {
                tracing::info!(
                    screen = screen.as_str(),
                    hdr = !hdr,
                    "mode HDR de l'écran changé"
                );
                encoder.finish(&mut send)?;
                return Err(EngineError::Capture(
                    "le mode HDR de l'écran a changé".into(),
                ));
            }
            let delivered = shared
                .latest()
                .map_or(frames_at_check, |latest| latest.count);
            let new_frames = delivered.saturating_sub(frames_at_check);
            frames_at_check = delivered;
            if primary {
                if let Some(alert) =
                    composition_rate().and_then(|rate| rate_watch.observe(now, rate, new_frames))
                {
                    alerts.try_send(alert).ok();
                }
                if let Some(alert) = freeze_watch.observe(now, new_frames, exclusive_fullscreen) {
                    alerts.try_send(alert).ok();
                }
            }
            let (emitted, late) = emitted_at_check;
            if let Some(alert) =
                saturation_watch.observe(now, stats.frames - emitted, stats.late - late)
            {
                alerts.try_send(alert).ok();
            }
            emitted_at_check = (stats.frames, stats.late);
        }
        let (due, dropped) = bounded_due(clock.take_due(now), backlog);
        if dropped > 0 {
            stats.dropped += dropped;
            stats.late += dropped;
            tracing::debug!(dropped, "encodage en retard : échéances sautées");
        }
        if due.is_empty() {
            continue;
        }
        stats.max_lateness = stats.max_lateness.max(now - clock.tick_time(due.start));
        let Some(latest) = shared.latest() else {
            continue;
        };
        if converter
            .as_ref()
            .is_none_or(|current| current.input() != latest.size)
        {
            if converter.is_some() {
                stats.resolution_changes += 1;
                tracing::info!(
                    width = latest.size.width,
                    height = latest.size.height,
                    "changement de résolution de la source"
                );
            }
            converter = Some(FrameConverter::new(
                &device,
                latest.size,
                output,
                clock.fps(),
                &encoder,
            )?);
        }
        let Some(converter) = converter.as_mut() else {
            continue;
        };
        let frame_period = Hns(HNS_PER_SECOND / i64::from(clock.fps()));
        let has_new_image = latest.count != last_count;
        emit_due(due, |index, newest| {
            if now - clock.tick_time(index) > frame_period {
                stats.late += 1;
            }
            let frame = match previous.as_ref() {
                Some(previous) if !(newest && has_new_image) => {
                    stats.duplicated += 1;
                    previous.clone()
                }
                _ => {
                    let source = match hdr.as_mut() {
                        Some(hdr) => hdr.convert(&latest.texture.0)?,
                        None => latest.texture.0.clone(),
                    };
                    let mut frame = encoder.acquire_frame()?;
                    converter.convert(&source, &mut frame)?;
                    previous = Some(frame.clone());
                    frame
                }
            };
            encoder.encode(frame, index, &mut send)?;
            stats.frames += 1;
            Ok::<_, EngineError>(())
        })?;
        last_count = latest.count;
        stats.captured = latest.count;
        if primary {
            let mut counters = shared.counters.lock();
            counters.frames = stats.frames;
            counters.repeated = stats.duplicated;
            counters.late = stats.late;
            counters.max_lateness = stats.max_lateness;
        }
        if let Some(origin) = origin.take() {
            if primary {
                clock = CfrClock::new(Hns::now(), fps);
                clock.take_due(clock.origin());
            }
            origin.send(clock.origin()).ok();
        }
    }
    encoder.finish(&mut send)?;
    tracing::info!(
        frames = stats.frames,
        captured = stats.captured,
        duplicated = stats.duplicated,
        late = stats.late,
        dropped = stats.dropped,
        max_lateness_ms = stats.max_lateness.0 as f64 / 10_000.0,
        "capture vidéo arrêtée"
    );
    Ok(stats)
}

fn fallback_alert(title: &str) -> EngineEvent {
    EngineEvent::Alert {
        severity: Severity::Warning,
        text: Localized::new("capture_fallback_monitor").with("title", title),
        message: format!(
            "La fenêtre « {title} » ne peut pas être capturée : PalaClips capture son écran à la place."
        ),
    }
}
