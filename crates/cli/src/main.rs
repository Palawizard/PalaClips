#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod probe_audio;

use anyhow::Result;
use clap::{Parser, Subcommand};
use palaclips_engine::{
    EngineConfig, VERSION,
    config::{CaptureTarget, OutputResolution, VideoCodec, VideoQuality, VideoSettings},
    replay::estimate_buffer_bytes,
};

#[derive(Debug, Parser)]
#[command(
    name = "palaclips-cli",
    version,
    about = "Banc de test du moteur PalaClips"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Affiche la version et la configuration par défaut (JSON)")]
    Info,
    #[command(about = "Estime la RAM du buffer de replay")]
    Estimate {
        #[arg(long, default_value_t = 40_000)]
        video_kbps: u32,
        #[arg(long, default_value_t = 3)]
        audio_tracks: usize,
        #[arg(long, default_value_t = 60)]
        duration: u32,
    },
    #[cfg(windows)]
    #[command(about = "Affiche la version de FFmpeg liée et les encodeurs vidéo matériels")]
    FfmpegInfo,
    #[cfg(windows)]
    #[command(about = "Liste les écrans et fenêtres capturables")]
    ListTargets {
        #[arg(long, help = "Sortie JSON")]
        json: bool,
    },
    #[cfg(windows)]
    #[command(about = "Capture la vidéo d'une cible et l'encode sur le GPU dans un MP4")]
    CaptureVideo {
        #[arg(long, value_parser = parse_target, default_value = "primary")]
        target: CaptureTarget,
        #[arg(long, default_value_t = 60)]
        fps: u32,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
        #[arg(long)]
        out: std::path::PathBuf,
        #[arg(long, value_parser = parse_codec, default_value = "h264")]
        codec: VideoCodec,
        #[arg(
            long,
            help = "Hauteur de sortie (mise à l'échelle GPU) ; absente = native"
        )]
        height: Option<u32>,
        #[arg(long, value_parser = parse_quality, default_value = "equilibre")]
        quality: VideoQuality,
        #[arg(long, help = "Débit libre en Mb/s (remplace le préréglage)")]
        bitrate_mbps: Option<u32>,
        #[arg(long)]
        no_cursor: bool,
        #[arg(long)]
        border: bool,
    },
    #[cfg(windows)]
    #[command(about = "Test WGC : Video Processor sur le périphérique, cadence et horodatages QPC")]
    ProbeWgc {
        #[arg(long, value_parser = parse_target, default_value = "primary")]
        target: CaptureTarget,
        #[arg(long, default_value_t = 3)]
        seconds: u64,
        #[arg(
            long,
            default_value_t = 0,
            help = "MinUpdateInterval en ms (0 = défaut Windows)"
        )]
        min_interval_ms: u64,
    },
    #[cfg(windows)]
    #[command(
        about = "Test HDR : état HDR et blanc SDR des écrans, valeurs d'une image BGRA8 et FP16"
    )]
    ProbeHdr {
        #[arg(long, value_parser = parse_target, default_value = "primary")]
        target: CaptureTarget,
    },
    #[cfg(windows)]
    #[command(
        about = "Observe le flux WASAPI (loopback de la sortie par défaut ou micro) : horodatages QPC, drapeaux, trous, périphériques"
    )]
    ProbeAudio {
        #[arg(long, help = "Micro par défaut au lieu du loopback de la sortie")]
        mic: bool,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
    #[cfg(windows)]
    #[command(about = "Montre ce qu'une cible désigne (écran ou fenêtre retrouvée par nom d'exe)")]
    ResolveTarget {
        #[arg(long, value_parser = parse_target)]
        target: CaptureTarget,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Info => {
            println!("palaclips-engine {VERSION}");
            println!(
                "{}",
                serde_json::to_string_pretty(&EngineConfig::default())?
            );
        }
        Command::Estimate {
            video_kbps,
            audio_tracks,
            duration,
        } => {
            let bytes = estimate_buffer_bytes(video_kbps, audio_tracks, duration);
            println!("≈ {:.0} Mo", bytes as f64 / 1_000_000.0);
        }
        #[cfg(windows)]
        Command::FfmpegInfo => ffmpeg_info()?,
        #[cfg(windows)]
        Command::ListTargets { json } => list_targets(json)?,
        #[cfg(windows)]
        Command::CaptureVideo {
            target,
            fps,
            seconds,
            out,
            codec,
            height,
            quality,
            bitrate_mbps,
            no_cursor,
            border,
        } => {
            let video = VideoSettings {
                fps,
                resolution: height.map_or(OutputResolution::Native, |height| {
                    OutputResolution::Height { height }
                }),
                codec,
                quality: if bitrate_mbps.is_some() {
                    VideoQuality::Custom
                } else {
                    quality
                },
                bitrate_kbps: bitrate_mbps
                    .map_or(VideoSettings::default().bitrate_kbps, |mbps| mbps * 1_000),
                capture_cursor: !no_cursor,
                draw_border: border,
                ..VideoSettings::default()
            };
            capture_video(&target, &video, seconds, &out)?
        }
        #[cfg(windows)]
        Command::ProbeWgc {
            target,
            seconds,
            min_interval_ms,
        } => probe::probe_wgc(&target, seconds, min_interval_ms)?,
        #[cfg(windows)]
        Command::ProbeHdr { target } => probe::probe_hdr(&target)?,
        #[cfg(windows)]
        Command::ProbeAudio { mic, seconds } => probe_audio::probe_audio(mic, seconds)?,
        #[cfg(windows)]
        Command::ResolveTarget { target } => resolve_target(&target)?,
    }
    Ok(())
}

