use std::io::BufRead;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, select};
use palaclips_engine::EngineConfig;
use palaclips_engine::chime::{self, Sound};
use palaclips_engine::config::{
    AppTrack, AudioSettings, RecordSettings, RecordSplit, ReplaySettings, ReplayStorage,
};
use palaclips_engine::engine::{Engine, LevelRate};
use palaclips_engine::events::{EngineEvent, EngineState};
use palaclips_engine::paths::private_bytes;

const STATS_EVERY: u32 = 5;
const PENDING_SAVES_TIMEOUT: Duration = Duration::from_secs(20);
const ARM_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, clap::Args)]
pub struct ReplayArgs {
    #[command(flatten)]
    pub video: crate::VideoArgs,
    #[arg(long, default_value_t = 60, help = "Durée du replay en secondes")]
    pub duration: u32,
    #[arg(
        long,
        default_value_t = 0,
        help = "Durée après l'appui en secondes (0 à 600) : le clip continue en direct"
    )]
    pub after: u32,
    #[arg(long, help = "Tampon sur disque au lieu de la RAM")]
    pub disk: bool,
    #[arg(long, help = "Dossier des clips ; absent = Vidéos\\PalaClips")]
    pub clips_dir: Option<PathBuf>,
    #[arg(long = "app", help = "Nom d'exe d'une appli (répétable)")]
    pub apps: Vec<String>,
    #[arg(long, help = "Sans la piste « Jeu »")]
    pub no_game: bool,
    #[arg(long, help = "Sans la piste mix complet")]
    pub no_mix: bool,
    #[arg(long, help = "Sans la piste micro")]
    pub no_mic: bool,
    #[arg(
        long,
        help = "Piste mix complet = somme des autres pistes au lieu de toute la sortie audio"
    )]
    pub tracks_mix: bool,
    #[arg(
        long,
        help = "Sortie à enregistrer en loopback pour la piste mix (partie du nom) ; absente = sortie par défaut"
    )]
    pub mix_device: Option<String>,
    #[arg(
        long,
        help = "Piste mix = loopback de la sortie (sons de PalaClips compris) au lieu de tout sauf PalaClips"
    )]
    pub mix_output: bool,
    #[arg(long, help = "Démarre le moteur sans armer le replay (mesure mémoire)")]
    pub no_arm: bool,
    #[arg(
        long,
        help = "Affiche chaque seconde les mesures du panneau de performances"
    )]
    pub performance: bool,
    #[arg(
        long,
        help = "Demande les niveaux audio (20 i/s) et affiche leur cadence"
    )]
    pub levels: bool,
    #[arg(
        long,
        help = "Joue les sons de PalaClips comme l'app : appui avec durée après, clip sauvegardé, début et fin d'enregistrement"
    )]
    pub chime: bool,
    #[arg(long, help = "Enregistrement : commence par le replay en mémoire")]
    pub record_include_replay: bool,
    #[arg(long, help = "Enregistrement : nouveau fichier toutes les N minutes")]
    pub record_split_minutes: Option<u32>,
    #[arg(long, help = "Enregistrement : nouveau fichier tous les N Mo")]
    pub record_split_mb: Option<u32>,
    #[arg(
        long,
        help = "Enregistrement dans sa propre qualité : hauteur (0 = native)"
    )]
    pub record_height: Option<u32>,
    #[arg(
        long,
        help = "Enregistrement dans sa propre qualité : images par seconde"
    )]
    pub record_fps: Option<u32>,
    #[arg(long, value_parser = crate::parse_codec, help = "Enregistrement dans sa propre qualité : codec")]
    pub record_codec: Option<palaclips_engine::config::VideoCodec>,
    #[arg(long, help = "Enregistrement dans sa propre qualité : débit en Mb/s")]
    pub record_bitrate_mbps: Option<u32>,
}

impl ReplayArgs {
    fn record_video(&self) -> Option<palaclips_engine::config::RecordVideo> {
        use palaclips_engine::config::{OutputResolution, RecordVideo, VideoQuality};
        if self.record_height.is_none()
            && self.record_fps.is_none()
            && self.record_codec.is_none()
            && self.record_bitrate_mbps.is_none()
        {
            return None;
        }
        let mut record = RecordVideo::from_video(&self.video.video_settings());
        if let Some(height) = self.record_height {
            record.resolution = match height {
                0 => OutputResolution::Native,
                height => OutputResolution::Height { height },
            };
        }
        record.fps = self.record_fps.unwrap_or(record.fps);
        record.codec = self.record_codec.unwrap_or(record.codec);
        if let Some(mbps) = self.record_bitrate_mbps {
            record.quality = VideoQuality::Custom;
            record.bitrate_kbps = mbps * 1_000;
        }
        Some(record)
    }

