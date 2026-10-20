#[cfg(windows)]
mod probe;
#[cfg(windows)]
mod probe_audio;
#[cfg(windows)]
mod probe_av;
#[cfg(windows)]
mod probe_gpu;
#[cfg(windows)]
mod probe_memory;
#[cfg(windows)]
mod replay;

use anyhow::Result;
use clap::{Parser, Subcommand};
use palaclips_engine::{
    EngineConfig, VERSION,
    config::{CaptureTarget, OutputResolution, VideoCodec, VideoQuality, VideoSettings},
    replay::{buffer_window, estimate_buffer_bytes},
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
    #[command(
        about = "Derniers clips du dossier : durée, pistes lues dans le fichier, miniature (JSON)"
    )]
    RecentClips {
        #[arg(long, help = "Dossier des clips ; absent = Vidéos\\PalaClips")]
        clips_dir: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    #[cfg(windows)]
    #[command(
        about = "Nom de jeu choisi pour un exe en cours d'exécution (métadonnées, puis nom d'exe nettoyé)"
    )]
    GameName {
        #[arg(help = "Nom d'exe, ex. FortniteClient-Win64-Shipping.exe")]
        exe_name: String,
    },
    #[cfg(windows)]
    #[command(about = "Affiche la version de FFmpeg liée et les encodeurs vidéo matériels")]
    FfmpegInfo,
    #[cfg(windows)]
    #[command(
        about = "Calibre la synchro par piste (flash + bip plein écran ~12 s) avec les réglages audio d'un settings.json, sans rien enregistrer"
    )]
    Calibrate {
        #[arg(long)]
        settings: Option<std::path::PathBuf>,
    },
    #[cfg(windows)]
    #[command(about = "Liste les périphériques audio et les applis qui ont une session audio")]
    AudioSources,
    #[cfg(windows)]
    #[command(about = "Capture des aperçus (JPEG ~1/s) et l'icône d'une cible dans un dossier")]
    Preview {
        #[arg(long, value_parser = parse_target, default_value = "primary")]
        target: CaptureTarget,
        #[arg(long, default_value_t = 3)]
        seconds: u64,
        #[arg(long)]
        out: std::path::PathBuf,
    },
    #[cfg(windows)]
    #[command(about = "Essaie d'ouvrir l'encodeur matériel à une taille et un fps donnés")]
    TryOutput {
        #[arg(long, value_parser = parse_codec, default_value = "h264")]
        codec: VideoCodec,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long, default_value_t = 60)]
        fps: u32,
        #[arg(long, help = "Ignore la limite connue du codec et essaie quand même")]
        skip_limit: bool,
    },
    #[cfg(windows)]
    #[command(about = "Liste les écrans et fenêtres capturables")]
    ListTargets {
        #[arg(long, help = "Sortie JSON")]
        json: bool,
    },
    #[cfg(windows)]
    #[command(
        about = "Replay armé : Entrée sauvegarde les dernières secondes (début sur image clé), q quitte ; affiche la RAM et la latence"
    )]
    Replay {
        #[command(flatten)]
        replay: replay::ReplayArgs,
    },
    #[cfg(windows)]
    #[command(
        about = "Mémoire du processus étape par étape : D3D11, sondage des encodeurs, capture + encodeur, pistes audio"
    )]
    ProbeMemory {
        #[command(flatten)]
        probe: probe_memory::ProbeMemoryArgs,
    },
    #[cfg(windows)]
    #[command(
        about = "Latence de la conversion d'image sous GPU saturé (processus de charge lancé puis arrêté), selon la priorité GPU demandée"
    )]
    ProbeGpuPriority {
        #[arg(long, default_value_t = 5, help = "Durée de chaque essai")]
        seconds: u64,
        #[arg(long, default_value_t = 2)]
        rounds: u32,
        #[arg(
            long,
            default_value_t = 20.0,
            help = "Durée d'un appel de dessin de la charge, en ms (comme une image de jeu)"
        )]
        load_ms: f64,
        #[arg(
            long,
            help = "La charge ouvre une fenêtre 640×360 au premier plan, comme un jeu"
        )]
        window: bool,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    GpuLoad {
        #[arg(long)]
        seconds: u64,
        #[arg(long)]
        draw_ms: f64,
        #[arg(long)]
        window: bool,
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
            help = "Sortie à enregistrer en loopback (partie du nom) ; absente = sortie par défaut"
        )]
        mix_device: Option<String>,
        #[arg(long, help = "Micro (partie du nom) ; absent = micro par défaut")]
        mic_device: Option<String>,
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
    #[command(
        about = "Capture vidéo + une piste par appli (piste 1 = mix complet, puis micro en option, puis les applis)"
    )]
    CaptureApps {
        #[command(flatten)]
        capture: CaptureArgs,
        #[arg(
            long = "app",
            help = "Nom d'exe d'une appli (répétable), capturée avec ses processus enfants"
        )]
        apps: Vec<String>,
        #[arg(
            long,
            help = "Ajoute la piste « Jeu » (dernière appli en plein écran ou sans bordure, ou l'appli de la fenêtre capturée)"
        )]
        game: bool,
        #[arg(long, help = "Fixe la piste « Jeu » sur ce nom d'exe")]
        game_exe: Option<String>,
        #[arg(long, help = "Sans la piste mix complet")]
        no_mix: bool,
        #[arg(
            long,
            help = "Sortie à enregistrer en loopback pour la piste mix (partie du nom) ; absente = sortie par défaut"
        )]
        mix_device: Option<String>,
        #[arg(long, help = "Ajoute la piste micro (micro par défaut)")]
        mic: bool,
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
        about = "Chaque seconde : plein écran exclusif (D3DKMTCheckExclusiveOwnership), état des notifications et appli au premier plan"
    )]
    ProbeFullscreen {
        #[arg(long, default_value_t = 30)]
        seconds: u64,
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
        #[arg(
            long,
            help = "Process loopback de cette appli (nom d'exe, arbre de processus) au lieu du loopback de la sortie"
        )]
        app: Option<String>,
        #[arg(
            long,
            help = "Loopback de cette sortie (partie du nom) au lieu de la sortie par défaut"
        )]
        device: Option<String>,
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
        #[arg(
            long,
            help = "Sortie à utiliser (partie du nom) ; absente = sortie par défaut"
        )]
        device: Option<String>,
    },
    #[cfg(windows)]
    #[command(
        about = "Liste les sessions audio de toutes les sorties : appli, PID racine, crête instantanée"
    )]
    ListAudioSessions {
        #[arg(long, help = "Sortie JSON")]
        json: bool,
    },
    #[command(
        about = "Déplace les données d'anciens dossiers vers les nouveaux (migration du premier lancement)"
    )]
    MigrateData {
        #[arg(long)]
        old_roaming: std::path::PathBuf,
        #[arg(long)]
        old_local: std::path::PathBuf,
        #[arg(long)]
        new_roaming: std::path::PathBuf,
        #[arg(long)]
        new_local: std::path::PathBuf,
    },
    #[cfg(windows)]
    #[command(about = "Joue une sinusoïde sur une sortie (source de test), avec ou sans fenêtre")]
    PlayTone {
        #[arg(long, default_value_t = 440.0)]
        hz: f64,
        #[arg(
            long,
            default_value_t = 0.25,
            help = "Amplitude (0 = silence numérique)"
        )]
        amplitude: f64,
        #[arg(
            long,
            value_enum,
            help = "Ouvre une fenêtre noire : sans bordure plein écran (écran principal) ou normale"
        )]
        window: Option<probe_av::ToneWindow>,
        #[arg(long, default_value_t = 30)]
        seconds: u64,
        #[arg(
            long,
            help = "Sortie à utiliser (partie du nom) ; absente = sortie par défaut"
        )]
        device: Option<String>,
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
            let window = buffer_window(
                duration,
                palaclips_engine::config::VideoSettings::default().keyframe_interval_secs,
            );
            let bytes = estimate_buffer_bytes(video_kbps, audio_tracks, window);
            println!("≈ {:.0} Mo", bytes as f64 / 1_000_000.0);
        }
        #[cfg(windows)]
        Command::RecentClips { clips_dir, limit } => {
            let root = match clips_dir {
                Some(dir) => dir,
                None => palaclips_engine::paths::default_clips_dir()?,
            };
            let clips = palaclips_engine::library::recent_clips(&root, limit);
            let space = palaclips_engine::library::disk_space(&root)?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "clips": clips, "disk": space })
                )?
            );
        }
        #[cfg(windows)]
        Command::GameName { exe_name } => {
            let resolved = palaclips_engine::games::resolve(&exe_name);
            println!(
                "{} ({})",
                resolved.name,
                if resolved.from_metadata_lookup {
                    "processus trouvé, nom figé à la première sauvegarde"
                } else {
                    "processus absent, nom d'exe nettoyé, non figé"
                }
            );
        }
        #[cfg(windows)]
        Command::FfmpegInfo => ffmpeg_info()?,
        #[cfg(windows)]
        Command::Calibrate { settings } => calibrate(settings.as_deref())?,
        #[cfg(windows)]
        Command::AudioSources => audio_sources()?,
        #[cfg(windows)]
        Command::Preview {
            target,
            seconds,
            out,
        } => preview(&target, seconds, &out)?,
        #[cfg(windows)]
        Command::TryOutput {
            codec,
            width,
            height,
            fps,
            skip_limit,
        } => try_output(codec, width, height, fps, skip_limit)?,
        #[cfg(windows)]
        Command::ListTargets { json } => list_targets(json)?,
        #[cfg(windows)]
        Command::CaptureVideo { capture } => run_capture(&capture, Vec::new())?,
        #[cfg(windows)]
        Command::Replay { replay } => replay::run_replay(&replay)?,
        #[cfg(windows)]
        Command::ProbeMemory { probe } => probe_memory::probe_memory(&probe)?,
        #[cfg(windows)]
        Command::ProbeGpuPriority {
            seconds,
            rounds,
            load_ms,
            window,
        } => probe_gpu::probe_gpu_priority(seconds, rounds, load_ms, window)?,
        #[cfg(windows)]
        Command::GpuLoad {
            seconds,
            draw_ms,
            window,
        } => probe_gpu::gpu_load(seconds, draw_ms, window)?,
        #[cfg(windows)]
        Command::CaptureAv {
            capture,
            no_mix,
            no_mic,
            mix_device,
            mic_device,
            mic_gain_db,
            mix_offset_ms,
            mic_offset_ms,
        } => {
            use palaclips_engine::audio::AudioSource;
            use wasapi::Direction;
            let device_id = |direction, name: Option<String>| -> Result<Option<String>> {
                name.map(|name| Ok(probe_audio::find_device(direction, &name)?.get_id()?))
                    .transpose()
            };
            let mut audio = Vec::new();
            if !no_mix {
                audio.push((
                    AudioSource::SystemMix {
                        device_id: device_id(Direction::Render, mix_device)?,
                    },
                    mix_offset_ms,
                ));
            }
            if !no_mic {
                audio.push((
                    AudioSource::Microphone {
                        device_id: device_id(Direction::Capture, mic_device)?,
                        gain_db: mic_gain_db,
                    },
                    mic_offset_ms,
                ));
            }
            run_capture(&capture, audio)?
        }
        #[cfg(windows)]
        Command::CaptureApps {
            capture,
            apps,
            game,
            game_exe,
            no_mix,
            mix_device,
            mic,
        } => {
            use palaclips_engine::audio::AudioSource;
            let mut audio = Vec::new();
            if !no_mix {
                let device_id = mix_device
                    .map(|name| -> Result<String> {
                        Ok(probe_audio::find_device(wasapi::Direction::Render, &name)?.get_id()?)
                    })
                    .transpose()?;
                audio.push((AudioSource::SystemMix { device_id }, 0));
            }
            if mic {
                audio.push((
                    AudioSource::Microphone {
                        device_id: None,
                        gain_db: 0.0,
                    },
                    0,
                ));
            }
            for exe_name in apps {
                audio.push((AudioSource::Application { exe_name }, 0));
            }
            if game || game_exe.is_some() {
                let current =
                    palaclips_engine::sources::CurrentGame::new(game_exe, &capture.video.target);
                audio.push((AudioSource::Game(current), 0));
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
        Command::ProbeFullscreen { seconds } => probe::probe_fullscreen(seconds)?,
        #[cfg(windows)]
        Command::ProbeAudio {
            mic,
            app,
            device,
            seconds,
        } => probe_audio::probe_audio(mic, app.as_deref(), device.as_deref(), seconds)?,
        #[cfg(windows)]
        Command::ProbeAvEmitter { seconds, device } => {
            probe_av::probe_av_emitter(seconds, device.as_deref())?
        }
        #[cfg(windows)]
        Command::ResolveTarget { target } => resolve_target(&target)?,
        #[cfg(windows)]
        Command::ListAudioSessions { json } => list_audio_sessions(json)?,
        Command::MigrateData {
            old_roaming,
            old_local,
            new_roaming,
            new_local,
        } => {
            let migration = palaclips_engine::migrate::migrate_data(
                &palaclips_engine::migrate::DataDirs {
                    roaming: old_roaming,
                    local: old_local,
                },
                &palaclips_engine::migrate::DataDirs {
                    roaming: new_roaming,
                    local: new_local,
                },
            )?;
            println!(
                "{} élément(s) déplacé(s), {} gardé(s) à l'ancien emplacement",
                migration.moved.len(),
                migration.kept.len()
            );
        }
        #[cfg(windows)]
        Command::PlayTone {
            hz,
            amplitude,
            window,
            seconds,
            device,
        } => probe_av::play_tone(hz, amplitude, seconds, device.as_deref(), window)?,
    }
    Ok(())
}

