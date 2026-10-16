use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, SendTimeoutError, Sender, bounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avutil::AVRational;
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
use super::pacer::CfrClock;
use super::targets::{ResolvedTarget, resolve_target};
use crate::clock::Hns;
use crate::config::{CaptureTarget, VideoSettings};
use crate::encode::{
    GpuConverter, HdrToSdr, Size, VideoEncoder, VideoEncoderSettings, frame_texture, output_size,
    render_texture,
};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Severity};

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const SEND_RETRY: Duration = Duration::from_millis(100);

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
    pub resolution_changes: u64,
    pub max_lateness: Hns,
}

pub struct VideoCapture {
    stream: VideoStream,
    packets: Receiver<AVPacket>,
    alert: Option<EngineEvent>,
    shared: Arc<Shared>,
    encoder_thread: Option<JoinHandle<Result<VideoCaptureStats>>>,
    session: Option<CaptureControl<FrameSink, EngineError>>,
}

impl VideoCapture {
    pub fn start(target: &CaptureTarget, settings: &VideoSettings, encoder: &str) -> Result<Self> {
        settings.validate()?;
        keep_mta_alive();
        let resolved = resolve_target(target)?;
        let hdr_white = resolved
            .monitor()
            .and_then(|monitor| monitor.device_name().ok())
            .and_then(|name| display_color(&name))
            .filter(|color| color.hdr)
            .map(|color| color.sdr_white_scrgb());
        let color_format = if hdr_white.is_some() {
            ColorFormat::Rgba16F
        } else {
            ColorFormat::Bgra8
        };
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
                let message = format!(
                    "La fenêtre « {} » ne peut pas être capturée : PalaClips capture son écran à la place.",
                    window.title().unwrap_or_default()
                );
                (session, Some(fallback_alert(message)))
            }
        };
        let first = shared.latest().ok_or_else(|| {
            EngineError::Capture("aucune image reçue de Windows Graphics Capture".into())
        })?;

        // SAFETY: GetDevice has no preconditions on a live texture.
        let device = unsafe { first.texture.0.GetDevice() }
            .map_err(|error| EngineError::Capture(error.message()))?;
        let size = output_size(first.size, settings.resolution);
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
                    run_encoder(shared, device, hdr, encoder, fps, origin_tx, packet_tx)
                }
            })?;
        let mut capture = Self {
            stream,
            packets,
            alert,
            shared,
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

fn keep_mta_alive() {
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

fn run_encoder(
    shared: Arc<Shared>,
    device: SendCom<ID3D11Device>,
    mut hdr: Option<HdrToSdr>,
    mut encoder: VideoEncoder,
    fps: u32,
    origin: Sender<Hns>,
    packets: Sender<AVPacket>,
) -> Result<VideoCaptureStats> {
    let device = device.0;
    let output = encoder.size();
    let mut converter: Option<GpuConverter> = None;
    let mut stats = VideoCaptureStats::default();
    let mut last_count = 0;
    let mut send = |mut packet: AVPacket| loop {
        match packets.send_timeout(packet, SEND_RETRY) {
            Ok(()) => return Ok(()),
            Err(SendTimeoutError::Timeout(_)) if shared.stop.load(Ordering::Relaxed) => {
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

    let mut clock = CfrClock::new(Hns::now(), fps);
    let mut origin = Some(origin);
    while !shared.stop.load(Ordering::Relaxed) {
        if shared.closed.load(Ordering::Relaxed) {
            encoder.finish(&mut send)?;
            return Err(EngineError::CaptureSourceLost(
                "la fenêtre capturée a été fermée".into(),
            ));
        }
        let wait = clock.next_deadline() - Hns::now();
        if wait > Hns::ZERO {
            std::thread::sleep(Duration::from_nanos(wait.0 as u64 * 100));
            if shared.stop.load(Ordering::Relaxed) {
                break;
            }
        }
        let now = Hns::now();
        let due = clock.take_due(now);
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
            converter = Some(GpuConverter::new(
                &device,
                latest.size,
                output,
                clock.fps(),
            )?);
        }
        let Some(converter) = converter.as_ref() else {
            continue;
        };
        let source = match hdr.as_mut() {
            Some(hdr) => hdr.convert(&latest.texture.0)?,
            None => latest.texture.0.clone(),
        };
        for index in due {
            let frame = encoder.acquire_frame()?;
            let (texture, slice) = frame_texture(&frame)
                .ok_or_else(|| EngineError::Encoder("image matérielle sans texture".into()))?;
            converter.convert(&source, texture, slice)?;
            encoder.encode(frame, index, &mut send)?;
            stats.frames += 1;
            if latest.count == last_count {
                stats.duplicated += 1;
            }
            last_count = latest.count;
        }
        stats.captured = latest.count;
        if let Some(origin) = origin.take() {
            clock = CfrClock::new(Hns::now(), fps);
            clock.take_due(clock.origin());
            origin.send(clock.origin()).ok();
        }
    }
    encoder.finish(&mut send)?;
    tracing::info!(
        frames = stats.frames,
        captured = stats.captured,
        duplicated = stats.duplicated,
        max_lateness_ms = stats.max_lateness.0 as f64 / 10_000.0,
        "capture vidéo arrêtée"
    );
    Ok(stats)
}

fn fallback_alert(message: String) -> EngineEvent {
    EngineEvent::Alert {
        severity: Severity::Warning,
        code: "capture_fallback_monitor".into(),
        message,
    }
}
