use std::io::BufRead;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, select};
use palaclips_engine::EngineConfig;
use palaclips_engine::config::{AppTrack, AudioSettings, ReplaySettings, ReplayStorage};
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
        help = "Sortie à enregistrer en loopback pour la piste mix (partie du nom) ; absente = sortie par défaut"
    )]
    pub mix_device: Option<String>,
    #[arg(long, help = "Démarre le moteur sans armer le replay (mesure mémoire)")]
    pub no_arm: bool,
    #[arg(
        long,
        help = "Demande les niveaux audio (20 i/s) et affiche leur cadence"
    )]
    pub levels: bool,
}

impl ReplayArgs {
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
                storage: if self.disk {
                    ReplayStorage::Disk
                } else {
                    ReplayStorage::Ram
                },
            },
            ..EngineConfig::default()
        };
        config.storage.clips_dir = self.clips_dir.clone();
        config.audio.max_app_tracks = config.audio.max_app_tracks.max(self.apps.len());
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
        "replay de {} s ({}) : Entrée = sauvegarder, r = changer les sources (piste Jeu) et redémarrer le tampon, q = quitter",
        args.duration,
        if args.disk { "disque" } else { "RAM" }
    );
    let lines = spawn_stdin_reader();
    let mut requested = 0u32;
    let mut finished = 0u32;
    let mut latencies = Vec::new();
    let mut stats_seen = 0u32;
    let mut level_events = 0u32;
    let mut last_levels = Vec::new();
    let mut quitting = false;
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
                Ok(line) if line.trim() != "q" => {
                    requested += 1;
                    handle.save_replay()?;
                }
                _ => quitting = true,
            },
            recv(engine.events()) -> event => {
                let Ok(event) = event else { break };
                match event {
                    EngineEvent::ClipSaved { path, duration_secs, save_latency_ms } => {
                        finished += 1;
                        latencies.push(save_latency_ms);
                        println!(
                            "[{:>6} s] clip {finished} : {duration_secs:.3} s, prêt en {save_latency_ms} ms ; mémoire privée {:.1} Mo → {path}",
                            started.elapsed().as_secs(),
                            private_bytes().unwrap_or_default() as f64 / 1e6
                        );
                    }
                    EngineEvent::ReplayStats { buffered_secs, bytes } => {
                        stats_seen += 1;
                        if stats_seen.is_multiple_of(STATS_EVERY) {
                            if args.levels {
                                let peaks: Vec<String> = last_levels
                                    .iter()
                                    .map(|(title, peak): &(String, f32)| format!("{title} {:.1} dB", 20.0 * peak.max(1e-6).log10()))
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
                    EngineEvent::Alert { severity, code, message } => {
                        match code.as_str() {
                            "clip_save_failed" => finished += 1,
                            "replay_stopped" | "replay_arm_failed" => quitting = true,
                            _ => {}
                        }
                        println!("alerte {severity:?} {code} : {message}");
                    }
                    EngineEvent::StateChanged { state } => println!("état : {state:?}"),
                    EngineEvent::AudioLevels { levels } => {
                        level_events += 1;
                        last_levels = levels;
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
            EngineEvent::Alert { code, message, .. } if code == "replay_arm_failed" => {
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
