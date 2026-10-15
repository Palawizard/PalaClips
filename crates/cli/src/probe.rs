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