#[cfg(windows)]
#[cfg(windows)]
fn try_output(
    codec: VideoCodec,
    width: u32,
    height: u32,
    fps: u32,
    skip_limit: bool,
) -> Result<()> {
    use palaclips_engine::encode::{Size, hardware_video_encoders, probe_output, try_output};

    let encoders = hardware_video_encoders()?;
    let size = Size::new(width, height);
    let outcome = if skip_limit {
        probe_output(&encoders, codec, size, fps)
    } else {
        try_output(&encoders, codec, size, fps)
    };
    match outcome {
        Ok(()) => println!("{codec:?} {width}x{height} {fps} i/s : accepté"),
        Err(error) => println!("{codec:?} {width}x{height} {fps} i/s : {error}"),
    }
    Ok(())
}

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
                minimized,
                ..
            } => {
                let minimized = if *minimized { " (réduite)" } else { "" };
                println!("window:{exe_name:<30} {width}x{height}  {title}{minimized}")
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn calibrate(settings: Option<&std::path::Path>) -> Result<()> {
    use palaclips_engine::engine::Engine;

    let config = match settings {
        Some(path) => {
            let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
            serde_json::from_value(json.get("engine").cloned().unwrap_or(json))?
        }
        None => EngineConfig::default(),
    };
    let engine = Engine::start(config)?;
    let started = std::time::Instant::now();
    let calibration = engine.handle().calibrate()?;
    println!("calibrage en {:.1} s", started.elapsed().as_secs_f64());
    for track in &calibration.tracks {
        match track.measurement {
            Some(measured) => println!(
                "  {:<14} {:+7.1} ms (paires {}, écart {:.1} ms)",
                track.title, measured.late_ms, measured.pairs, measured.spread_ms
            ),
            None => println!("  {:<14} non mesurée", track.title),
        }
    }
    println!(
        "propositions : mix {:?}, micro {:?}, jeu {:?}, applis {:?}",
        calibration.mix_ms, calibration.microphone_ms, calibration.game_ms, calibration.apps
    );
    Ok(())
}

#[cfg(windows)]
fn audio_sources() -> Result<()> {
    use palaclips_engine::sources::{app_sessions, audio_devices};

    for (input, title) in [(false, "Sorties"), (true, "Entrées")] {
        println!("{title} :");
        for device in audio_devices(input)? {
            let default = if device.default { " (défaut)" } else { "" };
            println!("  {}{default}", device.name);
        }
    }
    println!("Applis avec une session audio :");
    for session in app_sessions()? {
        println!(
            "  {:<28} {:<34} PID {:>6}  crête {:.3}",
            session.app_name, session.exe_name, session.process_id, session.peak
        );
    }
    Ok(())
}

#[cfg(windows)]
fn preview(target: &CaptureTarget, seconds: u64, out: &std::path::Path) -> Result<()> {
    use palaclips_engine::capture::{PreviewCapture, TargetInfo, app_icon, list_targets};
    use std::sync::atomic::{AtomicUsize, Ordering};

    std::fs::create_dir_all(out)?;
    if let CaptureTarget::Window { exe_name, .. } = target {
        let process_id = list_targets()?.into_iter().find_map(|info| match info {
            TargetInfo::Window {
                exe_name: name,
                process_id,
                ..
            } if name.eq_ignore_ascii_case(exe_name) => Some(process_id),
            _ => None,
        });
        match process_id.and_then(app_icon) {
            Some(png) => {
                std::fs::write(out.join("icon.png"), &png)?;
                println!("icône : {} octets", png.len());
            }
            None => println!("icône : aucune"),
        }
    }
    let count = std::sync::Arc::new(AtomicUsize::new(0));
    let written = count.clone();
    let dir = out.to_path_buf();
    let started = std::time::Instant::now();
    let capture = PreviewCapture::start(
        target,
        Box::new(move |jpeg| {
            let index = written.fetch_add(1, Ordering::Relaxed);
            let path = dir.join(format!("preview-{index}.jpg"));
            println!(
                "{:>5} ms  {} octets  {}",
                started.elapsed().as_millis(),
                jpeg.len(),
                path.display()
            );
            if let Err(error) = std::fs::write(&path, jpeg) {
                eprintln!("écriture impossible : {error}");
            }
        }),
    )?;
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    drop(capture);
    println!("{} aperçus en {seconds} s", count.load(Ordering::Relaxed));
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
    #[command(flatten)]
    video: VideoArgs,
    #[arg(long, default_value_t = 10)]
    seconds: u64,
    #[arg(long)]
    out: std::path::PathBuf,
    #[arg(
        long,
        help = "Garde la priorité GPU normale (sans la classe temps réel)"
    )]
    no_gpu_priority: bool,
}

