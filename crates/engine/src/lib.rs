//! # palaclips-engine
//!
//! Moteur de PalaClips : capture vidéo (Windows Graphics Capture), capture audio
//! par application (WASAPI process loopback), encodage matériel (NVENC/AMF/QSV via
//! FFmpeg), replay buffer en mémoire et écriture MP4.
//!
//! **Règle d'or** : cette crate ne dépend jamais de Tauri. Elle doit pouvoir tourner
//! seule depuis `palaclips-cli`. L'UI ne fait que la piloter.
//!
//! Voir `docs/ARCHITECTURE.md` pour le flux de données complet et les threads.

pub mod clock;
pub mod config;
pub mod error;
pub mod events;
pub mod replay;

// Modules Windows : squelettes documentés, implémentés phase par phase.
pub mod audio;
pub mod capture;
pub mod encode;
pub mod mux;
pub mod record;
pub mod sources;

pub use config::EngineConfig;
pub use error::{EngineError, Result};

/// Version du moteur, exposée à l'UI et dans les métadonnées des fichiers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
