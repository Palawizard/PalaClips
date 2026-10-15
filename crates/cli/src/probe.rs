use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use palaclips_engine::capture::{ResolvedTarget, resolve_target};
use palaclips_engine::clock::Hns;
use palaclips_engine::config::CaptureTarget;
use windows::Win32::Graphics::Direct3D11::{ID3D11VideoContext1, ID3D11VideoDevice};
use windows::core::Interface;
use windows_capture::capture::{Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

#[derive(Debug, Default)]
struct Report {
    video_device: bool,
    video_context1: bool,
    frames: u64,
    sizes: Vec<(u32, u32)>,
    format: i32,
    first_timestamp: Option<Hns>,
    last_timestamp: Hns,
    intervals: Vec<i64>,
    latencies: Vec<i64>,
}

#[derive(Debug)]
struct Probe(Arc<Mutex<Report>>);

impl GraphicsCaptureApiHandler for Probe {
    type Flags = Arc<Mutex<Report>>;
    type Error = anyhow::Error;

    fn new(context: Context<Self::Flags>) -> Result<Self> {
        let mut report = context.flags.lock().map_err(|_| anyhow!("verrou"))?;
        report.video_device = context.device.cast::<ID3D11VideoDevice>().is_ok();
        report.video_context1 = context.device_context.cast::<ID3D11VideoContext1>().is_ok();
        drop(report);
        Ok(Self(context.flags))
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<()> {
        let now = Hns::now();
        let timestamp = Hns(frame.timestamp()?.Duration);
        let mut report = self.0.lock().map_err(|_| anyhow!("verrou"))?;
        report.frames += 1;
        report.format = frame.desc().Format.0;
        let size = (frame.width(), frame.height());
        if report.sizes.last() != Some(&size) {
            report.sizes.push(size);
        }
        if report.first_timestamp.is_some() {
            let interval = (timestamp - report.last_timestamp).0;
            report.intervals.push(interval);
        } else {
            report.first_timestamp = Some(timestamp);
        }
        report.last_timestamp = timestamp;
        report.latencies.push((now - timestamp).0);
        Ok(())
    }
}

pub fn probe_wgc(target: &CaptureTarget, seconds: u64, min_interval_ms: u64) -> Result<()> {
    let report = Arc::new(Mutex::new(Report::default()));
    let interval = match min_interval_ms {
        0 => MinimumUpdateIntervalSettings::Default,
        ms => MinimumUpdateIntervalSettings::Custom(Duration::from_millis(ms)),
    };
    let control = match resolve_target(target)? {
        ResolvedTarget::Monitor(monitor) => {
            Probe::start_free_threaded(settings(monitor, interval, report.clone()))
        }
        ResolvedTarget::Window(window) => {
            Probe::start_free_threaded(settings(window, interval, report.clone()))
        }
    }
    .map_err(|error| anyhow!("{error}"))?;
    std::thread::sleep(Duration::from_secs(seconds));
    control.stop().map_err(|error| anyhow!("{error}"))?;

    let report = report.lock().map_err(|_| anyhow!("verrou"))?;
    let ms = |hns: i64| hns as f64 / 10_000.0;
    let stats = |values: &[i64]| {
        let min = values.iter().copied().min().unwrap_or_default();
        let max = values.iter().copied().max().unwrap_or_default();
        let avg = values.iter().sum::<i64>() / values.len().max(1) as i64;
        format!(
            "min {:.2} ms, moy {:.2} ms, max {:.2} ms",
            ms(min),
            ms(avg),
            ms(max)
        )
    };
    println!(
        "ID3D11VideoDevice sur le périphérique WGC : {}",
        report.video_device
    );
    println!("ID3D11VideoContext1 : {}", report.video_context1);
    println!("images reçues : {} en {seconds} s", report.frames);
    println!(
        "tailles : {:?}, format DXGI {}",
        report.sizes, report.format
    );
    println!(
        "intervalle entre horodatages : {}",
        stats(&report.intervals)
    );
    println!(
        "latence QPC (réception − horodatage) : {}",
        stats(&report.latencies)
    );
    Ok(())
}

fn settings<T: TryInto<windows_capture::settings::GraphicsCaptureItemType>>(
    item: T,
    interval: MinimumUpdateIntervalSettings,
    report: Arc<Mutex<Report>>,
) -> Settings<Arc<Mutex<Report>>, T> {
    Settings::new(
        item,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        interval,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        report,
    )
}

type GrabbedFrame = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

#[derive(Debug)]
struct FrameGrab(GrabbedFrame);

impl GraphicsCaptureApiHandler for FrameGrab {
    type Flags = GrabbedFrame;
    type Error = anyhow::Error;

    fn new(context: Context<Self::Flags>) -> Result<Self> {
        Ok(Self(context.flags))
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<()> {
        let (width, height) = (frame.width(), frame.height());
        let buffer = frame.buffer()?;
        let mut packed = Vec::new();
        let bytes = buffer.as_nopadding_buffer(&mut packed).to_vec();
        *self.0.lock().map_err(|_| anyhow!("verrou"))? = Some((width, height, bytes));
        control.stop();
        Ok(())
    }
}

fn grab(target: ResolvedTarget, format: ColorFormat) -> Result<(u32, u32, Vec<u8>)> {
    let slot = Arc::new(Mutex::new(None));
    let settings = |item| {
        Settings::new(
            item,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            format,
            slot.clone(),
        )
    };
    match target {
        ResolvedTarget::Monitor(monitor) => FrameGrab::start(settings(monitor)),
        ResolvedTarget::Window(window) => FrameGrab::start(Settings::new(
            window,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            format,
            slot.clone(),
        )),
    }
    .map_err(|error| anyhow!("{error}"))?;
    let grabbed = slot.lock().map_err(|_| anyhow!("verrou"))?.take();
    grabbed.ok_or_else(|| anyhow!("aucune image"))
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let mantissa = (bits & 0x3ff) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

fn percentiles(mut values: Vec<f32>) -> String {
    values.sort_by(f32::total_cmp);
    let at = |q: f64| values[((values.len() - 1) as f64 * q) as usize];
    format!(
        "p50 {:.3}  p90 {:.3}  p99 {:.3}  max {:.3}",
        at(0.5),
        at(0.9),
        at(0.99),
        at(1.0)
    )
}

pub fn probe_hdr(target: &CaptureTarget) -> Result<()> {
    use palaclips_engine::capture::{display_color, display_color_info};
    use windows_capture::monitor::Monitor;

    for monitor in Monitor::enumerate()? {
        let name = monitor.device_name()?;
        let Some(info) = display_color_info(&name) else {
            println!("{name} : chemin d'affichage introuvable");
            continue;
        };
        println!("{name}");
        match info.legacy_flags {
            Some(flags) => println!(
                "  ADVANCED_COLOR_INFO   : bits {flags:#06b} (supported {}, enabled {}, wideColorEnforced {}, forceDisabled {})",
                flags & 1,
                flags >> 1 & 1,
                flags >> 2 & 1,
                flags >> 3 & 1
            ),
            None => println!("  ADVANCED_COLOR_INFO   : échec"),
        }
        match info.info2 {
            Some(info2) => println!(
                "  ADVANCED_COLOR_INFO_2 : bits {:#010b} (supported {}, active {}, limitedByPolicy {}, hdrSupported {}, hdrUserEnabled {}, wcgSupported {}, wcgUserEnabled {}), {} bits/canal, activeColorMode {} ({})",
                info2.flags,
                info2.flags & 1,
                info2.flags >> 1 & 1,
                info2.flags >> 3 & 1,
                info2.flags >> 4 & 1,
                info2.flags >> 5 & 1,
                info2.flags >> 6 & 1,
                info2.flags >> 7 & 1,
                info2.bits_per_color_channel,
                info2.active_color_mode,
                ["SDR", "WCG", "HDR"]
                    .get(info2.active_color_mode as usize)
                    .unwrap_or(&"?")
            ),
            None => println!("  ADVANCED_COLOR_INFO_2 : échec"),
        }
        match info.sdr_white_level {
            Some(level) => println!(
                "  SDRWhiteLevel         : {level} ({:.0} nits)",
                info.sdr_white_nits()
            ),
            None => println!("  SDRWhiteLevel         : échec"),
        }
        match display_color(&name) {
            Some(color) => println!(
                "  => HDR {}, blanc SDR {:.0} nits (scRGB {:.2})",
                if color.hdr { "activé" } else { "désactivé" },
                color.sdr_white_nits,
                color.sdr_white_scrgb()
            ),
            None => println!("  => informations couleur indisponibles"),
        }
    }

    let resolved = resolve_target(target)?;
    let (width, height, bgra) = grab(resolved, ColorFormat::Bgra8)?;
    let bgra_max: Vec<f32> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| px[0].max(px[1]).max(px[2]) as f32 / 255.0)
        .collect();
    println!(
        "BGRA8 {width}x{height}, max(R,G,B) normalisé : {}",
        percentiles(bgra_max.clone())
    );

    let (width, height, half) = grab(resolved, ColorFormat::Rgba16F)?;
    let scrgb_max: Vec<f32> = half
        .as_chunks::<8>()
        .0
        .iter()
        .map(|px| {
            let channel = |i: usize| half_to_f32(u16::from_le_bytes([px[i], px[i + 1]]));
            channel(0).max(channel(2)).max(channel(4))
        })
        .collect();
    println!(
        "RGBA16F {width}x{height}, max(R,G,B) scRGB linéaire : {}",
        percentiles(scrgb_max.clone())
    );
    println!("BGRA8 moyen selon le scRGB du même pixel :");
    let bins = [0.0, 0.05, 0.2, 0.5, 1.0, 2.0, 3.0, 4.0, 100.0];
    for range in bins.windows(2) {
        let matching: Vec<f32> = scrgb_max
            .iter()
            .zip(&bgra_max)
            .filter(|(scrgb, _)| (range[0]..range[1]).contains(*scrgb))
            .map(|(_, bgra)| *bgra)
            .collect();
        if !matching.is_empty() {
            let mean = matching.iter().sum::<f32>() / matching.len() as f32;
            println!(
                "  scRGB [{:.2} ; {:.2}[ : {:>8} px, BGRA8 moyen {mean:.3}",
                range[0],
                range[1],
                matching.len()
            );
        }
    }
    Ok(())
}
