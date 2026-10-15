#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod probe_audio;
#[cfg(windows)]
mod probe_av;

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
        #[command(flatten)]
        capture: CaptureArgs,
    },
    #[cfg(windows)]
    #[command(about = "Capture vidéo + audio (piste 1 = mix complet, piste 2 = micro) dans un MP4")]
    CaptureAv {
        #[command(flatten)]
        capture: CaptureArgs,
        #[arg(long, help = "Sans la piste mix complet")]
        no_mix: bool,
        #[arg(long, help = "Sans la piste micro")]
        no_mic: bool,
        #[arg(
            long,
            default_value_t = 0.0,
            allow_hyphen_values = true,
            help = "Gain du micro en dB (-20 à +20)"
        )]
        mic_gain_db: f32,
        #[arg(
            long,
            default_value_t = 0,
            allow_hyphen_values = true,
            help = "Décalage de synchro de la piste mix en ms (positif = son retardé)"
        )]
        mix_offset_ms: i32,
        #[arg(
            long,
            default_value_t = 0,
            allow_hyphen_values = true,
            help = "Décalage de synchro de la piste micro en ms (positif = son retardé)"
        )]
        mic_offset_ms: i32,
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
    #[command(
        about = "Émetteur de test A/V : fenêtre plein écran qui flashe en même temps qu'un bip WASAPI, chaque seconde, sans compensation de latence (comme un jeu)"
    )]
    ProbeAvEmitter {
        #[arg(long, default_value_t = 30)]
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
        Command::CaptureVideo { capture } => run_capture(&capture, Vec::new())?,
        #[cfg(windows)]
        Command::CaptureAv {
            capture,
            no_mix,
            no_mic,
            mic_gain_db,
            mix_offset_ms,
            mic_offset_ms,
        } => {
            use palaclips_engine::audio::AudioSource;
            let mut audio = Vec::new();
            if !no_mix {
                audio.push((AudioSource::SystemMix { device_id: None }, mix_offset_ms));
            }
            if !no_mic {
                audio.push((
                    AudioSource::Microphone {
                        device_id: None,
                        gain_db: mic_gain_db,
                    },
                    mic_offset_ms,
                ));
            }
            run_capture(&capture, audio)?
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
        Command::ProbeAvEmitter { seconds } => probe_av::probe_av_emitter(seconds)?,
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
#[derive(Debug, clap::Args)]
struct CaptureArgs {
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
}

#[cfg(windows)]
impl CaptureArgs {
    fn video_settings(&self) -> VideoSettings {
        VideoSettings {
            fps: self.fps,
            resolution: self.height.map_or(OutputResolution::Native, |height| {
                OutputResolution::Height { height }
            }),
            codec: self.codec,
            quality: if self.bitrate_mbps.is_some() {
                VideoQuality::Custom
            } else {
                self.quality
            },
            bitrate_kbps: self
                .bitrate_mbps
                .map_or(VideoSettings::default().bitrate_kbps, |mbps| mbps * 1_000),
            capture_cursor: !self.no_cursor,
            draw_border: self.border,
            ..VideoSettings::default()
        }
    }
}

#[cfg(windows)]
fn run_capture(
    args: &CaptureArgs,
    audio: Vec<(palaclips_engine::audio::AudioSource, i32)>,
) -> Result<()> {
    use palaclips_engine::audio::{AudioTrack, SAMPLE_RATE};
    use palaclips_engine::capture::VideoCapture;
    use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
    use palaclips_engine::encode::{hardware_video_encoders, pick_encoder};
    use palaclips_engine::events::EngineEvent;
    use palaclips_engine::mux::{Mp4Writer, MuxStream};

    let video = args.video_settings();
    let encoders = hardware_video_encoders()?;
    let encoder = pick_encoder(&encoders, video.codec)?;
    let capture = VideoCapture::start(&args.target, &video, &encoder.name)?;
    if let Some(EngineEvent::Alert { message, .. }) = capture.alert() {
        println!("{message}");
    }
    let stream = capture.stream().clone();
    let tracks = audio
        .into_iter()
        .map(|(source, offset_ms)| {
            let title = source.label();
            AudioTrack::start(
                source,
                title,
                stream.origin,
                Hns::from_millis(offset_ms.into()),
            )
        })
        .collect::<palaclips_engine::Result<Vec<_>>>()?;
    println!(
        "{} → {}x{} @ {} i/s, {} s, {} piste(s) audio",
        encoder.name,
        stream.size.width,
        stream.size.height,
        stream.fps,
        args.seconds,
        tracks.len()
    );

    let mut specs = vec![MuxStream {
        codec_parameters: stream.codec_parameters.clone(),
        time_base: stream.time_base,
        frame_rate: Some(stream.fps),
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
    let mut writer = Mp4Writer::create(&args.out, &specs)?;
    let writer_thread = std::thread::spawn(move || -> Result<Mp4Writer> {
        let mut select = crossbeam_channel::Select::new();
        for receiver in &receivers {
            select.recv(receiver);
        }
        let mut open = receivers.len();
        while open > 0 {
            let operation = select.select();
            let index = operation.index();
            match operation.recv(&receivers[index]) {
                Ok(packet) => writer.write(index, packet)?,
                Err(_) => {
                    select.remove(index);
                    open -= 1;
                }
            }
        }
        Ok(writer)
    });

    let half_frame = Hns(HNS_PER_SECOND / stream.fps as i64 / 2);
    let end = stream.origin + Hns(args.seconds as i64 * HNS_PER_SECOND);
    let remaining = (end - half_frame - Hns::now()).0.max(0) as u64;
    std::thread::sleep(std::time::Duration::from_nanos(remaining * 100));
    for track in &tracks {
        track.finish_at(end);
    }
    let stats = capture.stop()?;
    let mut track_reports = Vec::new();
    for track in tracks {
        let title = track.stream().title.clone();
        let alerts = track.alerts().clone();
        let track_stats = track.stop(end)?;
        track_reports.push((title, track_stats, alerts.try_iter().collect::<Vec<_>>()));
    }
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
        args.out.display()
    );
    let ms = |frames: u64| frames * 1_000 / u64::from(SAMPLE_RATE);
    for (title, track, alerts) in track_reports {
        println!(
            "piste « {title} » : {} paquets WASAPI, silence comblé {} ms, chevauchement coupé {} ms, {} corrections de dérive, {} changements de périphérique, crête {:.3}",
            track.packets,
            ms(track.fill.silence_frames),
            ms(track.fill.dropped_frames),
            track.fill.drift_corrections,
            track.device_switches,
            track.peak
        );
        for alert in alerts {
            if let EngineEvent::Alert { message, .. } = alert {
                println!("  alerte : {message}");
            }
        }
    }
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
