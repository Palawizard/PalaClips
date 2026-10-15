#[cfg(windows)]
mod probe;

use anyhow::Result;
use clap::{Parser, Subcommand};
use palaclips_engine::{
    EngineConfig, VERSION,
    config::CaptureTarget,
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
        Command::ProbeWgc {
            target,
            seconds,
            min_interval_ms,
        } => probe::probe_wgc(&target, seconds, min_interval_ms)?,
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
