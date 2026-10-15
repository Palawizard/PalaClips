use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, bounded};
use parking_lot::Mutex;
use rsmpeg::avcodec::{AVCodecParameters, AVPacket};
use rsmpeg::avutil::AVRational;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Multithread, ID3D11Texture2D};
use windows::core::Interface;
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    GraphicsCaptureItemType, MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

use super::pacer::CfrClock;
use super::targets::{ResolvedTarget, resolve_target};
use crate::clock::Hns;
use crate::config::{CaptureTarget, VideoSettings};
use crate::encode::{
    GpuConverter, Size, VideoEncoder, VideoEncoderSettings, bgra_texture, frame_texture,
    output_size,
};
use crate::error::{EngineError, Result};
use crate::events::{EngineEvent, Severity};

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(3);

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
    stop: Arc<AtomicBool>,
    encoder_thread: Option<JoinHandle<Result<VideoCaptureStats>>>,
    session: Option<CaptureControl<FrameSink, EngineError>>,
}

impl VideoCapture {
    pub fn start(target: &CaptureTarget, settings: &VideoSettings, encoder: &str) -> Result<Self> {
        settings.validate()?;
        let resolved = resolve_target(target)?;
        let ((shared, session), alert) = match start_session(resolved, settings) {
            Ok(session) => (session, None),
            Err(error) => {
                let (ResolvedTarget::Window(window), Some(monitor)) =
                    (resolved, resolved.monitor())
                else {
                    return Err(error);
                };
                tracing::warn!(%error, "capture de fenêtre impossible, repli sur son écran");
                let session =
                    start_session(ResolvedTarget::Monitor(monitor), settings).map_err(|_| error)?;
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
                bitrate_kbps: settings.bitrate_kbps,
                keyframe_interval_secs: settings.keyframe_interval_secs,
            },
        )?;
        let clock = CfrClock::new(Hns::now(), settings.fps);
        let stream = VideoStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            fps: settings.fps,
            size,
            origin: clock.origin(),
        };
        let (packet_tx, packets) = bounded(settings.fps as usize * 2);
        let stop = Arc::new(AtomicBool::new(false));
        let encoder_thread = std::thread::Builder::new()
            .name("palaclips-video".into())
            .spawn({
                let stop = stop.clone();
                let device = SendCom(device);
                move || run_encoder(shared, device, encoder, clock, size, packet_tx, stop)
            })?;
        Ok(Self {
            stream,
            packets,
            alert,
            stop,
            encoder_thread: Some(encoder_thread),
            session: Some(session),
        })
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
        self.stop.store(true, Ordering::Relaxed);
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

struct SendCom<T>(T);

// SAFETY: D3D11 devices and textures are free-threaded; the immediate context is used only with
// ID3D11Multithread protection enabled before any frame is processed.
unsafe impl<T> Send for SendCom<T> {}

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
                Arc::new(SendCom(bgra_texture(frame.device(), size)?)),
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
) -> Result<(Arc<Shared>, CaptureControl<FrameSink, EngineError>)> {
    let shared = Arc::new(Shared::default());
    let (first_tx, first_rx) = bounded(1);
    let flags = (shared.clone(), first_tx);
    let control = match target {
        ResolvedTarget::Monitor(monitor) => {
            FrameSink::start_free_threaded(session_settings(monitor, settings, flags))
        }
        ResolvedTarget::Window(window) => {
            FrameSink::start_free_threaded(session_settings(window, settings, flags))
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
        ColorFormat::Bgra8,
        flags,
    )
}

fn run_encoder(
    shared: Arc<Shared>,
    device: SendCom<ID3D11Device>,
    mut encoder: VideoEncoder,
    mut clock: CfrClock,
    output: Size,
    packets: Sender<AVPacket>,
    stop: Arc<AtomicBool>,
) -> Result<VideoCaptureStats> {
    let device = device.0;
    let mut converter: Option<GpuConverter> = None;
    let mut stats = VideoCaptureStats::default();
    let mut last_count = 0;
    let mut send = |packet: AVPacket| {
        packets
            .send(packet)
            .map_err(|_| EngineError::Encoder("plus personne ne lit les paquets vidéo".into()))
    };

    while !stop.load(Ordering::Relaxed) {
        if shared.closed.load(Ordering::Relaxed) {
            encoder.finish(&mut send)?;
            return Err(EngineError::CaptureSourceLost(
                "la fenêtre capturée a été fermée".into(),
            ));
        }
        let wait = clock.next_deadline() - Hns::now();
        if wait > Hns::ZERO {
            std::thread::sleep(Duration::from_nanos(wait.0 as u64 * 100));
            if stop.load(Ordering::Relaxed) {
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
        for index in due {
            let frame = encoder.acquire_frame()?;
            let (texture, slice) = frame_texture(&frame)
                .ok_or_else(|| EngineError::Encoder("image matérielle sans texture".into()))?;
            converter.convert(&latest.texture.0, texture, slice)?;
            encoder.encode(frame, index, &mut send)?;
            stats.frames += 1;
            if latest.count == last_count {
                stats.duplicated += 1;
            }
            last_count = latest.count;
        }
        stats.captured = latest.count;
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