    fn config(&self) -> Result<EngineConfig> {
        let output_device_id = self
            .mix_device
            .as_deref()
            .map(|name| -> Result<String> {
                Ok(crate::probe_audio::find_device(wasapi::Direction::Render, name)?.get_id()?)
            })
            .transpose()?;
        let mut config = EngineConfig {
            capture: self.video.target.clone(),
            video: self.video.video_settings(),
            audio: AudioSettings {
                record_full_mix: !self.no_mix,
                desktop_in_full_mix: !self.tracks_mix,
                desktop_excludes_palaclips: !self.mix_output && self.mix_device.is_none(),
                output_device_id,
                record_microphone: !self.no_mic,
                app_tracks: self
                    .apps
                    .iter()
                    .map(|exe_name| AppTrack {
                        exe_name: exe_name.clone(),
                        label: palaclips_engine::paths::game_label(Some(exe_name)),
                        enabled: true,
                        sync_offset_ms: 0,
                    })
                    .collect(),
                auto_game_track: !self.no_game,
                ..AudioSettings::default()
            },
            replay: ReplaySettings {
                arm_on_startup: true,
                duration_secs: self.duration,
                after_secs: self.after,
                storage: if self.disk {
                    ReplayStorage::Disk
                } else {
                    ReplayStorage::Ram
                },
            },
            record: RecordSettings {
                include_replay: self.record_include_replay,
                split: match (self.record_split_minutes, self.record_split_mb) {
                    (Some(minutes), _) => RecordSplit::Duration { minutes },
                    (None, Some(megabytes)) => RecordSplit::Size { megabytes },
                    (None, None) => RecordSplit::Never,
                },
                video: self.record_video(),
            },
            ..EngineConfig::default()
        };
        config.storage.clips_dir = self.clips_dir.clone();
        Ok(config)
    }
}