#[cfg(windows)]
#[derive(Debug, clap::Args)]
struct VideoArgs {
    #[arg(long, value_parser = parse_target, default_value = "primary")]
    target: CaptureTarget,
    #[arg(long, default_value_t = 60)]
    fps: u32,
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
impl VideoArgs {
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
    use palaclips_engine::audio::{AudioTrack, Mixdown, SAMPLE_RATE};
    use palaclips_engine::capture::VideoCapture;
    use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
    use palaclips_engine::encode::{hardware_video_encoders, pick_encoder};
    use palaclips_engine::events::EngineEvent;
    use palaclips_engine::mux::{Mp4Writer, MuxStream};

    let video = args.video.video_settings();
    if !args.no_gpu_priority {
        palaclips_engine::capture::raise_gpu_priority();
    }
    let encoders = hardware_video_encoders()?;
    let encoder = pick_encoder(&encoders, video.codec)?;
    let capture = VideoCapture::start(&args.video.target, &video, &encoder.name)?;
    if let Some(EngineEvent::Alert { message, .. }) = capture.alert() {
        println!("{message}");
    }
    let stream = capture.stream().clone();
    let tracks = audio
        .into_iter()
        .enumerate()
        .map(|(index, (source, offset_ms))| {
            let title = source.label();
            AudioTrack::start(
                index,
                source,
                title,
                stream.origin,
                Hns::from_millis(offset_ms.into()),
                Mixdown::None,
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
    let writer_thread = std::thread::spawn(move || -> Result<(Mp4Writer, Vec<Hns>)> {
        let mut last_arrival = vec![None::<Hns>; receivers.len()];
        let mut max_arrival_gap = vec![Hns(0); receivers.len()];
        let mut select = crossbeam_channel::Select::new();
        for receiver in &receivers {
            select.recv(receiver);
        }
        let mut open = receivers.len();
        while open > 0 {
            let operation = select.select();
            let index = operation.index();
            match operation.recv(&receivers[index]) {
                Ok(packet) => {
                    let now = Hns::now();
                    if let Some(last) = last_arrival[index] {
                        max_arrival_gap[index] = max_arrival_gap[index].max(now - last);
                    }
                    last_arrival[index] = Some(now);
                    writer.write(index, packet)?
                }
                Err(_) => {
                    select.remove(index);
                    open -= 1;
                }
            }
        }
        Ok((writer, max_arrival_gap))
    });

    let half_frame = Hns(HNS_PER_SECOND / stream.fps as i64 / 2);
    let end = stream.origin + Hns(args.seconds as i64 * HNS_PER_SECOND);
    let audio_lead = Hns::from_millis(100);
    let until_audio_stop = (end - audio_lead - Hns::now()).0.max(0) as u64;
    std::thread::sleep(std::time::Duration::from_nanos(until_audio_stop * 100));
    for track in &tracks {
        track.finish_at(end);
    }
    let remaining = (end - half_frame - Hns::now()).0.max(0) as u64;
    std::thread::sleep(std::time::Duration::from_nanos(remaining * 100));
    let stats = capture.stop()?;
    let mut track_reports = Vec::new();
    for track in tracks {
        let title = track.stream().title.clone();
        let alerts = track.alerts().clone();
        let track_stats = track.stop(end)?;
        track_reports.push((title, track_stats, alerts.try_iter().collect::<Vec<_>>()));
    }
    let (writer, max_arrival_gap) = writer_thread
        .join()
        .map_err(|_| anyhow::anyhow!("le fil d'écriture a paniqué"))??;
    writer.finish()?;
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
    for ((title, track, alerts), arrival_gap) in
        track_reports.into_iter().zip(&max_arrival_gap[1..])
    {
        println!(
            "piste « {title} » : {} paquets WASAPI, silence comblé {} ms, chevauchement coupé {} ms, {} corrections de dérive, {} changements de périphérique ou rattachements, crête {:.3}, écart max entre paquets AAC reçus {:.0} ms",
            track.packets,
            ms(track.fill.silence_frames),
            ms(track.fill.dropped_frames),
            track.fill.drift_corrections,
            track.device_switches,
            track.peak,
            arrival_gap.0 as f64 / 10_000.0
        );
        for alert in alerts {
            if let EngineEvent::Alert { message, .. } = alert {
                println!("  alerte : {message}");
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn list_audio_sessions(json: bool) -> Result<()> {
    use palaclips_engine::sources;
    wasapi::initialize_mta()
        .ok()
        .map_err(|error| anyhow::anyhow!("COM : {error}"))?;
    let processes = sources::processes()?;
    let sessions = sources::audio_sessions(&processes)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }
    for session in &sessions {
        let exe = session.exe_name.as_deref().unwrap_or("?");
        let root = sources::root_pid(&processes, exe).unwrap_or(session.pid);
        println!(
            "{:<28} PID {:>6} (racine {:>6})  {:<8} crête {:.3}  {}  {}",
            exe,
            session.pid,
            root,
            if session.active { "active" } else { "inactive" },
            session.peak,
            session.device,
            session.display_name
        );
    }
    if let Some(window) = sources::foreground_window() {
        println!(
            "premier plan : {} (PID {}, couvre son écran : {}, barre de titre : {}, jeu : {})",
            window.exe_name,
            window.pid,
            window.covers_monitor,
            window.has_caption,
            window.is_game()
        );
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