#[cfg(windows)]
fn ffmpeg_info() -> Result<()> {
    use palaclips_engine::encode::{ffmpeg_version, hardware_video_encoders};

    println!("FFmpeg {}", ffmpeg_version());
    let encoders = hardware_video_encoders()?;
    for encoder in &encoders {
        match &encoder.failure {
            None => println!(
                "  {:<14} disponible     {}",
                encoder.name, encoder.description
            ),
            Some(reason) => println!("  {:<14} indisponible   {reason}", encoder.name),
        }
    }
    if !encoders.iter().any(|encoder| encoder.is_available()) {
        anyhow::bail!(palaclips_engine::EngineError::NoHardwareEncoder);
    }
    Ok(())
}

#[cfg(windows)]
fn list_targets(json: bool) -> Result<()> {
    use palaclips_engine::capture::{TargetInfo, list_targets};

    let targets = list_targets()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&targets)?);
        return Ok(());
    }
    for target in &targets {
        match target {
            TargetInfo::Monitor {
                device_name,
                name,
                width,
                height,
                refresh_rate,
                primary,
            } => {
                let primary = if *primary { " (principal)" } else { "" };
                println!(
                    "monitor:{device_name:<14} {width}x{height} @ {refresh_rate} Hz  {name}{primary}"
                );
            }
            TargetInfo::Window {
                exe_name,
                title,
                width,
                height,
                ..
            } => println!("window:{exe_name:<30} {width}x{height}  {title}"),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn resolve_target(target: &CaptureTarget) -> Result<()> {
    use palaclips_engine::capture::{ResolvedTarget, resolve_target};

    match resolve_target(target)? {
        ResolvedTarget::Monitor(monitor) => {
            println!("écran {}", monitor.device_name()?);
        }
        ResolvedTarget::Window(window) => {
            let monitor = window
                .monitor()
                .and_then(|monitor| monitor.device_name().ok())
                .unwrap_or_default();
            println!("fenêtre « {} » sur {monitor}", window.title()?);
        }
    }
    Ok(())
}

#[cfg(windows)]
fn capture_video(
    target: &CaptureTarget,
    video: &VideoSettings,
    seconds: u64,
    out: &std::path::Path,
) -> Result<()> {
    use palaclips_engine::capture::VideoCapture;
    use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
    use palaclips_engine::encode::{hardware_video_encoders, pick_encoder};
    use palaclips_engine::events::EngineEvent;
    use palaclips_engine::mux::Mp4Writer;

    let encoders = hardware_video_encoders()?;
    let encoder = pick_encoder(&encoders, video.codec)?;
    let capture = VideoCapture::start(target, video, &encoder.name)?;
    if let Some(EngineEvent::Alert { message, .. }) = capture.alert() {
        println!("{message}");
    }
    let stream = capture.stream().clone();
    println!(
        "{} → {}x{} @ {} i/s, {} s",
        encoder.name, stream.size.width, stream.size.height, stream.fps, seconds
    );
    let mut writer =
        Mp4Writer::create(out, &stream.codec_parameters, stream.time_base, stream.fps)?;
    let packets = capture.packets().clone();
    let writer_thread = std::thread::spawn(move || -> Result<Mp4Writer> {
        for packet in packets.iter() {
            writer.write(packet)?;
        }
        Ok(writer)
    });
    let half_frame = Hns(HNS_PER_SECOND / stream.fps as i64 / 2);
    let stop_at = stream.origin + Hns(seconds as i64 * HNS_PER_SECOND) - half_frame;
    let remaining = (stop_at - Hns::now()).0.max(0) as u64;
    std::thread::sleep(std::time::Duration::from_nanos(remaining * 100));
    let stats = capture.stop()?;
    writer_thread
        .join()
        .map_err(|_| anyhow::anyhow!("le fil d'écriture a paniqué"))??
        .finish()?;
    println!(
        "{} images écrites ({} reçues de Windows, {} dupliquées, {} changements de résolution, retard max {:.2} ms) → {}",
        stats.frames,
        stats.captured,
        stats.duplicated,
        stats.resolution_changes,
        stats.max_lateness.0 as f64 / 10_000.0,
        out.display()
    );
    Ok(())
}

fn parse_quality(value: &str) -> std::result::Result<VideoQuality, String> {
    match value.to_lowercase().as_str() {
        "qualite" | "qualité" => Ok(VideoQuality::Quality),
        "equilibre" | "équilibré" => Ok(VideoQuality::Balanced),
        "leger" | "léger" => Ok(VideoQuality::Light),
        _ => Err("qualité attendue : qualite, equilibre ou leger".into()),
    }
}

fn parse_codec(value: &str) -> std::result::Result<VideoCodec, String> {
    match value.to_ascii_lowercase().as_str() {
        "h264" => Ok(VideoCodec::H264),
        "hevc" | "h265" => Ok(VideoCodec::Hevc),
        "av1" => Ok(VideoCodec::Av1),
        _ => Err("codec attendu : h264, hevc ou av1".into()),
    }
}

fn parse_target(value: &str) -> std::result::Result<CaptureTarget, String> {
    if value.eq_ignore_ascii_case("primary") {
        return Ok(CaptureTarget::PrimaryMonitor);
    }
    match value.split_once(':') {
        Some(("monitor", device_name)) if !device_name.is_empty() => Ok(CaptureTarget::Monitor {
            device_name: device_name.into(),
        }),
        Some(("window", rest)) if !rest.is_empty() => {
            let (exe_name, title_hint) = match rest.split_once(':') {
                Some((exe, hint)) => (exe, Some(hint.to_owned())),
                None => (rest, None),
            };
            Ok(CaptureTarget::Window {
                exe_name: exe_name.into(),
                title_hint,
            })
        }
        _ => {
            Err("cible attendue : primary, monitor:<périphérique> ou window:<exe>[:<titre>]".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_primary_monitor_and_window_targets() {
        assert_eq!(parse_target("primary"), Ok(CaptureTarget::PrimaryMonitor));
        assert_eq!(
            parse_target(r"monitor:\.\DISPLAY2"),
            Ok(CaptureTarget::Monitor {
                device_name: r"\.\DISPLAY2".into()
            })
        );
        assert_eq!(
            parse_target("window:FortniteClient-Win64-Shipping.exe:Fortnite"),
            Ok(CaptureTarget::Window {
                exe_name: "FortniteClient-Win64-Shipping.exe".into(),
                title_hint: Some("Fortnite".into())
            })
        );
    }

    #[test]
    fn rejects_unknown_target() {
        assert!(parse_target("screen").is_err());
        assert!(parse_target("window:").is_err());
    }
}