pub fn run_replay(args: &ReplayArgs) -> Result<()> {
    let mut config = args.config()?;
    let engine = Engine::start(config.clone())?;
    let handle = engine.handle();
    if args.levels {
        handle.set_level_rate(LevelRate::Focused)?;
    }
    if !args.no_arm {
        handle.arm()?;
        wait_until_armed(engine.events())?;
    }
    let started = Instant::now();
    println!(
        "replay de {} s ({}) : Entrée = sauvegarder, f = terminer le clip en cours, e = démarrer/arrêter l'enregistrement, r = changer les sources (piste Jeu) et redémarrer le tampon, q = quitter",
        args.duration,
        if args.disk { "disque" } else { "RAM" }
    );
    if args.performance {
        handle.set_performance(true)?;
    }
    let lines = spawn_stdin_reader();
    let mut requested = 0u32;
    let mut finished = 0u32;
    let mut latencies = Vec::new();
    let mut stats_seen = 0u32;
    let mut level_events = 0u32;
    let mut last_levels = Vec::new();
    let mut quitting = false;
    let mut recording = false;
    let mut post_roll = false;
    let mut state_recording = false;
    let chime_on = |sound: Sound| {
        if args.chime {
            chime::play(sound);
        }
    };
    loop {
        if quitting && finished >= requested {
            break;
        }
        let timeout = if quitting {
            PENDING_SAVES_TIMEOUT
        } else {
            Duration::MAX
        };
        select! {
            recv(lines) -> line => match line {
                Ok(line) if line.trim() == "r" => {
                    config.audio.auto_game_track = !config.audio.auto_game_track;
                    println!("sources changées : piste Jeu {}", if config.audio.auto_game_track { "ajoutée" } else { "retirée" });
                    handle.configure(config.clone())?;
                }
                Ok(line) if line.trim() == "f" => {
                    handle.finish_clip()?;
                }
                Ok(line) if line.trim() == "e" => {
                    if recording {
                        requested += 1;
                        handle.stop_recording()?;
                    } else {
                        handle.start_recording()?;
                    }
                    recording = !recording;
                }
                Ok(line) if line.trim() != "q" => {
                    if !post_roll {
                        requested += 1;
                    }
                    post_roll = args.after > 0;
                    handle.save_replay()?;
                }
                _ => quitting = true,
            },
            recv(engine.events()) -> event => {
                let Ok(event) = event else { break };
                match event {
                    EngineEvent::ClipSaved { path, duration_secs, save_latency_ms, .. } => {
                        finished += 1;
                        latencies.push(save_latency_ms);
                        chime_on(Sound::Saved);
                        println!(
                            "[{:>6} s] clip {finished} : {duration_secs:.3} s, prêt en {save_latency_ms} ms ; mémoire privée {:.1} Mo → {path}",
                            started.elapsed().as_secs(),
                            private_bytes().unwrap_or_default() as f64 / 1e6
                        );
                    }
                    EngineEvent::PostRoll { ends_in_secs } => {
                        post_roll = ends_in_secs.is_some();
                        if post_roll {
                            chime_on(Sound::Saved);
                        }
                        match ends_in_secs {
                            Some(seconds) => println!("[{:>6} s] clip en cours : fin dans {seconds:.1} s", started.elapsed().as_secs()),
                            None => println!("[{:>6} s] clip en cours terminé", started.elapsed().as_secs()),
                        }
                    }
                    EngineEvent::RecordingStats { elapsed_secs, bytes } => {
                        println!("enregistrement : {elapsed_secs:.0} s, {:.1} Mo", bytes as f64 / 1e6);
                    }
                    EngineEvent::ReplayStats { buffered_secs, bytes } => {
                        stats_seen += 1;
                        if stats_seen.is_multiple_of(STATS_EVERY) {
                            if args.levels {
                                let peaks: Vec<String> = last_levels
                                    .iter()
                                    .enumerate()
                                    .map(|(index, peak): (usize, &f32)| format!("piste {} {:.1} dB", index + 1, 20.0 * peak.max(1e-6).log10()))
                                    .collect();
                                println!(
                                    "niveaux : {:.1} par seconde ; {}",
                                    f64::from(level_events) / f64::from(STATS_EVERY),
                                    peaks.join(", ")
                                );
                                level_events = 0;
                            }
                            println!(
                                "[{:>6} s] tampon {buffered_secs:.1} s, {:.0} Mo ; mémoire privée du processus {:.0} Mo",
                                started.elapsed().as_secs(),
                                bytes as f64 / 1e6,
                                private_bytes().unwrap_or_default() as f64 / 1e6
                            );
                        }
                    }
                    EngineEvent::Alert { severity, text, message } => {
                        match text.code {
                            "clip_save_failed" => finished += 1,
                            "replay_stopped" | "replay_arm_failed" => quitting = true,
                            _ => {}
                        }
                        println!("alerte {severity:?} {} : {message}", text.code);
                    }
                    EngineEvent::AlertCleared { code } => println!("alerte retirée : {code}"),
                    EngineEvent::StateChanged { state } => {
                        let now_recording = state == EngineState::Recording;
                        if now_recording != state_recording {
                            chime_on(if now_recording { Sound::RecordingStarted } else { Sound::RecordingStopped });
                            state_recording = now_recording;
                        }
                        println!("état : {state:?}");
                    }
                    EngineEvent::GameNamed { exe_name, name } => {
                        println!("nom de jeu : {exe_name} → {name}");
                        config.storage.game_names.insert(exe_name, name);
                    }
                    EngineEvent::CaptureStarted { source, width, height, fps, source_mode, tracks } => {
                        println!("capture : {source} ({}x{} {:?} Hz), {width}x{height} à {fps} i/s, pistes {}", source_mode.width, source_mode.height, source_mode.refresh_hz, tracks.iter().map(|track| track.title.as_str()).collect::<Vec<_>>().join(", "));
                    }
                    EngineEvent::SourceChanged { source_mode } => {
                        println!("source : {}x{} {:?} Hz", source_mode.width, source_mode.height, source_mode.refresh_hz);
                    }
                    EngineEvent::TracksRenamed { tracks } => {
                        println!("pistes : {}", tracks.iter().map(|track| track.title.as_str()).collect::<Vec<_>>().join(", "));
                    }
                    EngineEvent::TrackChanged { track, title, state, source } => {
                        println!("piste {} « {title} » : {state:?} {}", track + 1, source.unwrap_or_default());
                    }
                    EngineEvent::AudioLevels { levels } => {
                        level_events += 1;
                        last_levels = levels;
                    }
                    EngineEvent::PerformanceStats { cpu_percent, cpu_max_percent, engine_bytes, buffer_bytes, buffered_secs, capture } => {
                        let capture = capture.map_or_else(
                            || "capture arrêtée".to_owned(),
                            |capture| format!(
                                "{} {}x{} à {} i/s, répétées {:.1} % (10 s), en retard {} (1 min {}), décalage max {:.1} ms",
                                capture.encoder, capture.width, capture.height, capture.fps,
                                capture.repeated_percent, capture.late_total, capture.late_last_minute, capture.lag_max_ms
                            ),
                        );
                        println!(
                            "performances : CPU {} (max {}) ; moteur hors tampon {} ; tampon {:.0} Mo, {buffered_secs:.1} s ; {capture}",
                            cpu_percent.map_or("—".into(), |cpu| format!("{cpu:.2} %")),
                            cpu_max_percent.map_or("—".into(), |cpu| format!("{cpu:.2} %")),
                            engine_bytes.map_or("—".into(), |bytes| format!("{:.1} Mo", bytes as f64 / 1e6)),
                            buffer_bytes as f64 / 1e6
                        );
                    }
                }
            },
            default(timeout) => break,
        }
    }
    drop(handle);
    drop(engine);
    if let (Some(min), Some(max)) = (latencies.iter().min(), latencies.iter().max()) {
        let mean = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
        println!(
            "RÉSUMÉ : {} clips sur {requested} demandés, latence min {min} ms, moy {mean:.0} ms, max {max} ms",
            latencies.len()
        );
    }
    Ok(())
}

fn wait_until_armed(events: &Receiver<EngineEvent>) -> Result<()> {
    loop {
        match events.recv_timeout(ARM_TIMEOUT)? {
            EngineEvent::StateChanged {
                state: EngineState::ReplayArmed,
            } => return Ok(()),
            EngineEvent::Alert { text, message, .. } if text.code == "replay_arm_failed" => {
                anyhow::bail!(message)
            }
            EngineEvent::Alert { message, .. } => println!("alerte : {message}"),
            _ => {}
        }
    }
}

fn spawn_stdin_reader() -> Receiver<String> {
    let (sender, lines) = crossbeam_channel::bounded(16);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    lines
}
