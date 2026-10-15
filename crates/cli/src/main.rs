//! `palaclips-cli` — banc de test du moteur, sans UI.
//!
//! Chaque phase du moteur (docs/PLAN.md) ajoute ici une sous-commande qui permet
//! de valider ses critères d'acceptation à la main ou dans un script.

use anyhow::Result;
use clap::{Parser, Subcommand};
use palaclips_engine::{EngineConfig, VERSION, replay::estimate_buffer_bytes};

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
    /// Affiche la version et la configuration par défaut (JSON).
    Info,
    /// Estime la RAM du buffer de replay.
    Estimate {
        /// Débit vidéo en kbit/s.
        #[arg(long, default_value_t = 40_000)]
        video_kbps: u32,
        /// Nombre de pistes audio.
        #[arg(long, default_value_t = 3)]
        audio_tracks: usize,
        /// Durée du replay en secondes.
        #[arg(long, default_value_t = 60)]
        duration: u32,
    },
    // Phase 1 : `capture-video --target <...> --seconds 10 --out <fichier>`
    // Phase 2 : `capture-av --seconds 3600 --out <fichier>`
    // Phase 3 : `list-audio-sessions`, `capture-apps --app <exe>... --out <fichier>`
    // Phase 4 : `replay --duration 60` (raccourci de sauvegarde, stats RAM)
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
    }
    Ok(())
}
